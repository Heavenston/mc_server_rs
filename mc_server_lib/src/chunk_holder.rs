use crate::chunk::Chunk;
use mc_networking::packets::client_bound::{C1CUnloadChunk, C40UpdateViewPosition};
use mc_utils::ChunkData;
use crate::entity_manager::PlayerManager;

use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[async_trait]
pub trait ChunkGenerator {
    /// If true is returned, the chunk is ignored and nothing is sent to the client
    /// in that case, he chunk should probably be empty
    /// if it's really needed, the chunk should be handled be handled by another ChunkHolder
    async fn should_ignore(&self, _x: i32, _z: i32) -> bool { false }
    /// Generate a new chunk data
    async fn generate_chunk_data(&self, x: i32, z: i32) -> Box<ChunkData>;
}

/// Manage chunks loading
pub struct ChunkHolder<T: ChunkGenerator+Send+Sync> {
    chunk_generator: T,
    chunks: RwLock<HashMap<(i32, i32), Arc<RwLock<Chunk>>>>,
    view_distance: i32,
    pub players: RwLock<PlayerManager>,
}

impl<T: 'static+ChunkGenerator+Send+Sync> ChunkHolder<T> {
    pub fn new(chunk_generator: T, view_distance: i32) -> Self {
        Self {
            chunk_generator,
            view_distance,
            chunks: RwLock::new(HashMap::new()),
            players: RwLock::new(PlayerManager::new()),
        }
    }

    pub async fn generate_chunk(&self, x: i32, z: i32, chunk_generator: impl ChunkGenerator) {
        let data = chunk_generator.generate_chunk_data(x, z).await;
        let chunk = self.chunks.read().await.get(&(x, z)).cloned();
        match chunk {
            Some(chunk) => {
                chunk.write().await.data = data;
            }
            None => {
                self.chunks.write().await.insert(
                    (x, z),
                    Arc::new(RwLock::new(Chunk::new(
                        x,
                        z,
                        data,
                    )))
                );
            }
        }
        for player in self.players.read().await.entities() {
            player.send_packet(&C1CUnloadChunk {
                chunk_x: x,
                chunk_z: z
            }).await.unwrap();
            player.write().await.as_player_mut().unwrap().loaded_chunks.remove(&(x, z));
            let eid = player.entity_id().await;
            let location = player.read().await.location().clone();
            self.update_player_view_position(eid, location.chunk_x(), location.chunk_z()).await;
        }
    }

    pub async fn get_chunk(&self, x: i32, z: i32) -> Option<Arc<RwLock<Chunk>>> {
        if !self.chunks.read().await.contains_key(&(x, z))
            && !self.chunk_generator.should_ignore(x, z).await
        {
            let chunk = Arc::new(RwLock::new(Chunk::new(
                x,
                z,
                self.chunk_generator.generate_chunk_data(x, z).await,
            )));
            self.chunks.write().await.insert((x, z), Arc::clone(&chunk));
        }
        self.chunks.read().await.get(&(x, z)).cloned()
    }

    pub async fn update_player_view_position(
        &self,
        player_id: i32,
        chunk_x: i32,
        chunk_z: i32,
    ) {
        let player = self.players.read().await.get_entity(player_id).unwrap().clone();
        let view_distance = self.view_distance;
        for dx in -view_distance..view_distance {
            for dz in -view_distance..view_distance {
                if dx * dx + dz * dz > view_distance * view_distance {
                    continue;
                }
                if player
                    .read()
                    .await
                    .as_player()
                    .unwrap()
                    .loaded_chunks
                    .contains(&(chunk_x + dx, chunk_z + dz))
                {
                    continue;
                }
                if let Some(chunk) = self.get_chunk(chunk_x + dx, chunk_z + dz).await {
                    player
                        .send_packet(&chunk.read().await.encode())
                        .await
                        .unwrap();
                    player
                        .write()
                        .await
                        .as_player_mut()
                        .unwrap()
                        .loaded_chunks
                        .insert((chunk_x + dx, chunk_z + dz));
                }
            }
        }
        let loaded_chunks = player
            .read()
            .await
            .as_player()
            .unwrap()
            .loaded_chunks
            .clone();
        for chunk in loaded_chunks {
            let (dx, dz) = (chunk_x - chunk.0, chunk_z - chunk.1);
            if dx * dx + dz * dz >= view_distance * view_distance {
                player
                    .send_packet(&C1CUnloadChunk {
                        chunk_x: chunk.0,
                        chunk_z: chunk.1,
                    })
                    .await
                    .unwrap();
                player
                    .write()
                    .await
                    .as_player_mut()
                    .unwrap()
                    .loaded_chunks
                    .remove(&chunk);
            }
        }
        player
            .send_packet(&C40UpdateViewPosition { chunk_x, chunk_z })
            .await
            .unwrap();
    }

    pub async fn tick(&self) {
        // TODO: Tick all chunks
    }
}

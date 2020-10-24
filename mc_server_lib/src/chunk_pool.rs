use crate::{chunk::Chunk, entity::BoxedEntity};
use mc_utils::ChunkData;
use crate::entity_manager::{PlayerManager, PlayerWrapper};

use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::{RwLock, Mutex};

#[async_trait]
pub trait ChunkGenerator: Send + Sync {
    async fn generate_chunk_data(&mut self, x: i32, z: i32) -> Box<ChunkData>;
}

/// Manage chunks loading
/// And syncing chunks with players
/// A player cannot be in multiple chunk_pool
pub struct ChunkPool<T: ChunkGenerator> {
    pub view_distance: i32,
    chunk_generator: Arc<Mutex<T>>,
    chunks: Arc<RwLock<HashMap<(i32, i32), Arc<RwLock<Chunk>>>>>,
    chunks_to_update: Vec<(i32, i32)>,
    players: PlayerManager,
    synced_player_chunks: HashMap<i32, (i32, i32)>,
}

impl<T: 'static + ChunkGenerator> ChunkPool<T> {
    pub fn new(chunk_generator: T, view_distance: i32) -> Self {
        Self {
            view_distance,
            chunk_generator: Arc::new(Mutex::new(chunk_generator)),
            chunks: Arc::new(RwLock::new(HashMap::new())),
            chunks_to_update: vec![],
            players: PlayerManager::new(),
            synced_player_chunks: HashMap::new(),
        }
    }

    pub async fn ensure_chunk(&mut self, x: i32, z: i32) -> Arc<RwLock<Chunk>> {
        if let Some(chunk) = self.chunks.read().await.get(&(x, z)) {
            return Arc::clone(chunk);
        }
        let mut chunk = Chunk::new(x, z);
        chunk.data = self.chunk_generator.lock().await.generate_chunk_data(x, z).await;
        let chunk = Arc::new(RwLock::new(chunk));
        self.chunks.write().await.insert((x, z), Arc::clone(&chunk));
        chunk
    }
    pub async fn get_chunk(&self, x: i32, z: i32) -> Option<Arc<RwLock<Chunk>>> {
        self.chunks.read().await.get(&(x, z)).cloned()
    }
    pub fn update_chunk(&mut self, x: i32, z: i32) {
        self.chunks_to_update.push((x, z));
    }

    pub async fn add_player(&mut self, player: Arc<RwLock<BoxedEntity>>) {
        self.players.add_entity(Arc::clone(&player)).await;
        let eid = player.read().await.entity_id();
        let location = player.read().await.location().clone();
        self.update_player_view_position(eid, location.chunk_x(), location.chunk_z()).await;
        self.synced_player_chunks.insert(eid, (location.chunk_x(), location.chunk_z()));
    }
    pub fn remove_player(&mut self, id: i32) -> Option<PlayerWrapper> {
        self.synced_player_chunks.remove(&id);
        self.players.remove_entity(id)
    }

    async fn update_player_view_position(&mut self, player_id: i32, chunk_x: i32, chunk_z: i32) {
        let view_distance = self.view_distance;
        let chunks = Arc::clone(&self.chunks);
        let chunk_generator = Arc::clone(&self.chunk_generator);
        let player = Arc::clone(&self.players[player_id]);
        tokio::task::spawn(async move {
            for dx in -view_distance..view_distance {
                for dz in -view_distance..view_distance {
                    if dx*dx + dz*dz > view_distance*view_distance {
                        continue;
                    }
                    if player
                        .read()
                        .await
                        .as_player().unwrap()
                        .loaded_chunks
                        .contains(&(chunk_x + dx, chunk_z + dz)) {
                        continue;
                    }
                    let chunk = {
                        if chunks.read().await.contains_key(&(chunk_x + dx, chunk_z + dz)) {
                            Arc::clone(&chunks.read().await[&(chunk_x + dx, chunk_z + dz)])
                        }
                        else {
                            let mut chunk = Chunk::new(chunk_x + dx, chunk_z + dz);
                            chunk.data = chunk_generator.lock().await.generate_chunk_data(chunk_x + dx, chunk_z + dz).await;
                            let chunk = Arc::new(RwLock::new(chunk));
                            chunks.write().await.insert((chunk_x + dx, chunk_z + dz), Arc::clone(&chunk));
                            chunk
                        }
                    };
                    player
                        .read()
                        .await
                        .as_player().unwrap()
                        .client
                        .lock()
                        .await
                        .send_packet(&chunk.read().await.encode(true))
                        .await.unwrap();
                    player
                        .write()
                        .await
                        .as_player_mut().unwrap()
                        .loaded_chunks
                        .insert((chunk_x + dx, chunk_z + dz));
                }
            }
            let loaded_chunks = player.read().await.as_player().unwrap().loaded_chunks.clone();
            for chunk in loaded_chunks {
                let (dx, dz) = (chunk_x - chunk.0, chunk_z - chunk.1);
                if dx*dx + dz*dz >= view_distance*view_distance {
                    player
                        .read()
                        .await
                        .as_player().unwrap()
                        .client
                        .lock()
                        .await
                        .unload_chunk(chunk.0, chunk.1)
                        .await.unwrap();
                    player
                        .write()
                        .await
                        .as_player_mut().unwrap()
                        .loaded_chunks
                        .remove(&chunk);
                }
            }
            player
                .read()
                .await
                .as_player().unwrap()
                .client
                .lock()
                .await
                .update_view_position(chunk_x, chunk_z)
                .await.unwrap();
        });
    }

    pub async fn tick(&mut self) -> Result<()> {
        let players = self.players.clone();
        for (eid, player) in players {
            let position = player.read().await.location().clone();
            let current_chunk = (position.chunk_x(), position.chunk_z());
            for chunk in self.chunks_to_update.iter().cloned() {
                if player.read().await.as_player().unwrap().loaded_chunks.contains(&chunk) {
                    if let Some(chunk) = self.get_chunk(chunk.0, chunk.1).await {
                        player
                            .send_packet(&chunk.read().await.encode(false))
                            .await?;
                    }
                }
            }
            let synced_chunk = self.synced_player_chunks.get(&eid);
            if synced_chunk
                .cloned()
                .map(|s| s != current_chunk)
                .unwrap_or(true)
            {
                self.update_player_view_position(eid, current_chunk.0, current_chunk.1).await;
                self.synced_player_chunks.insert(eid, current_chunk);
            }
        }
        self.chunks_to_update.clear();
        Ok(())
    }
}

use crate::{chunk::Chunk, entity_manager::PlayerWrapper};
use mc_networking::packets::client_bound::{C1CUnloadChunk, C40UpdateViewPosition};
use mc_utils::ChunkData;

use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[async_trait]
pub trait ChunkGenerator {
    async fn generate_chunk_data(&self, x: i32, z: i32) -> Box<ChunkData>;
}

/// Manage chunks loading
pub struct ChunkHolder<T: ChunkGenerator+Send+Sync> {
    chunk_generator: T,
    chunks: RwLock<HashMap<(i32, i32), Arc<RwLock<Chunk>>>>,
}

impl<T: 'static+ChunkGenerator+Send+Sync> ChunkHolder<T> {
    pub fn new(chunk_generator: T) -> Self {
        Self {
            chunk_generator,
            chunks: RwLock::new(HashMap::new()),
        }
    }

    pub async fn ensure_chunk(&self, x: i32, z: i32) -> Arc<RwLock<Chunk>> {
        if let Some(chunk) = self.chunks.read().await.get(&(x, z)) {
            return Arc::clone(chunk);
        }
        let chunk = Arc::new(RwLock::new(
            Chunk::new(x, z, self.chunk_generator.generate_chunk_data(x, z).await)
        ));
        self.chunks.write().await.insert((x, z), Arc::clone(&chunk));
        chunk
    }

    pub async fn update_player_view_position(
        &self,
        view_distance: i32,
        player: PlayerWrapper,
        chunk_x: i32,
        chunk_z: i32,
    ) {
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
                let chunk = self.ensure_chunk(chunk_x + dx, chunk_z + dz).await;
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

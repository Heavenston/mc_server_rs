use crate::{chunk::Chunk, entity::BoxedEntity};
use mc_utils::ChunkData;

use anyhow::Result;
use async_trait::async_trait;
use std::{collections::HashMap, sync::Arc};
use tokio::sync::RwLock;

#[async_trait]
pub trait ChunkGenerator {
    async fn generate_chunk_data(&mut self, x: i32, z: i32) -> ChunkData;
}

/// Manage chunks loading
/// And syncing chunks with players
/// A player cannot be in multiple chunk_pool
pub struct ChunkPool<T: ChunkGenerator> {
    pub view_distance: i32,
    chunk_generator: T,
    chunks: HashMap<(i32, i32), Arc<RwLock<Chunk>>>,
    players: HashMap<i32, Arc<RwLock<BoxedEntity>>>,
    synced_player_chunks: HashMap<i32, (i32, i32)>,
}

impl<T: ChunkGenerator> ChunkPool<T> {
    pub fn new(chunk_generator: T, view_distance: i32) -> Self {
        Self {
            view_distance,
            chunk_generator,
            chunks: HashMap::new(),
            players: HashMap::new(),
            synced_player_chunks: HashMap::new(),
        }
    }

    pub async fn ensure_chunk(&mut self, x: i32, z: i32) -> Arc<RwLock<Chunk>> {
        if let Some(chunk) = self.chunks.get(&(x, z)) {
            return Arc::clone(chunk);
        }
        let mut chunk = Chunk::new(x, z);
        chunk.data = self.chunk_generator.generate_chunk_data(x, z).await;
        let chunk = Arc::new(RwLock::new(chunk));
        self.chunks.insert((x, z), Arc::clone(&chunk));
        chunk
    }
    pub fn get_chunk(&self, x: i32, z: i32) -> Option<Arc<RwLock<Chunk>>> {
        self.chunks.get(&(x, z)).cloned()
    }

    pub fn get_players(&self) -> &HashMap<i32, Arc<RwLock<BoxedEntity>>> { &self.players }
    pub fn has_player(&self, id: i32) -> bool { self.players.contains_key(&id) }
    pub async fn add_player(&mut self, player: Arc<RwLock<BoxedEntity>>) {
        let eid = player.read().await.entity_id();
        self.players.insert(eid, Arc::clone(&player));
        let location = player.read().await.location().clone();
        self.update_player_view_position(eid, location.chunk_x(), location.chunk_z()).await.unwrap();
        self.synced_player_chunks.insert(eid, (location.chunk_x(), location.chunk_z()));
    }
    pub fn remove_player(&mut self, id: i32) -> Option<Arc<RwLock<BoxedEntity>>> {
        self.synced_player_chunks.remove(&id);
        self.players.remove(&id)
    }

    async fn update_player_view_position(&mut self, player_id: i32, chunk_x: i32, chunk_z: i32) -> Result<()> {
        println!("UPDATE {}", player_id);
        let player = Arc::clone(&self.players[&player_id]);
        for dx in (-self.view_distance / 2)..self.view_distance / 2 {
            for dz in (-self.view_distance / 2)..self.view_distance / 2 {
                if player
                    .read()
                    .await
                    .as_player()?
                    .loaded_chunks
                    .contains(&(chunk_x + dx, chunk_z + dz)) {
                    continue;
                }
                let chunk = self
                    .ensure_chunk(chunk_x + dx, chunk_z + dz)
                    .await;
                player
                    .read()
                    .await
                    .as_player()?
                    .client
                    .lock()
                    .await
                    .send_packet(&chunk.read().await.encode())
                    .await?;
                player
                    .write()
                    .await
                    .as_player_mut()?
                    .loaded_chunks
                    .insert((chunk_x + dx, chunk_z + dz));
            }
        }
        let loaded_chunks = player.read().await.as_player()?.loaded_chunks.clone();
        for chunk in loaded_chunks {
            if (chunk.0 - chunk_x).abs() >= self.view_distance / 2
                || (chunk.1 - chunk_z).abs() >= self.view_distance / 2
            {
                player
                    .read()
                    .await
                    .as_player()?
                    .client
                    .lock()
                    .await
                    .unload_chunk(chunk.0, chunk.1)
                    .await?;
                player
                    .write()
                    .await
                    .as_player_mut()?
                    .loaded_chunks
                    .remove(&chunk);
            }
        }
        player
            .read()
            .await
            .as_player()?
            .client
            .lock()
            .await
            .update_view_position(chunk_x, chunk_z)
            .await?;
        Ok(())
    }

    pub async fn tick(&mut self) -> Result<()> {
        let players = self.players.clone();
        for (eid, player) in players {
            let position = player.read().await.location().clone();
            let current_chunk = (position.chunk_x(), position.chunk_z());
            let synced_chunk = self.synced_player_chunks.get(&eid);
            if synced_chunk
                .cloned()
                .map(|s| s != current_chunk)
                .unwrap_or(true)
            {
                self.update_player_view_position(eid, current_chunk.0, current_chunk.1).await?;
                self.synced_player_chunks.insert(eid, current_chunk);
            }
        }
        Ok(())
    }
}

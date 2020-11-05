use crate::{chunk::Chunk, entity_manager::PlayerManager};
use mc_networking::packets::client_bound::*;
use mc_utils::ChunkData;

use async_trait::async_trait;
use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};
use tokio::{
    sync::RwLock,
    time::{sleep_until, Duration, Instant},
};

#[async_trait]
pub trait ChunkGenerator {
    /// If true is returned, the chunk is ignored and nothing is sent to the client
    /// in that case, he chunk should probably be empty
    /// if it's really needed, the chunk should be handled be handled by another ChunkHolder
    async fn should_ignore(&self, _x: i32, _z: i32) -> bool {
        false
    }
    /// Generate a new chunk data
    async fn generate_chunk_data(&self, x: i32, z: i32) -> Box<ChunkData>;
}

struct BlockChange {
    x: u8,
    y: u8,
    z: u8,
    block: u16,
}

/// Manage chunks loading
pub struct ChunkHolder<T: ChunkGenerator + Send + Sync> {
    chunk_generator: T,
    chunks: RwLock<HashMap<(i32, i32), Arc<RwLock<Chunk>>>>,
    view_distance: i32,
    pub players: RwLock<PlayerManager>,
    synced_player_chunks: RwLock<HashMap<i32, (i32, i32)>>,
    update_view_position_interrupts: RwLock<HashMap<i32, Arc<AtomicBool>>>,
    block_changes: RwLock<HashMap<(i32, i32, i32), Vec<BlockChange>>>,
}

impl<T: 'static + ChunkGenerator + Send + Sync> ChunkHolder<T> {
    pub fn new(chunk_generator: T, view_distance: i32) -> Self {
        Self {
            chunk_generator,
            view_distance,
            chunks: RwLock::new(HashMap::new()),
            players: RwLock::new(PlayerManager::new()),
            synced_player_chunks: RwLock::default(),
            update_view_position_interrupts: RwLock::default(),
            block_changes: RwLock::default(),
        }
    }

    pub async fn set_block(&self, x: i32, y: u8, z: i32, block: u16) {
        let chunk_pos = (
            ((x as f64) / 16.0).floor() as i32,
            ((z as f64) / 16.0).floor() as i32,
        );
        let chunk = self.chunks.read().await.get(&chunk_pos).cloned().unwrap();
        let (local_x, local_y, local_z) = (
            x.rem_euclid(16) as u8,
            y.rem_euclid(16),
            z.rem_euclid(16) as u8,
        );
        chunk
            .write()
            .await
            .data
            .set_block(local_x, y, local_z, block);

        let section_pos = (chunk_pos.0, ((y as f64) / 16.0).floor() as i32, chunk_pos.1);
        if !self.block_changes.read().await.contains_key(&section_pos) {
            self.block_changes.write().await.insert(section_pos, vec![]);
        }
        self.block_changes
            .write()
            .await
            .get_mut(&section_pos)
            .unwrap()
            .push(BlockChange {
                x: local_x,
                y: local_y,
                z: local_z,
                block,
            });
    }

    /// Regenerate a chunk using a new chunk_generator and reload the chunk to every player
    pub async fn generate_chunk(&self, x: i32, z: i32, chunk_generator: impl ChunkGenerator) {
        let data = chunk_generator.generate_chunk_data(x, z).await;
        let chunk = self.chunks.read().await.get(&(x, z)).cloned();
        match chunk {
            Some(chunk) => {
                chunk.write().await.data = data;
            }
            None => {
                self.chunks
                    .write()
                    .await
                    .insert((x, z), Arc::new(RwLock::new(Chunk::new(x, z, data))));
            }
        }
        for player in self.players.read().await.entities() {
            player
                .send_packet(&C1CUnloadChunk {
                    chunk_x: x,
                    chunk_z: z,
                })
                .await
                .unwrap();
            player
                .write()
                .await
                .as_player_mut()
                .loaded_chunks
                .remove(&(x, z));
            let eid = player.entity_id().await;
            let location = player.read().await.location().clone();
            self.update_player_view_position(eid, location.chunk_x(), location.chunk_z(), false)
                .await;
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

    /// Update a player view position, unloading/loading chunks accordingly
    /// if do_delay is true a delay is added between chunk load to reduce lag spikes (should be false on player spawning)
    pub async fn update_player_view_position(
        &self,
        player_id: i32,
        chunk_x: i32,
        chunk_z: i32,
        do_delay: bool,
    ) {
        if let Some(interrupt) = self
            .update_view_position_interrupts
            .read()
            .await
            .get(&player_id)
        {
            interrupt.store(true, Ordering::Relaxed);
        }
        let interrupt = Arc::new(AtomicBool::new(false));
        self.update_view_position_interrupts
            .write()
            .await
            .insert(player_id, Arc::clone(&interrupt));

        let view_distance = self.view_distance;

        let player = self
            .players
            .read()
            .await
            .get_entity(player_id)
            .unwrap()
            .clone();
        player
            .send_packet(&C40UpdateViewPosition { chunk_x, chunk_z })
            .await
            .unwrap();
        let loaded_chunks = player.read().await.as_player().loaded_chunks.clone();

        tokio::join!(
            {
                let player = player.clone();
                async move {
                    // Unload already loaded chunks that are now too far
                    for chunk in loaded_chunks {
                        let (dx, dz) = (chunk_x - chunk.0, chunk_z - chunk.1);
                        if dx.abs() > view_distance || dz.abs() > view_distance {
                            let start = Instant::now();
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
                                .loaded_chunks
                                .remove(&chunk);
                            sleep_until(start + Duration::from_millis(10)).await;
                        }
                    }
                }
            },
            {
                let player = player.clone();
                async move {
                    // Load new chunks in squares bigger around the player
                    'chunk_loading: for square_i in 0..view_distance {
                        let square_width = 1 + square_i * 2;
                        let delay = Duration::from_millis(if do_delay { 15 } else { 0 });
                        for dx in -square_width / 2..=square_width / 2 {
                            for dz in [-square_width / 2, square_width / 2].iter().cloned() {
                                for (dx, dz) in &[(dx, dz), (dz, dx)] {
                                    let start = Instant::now();
                                    if interrupt.load(Ordering::Relaxed) {
                                        break 'chunk_loading;
                                    }
                                    if player
                                        .read()
                                        .await
                                        .as_player()
                                        .loaded_chunks
                                        .contains(&(chunk_x + dx, chunk_z + dz))
                                    {
                                        continue;
                                    }
                                    if let Some(chunk) =
                                        self.get_chunk(chunk_x + dx, chunk_z + dz).await
                                    {
                                        let mut player_write = player.write().await;
                                        let player_write = player_write.as_player_mut();
                                        let client = player_write.client.write().await;
                                        let chunk = chunk.read().await.encode();
                                        client.send_packet(&chunk).await.unwrap();
                                        player_write
                                            .loaded_chunks
                                            .insert((chunk_x + dx, chunk_z + dz));
                                        if do_delay {
                                            sleep_until(start + delay).await;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        );

        player.write().await.as_player_mut().view_position = Some((chunk_x, chunk_z));
    }

    async fn get_synced_player_chunk(&self, player: i32) -> (i32, i32) {
        if !self.synced_player_chunks.read().await.contains_key(&player) {
            self.synced_player_chunks
                .write()
                .await
                .insert(player, (i32::MAX, i32::MAX));
        }
        self.synced_player_chunks.read().await[&player]
    }

    pub async fn refresh_player_chunks(&self, player_id: i32) {
        let entity = self
            .players
            .read()
            .await
            .get_entity(player_id)
            .unwrap()
            .clone();
        let loaded_chunks = entity.write().await.as_player_mut().loaded_chunks.clone();
        for (chunk_x, chunk_z) in loaded_chunks {
            entity
                .send_packet(&C1CUnloadChunk { chunk_x, chunk_z })
                .await
                .unwrap();
        }
        entity.write().await.as_player_mut().loaded_chunks.clear();
        self.synced_player_chunks.write().await.remove(&player_id);
        let location = entity.read().await.location().clone();
        self.update_player_view_position(player_id, location.chunk_x(), location.chunk_z(), true)
            .await;
    }

    pub async fn tick(this: Arc<Self>) {
        let mut block_changes: Vec<C0BBlockChange> = vec![];
        let mut multi_block_changes: Vec<C3BMultiBlockChange> = vec![];

        for (section_pos, changes) in this.block_changes.read().await.iter() {
            match changes.len() {
                0 => (),
                1 => {
                    let change = &changes[0];
                    block_changes.push(C0BBlockChange {
                        position: mc_networking::data_types::Position {
                            x: section_pos.0 * 16 + change.x as i32,
                            y: section_pos.1 * 16 + change.y as i32,
                            z: section_pos.2 * 16 + change.z as i32,
                        },
                        block_id: change.block as i32,
                    })
                }
                _ => {
                    let mut multi_block_change = C3BMultiBlockChange {
                        section_x: section_pos.0,
                        section_y: section_pos.1,
                        section_z: section_pos.2,
                        inverted_trust_edges: false,
                        blocks: vec![],
                    };
                    for change in changes {
                        multi_block_change
                            .blocks
                            .push(C3BMultiBlockChangeBlockChange {
                                x: change.x,
                                y: change.y,
                                z: change.z,
                                block_id: change.block as i32,
                            });
                    }
                    multi_block_changes.push(multi_block_change);
                }
            }
        }

        this.block_changes.write().await.clear();

        let players = this
            .players
            .read()
            .await
            .entities()
            .cloned()
            .collect::<Vec<_>>();
        for player in players {
            let id = player.entity_id().await;
            let location = player.read().await.location().clone();
            let synced_chunk = this.get_synced_player_chunk(id).await;
            let current_chunk = (location.chunk_x(), location.chunk_z());
            if current_chunk != synced_chunk {
                let this = Arc::clone(&this);
                this.synced_player_chunks
                    .write()
                    .await
                    .insert(id, current_chunk);
                tokio::task::spawn(async move {
                    this.update_player_view_position(id, current_chunk.0, current_chunk.1, true)
                        .await;
                });
            }

            for block_change in block_changes.iter() {
                player.send_packet(block_change).await.unwrap();
            }
            for multi_block_change in multi_block_changes.iter() {
                player.send_packet(multi_block_change).await.unwrap();
            }
        }
    }
}

use crate::{
    chunk::Chunk,
    entity_manager::{PlayerManager, PlayerWrapper},
};
use mc_networking::packets::client_bound::*;
use mc_utils::ChunkData;

use async_trait::async_trait;
use fxhash::FxBuildHasher;
use indexmap::IndexMap;
use log::*;
use std::sync::{
    atomic::{AtomicBool, AtomicI16, Ordering},
    Arc,
};
use tokio::{
    sync::RwLock,
    time::{sleep_until, Duration, Instant},
};

type FxIndexMap<K, V> = IndexMap<K, V, FxBuildHasher>;

#[async_trait]
pub trait ChunkProvider {
    /// Get a chunk data
    /// If None is returned, the chunk is ignored and nothing is sent to the client
    /// in that case, the chunk should probably be empty instead
    /// if it's really needed, the chunk should be handled be handled by another ChunkHolder
    async fn load_chunk_data(&self, x: i32, z: i32) -> Option<Box<ChunkData>>;
    /// Save chunk data, may be loaded later
    async fn save_chunk_data(&self, x: i32, z: i32, chunk_data: Box<ChunkData>);
}

struct BlockChange {
    x: u8,
    y: u8,
    z: u8,
    block: u16,
}

/// Manage chunks loading
pub struct ChunkHolder<T: ChunkProvider + Send + Sync> {
    chunk_provider: T,
    chunks: RwLock<FxIndexMap<(i32, i32), Arc<RwLock<Chunk>>>>,
    chunk_loadings: RwLock<FxIndexMap<(i32, i32), AtomicI16>>,
    view_distance: i32,
    players: RwLock<PlayerManager>,
    synced_player_chunks: RwLock<FxIndexMap<i32, (i32, i32)>>,
    update_view_position_interrupts: RwLock<FxIndexMap<i32, Arc<AtomicBool>>>,
    block_changes: RwLock<FxIndexMap<(i32, i32, i32), Vec<BlockChange>>>,
}

impl<T: 'static + ChunkProvider + Send + Sync> ChunkHolder<T> {
    pub fn new(chunk_provider: T, view_distance: i32) -> Self {
        Self {
            chunk_provider,
            view_distance,
            chunks: RwLock::default(),
            players: RwLock::new(PlayerManager::new()),
            chunk_loadings: RwLock::default(),
            synced_player_chunks: RwLock::default(),
            update_view_position_interrupts: RwLock::default(),
            block_changes: RwLock::default(),
        }
    }

    pub async fn add_player(&self, player: PlayerWrapper) {
        self.players.write().await.add_entity(player).await;
    }
    pub async fn remove_player(&self, id: i32) {
        if let Some(interrupt) = self.update_view_position_interrupts.read().await.get(&id) {
            interrupt.store(true, Ordering::Relaxed);
        }
        let player = self.players.write().await.remove_entity(id).unwrap();
        let player = player.read().await;
        let player = player.as_player();
        for (x, z) in player.loaded_chunks.clone() {
            self.reduce_chunk_load_count(x, z).await;
        }
    }

    async fn load_chunk(&self, x: i32, z: i32) -> Option<Arc<RwLock<Chunk>>> {
        let chunk_data = self.chunk_provider.load_chunk_data(x, z).await;
        if chunk_data.is_none() {
            return None;
        }
        let chunk_data = chunk_data.unwrap();

        let chunk = Chunk::new(x, z, chunk_data);
        let chunk = Arc::new(RwLock::new(chunk));
        self.chunks.write().await.insert((x, z), chunk.clone());

        Some(chunk)
    }
    async fn save_chunk(&self, x: i32, z: i32) {
        let chunk = self.chunks.write().await.remove(&(x, z));
        if chunk.is_none() {
            return;
        }
        let mut chunk = chunk.unwrap();
        let mut i = 0;
        let chunk = loop {
            assert!(i < 100, "Could not unwrap chunk");
            i += 1;
            match Arc::try_unwrap(chunk) {
                Ok(c) => break c,
                Err(c) => {
                    tokio::task::yield_now().await;
                    chunk = c;
                    debug!("CHUNK UNWRAP MISS");
                    /*panic!(
                        "Chunk {}-{} still has {} references",
                        x,
                        z,
                        Arc::strong_count(&c)
                    );*/
                }
            }
        };
        let chunk = chunk.into_inner();
        self.chunk_provider.save_chunk_data(x, z, chunk.data).await;
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
    pub async fn get_block(&self, x: i32, y: u8, z: i32) -> u16 {
        let chunk_pos = (
            ((x as f64) / 16.0).floor() as i32,
            ((z as f64) / 16.0).floor() as i32,
        );
        let (local_x, local_z) = (x.rem_euclid(16) as u8, z.rem_euclid(16) as u8);
        self.chunks.read().await[&chunk_pos]
            .read()
            .await
            .data
            .get_block(local_x, y, local_z)
    }

    pub async fn get_chunk(&self, x: i32, z: i32) -> Option<Arc<RwLock<Chunk>>> {
        if !self.chunks.read().await.contains_key(&(x, z)) {
            return self.load_chunk(x, z).await;
        }
        self.chunks.read().await.get(&(x, z)).cloned()
    }
    async fn reduce_chunk_load_count(&self, x: i32, z: i32) {
        if let Some(n) = self.chunk_loadings.read().await.get(&(x, z)) {
            if n.fetch_sub(1, Ordering::Relaxed) - 1 <= 0 {
                self.save_chunk(x, z).await;
            }
        }
    }
    async fn increase_chunk_load_count(&self, x: i32, z: i32) {
        let chunk_loadings = self.chunk_loadings.read().await;
        match chunk_loadings.get(&(x, z)) {
            Some(counter) => {
                counter.fetch_add(1, Ordering::Relaxed);
            }
            None => {
                // Must drop the ReadGuard to avoid a deadlock
                drop(chunk_loadings);
                self.chunk_loadings
                    .write()
                    .await
                    .insert((x, z), AtomicI16::new(1));
            }
        }
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
                            self.reduce_chunk_load_count(chunk.0, chunk.1).await;
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
                                        if do_delay {
                                            sleep_until(start + delay).await;
                                        }
                                        if interrupt.load(Ordering::Relaxed) {
                                            break 'chunk_loading;
                                        }
                                        self.increase_chunk_load_count(chunk_x + dx, chunk_z + dz)
                                            .await;
                                        let mut player_write = player.write().await;
                                        let player_write = player_write.as_player_mut();
                                        let client = player_write.client.read().await;
                                        let chunk = chunk.read().await.encode();
                                        client.send_packet(&chunk).await.unwrap();
                                        player_write
                                            .loaded_chunks
                                            .insert((chunk_x + dx, chunk_z + dz));
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

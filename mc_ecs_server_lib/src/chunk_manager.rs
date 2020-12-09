extern crate static_assertions as sa;

use mc_utils::{abort_contract::AbortContract, ChunkData};

use dashmap::{DashMap, DashSet};
use legion::{Entity, World};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Condvar, Mutex, RwLock,
};

pub struct Chunk {
    x: i32,
    z: i32,
    data: Box<ChunkData>,
}

pub trait ChunkLoader: Send + Sync {
    fn load_chunk(&self, x: i32, z: i32) -> Box<ChunkData>;
    fn save_chunk(&self, chunk: Chunk);
}

pub struct ChunkManager {
    chunk_loader: Arc<dyn ChunkLoader>,
    loaded_chunks: Arc<DashMap<(i32, i32), Chunk>>,
    loading_contracts: Arc<DashMap<(i32, i32), Arc<AbortContract>>>,
    unloading_contracts: Arc<DashMap<(i32, i32), Arc<AbortContract>>>,
}
impl ChunkManager {
    pub fn new(chunk_loader: Arc<impl ChunkLoader + 'static>) -> Self {
        Self {
            chunk_loader: chunk_loader as Arc<dyn ChunkLoader>,
            loaded_chunks: Arc::default(),
            loading_contracts: Arc::default(),
            unloading_contracts: Arc::default(),
        }
    }

    pub fn scheduler(&self) -> ChunkScheduler {
        ChunkScheduler {
            chunk_loader: self.chunk_loader.clone(),
            loaded_chunks: self.loaded_chunks.clone(),
            loading_contracts: self.loading_contracts.clone(),
            unloading_contracts: self.unloading_contracts.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ChunkScheduler {
    chunk_loader: Arc<dyn ChunkLoader>,
    loaded_chunks: Arc<DashMap<(i32, i32), Chunk>>,
    loading_contracts: Arc<DashMap<(i32, i32), Arc<AbortContract>>>,
    unloading_contracts: Arc<DashMap<(i32, i32), Arc<AbortContract>>>,
}
impl ChunkScheduler {
    /// returns true if the chunk is fully loaded
    pub fn is_chunk_loaded(&self, x: i32, z: i32) -> bool {
        self.loaded_chunks.contains_key(&(x, z))
    }
    /// returns true if the chunk is currently loading
    pub fn is_chunk_loading(&self, x: i32, z: i32) -> bool {
        self.loading_contracts
            .get(&(x, z))
            // Try to lock the contract to know if it was aborted but do not block
            .map(|a| a.try_is_aborted().unwrap_or(true))
            .unwrap_or(false)
    }
    /// returns true if the chunk is currently unloading
    pub fn is_chunk_unloading(&self, x: i32, z: i32) -> bool {
        self.unloading_contracts
        .get(&(x, z))
        // Try to lock the contract to know if it was aborted but do not block
        .map(|a| a.try_is_aborted().unwrap_or(true))
        .unwrap_or(false)
    }

    /// Starts the loading of a chunk, return false if the chunks was already loaded/loading
    /// if the chunk is unloading, it first waits for its completion
    pub fn load_chunk(&self, x: i32, z: i32) -> bool {
        if self.is_chunk_loaded(x, z) || self.is_chunk_loading(x, z) {
            return false;
        }
        let contract = Arc::new(AbortContract::new());
        self.loading_contracts.insert((x, z), contract.clone());

        rayon::spawn({
            let scheduler = self.clone();
            move || {
                if scheduler.is_chunk_unloading(x, z) {
                    scheduler
                        .unloading_contracts
                        .get(&(x, z))
                        .unwrap()
                        .wait_for_abort();
                }
                let chunk_data = scheduler.chunk_loader.load_chunk(x, z);
                scheduler.loading_contracts.remove(&(x, z));
                if !contract.is_aborted() {
                    scheduler.loaded_chunks.insert(
                        (x, z),
                        Chunk {
                            x,
                            z,
                            data: chunk_data,
                        },
                    );
                    contract.abort();
                }
            }
        });
        true
    }

    /// Starts the unloading of a chunk, return false if the chunks wasn't loaded / was unloading
    /// if the chunk is loading, it first waits for its completion
    pub fn unload_chunk(&self, x: i32, z: i32) -> bool {
        if !self.is_chunk_loaded(x, z) || self.is_chunk_unloading(x, z) {
            return false;
        }
        let contract = Arc::new(AbortContract::new());
        self.unloading_contracts.insert((x, z), contract.clone());

        rayon::spawn({
            let scheduler = self.clone();
            move || {
                if scheduler.is_chunk_loading(x, z) {
                    scheduler
                        .loading_contracts
                        .get(&(x, z))
                        .unwrap()
                        .wait_for_abort();
                }
                let chunk = scheduler.loaded_chunks.remove(&(x, z)).unwrap().1;
                scheduler.chunk_loader.save_chunk(chunk);
                contract.abort();
            }
        });

        true
    }
}

sa::assert_impl_all!(ChunkScheduler: Send, Sync);

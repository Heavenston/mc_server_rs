extern crate static_assertions as sa;

use dashmap::{DashMap, DashSet};
use legion::{Entity, World};
use std::sync::{atomic, Arc, RwLock};

pub trait ChunkLoader: Send + Sync {}

pub struct ChunkManager {
    chunk_loader: Arc<dyn ChunkLoader>,
    loaded_chunks: Arc<DashMap<(i32, i32), Entity>>,
}
impl ChunkManager {
    pub fn new(chunk_loader: Arc<impl ChunkLoader + 'static>) -> Self {
        Self {
            chunk_loader: chunk_loader as Arc<dyn ChunkLoader>,
            loaded_chunks: Arc::new(DashMap::new()),
        }
    }

    pub fn scheduler(&self) -> ChunkScheduler {
        ChunkScheduler {
            chunk_loader: self.chunk_loader.clone(),
            loaded_chunks: self.loaded_chunks.clone(),
        }
    }
}

#[derive(Clone)]
pub struct ChunkScheduler {
    chunk_loader: Arc<dyn ChunkLoader>,
    loaded_chunks: Arc<DashMap<(i32, i32), Entity>>,
}
impl ChunkScheduler {}

sa::assert_impl_all!(ChunkScheduler: Send, Sync);

use dashmap::{DashMap, DashSet};
use legion::{Entity, World};
use std::sync::{atomic, Arc, RwLock};

pub trait ChunkLoader: Send + Sync {}

pub struct ChunkManager {
    loaded_chunks: Arc<DashMap<(i32, i32), Entity>>,

    world: Arc<RwLock<World>>,
}
impl ChunkManager {}

#[derive(Clone)]
pub struct ChunkScheduler {}
impl ChunkScheduler {}

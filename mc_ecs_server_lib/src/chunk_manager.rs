extern crate static_assertions as sa;

use crate::entity::chunk::*;
use mc_networking::packets::client_bound::ClientBoundPacket;
use mc_utils::{abort_contract::AbortContract, ChunkData};

use dashmap::{DashMap, DashSet};
use legion::{
    maybe_changed,
    query::*,
    system,
    systems::CommandBuffer,
    world::{SubWorld, World},
    Entity, EntityStore,
};
use rayon::{ThreadPool, ThreadPoolBuilder};
use std::{
    collections::{HashMap, HashSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Condvar, Mutex, RwLock,
    },
};

#[system]
#[write_component(ChunkComponent)]
#[write_component(LoadedChunkComponent)]
pub(crate) fn chunk_manager_loader(
    world: &mut SubWorld,
    cmd: &mut CommandBuffer,
    #[resource] chunk_manager: &ChunkManager,
) {
    <(Entity, &LoadedChunkComponent, &ChunkComponent)>::query().for_each(
        world,
        |(entity, loaded_chunk, chunk_component)| {
            if chunk_component.loaders.is_empty() {
                cmd.remove(*entity);
                chunk_manager
                    .chunks
                    .remove(&(chunk_component.x, chunk_component.z));
                chunk_manager.unload_chunk(
                    chunk_component.x,
                    chunk_component.z,
                    &loaded_chunk.data,
                );
            }
        },
    );

    let pending_chunks_data = {
        let mut empty = DashMap::new();
        let mut write_lock = chunk_manager.pending_chunks_data.write().unwrap();
        std::mem::swap(&mut empty, &mut *write_lock);
        empty
    };
    for ((chunk_x, chunk_z), (chunk, data)) in pending_chunks_data {
        let mut entry = world.entry_mut(chunk).unwrap();
        entry.get_component_mut::<ChunkComponent>().unwrap().loaded = true;
        cmd.add_component(chunk, LoadedChunkComponent { data });
        chunk_manager
            .chunks
            .insert((chunk_x, chunk_z), chunk);
    }
}

pub trait ChunkLoader: Send + Sync {
    fn load_chunk(&self, x: i32, z: i32) -> Box<ChunkData>;
    fn save_chunk(&self, x: i32, z: i32, data: &ChunkData);
}

pub struct ChunkManager {
    chunk_loader: Arc<dyn ChunkLoader>,
    thread_pool: rayon::ThreadPool,
    chunks: DashMap<(i32, i32), Entity>,
    loading_chunks: Arc<DashSet<(i32, i32)>>,
    pending_chunks_data: Arc<RwLock<DashMap<(i32, i32), (Entity, Box<ChunkData>)>>>,
}
impl ChunkManager {
    pub fn new(chunk_loader: Arc<impl ChunkLoader + 'static>) -> Self {
        Self {
            chunk_loader: chunk_loader as Arc<dyn ChunkLoader>,
            thread_pool: ThreadPoolBuilder::new().build().unwrap(),
            chunks: DashMap::new(),
            loading_chunks: Arc::default(),
            pending_chunks_data: Arc::default(),
        }
    }

    pub fn get_chunk(&self, x: i32, z: i32) -> Option<Entity> {
        self.chunks.get(&(x, z)).map(|e| e.clone())
    }

    pub(crate) fn load_chunk(
        &self,
        cmd_buffer: &mut CommandBuffer,
        x: i32,
        z: i32,
    ) -> Option<Entity> {
        if self.loading_chunks.contains(&(x, z)) {
            return None;
        }
        let chunk = cmd_buffer.push((ChunkComponent {
            loaded: false,
            loaders: HashSet::new(),
            x,
            z,
        },));

        let pending_chunks_data = self.pending_chunks_data.clone();
        let chunk_loader = self.chunk_loader.clone();
        let loading_chunks = self.loading_chunks.clone();
        loading_chunks.insert((x, z));

        self.thread_pool.spawn(move || {
            let data = chunk_loader.load_chunk(x, z);
            pending_chunks_data
                .read()
                .unwrap()
                .insert((x, z), (chunk, data));
            loading_chunks.remove(&(x, z));
        });
        Some(chunk)
    }
    pub(crate) fn unload_chunk(&self, x: i32, z: i32, data: &Box<ChunkData>) {}
}

sa::assert_impl_all!(ChunkManager: Send, Sync);

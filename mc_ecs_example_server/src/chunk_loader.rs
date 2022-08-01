use mc_ecs_server_lib::{chunk_manager::ChunkProvider, entity::ClientComponent};
use mc_networking::packets::client_bound::{C1AUnloadChunk, ClientBoundPacket};
use mc_utils::ChunkData;

use dashmap::DashMap;
use legion::{system, world::SubWorld, Entity, EntityStore};
use rayon::{iter::*, ThreadPool, ThreadPoolBuilder};
use std::sync::{Arc, RwLock};

#[derive(Default)]
struct ChunkLoadingData {
    data: Option<Box<ChunkData>>,
    waiters: Vec<Entity>,
}

pub struct StoneChunkProvider {
    loading_chunks: DashMap<(i32, i32), Arc<RwLock<ChunkLoadingData>>>,
    unloading_chunks: DashMap<(i32, i32), Vec<Entity>>,
    thread_pool: ThreadPool,
}
impl StoneChunkProvider {
    pub fn new() -> Self {
        Self {
            loading_chunks: DashMap::default(),
            unloading_chunks: DashMap::default(),
            thread_pool: ThreadPoolBuilder::new().build().unwrap(),
        }
    }
}
impl ChunkProvider for StoneChunkProvider {
    fn load_chunk(&self, player: &Entity, x: i32, z: i32) {
        println!("LOAD {x} {z}");
        if let Some(entry) = self.loading_chunks.get(&(x, z)) {
            let loading_data = &*entry;
            loading_data.write().unwrap().waiters.push(player.clone());
            return;
        }
        if self.loading_chunks.contains_key(&(x, z)) {
            return;
        }

        let final_chunk_data = Arc::default();
        self.loading_chunks
            .insert((x, z), Arc::clone(&final_chunk_data));

        self.thread_pool.spawn(move || {
            let mut chunk_data = Box::new(ChunkData::new());

            for x in 0..16 {
                for z in 0..16 {
                    chunk_data.set_block(x, 20, z, 1);
                }
            }

            let mut loading_data = final_chunk_data.write().unwrap();
            loading_data.data = Some(chunk_data);
        });
    }

    fn unload_chunk(&self, player: &Entity, x: i32, z: i32) {
        if let Some(entry) = self.loading_chunks.get(&(x, z)) {
            let mut loading_data = entry.write().unwrap();
            loading_data.waiters.retain(|s| s != player);
        }

        match self.unloading_chunks.get_mut(&(x, z)) {
            Some(mut players) => {
                players.push(player.clone());
            }
            None => {
                self.unloading_chunks.insert((x, z), vec![player.clone()]);
            }
        }
    }
}

#[system]
#[read_component(ClientComponent)]
pub fn stone_chunk_provider(
    world: &mut SubWorld,
    #[state] chunk_provider: &Arc<StoneChunkProvider>,
) {
    chunk_provider.unloading_chunks
        .iter().par_bridge()
        .for_each(|unloading_chunk| {
            let unload_packet = C1AUnloadChunk {
                chunk_x: unloading_chunk.key().0,
                chunk_z: unloading_chunk.key().1,
            }.to_rawpacket();

            (&*unloading_chunk).par_iter().for_each(|player| {
                if let Ok(entry) = world.entry_ref(player.clone()) {
                    entry
                        .get_component::<ClientComponent>()
                        .unwrap().0
                        .send_raw_packet_sync(unload_packet.clone());
                }
            });
        });
    chunk_provider.unloading_chunks.clear();
}

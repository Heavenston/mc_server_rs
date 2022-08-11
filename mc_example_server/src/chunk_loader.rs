use mc_server_lib::{ chunk_manager::ConstChunkProvider, entity::ClientComponent };
use mc_networking::packets::{
    client_bound::{ C1AUnloadChunk, ClientBoundPacket },
    RawPacket
};
use mc_utils::ChunkData;

use std::sync::{Arc, RwLock};

use dashmap::DashMap;
use rayon::{ ThreadPool, ThreadPoolBuilder};
use minecraft_data_rs::{ Api as McApi, models::version::Version as McVer };
use bevy_ecs::entity::Entity;
use bevy_ecs::system::{ Res, Commands };
use bevy_ecs::world::World;

lazy_static::lazy_static! {
    static ref MC_API: McApi = McApi::new(McVer {
        version: 759,
        minecraft_version: "1.19".into(),
        major_version: "1.19".into(),
    });
}

#[derive(Default)]
struct ChunkLoadingData {
    data: Option<(ChunkData, RawPacket)>,
    waiters: Vec<Entity>,
}

pub struct StoneChunkProvider {
    loading_chunks: DashMap<(i32, i32), Arc<RwLock<ChunkLoadingData>>>,
    unloading_chunks: DashMap<(i32, i32), Vec<Entity>>,
    thread_pool: ThreadPool,

    ground_block_state: u32,
}
impl StoneChunkProvider {
    pub fn new() -> Self {
        Self {
            loading_chunks: DashMap::default(),
            unloading_chunks: DashMap::default(),
            thread_pool: ThreadPoolBuilder::new().build().unwrap(),

            ground_block_state: MC_API.blocks.blocks_by_name().unwrap()["polished_andesite"].id,
        }
    }
}

impl ConstChunkProvider for StoneChunkProvider {
    fn const_load_chunk(
        &self, player: Entity, commands: &mut Commands,
        chunk_x: i32, chunk_z: i32
    ) {
        if let Some(entry) = self.loading_chunks.get(&(chunk_x, chunk_z)) {
            let loading_data = &*entry;
            loading_data.write().unwrap().waiters.push(player.clone());
            return;
        }
        if self.loading_chunks.contains_key(&(chunk_x, chunk_z)) {
            return;
        }

        let final_chunk_data = Arc::new(RwLock::new(ChunkLoadingData {
            data: None,
            waiters: vec![player],
        }));
        self.loading_chunks
            .insert((chunk_x, chunk_z), Arc::clone(&final_chunk_data));

        let ground_block_state = self.ground_block_state as u16;
        self.thread_pool.spawn(move || {
            let mut chunk_data = ChunkData::new(crate::WORLD_HEIGHT / 16);

            if (chunk_z == 0 || chunk_z == 2) && chunk_x >= 0 {
                for x in 0..16 {
                    chunk_data.set_block(x, 21, 7, ground_block_state);
                    chunk_data.set_block(x, 21, 8, ground_block_state);
                    chunk_data.set_block(x, 21, 9, ground_block_state);
                }
            }
            //chunk_data.get_section_mut(1).fill_with(ground_block_state);

            let packet = chunk_data.encode_full(chunk_x, chunk_z);
            let packet = packet.to_rawpacket();

            let mut loading_data = final_chunk_data.write().unwrap();
            loading_data.data = Some((chunk_data, packet));
        });
    }

    fn const_unload_chunk(
        &self, player: Entity, commands: &mut Commands,
        x: i32, z: i32
    ) {
        if let Some(entry) = self.loading_chunks.get(&(x, z)) {
            let mut loading_data = entry.write().unwrap();
            loading_data.waiters.retain(|s| *s != player);
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

pub fn stone_chunk_provider(
    world: &World,
    chunk_provider: Res<Arc<StoneChunkProvider>>,
) {
    chunk_provider.unloading_chunks
        .iter()
        .for_each(|unloading_chunk| {
            let unload_packet = C1AUnloadChunk {
                chunk_x: unloading_chunk.key().0,
                chunk_z: unloading_chunk.key().1,
            }.to_rawpacket();

            (&*unloading_chunk).iter().copied().for_each(|player| {
                if let Some(entry) = world.get_entity(player) {
                    entry.get::<ClientComponent>().unwrap()
                        .0.send_raw_packet_sync(unload_packet.clone());
                }
            });
        });
    chunk_provider.unloading_chunks.clear();

    chunk_provider.loading_chunks
        .retain(|_, v| {
            // We keep chunks that aren't yet loaded
            if v.read().unwrap().data.is_none()
            { return true }

            let mut final_data = v.write().unwrap();
            let data = final_data.data.take().unwrap();
            let (_, raw_packet) = data;

            for waiter in final_data.waiters.iter().copied() {
                if let Some(entry) = world.get_entity(waiter) {
                    entry.get::<ClientComponent>().unwrap()
                        .0.send_raw_packet_sync(raw_packet.clone());
                }
            }

            false
        });
}

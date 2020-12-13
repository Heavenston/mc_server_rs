use std::collections::HashSet;

use crate::{
    chunk_manager::ChunkManager,
    entity::{ClientComponent, LocationComponent},
};
use mc_networking::packets::client_bound::*;
use mc_utils::ChunkData;

use ahash::AHashSet;
use legion::{
    maybe_changed,
    system,
    systems::CommandBuffer,
    world::{SubWorld},
    Entity, EntityStore,
};


/// Makes chunks around the entity (Based on [LocationComponent]) loaded
pub struct ChunkLoaderComponent {
    pub radius: i32,
    pub loaded_chunks: AHashSet<(i32, i32)>,
}
/// Will send chunks to the Client (from [ClientComponent])
pub struct ChunkObserverComponent;

#[readonly::make]
pub struct ChunkLocationComponent {
    pub last_x: i32,
    pub last_z: i32,
    pub x: i32,
    pub z: i32,
    pub changed: bool,
}
impl ChunkLocationComponent {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            last_x: 0,
            last_z: 0,
            x,
            z,
            changed: true,
        }
    }
}

pub struct ChunkComponent {
    pub loaded: bool,
    pub x: i32,
    pub z: i32,
    pub loaders: HashSet<Entity>,
}

pub struct LoadedChunkComponent {
    pub data: Box<ChunkData>,
}

#[system(par_for_each)]
#[filter(maybe_changed::<LocationComponent>())]
pub(crate) fn chunk_locations_update(
    _: &ChunkLoaderComponent,
    location: &LocationComponent,
    chunk_loc: &mut ChunkLocationComponent,
) {
    let chunk_x = location.0.chunk_x();
    let chunk_z = location.0.chunk_z();

    chunk_loc.changed = if chunk_loc.x != chunk_x || chunk_loc.z != chunk_z {
        true
    }
    else {
        false
    };

    chunk_loc.last_x = chunk_loc.x;
    chunk_loc.last_z = chunk_loc.z;

    chunk_loc.x = chunk_x;
    chunk_loc.z = chunk_z;
}

#[system(for_each)]
#[filter(maybe_changed::<ChunkLocationComponent>())]
#[write_component(ChunkComponent)]
#[read_component(LoadedChunkComponent)]
pub(crate) fn chunk_loaders_updates(
    entity: &Entity,
    world: &mut SubWorld,
    cmd: &mut CommandBuffer,
    chunk_loader: &ChunkLoaderComponent,
    chunk_location: &ChunkLocationComponent,
    #[resource] chunk_manager: &ChunkManager,
) {
    if !chunk_location.changed {
        return;
    }
    let r = chunk_loader.radius;
    for x in chunk_location.x - r..chunk_location.x + r {
        for z in chunk_location.z - r..chunk_location.z + r {
            match chunk_manager.get_chunk(x, z) {
                Some(chunk) => {
                    let mut entry = world.entry_mut(chunk).unwrap();
                    let loaded_chunk = entry.get_component_mut::<ChunkComponent>().unwrap();
                    loaded_chunk.loaders.insert(*entity);
                }
                None => {
                    chunk_manager.load_chunk(cmd, x, z);
                }
            }
        }
    }
}

#[system(par_for_each)]
pub(crate) fn chunk_observer_chunk_loadings(
    chunk_loader: &ChunkLoaderComponent,
    _chunk_observer: &mut ChunkObserverComponent,
    chunk_pos: &ChunkLocationComponent,
    client: &ClientComponent,
    #[resource] _chunk_manager: &ChunkManager,
) {
    if !chunk_pos.changed {
        return;
    }

    client.0.send_packet_sync(&C40UpdateViewPosition {
        chunk_x: chunk_pos.x,
        chunk_z: chunk_pos.z,
    });

    for _square_dist in 0..chunk_loader.radius {}
}

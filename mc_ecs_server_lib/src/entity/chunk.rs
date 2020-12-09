use ahash::AHashSet;
use legion::{maybe_changed, query::*, system, world::SubWorld, Entity};
use rayon::prelude::*;
use smallvec::SmallVec;

use crate::{
    chunk_manager::ChunkScheduler,
    entity::{ClientComponent, LocationComponent},
};
use mc_networking::packets::client_bound::*;
use mc_utils::ChunkData;

/// Works like a flag that makes chunks around it (Based on [LocationComponent]) loaded
pub struct ChunkLoaderComponent {
    pub radius: i32,
}
/// Will send chunks to the Client (from [ClientComponent])
pub struct ChunkObserverComponent {
    pub loaded_chunks: AHashSet<(i32, i32)>,
}

#[readonly::make]
pub struct ChunkLocationComponent {
    pub last_x: i32,
    pub last_z: i32,
    pub x: i32,
    pub z: i32,
    pub changed: bool,
}
impl ChunkLocationComponent {
    pub fn new() -> Self {
        Self {
            last_x: 0,
            last_z: 0,
            x: 0,
            z: 0,
            changed: true,
        }
    }
}

#[readonly::make]
pub struct ChunkComponent {
    #[readonly]
    pub x: i32,
    #[readonly]
    pub z: i32,
    pub data: Box<ChunkData>,
}

#[system(par_for_each)]
#[filter(maybe_changed::<LocationComponent>())]
pub fn chunk_locations_update(
    _: &ChunkLoaderComponent,
    location: &LocationComponent,
    chunk_loc: &mut ChunkLocationComponent,
) {
    let chunk_x = location.loc.chunk_x();
    let chunk_z = location.loc.chunk_z();

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

#[system]
#[write_component(ChunkComponent)]
#[read_component(ChunkLoaderComponent)]
#[read_component(LocationComponent)]
pub fn chunk_loaders_updates(world: &mut SubWorld) {
    let mut loaders_query = <(&ChunkLoaderComponent, &LocationComponent)>::query();
    loaders_query
        .par_iter(world)
        .for_each(|(chunk_loader, location)| {});
}

#[system(par_for_each)]
pub fn chunk_observer_chunk_loadings(
    chunk_loader: &ChunkLoaderComponent,
    chunk_observer: &mut ChunkObserverComponent,
    chunk_pos: &ChunkLocationComponent,
    client: &ClientComponent,
    #[resource] chunk_scheduler: &ChunkScheduler,
) {
    if !chunk_pos.changed {
        return;
    }

    client.client.send_packet_sync(&C40UpdateViewPosition {
        chunk_x: chunk_pos.x,
        chunk_z: chunk_pos.z,
    });

    for square_dist in 0..chunk_loader.radius {}
}

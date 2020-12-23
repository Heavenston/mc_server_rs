use crate::{
    chunk_manager::ChunkProvider,
    entity::{ClientComponent, LocationComponent},
};
use mc_networking::packets::client_bound::*;

use ahash::AHashSet;
use legion::{maybe_changed, system, Entity};
use std::sync::Arc;

/// Will call load_chunk for every chunk in radius around it's [ChunkLocationComponent]
pub struct ChunkObserverComponent {
    pub radius: i32,
    pub loaded_chunks: AHashSet<(i32, i32)>,
    pub chunk_provider: Arc<dyn ChunkProvider>,
}

/// Represent the chunk location of an [Entity] with the [ChunkLoaderComponent]
/// This will be automatically updated based on the [LocationComponent]
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

/// System to update the [ChunkLocationComponent]
#[system(par_for_each)]
#[filter(maybe_changed::<LocationComponent>())]
pub(crate) fn chunk_locations_update(
    location: &LocationComponent,
    chunk_loc: &mut ChunkLocationComponent,
) {
    let chunk_x = location.0.chunk_x();
    let chunk_z = location.0.chunk_z();

    chunk_loc.changed = chunk_loc.x != chunk_x || chunk_loc.z != chunk_z;

    chunk_loc.last_x = chunk_loc.x;
    chunk_loc.last_z = chunk_loc.z;

    chunk_loc.x = chunk_x;
    chunk_loc.z = chunk_z;
}

#[system(par_for_each)]
pub(crate) fn chunk_observer_chunk_loadings(
    entity: &Entity,
    chunk_observer: &mut ChunkObserverComponent,
    chunk_loc: &ChunkLocationComponent,
    client: &ClientComponent,
) {
    if !chunk_loc.changed {
        return;
    }

    client.0.send_packet_sync(&C40UpdateViewPosition {
        chunk_x: chunk_loc.x,
        chunk_z: chunk_loc.z,
    });

    // Unload now too far chunks
    {
        let chunk_loc_x = chunk_loc.x;
        let chunk_loc_z = chunk_loc.z;
        let radius = chunk_observer.radius;
        chunk_observer
            .loaded_chunks
            .retain(|(loaded_chunk_x, loaded_chunk_z)| {
                let distance_x = (loaded_chunk_x - chunk_loc_x).abs();
                let distance_z = (loaded_chunk_z - chunk_loc_z).abs();
                distance_x <= radius && distance_z <= radius
            });
    }

    // Load close enough chunks
    for square_dist in 0..chunk_observer.radius {
        for chunk_dx in -square_dist..square_dist {
            for chunk_dz in -square_dist..square_dist {
                for (chunk_dx, chunk_dz) in [(chunk_dx, chunk_dz), (-chunk_dx, -chunk_dz)].to_vec()
                {
                    let chunk_x = chunk_loc.x + chunk_dx;
                    let chunk_z = chunk_loc.z + chunk_dz;
                    if !chunk_observer.loaded_chunks.contains(&(chunk_x, chunk_z)) {
                        chunk_observer.loaded_chunks.insert((chunk_x, chunk_z));
                        chunk_observer
                            .chunk_provider
                            .load_chunk(entity, chunk_x, chunk_z);
                    }
                }
            }
        }
    }
}

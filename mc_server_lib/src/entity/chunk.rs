use crate::{
    chunk_manager::ChunkProvider,
    entity::{ ClientComponent, LocationComponent },
};
use mc_networking::packets::client_bound::*;

use ahash::AHashSet;
use bevy_ecs::component::Component;
use bevy_ecs::system::Query;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::Changed;

/// Will call load_chunk for every chunk in radius around it's [ChunkLocationComponent]
#[derive(Component)]
pub struct ChunkObserverComponent {
    pub radius: i32,
    pub loaded_chunks: AHashSet<(i32, i32)>,
    pub chunk_provider: Box<dyn ChunkProvider>,
}

/// Represent the chunk location of an [Entity] with the [ChunkLoaderComponent]
/// This will be automatically updated based on the [LocationComponent]
#[readonly::make]
#[derive(Component, Debug, Clone, Copy)]
pub struct ChunkLocationComponent {
    pub x: i32,
    pub z: i32,
    pub changed: bool,
}
impl ChunkLocationComponent {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            changed: true,
        }
    }
}

/// System to update the [ChunkLocationComponent]
pub(crate) fn chunk_locations_update(
    mut query: Query<(&LocationComponent, &mut ChunkLocationComponent), Changed<LocationComponent>>,
) {
    query.for_each_mut(|(location, mut chunk_loc)| {
        let chunk_x = location.0.chunk_x();
        let chunk_z = location.0.chunk_z();

        chunk_loc.changed = chunk_loc.x != chunk_x || chunk_loc.z != chunk_z;

        chunk_loc.x = chunk_x;
        chunk_loc.z = chunk_z;
    });
}

pub(crate) fn chunk_observer_chunk_loadings(
    mut query: Query<(Entity, &mut ChunkObserverComponent, &ChunkLocationComponent, &ClientComponent)>
) {
    query.for_each_mut(|(entity, mut chunk_observer, chunk_loc, client)| {
        // This system only really runs for observers that just changed chunk
        if !chunk_loc.changed {
            return;
        }

        client.0.send_packet_sync(&C48SetCenterChunk {
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

        // Load close enough chunks from the closests to the farthests
        for square_dist in 0..chunk_observer.radius { // Iterate over chunk distance
            for chunk_dx in -square_dist..square_dist { // Load chunks of that distance
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
    });
}

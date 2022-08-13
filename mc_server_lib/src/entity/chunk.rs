use crate::{
    chunk_manager::ChunkProvider,
    entity::{ ClientComponent, LocationComponent },
};
use mc_networking::packets::client_bound::*;

use std::mem::size_of;

use ahash::AHashSet;
use smallvec::SmallVec;
use bevy_ecs::component::Component;
use bevy_ecs::system::{ Query, Commands };
use bevy_ecs::entity::Entity;
use bevy_ecs::query::Changed;

#[derive(Component)]
#[component(storage = "SparseSet")]
pub struct ForceChunkUpdatesComponent {
    pub targets: SmallVec<[Entity; 2]>,
    pub updates: SmallVec<[(i32, i32); 2]>,
}

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
    pub force_change: u8,
}
impl ChunkLocationComponent {
    pub fn new(x: i32, z: i32) -> Self {
        Self {
            x,
            z,
            changed: false,
            force_change: 0,
        }
    }

    pub fn with_force_change(self, force_change: u8) -> Self {
        Self {
            force_change,
            ..self
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
        if chunk_loc.force_change > 0 {
            chunk_loc.changed = true;
            chunk_loc.force_change -= 1;
        }

        chunk_loc.x = chunk_x;
        chunk_loc.z = chunk_z;
    });
}

pub(crate) fn chunk_observer_chunk_loadings(
    mut query: Query<(Entity, &mut ChunkObserverComponent, &ChunkLocationComponent, &ClientComponent)>,
    force_updates_query: Query<&ForceChunkUpdatesComponent>,
    mut commands: Commands,
) {
    type FcucVec<'a> = SmallVec<[&'a ForceChunkUpdatesComponent; 1]>;
    let force_updates =
        force_updates_query.iter().collect::<FcucVec>();

    query.for_each_mut(|(entity, mut chunk_observer, chunk_loc, client)| {
        // This system only really runs for observers that just changed chunk
        if !chunk_loc.changed {
            return;
        }
        let concerned_fcucs: FcucVec = force_updates.iter().copied()
            .filter(|fcuc| fcuc.targets.contains(&entity)).collect();

        client.0.send_packet_sync(&C48SetCenterChunk {
            chunk_x: chunk_loc.x,
            chunk_z: chunk_loc.z,
        });

        // Unload now too far chunks
        {
            let chunk_loc_x = chunk_loc.x;
            let chunk_loc_z = chunk_loc.z;
            let radius = chunk_observer.radius;
            let ChunkObserverComponent { loaded_chunks, chunk_provider, .. } = &mut *chunk_observer;
            loaded_chunks
                .retain(|(loaded_chunk_x, loaded_chunk_z)| {
                    let distance_x = (loaded_chunk_x - chunk_loc_x).abs();
                    let distance_z = (loaded_chunk_z - chunk_loc_z).abs();
                    let keep = distance_x <= radius && distance_z <= radius;
                    if !keep { 
                        chunk_provider
                            .unload_chunk(entity, &mut commands, *loaded_chunk_x, *loaded_chunk_z);
                    }
                    keep
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
                        let should_force_update = concerned_fcucs
                            .iter().any(|fcuc| fcuc.updates.contains(&(chunk_x, chunk_z)));
                        if should_force_update || !chunk_observer.loaded_chunks.contains(&(chunk_x, chunk_z)) {
                            chunk_observer.loaded_chunks.insert((chunk_x, chunk_z));
                            chunk_observer
                                .chunk_provider
                                .load_chunk(entity, &mut commands, chunk_x, chunk_z);
                        }
                    }
                }
            }
        }
    });
}

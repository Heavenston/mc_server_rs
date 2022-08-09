use mc_ecs_server_lib::entity::{ ClientComponent, NetworkIdComponent, LocationComponent };
use mc_networking::packets::client_bound::*;
use mc_utils::Location;

use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::Query;
use bevy_ecs::component::Component;

#[derive(Component)]
pub struct SpawnPositionComponent(pub Location);

pub fn game_systems() -> SystemSet {
    SystemSet::default()
        .with_system(teleport_if_dead)
}

pub fn teleport_if_dead(
    mut query: Query<(
        &ClientComponent,
        &NetworkIdComponent,
        Option<&SpawnPositionComponent>,
        &mut LocationComponent,
    )>,
) {
    query.for_each_mut(|(client_cp, network_id_cp, spawn_pos, mut location_cp)| {
        if location_cp.0.z > 6.5 && location_cp.0.z < 10.5 {
            return;
        }

        let spawn_pos = spawn_pos.map(|a| a.0).unwrap_or(Location {
            x: 0., y: 50., z: 0.,
            yaw: 0., pitch: 0.
        });
        location_cp.0 = spawn_pos;

        client_cp.0.send_packet_sync(&C63TeleportEntity {
            entity_id: network_id_cp.0,
            x: spawn_pos.x, y: spawn_pos.y, z: spawn_pos.z, yaw: spawn_pos.yaw_angle(), pitch: spawn_pos.pitch_angle(),
            on_ground: false,
        });
        client_cp.0.send_packet_sync(&C36SynchronizePlayerPosition {
            x: spawn_pos.x, y: spawn_pos.y, z: spawn_pos.z, yaw: spawn_pos.yaw, pitch: spawn_pos.pitch,
            flags: 0, teleport_id: 0, dismount_vehicle: false,
        });
    });
}

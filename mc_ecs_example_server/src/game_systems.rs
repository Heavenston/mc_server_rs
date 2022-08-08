use mc_ecs_server_lib::entity::{ ClientComponent, NetworkIdComponent, LocationComponent };
use mc_networking::packets::client_bound::*;
use mc_utils::Location;

use legion::{
    Schedule,
    system
};

pub struct SpawnPositionComponent(pub Location);

pub fn game_scheduler() -> Schedule {
    Schedule::builder()
        .add_system(teleport_if_dead_system())
        .build()
}

#[system(par_for_each)]
pub fn teleport_if_dead(
    client_cp: &ClientComponent,
    network_id_cp: &NetworkIdComponent,
    spawn_pos: &SpawnPositionComponent,
    location_cp: &mut LocationComponent,
) {
    if location_cp.0.y > -10. {
        return;
    }

    let spawn_pos = spawn_pos.0;
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
}

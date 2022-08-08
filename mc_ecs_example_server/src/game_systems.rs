use mc_ecs_server_lib::entity::{ ClientComponent, NetworkIdComponent, LocationComponent };
use mc_networking::packets::client_bound::*;

use legion::{
    Schedule,
    system
};

pub fn game_scheduler() -> Schedule {
    Schedule::builder()
        .add_system(teleport_if_dead_system())
        .build()
}

#[system(par_for_each)]
pub fn teleport_if_dead(
    client_cp: &ClientComponent,
    network_id_cp: &NetworkIdComponent,
    location_cp: &mut LocationComponent,
) {
    if location_cp.0.y > -10. {
        return;
    }

    location_cp.0.x =  0.5;
    location_cp.0.y =  25.;
    location_cp.0.z =  0.5;

    client_cp.0.send_packet_sync(&C63TeleportEntity {
        entity_id: network_id_cp.0,
        x: 0.5, y: 25., z: 0.5, yaw: 0, pitch: 0,
        on_ground: false,
    });
    client_cp.0.send_packet_sync(&C36SynchronizePlayerPosition {
        x: 0.5, y: 25., z: 0.5, yaw: 0., pitch: 0.,
        flags: 0, teleport_id: 0, dismount_vehicle: false,
    });
}

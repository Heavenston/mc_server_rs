use mc_server_lib::entity::{ ClientComponent, LocationComponent };
use mc_networking::packets::client_bound::*;
use mc_utils::Location;

use bevy_ecs::entity::Entity;
use bevy_ecs::schedule::SystemSet;
use bevy_ecs::system::{ Query, Commands };
use bevy_ecs::component::Component;
use bevy_ecs::query::{ With, Added };

#[derive(Component)]
pub struct SpawnPositionComponent(pub Location);

#[derive(Component)]
struct UpdateTimer {
    last_update: u32,
}

pub fn game_systems() -> SystemSet {
    SystemSet::default()
        .with_system(teleport_if_dead)
        .with_system(add_update_timer)
        .with_system(update_status)
}

fn add_update_timer(
    query: Query<Entity, (With<ClientComponent>, Added<LocationComponent>)>,
    mut commands: Commands,
) {
    query.for_each(|e| {
        commands.entity(e).insert(UpdateTimer { last_update: 0 });
    });
}

fn update_status(
    mut query: Query<(&ClientComponent, &LocationComponent, &mut UpdateTimer)>,
) {
    query.for_each_mut(|(client, location, mut timer)| {
        if timer.last_update > 0 {
            timer.last_update -= 1;
            return;
        }
        timer.last_update = 6;
        client.0.send_packet_sync(&C40SetActionBarText {
            text: format!(r#"{{"text": "{:.01}%"}}"#, 100. + (-1. / ((location.0.x - 1.5) / 25. + 1.).max(1.)) * 100.),
        });
    });
}

fn teleport_if_dead(
    mut query: Query<(
        &ClientComponent,
        Option<&SpawnPositionComponent>,
        &mut LocationComponent,
    )>,
) {
    query.for_each_mut(|(client_cp, spawn_pos, mut location_cp)| {
        if location_cp.0.z > 6.5 && location_cp.0.z < 10.5 && 
            location_cp.0.x > -0.3 && location_cp.0.y > 21. {
            return;
        }

        let spawn_pos = spawn_pos.map(|a| a.0).unwrap_or(Location {
            x: 0., y: 50., z: 0.,
            yaw: 0., pitch: 0.
        });
        location_cp.0 = spawn_pos;

        client_cp.0.send_packet_sync(&C36SynchronizePlayerPosition {
            x: spawn_pos.x, y: spawn_pos.y, z: spawn_pos.z, yaw: 0., pitch: 0.,
            flags: 0b11000, teleport_id: 0, dismount_vehicle: false,
        });
    });
}

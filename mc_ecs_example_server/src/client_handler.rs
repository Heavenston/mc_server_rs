use crate::chunk_loader::StoneChunkProvider;
use crate::game_systems::SpawnPositionComponent;
use mc_networking::client::{
    client_event::{ ClientEvent, LoginStartResult },
    Client,
};
use mc_networking::packets::{ client_bound::*, server_bound::* };
use mc_networking::data_types::{ Slot, Position, command_data::RootNode };
use mc_ecs_server_lib::entity::{
    NetworkIdComponent, LocationComponent, ObjectUuidComponent, UsernameComponent,
    ClientComponent,
    chunk::{ ChunkObserverComponent, ChunkLocationComponent }
};
use mc_ecs_server_lib::chunk_manager::ChunkProvider;
use mc_utils::Location;

use std::sync::Arc;

use uuid::Uuid;
use legion::{
    Entity, query::{ IntoQuery, Query }, system, systems::CommandBuffer, world::SubWorld
};
use rayon::prelude::*;
use log::{ debug, info };

pub struct ClientEventsComponent(pub flume::Receiver<ClientEvent>);

#[system(for_each)]
pub fn handle_clients(
    client_component: &ClientComponent, 
    client_events_component: &mut ClientEventsComponent,
    location_component: Option<&mut LocationComponent>,
    object_uuid: Option<&ObjectUuidComponent>,
    username_component: Option<&UsernameComponent>,
    entity: &Entity, cmd: &mut CommandBuffer,
    #[resource] stone_chunk_provider: &Arc<StoneChunkProvider>,
) {
    let chunk_provider: Arc<dyn ChunkProvider> = Arc::clone(stone_chunk_provider) as _;
    if let Ok(event) = client_events_component.0.try_recv() {
        handle_client_event(
            entity, client_component,
            location_component,
            object_uuid, username_component,
            cmd, event, &chunk_provider
        );
    }
}

fn handle_client_event(
    entity: &Entity, client_component: &ClientComponent,
    location_component: Option<&mut LocationComponent>,
    object_uuid: Option<&ObjectUuidComponent>, username_component: Option<&UsernameComponent>,
    cmd: &mut CommandBuffer,
    event: ClientEvent,
    chunk_provider: &Arc<dyn ChunkProvider>,
) {
    match event {
        ClientEvent::ServerListPing { response } => {
            response
                .send(serde_json::from_str(include_str!("slp_response.json")).unwrap())
                .unwrap();
        }

        ClientEvent::LoginStart { username, response } => {
            let uuid = Uuid::new_v3(
                &Uuid::new_v4(),
                format!("OfflinePlayer:{}", username).as_bytes(),
            );
            cmd.add_component(*entity, UsernameComponent(username.clone()));
            cmd.add_component(*entity, ObjectUuidComponent(uuid));

            response
                .send(LoginStartResult::Accept {
                    compress: false,
                    encrypt: false,
                    username, uuid,
                }).unwrap();
        }

        ClientEvent::LoggedIn => {
            let player_username = username_component.map(|a| a.0.clone()).unwrap_or("You".to_string());
            info!("Player {player_username} just logged in");

            let network_id = NetworkIdComponent::new();
            cmd.add_component(*entity, network_id);

            cmd.add_component(*entity, ChunkObserverComponent {
                radius: 12,
                loaded_chunks: Default::default(),
                chunk_provider: Arc::clone(chunk_provider),
            });
            cmd.add_component(*entity, ChunkLocationComponent::new(i32::MAX, i32::MAX));
            
            let spawn_location = Location {
                x: 0., y: 50., z: 8.5, yaw: -90., pitch: 0.,
            };
            cmd.add_component(*entity, LocationComponent(spawn_location));
            cmd.add_component(*entity, SpawnPositionComponent(spawn_location));

            client_component.0.send_packet_sync(&C23Login {
                entity_id: network_id.0,
                is_hardcore: false,
                gamemode: 2,
                previous_gamemode: -1,
                dimension_type: "heav:voidy".into(),
                dimension_name: "heav:voidy".into(),
                dimension_names: vec!["heav:voidy".into()],
                registry_codec: crate::registry_codec::REGISTRY_CODEC.clone(),
                hashed_seed: 0,
                max_players: 2,
                view_distance: 12,
                simulation_distance: 12,
                reduced_debug_info: false,
                enable_respawn_screen: true,
                is_debug: false,
                is_flat: true,
                death_location: None,
            });

            client_component.0.send_packet_sync(&{
                let mut bldr = C15PluginMessageBuilder::new("minecraft:brand".into());
                bldr.encoder.write_string(&username_component.map(|a| a.0.clone()).unwrap());
                bldr.build()
            });

            client_component.0.send_packet_sync(&C2FPlayerAbilities::new(
                true, false, false, false, 1., 0.1
            ));
            client_component.0.send_packet_sync(&C47SetHeldItem {
                slot: 3,
            });

            let default_player = C34AddPlayer {
                uuid: Uuid::new_v4(),
                name: "".to_string(),
                properties: vec![],
                gamemode: 0,
                ping: 0,
                display_name: None,
                sig_data: (),
            };
            client_component.0.send_packet_sync(&C34PlayerInfo::AddPlayers {
                players: vec![
                    C34AddPlayer {
                        uuid: object_uuid.map(|a| a.0.clone()).unwrap_or(Uuid::new_v4()),
                        name: player_username.clone(),
                        ..default_player.clone()
                    },
                    C34AddPlayer {
                        uuid: Uuid::new_v4(),
                        name: username_component.map(|a| a.0.clone()).unwrap_or("You".to_string()) + "2",
                        display_name: Some(format!(r#"{{"text": "{}", "strikethrough": true}}"#, player_username)),
                        ..default_player.clone()
                    },
                ],
            });

            client_component.0.send_packet_sync(&C4ASetDefaultSpawnPosition {
                location: spawn_location.block_position(),
                angle: spawn_location.pitch,
            });
            client_component.0.send_packet_sync(&C63TeleportEntity {
                entity_id: network_id.0,
                x: spawn_location.x, y: spawn_location.y, z: spawn_location.z,
                yaw: spawn_location.yaw_angle(), pitch: spawn_location.pitch_angle(),
                on_ground: false,
            });
            client_component.0.send_packet_sync(&C36SynchronizePlayerPosition {
                x: spawn_location.x, y: spawn_location.y, z: spawn_location.z,
                yaw: spawn_location.yaw, pitch: spawn_location.pitch,
                flags: 0, teleport_id: 0, dismount_vehicle: false,
            });
        }

        ClientEvent::Logout => {
            cmd.remove(*entity);
        }

        ClientEvent::PluginMessage(S0CPluginMessage { channel, data }) => {
            debug!("Received {channel:?}: {}", String::from_utf8_lossy(&data));
        }

        ClientEvent::SetPlayerPosition(p) => {
            let location_cp = if let Some(a) = location_component {
                a
            } else { return };

            location_cp.0.x = p.x;
            location_cp.0.y = p.feet_y;
            location_cp.0.z = p.z;
        },
        ClientEvent::SetPlayerPositionAndRotation(p) => {
            let location_cp = if let Some(a) = location_component {
                a
            } else { return };

            location_cp.0.x = p.x;
            location_cp.0.y = p.feet_y;
            location_cp.0.z = p.z;

            location_cp.0.yaw = p.yaw;
            location_cp.0.pitch = p.pitch;
        },
        ClientEvent::SetPlayerRotation(p) => {
            let location_cp = if let Some(a) = location_component {
                a
            } else { return };

            location_cp.0.yaw = p.yaw;
            location_cp.0.pitch = p.pitch;
        },

        _ => (),
    }
}

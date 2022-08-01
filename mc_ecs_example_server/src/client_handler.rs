use crate::chunk_loader::StoneChunkProvider;
use mc_networking::client::{
    client_event::{ ClientEvent, LoginStartResult },
    Client,
};
use mc_networking::packets::client_bound::*;
use mc_networking::data_types::{ Slot, Position, command_data::RootNode };
use mc_ecs_server_lib::entity::{
    NetworkIdComponent, LocationComponent, ObjectUuidComponent, UsernameComponent,
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

pub struct ClientComponent {
    pub client: Client,
    pub event_receiver: flume::Receiver<ClientEvent>,
}

#[system]
#[read_component(ClientComponent)]
pub fn test_clients(
    world: &mut SubWorld,
    cmd: &mut CommandBuffer,
) {
    <(Entity, &ClientComponent)>::query()
        .for_each(world, |(entity, _)| {
            let ent = *entity;
            println!("Client");
            cmd.exec_mut(move |world, _| {
                let entry = world.entry(ent).unwrap();
                println!(
                    "{:?} has {:?}",
                    ent,
                    entry.archetype().layout().component_types().into_iter().map(|a| format!("{a}")).collect::<Vec<_>>()
                );
            });
        });
}

#[system(for_each)]
pub fn handle_clients(
    client_component: &mut ClientComponent, object_uuid: Option<&ObjectUuidComponent>,
    username_component: Option<&UsernameComponent>,
    entity: &Entity, cmd: &mut CommandBuffer,
    #[resource] stone_chunk_provider: &Arc<StoneChunkProvider>,
) {
    let chunk_provider: Arc<dyn ChunkProvider> = Arc::clone(stone_chunk_provider) as _;
    if let Ok(event) = client_component.event_receiver.try_recv() {
        handle_client_event(
            entity, client_component, object_uuid, username_component,
            cmd, event, &chunk_provider
        );
    }
}

fn handle_client_event(
    entity: &Entity, client_component: &mut ClientComponent,
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
                    compress: true,
                    encrypt: false,
                    username, uuid,
                }).unwrap();
        }

        ClientEvent::LoggedIn => {
            let network_id = NetworkIdComponent::new();
            cmd.add_component(*entity, network_id);

            cmd.add_component(*entity, ChunkObserverComponent {
                radius: 23,
                loaded_chunks: Default::default(),
                chunk_provider: Arc::clone(chunk_provider),
            });
            cmd.add_component(*entity, ChunkLocationComponent::new(i32::MAX, i32::MAX));
            cmd.add_component(*entity, LocationComponent(Location {
                x: 0., y: 100., z: 0., yaw: 0., pitch: 0.,
            }));

            client_component.client.send_packet_sync(&C23Login {
                entity_id: network_id.0,
                is_hardcore: false,
                gamemode: 0,
                previous_gamemode: -1,
                dimension_type: "heav:voidy".into(),
                dimension_name: "heav:voidy".into(),
                dimension_names: vec!["heav:voidy".into()],
                registry_codec: crate::registry_codec::REGISTRY_CODEC.clone(),
                hashed_seed: 0,
                max_players: 0,
                view_distance: 23,
                simulation_distance: 23,
                reduced_debug_info: false,
                enable_respawn_screen: true,
                is_debug: false,
                is_flat: true,
                death_location: None,
            });

            client_component.client.send_packet_sync(&C34PlayerInfo::AddPlayers {
                players: vec![C34AddPlayer {
                    uuid: object_uuid.unwrap().0.clone(),
                    name: username_component.unwrap().0.clone(),
                    properties: vec![],
                    gamemode: 1,
                    ping: 10000,
                    display_name: None,
                    sig_data: (),
                }],
            });

            client_component.client.send_packet_sync(&C4ASetDefaultSpawnPosition {
                location: Position {
                    x: 0, y: 100, z: 0,
                },
                angle: 0.,
            });
            client_component.client.send_packet_sync(&C2FPlayerAbilities::new(
                true, false, true, true, 1., 1.
            ));
            client_component.client.send_packet_sync(&C0FCommands {
                root_node: Arc::new(RootNode {
                    is_executable: false,
                    children_nodes: vec![],
                    redirect_node: None,
                })
            });
            client_component.client.send_packet_sync(&C63TeleportEntity {
                entity_id: network_id.0,
                x: 0., y: 100., z: 0., yaw: 0, pitch: 0,
                on_ground: false,
            });
            client_component.client.send_packet_sync(&C36SynchronizePlayerPosition {
                x: 0., y: 0., z: 0., yaw: 0., pitch: 0.,
                flags: 0, teleport_id: 0, dismount_vehicle: false,
            });
            client_component.client.send_packet_sync(&C48SetCenterChunk {
                chunk_x: 0,
                chunk_z: 0,
            });
            client_component.client.send_packet_sync(&C11SetContainerContent {
                window_id: 0, state_id: 0,
                slots: vec![Slot::NotPresent; 51], carried_item: Slot::NotPresent,
            });
        }

        ClientEvent::Logout => {
            cmd.remove(*entity);
        }

        _ => (),
    }
}

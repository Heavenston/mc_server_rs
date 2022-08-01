use crate::chunk_loader::StoneChunkProvider;
use mc_networking::client::{
    client_event::{ ClientEvent, LoginStartResult },
    Client,
};
use mc_networking::packets::client_bound::*;
use mc_ecs_server_lib::entity::{
    NetworkIdComponent, LocationComponent, ObjectUuidComponent, UsernameComponent,
    chunk::{ ChunkObserverComponent, ChunkLocationComponent }
};
use mc_ecs_server_lib::chunk_manager::ChunkProvider;
use mc_utils::Location;

use std::sync::Arc;

use uuid::Uuid;
use legion::{
    Entity, IntoQuery, system, systems::CommandBuffer, world::SubWorld
};
use rayon::prelude::*;

pub struct ClientComponent {
    pub client: Client,
    pub event_receiver: flume::Receiver<ClientEvent>,
}

#[system(for_each)]
#[write_component(ClientComponent)]
pub fn handle_clients(
    client_component: &mut ClientComponent,
    entity: &Entity, cmd: &mut CommandBuffer,
    #[resource] stone_chunk_provider: &Arc<StoneChunkProvider>,
) {
    let chunk_provider: Arc<dyn ChunkProvider> = Arc::clone(stone_chunk_provider) as _;
    while let Ok(event) = client_component.event_receiver.try_recv() {
        handle_client_event(entity, client_component, cmd, event, &chunk_provider);
    }
}

fn handle_client_event(
    entity: &Entity,
    client_component: &mut ClientComponent,
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
            cmd.add_component(*entity, ChunkLocationComponent::new(0, 0));
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
        }

        ClientEvent::Logout => {
            cmd.remove(*entity);
        }

        _ => (),
    }
}

use crate::entity::player::Player;
use crate::entity::BoxedEntity;
use crate::entity_pool::EntityPool;
use mc_networking::client::client_event::*;
use mc_networking::client::Client;
use mc_networking::map;
use mc_networking::packets::client_bound::*;
use mc_utils::{ChunkData, Location};

use anyhow::{Error, Result};
use log::*;
use serde_json::json;
use std::sync::Arc;
use tokio::net::{TcpListener, ToSocketAddrs};
use tokio::stream::StreamExt;
use tokio::sync::{Mutex, RwLock};
use tokio::time::Duration;
use uuid::Uuid;

pub struct Server {
    entity_pool: Arc<RwLock<EntityPool>>,
    entity_id_counter: i32,
    max_players: u16,
    view_distance: u16,
    brand: String,
}

impl Server {
    pub fn new() -> Self {
        Self {
            entity_pool: Arc::new(RwLock::new(EntityPool::new(10 * 16))),
            entity_id_counter: 0,
            max_players: 10,
            view_distance: 10,
            brand: "BEST SERVER EVER".to_string(),
        }
    }
    pub async fn listen(server: Arc<RwLock<Server>>, addr: impl ToSocketAddrs) -> Result<()> {
        let mut listener = TcpListener::bind(addr).await?;
        loop {
            let (socket, ..) = listener.accept().await?;
            let (client, event_receiver) = Client::new(socket);
            let client = Arc::new(Mutex::new(client));

            tokio::task::spawn({
                let server = Arc::clone(&server);
                let client = Arc::clone(&client);
                async move {
                    Server::handle_client(server, client, event_receiver)
                        .await
                        .unwrap();
                }
            });
        }
    }
    async fn handle_client(
        server: Arc<RwLock<Server>>,
        client: Arc<Mutex<Client>>,
        mut event_receiver: tokio::sync::mpsc::Receiver<ClientEvent>,
    ) -> Result<()> {
        let mut player: Option<Arc<RwLock<BoxedEntity>>> = None;
        let mut player_eid = -1i32;
        let entity_pool = Arc::clone(&server.read().await.entity_pool);

        while let Some(event) = event_receiver.recv().await {
            match event {
                ClientEvent::ServerListPing { response } => {
                    response
                        .send(json!({
                            "version": {
                                "name": "1.16.3",
                                "protocol": 753
                            },
                            "players": {
                                "max": server.read().await.max_players,
                                "online": entity_pool.read().await.get_players().len(),
                                "sample": []
                            },
                            "description": "Hi"
                        }))
                        .unwrap();
                }
                ClientEvent::LoginStart { response, username } => {
                    if (server.read().await.max_players as usize)
                        <= entity_pool.read().await.get_players().len()
                    {
                        response
                            .send(LoginStartResult::Disconnect {
                                reason: "The server is full :(".to_string(),
                            })
                            .unwrap();
                    } else {
                        let mut server_write = server.write().await;
                        server_write.entity_id_counter += 1;
                        player_eid = server_write.entity_id_counter;
                        let uuid = Uuid::new_v4();
                        let entity = Arc::new(RwLock::new(BoxedEntity::new(Player::new(
                            username.clone(),
                            server_write.entity_id_counter,
                            uuid.clone(),
                            Arc::clone(&client),
                        ))));
                        player = Some(entity);

                        info!(
                            "{} joined the game, EID: {}, UUID: {}",
                            username.clone(),
                            player_eid,
                            uuid.clone()
                        );

                        response
                            .send(LoginStartResult::Accept {
                                uuid: uuid.clone(),
                                username,
                            })
                            .unwrap();
                    }
                }
                ClientEvent::LoggedIn => {
                    let player = player.as_ref().unwrap();

                    // Join Game
                    client
                        .lock()
                        .await
                        .send_packet(&{
                            let server = server.read().await;
                            let player = player.read().await;
                            let player = player.as_player().unwrap();

                            C24JoinGame {
                                entity_id: player.entity_id,
                                is_hardcore: false,
                                gamemode: player.gamemode,
                                previous_gamemode: player.gamemode,
                                world_names: vec!["heav:world".to_owned()],
                                dimension_codec: C24JoinGameDimensionCodec {
                                    dimensions: map! {
                                        "heav:world".to_owned() => C24JoinGameDimensionElement {
                                            natural: 1,
                                            ambient_light: 1.0,
                                            has_ceiling: 0,
                                            has_skylight: 1,
                                            fixed_time: 6000,
                                            shrunk: 0,
                                            ultrawarm: 0,
                                            has_raids: 0,
                                            respawn_anchor_works: 0,
                                            bed_works: 0,
                                            coordinate_scale: 1.0,
                                            piglin_safe: 0,
                                            logical_height: 256,
                                            infiniburn: "".to_owned(),
                                        }
                                    },
                                    biomes: map! {
                                        "minecraft:plains".to_owned() => C24JoinGameBiomeElement {
                                            precipitation: "none".to_owned(),
                                            effects: C24JoinGameBiomeEffects {
                                                sky_color: 7907327,
                                                water_fog_color: 329011,
                                                fog_color: 12638463,
                                                water_color: 4159204,
                                                mood_sound: C24JoinGameBiomeEffectsMoodSound {
                                                    tick_delay: 6000,
                                                    offset: 2.0,
                                                    sound: "minecraft:ambient.cave".to_owned(),
                                                    block_search_extent: 8,
                                                }
                                            },
                                            depth: 0.125,
                                            temperature: 0.8,
                                            scale: 0.5,
                                            downfall: 0.4,
                                            category: "none".to_owned(),
                                        },
                                        "heav:plot".to_owned() => C24JoinGameBiomeElement {
                                            precipitation: "none".to_owned(),
                                            effects: C24JoinGameBiomeEffects {
                                                sky_color: 0x7BA4FF,
                                                water_fog_color: 0x050533,
                                                fog_color: 0xC0D8FF,
                                                water_color: 0x3F76E4,
                                                mood_sound: C24JoinGameBiomeEffectsMoodSound {
                                                    tick_delay: 6000,
                                                    offset: 2.0,
                                                    sound: "minecraft:ambient.cave".to_owned(),
                                                    block_search_extent: 8,
                                                }
                                            },
                                            depth: 0.1,
                                            temperature: 0.5,
                                            scale: 0.2,
                                            downfall: 0.5,
                                            category: "none".to_owned(),
                                        }
                                    },
                                },
                                dimension: C24JoinGameDimensionElement {
                                    natural: 1,
                                    ambient_light: 1.0,
                                    has_ceiling: 0,
                                    has_skylight: 1,
                                    fixed_time: 6000,
                                    shrunk: 0,
                                    ultrawarm: 0,
                                    has_raids: 0,
                                    respawn_anchor_works: 0,
                                    bed_works: 0,
                                    coordinate_scale: 1.0,
                                    piglin_safe: 0,
                                    logical_height: 256,
                                    infiniburn: "".to_owned(),
                                },
                                world_name: "heav:world".to_owned(),
                                hashed_seed: 0,
                                max_players: server.max_players as i32,
                                view_distance: server.view_distance as i32,
                                reduced_debug_info: false,
                                enable_respawn_screen: true,
                                is_debug: false,
                                is_flat: true,
                            }
                        })
                        .await
                        .unwrap();

                    let my_player_info = C32PlayerInfoPlayerUpdate::AddPlayer {
                        uuid: player.read().await.uuid().clone(),
                        name: player.read().await.as_player().unwrap().username.clone(),
                        properties: vec![],
                        gamemode: player.read().await.as_player().unwrap().gamemode as i32,
                        ping: player.read().await.as_player().unwrap().ping,
                        display_name: None,
                    };
                    // Send to him all players (and himself)
                    {
                        let entity_pool = entity_pool.read().await;
                        let players = {
                            let mut players = vec![my_player_info.clone()];
                            for (.., player) in entity_pool.get_players().iter() {
                                let player = player.read().await;
                                let player = player.as_player().unwrap();
                                players.push(C32PlayerInfoPlayerUpdate::AddPlayer {
                                    uuid: player.uuid.clone(),
                                    name: player.username.clone(),
                                    properties: vec![],
                                    gamemode: player.gamemode as i32,
                                    ping: player.ping,
                                    display_name: None,
                                });
                            }
                            players
                        };
                        client
                            .lock()
                            .await
                            .send_packet(&C32PlayerInfo { players })
                            .await
                            .unwrap();
                    }
                    // Send to all his player info
                    entity_pool
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![my_player_info],
                        })
                        .await
                        .unwrap();

                    {
                        let player = player.read().await;
                        let player = player.as_player().unwrap();

                        client
                            .lock()
                            .await
                            .send_player_abilities(
                                player.invulnerable,
                                player.is_flying,
                                player.can_fly,
                                player.gamemode == 1,
                                player.flying_speed,
                                player.fov_modifier,
                            )
                            .await
                            .unwrap();
                    }

                    entity_pool
                        .write()
                        .await
                        .add_entity(Arc::clone(player))
                        .await;
                    entity_pool
                        .write()
                        .await
                        .add_player(Arc::clone(player))
                        .await;

                    // Send server brand
                    {
                        let brand = server.read().await.brand.clone();
                        entity_pool
                            .read()
                            .await
                            .send_to_player(player_eid, &{
                                let mut builder =
                                    C17PluginMessageBuilder::new("minecraft:brand".to_string());
                                builder.encoder.write_string(&brand);
                                builder.build()
                            })
                            .await
                            .unwrap();
                    }

                    server
                        .read()
                        .await
                        .update_player_view_position(player_eid)
                        .await
                        .unwrap();
                    // Update position
                    entity_pool
                        .read()
                        .await
                        .teleport_entity(
                            player_eid,
                            Location {
                                x: 0.0,
                                y: 20.0,
                                z: 0.0,
                                yaw: 0.0,
                                pitch: 0.0,
                            },
                        )
                        .await;
                }
                ClientEvent::Logout => {
                    entity_pool.write().await.remove_entity(player_eid);
                    let uuid = player.unwrap().read().await.uuid().clone();
                    entity_pool
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::RemovePlayer { uuid }],
                        })
                        .await
                        .unwrap();
                    break;
                }

                ClientEvent::Ping { delay } => {
                    let player = player.as_ref().unwrap();
                    player.write().await.as_player_mut().unwrap().ping = delay as i32;
                    let uuid = player.read().await.as_player().unwrap().uuid.clone();
                    entity_pool
                        .read()
                        .await
                        .broadcast(&C32PlayerInfo {
                            players: vec![C32PlayerInfoPlayerUpdate::UpdateLatency {
                                uuid: uuid.clone(),
                                ping: delay as i32,
                            }],
                        })
                        .await
                        .unwrap();
                }

                ClientEvent::ChatMessage { message } => {
                    let player = player.as_ref().unwrap();
                    entity_pool
                        .read()
                        .await
                        .broadcast(&C0EChatMessage {
                            json_data: json!({
                                "text":
                                    format!(
                                        "<{}> {}",
                                        player.read().await.as_player().unwrap().username,
                                        message
                                    )
                            }),
                            position: 0,
                            sender: Some(player.read().await.uuid().clone()),
                        })
                        .await
                        .unwrap();
                }
                ClientEvent::PlayerPosition { x, y, z, on_ground } => {
                    let last_location = player.as_ref().unwrap().read().await.location().clone();
                    let new_location = Location {
                        x,
                        y,
                        z,
                        yaw: last_location.yaw,
                        pitch: last_location.pitch,
                    };
                    {
                        let mut player = player.as_ref().unwrap().write().await;
                        player.set_on_ground(on_ground);
                        player.set_location(new_location.clone());
                    }
                    let player_id = player.as_ref().unwrap().read().await.entity_id();
                    if last_location.chunk_x() != new_location.chunk_x()
                        || last_location.chunk_z() != new_location.chunk_z()
                    {
                        server
                            .write()
                            .await
                            .update_player_view_position(player_id)
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerPositionAndRotation {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    let last_location = player.as_ref().unwrap().read().await.location().clone();
                    let new_location = Location {
                        x,
                        y,
                        z,
                        yaw,
                        pitch,
                    };
                    {
                        let mut player = player.as_ref().unwrap().write().await;
                        player.set_on_ground(on_ground);
                        player.set_location(new_location.clone());
                    }
                    let player_id = player.as_ref().unwrap().read().await.entity_id();
                    if last_location.chunk_x() != new_location.chunk_x()
                        || last_location.chunk_z() != new_location.chunk_z()
                    {
                        server
                            .write()
                            .await
                            .update_player_view_position(player_id)
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerRotation {
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    let mut player = player.as_ref().unwrap().write().await;
                    player.set_on_ground(on_ground);
                    let location = player.location().clone();
                    player.set_location(Location {
                        x: location.x,
                        y: location.y,
                        z: location.z,
                        yaw,
                        pitch,
                    });
                }

                ClientEvent::EntityAction {
                    entity_id,
                    action_id,
                    ..
                } => {
                    if entity_id == player_eid {
                        {
                            let mut player = player.as_ref().unwrap().write().await;
                            let mut player = player.as_player_mut().unwrap();
                            match action_id {
                                0 => {
                                    player.is_sneaking = true;
                                }
                                1 => {
                                    player.is_sneaking = false;
                                }
                                3 => {
                                    player.is_sprinting = true;
                                }
                                4 => {
                                    player.is_sprinting = false;
                                }
                                _ => (),
                            }
                        }
                        entity_pool
                            .read()
                            .await
                            .update_entity_metadata(player_eid)
                            .await
                            .unwrap();
                    }
                }
                ClientEvent::PlayerAbilities { is_flying } => {
                    player
                        .as_ref()
                        .unwrap()
                        .write()
                        .await
                        .as_player_mut()
                        .unwrap()
                        .is_flying = is_flying;
                    entity_pool
                        .read()
                        .await
                        .update_entity_metadata(player_eid)
                        .await
                        .unwrap();
                }
            }
        }

        Ok(())
    }

    async fn update_player_view_position(&self, player_id: i32) -> Result<()> {
        let entity = self
            .entity_pool
            .read()
            .await
            .get_player(player_id)
            .ok_or(Error::msg("No player found"))?;
        let location = entity.read().await.location().clone();

        let (chunk_x, chunk_z) = (location.chunk_x(), location.chunk_z());

        self.entity_pool
            .read()
            .await
            .send_to_player(player_id, &C40UpdateViewPosition { chunk_x, chunk_z })
            .await
            .unwrap();
        let loaded_chunks = entity.read().await.as_player()?.loaded_chunks.clone();

        let view_distance = self.view_distance as i32;
        let view_distance2 = self.view_distance.pow(2) as i32;

        for (x, z) in loaded_chunks.iter().cloned() {
            if loaded_chunks.contains(&(x, z))
                && ((chunk_x - x).pow(2) + (chunk_z - z).pow(2)) >= view_distance2
            {
                self.entity_pool
                    .read()
                    .await
                    .send_to_player(
                        player_id,
                        &C1CUnloadChunk {
                            chunk_x: x,
                            chunk_z: z,
                        },
                    )
                    .await
                    .unwrap();
                entity
                    .write()
                    .await
                    .as_player_mut()?
                    .loaded_chunks
                    .remove(&(x, z));
            }
        }
        for x in chunk_x - (view_distance * 2)..=chunk_x + (view_distance * 2) {
            for z in chunk_z - (view_distance * 2)..=chunk_z + (view_distance * 2) {
                if !loaded_chunks.contains(&(x, z))
                    && (chunk_x - x).pow(2) + (chunk_z - z).pow(2) <= view_distance2
                {
                    let chunk_data = self.get_chunk(x, z).await;
                    self.entity_pool
                        .read()
                        .await
                        .send_to_player(player_id, &chunk_data)
                        .await
                        .unwrap();
                    entity
                        .write()
                        .await
                        .as_player_mut()?
                        .loaded_chunks
                        .insert((x, z));
                }
            }
        }
        Ok(())
    }
    async fn get_chunk(&self, chunk_x: i32, chunk_z: i32) -> C20ChunkData {
        let mut chunk_data = ChunkData::new();

        for x in 0..16 {
            for z in 0..16 {
                chunk_data.set_block(x, 5, z, 1);
            }
        }

        chunk_data.encode(chunk_x, chunk_z)
    }

    pub async fn start_ticker(server: Arc<RwLock<Server>>) {
        tokio::task::spawn(async move {
            let mut tps_interval = tokio::time::interval(Duration::from_secs_f64(1.0 / 20.0));
            let ticks = Arc::new(RwLock::new(0i32));
            tokio::task::spawn({
                let ticks = Arc::clone(&ticks);
                async move {
                    loop {
                        tokio::time::delay_for(Duration::from_secs(10)).await;
                        let n = (*ticks.read().await as f64) / 10f64;
                        *ticks.write().await = 0;
                        info!("{} TPS", n);
                    }
                }
            });

            loop {
                *ticks.write().await += 1;
                server.write().await.tick().await;
                tps_interval.next().await;
            }
        });
    }
    pub async fn tick(&mut self) {
        self.entity_pool.write().await.tick().await;
    }
}

use mc_networking::client::{client_event::*, Client};
use mc_networking::map;

use crate::location::Location;
use log::*;
use mc_networking::data_types::bitbuffer::BitBuffer;
use mc_networking::data_types::{Slot, MetadataValue};
use mc_networking::packets::client_bound::*;
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub struct Player {
    pub server: Arc<RwLock<Server>>,
    pub client: Arc<Mutex<Client>>,
    pub uuid: Uuid,
    pub entity_id: i32,
    pub username: String,
    pub ping: i32,
    pub gamemode: u8,
    pub on_ground: bool,
    pub is_sneaking: bool,
    location: Location,
    loaded_players: HashSet<i32>,
}
impl Player {
    pub fn new(
        server: Arc<RwLock<Server>>,
        client: Arc<Mutex<Client>>,
        uuid: Uuid,
        entity_id: i32,
        username: String,
        ping: i32,
        gamemode: u8,
    ) -> Self {
        Self {
            server,
            client,
            uuid,
            entity_id,
            username,
            ping,
            gamemode,
            on_ground: false,
            location: Location::default(),
            loaded_players: HashSet::new(),
            is_sneaking: false,
        }
    }

    pub async fn send_brand(&self, brand: String) {
        let brand = {
            let mut builder = C17PluginMessageBuilder::new("minecraft:brand".to_string());
            builder.encoder.write_string(&brand);
            builder.build()
        };
        self.client
            .lock()
            .await
            .send_plugin_message(&brand)
            .await
            .unwrap();
    }

    async fn broadcast_to_player_in_viewdistance<T: ClientBoundPacket>(&self, packet: &T) {
        let view_distance2 = (self.server.read().await.view_distance as f64 * 16.0).powf(2.0);
        for (entity_id, player) in self.server.read().await.players.iter() {
            if *entity_id == self.entity_id {
                continue;
            };
            let player = player.read().await;
            if player.location.distance2(&self.location) < view_distance2 {
                unsafe { player.client.lock().await.send_packet(packet) }
                    .await
                    .unwrap();
            }
        }
    }

    /// Change current location
    pub fn set_location(&mut self, new_location: Location) {
        self.location = new_location;
    }
    pub async fn set_position(&mut self, x: f64, y: f64, z: f64) {
        let previous_location = self.location.clone();
        let new_location = Location {
            x,
            y,
            z,
            yaw: self.location.yaw,
            pitch: self.location.pitch,
        };
        self.location = new_location.clone();

        if previous_location.distance2(&new_location) > 8.0 * 8.0 {
            self.teleport(new_location.clone()).await;
        } else {
            self.broadcast_to_player_in_viewdistance(&C27EntityPosition {
                entity_id: self.entity_id,
                delta_x: ((new_location.x * 32f64 - previous_location.x * 32f64) * 128f64).floor()
                    as i16,
                delta_y: ((new_location.y * 32f64 - previous_location.y * 32f64) * 128f64).floor()
                    as i16,
                delta_z: ((new_location.z * 32f64 - previous_location.z * 32f64) * 128f64).floor()
                    as i16,
                on_ground: self.on_ground,
            })
            .await;
        }

        if previous_location.chunk_x() != new_location.chunk_x()
            || previous_location.chunk_z() != new_location.chunk_z()
        {
            self.client
                .lock()
                .await
                .update_view_position(new_location.chunk_x(), new_location.chunk_z())
                .await
                .unwrap();
        }
    }
    pub async fn set_rotation(&mut self, yaw: f32, pitch: f32) {
        self.location.yaw = yaw;
        self.location.pitch = pitch;
        self.broadcast_to_player_in_viewdistance(&C29EntityRotation {
            entity_id: self.entity_id,
            yaw: self.location.yaw_angle(),
            pitch: self.location.pitch_angle(),
            on_ground: self.on_ground,
        })
        .await;
        self.broadcast_to_player_in_viewdistance(&C3AEntityHeadLook {
            entity_id: self.entity_id,
            head_yaw: self.location.yaw_angle(),
        })
        .await;
    }
    pub async fn set_position_and_rotation(
        &mut self,
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
    ) {
        let previous_location = self.location.clone();
        let new_location = Location {
            x,
            y,
            z,
            yaw,
            pitch,
        };
        self.location = new_location.clone();

        if previous_location.distance2(&new_location) > 8.0 * 8.0 {
            self.teleport(new_location.clone()).await;
        } else {
            self.broadcast_to_player_in_viewdistance(&C28EntityPositionAndRotation {
                entity_id: self.entity_id,
                delta_x: ((new_location.x * 32f64 - previous_location.x * 32f64) * 128f64).floor()
                    as i16,
                delta_y: ((new_location.y * 32f64 - previous_location.y * 32f64) * 128f64).floor()
                    as i16,
                delta_z: ((new_location.z * 32f64 - previous_location.z * 32f64) * 128f64).floor()
                    as i16,
                yaw: self.location.yaw_angle(),
                pitch: self.location.pitch_angle(),
                on_ground: self.on_ground,
            })
            .await;
            self.broadcast_to_player_in_viewdistance(&C3AEntityHeadLook {
                entity_id: self.entity_id,
                head_yaw: self.location.yaw_angle(),
            })
            .await;
        }

        if previous_location.chunk_x() != new_location.chunk_x()
            || previous_location.chunk_z() != new_location.chunk_z()
        {
            self.client
                .lock()
                .await
                .update_view_position(new_location.chunk_x(), new_location.chunk_z())
                .await
                .unwrap();
        }
    }

    /// Change current location and send location to the client
    pub async fn teleport(&mut self, new_location: Location) {
        self.location = new_location;
        self.client
            .lock()
            .await
            .player_position_and_look(&C34PlayerPositionAndLook {
                x: self.location.x,
                y: self.location.y,
                z: self.location.z,
                yaw: self.location.yaw,
                pitch: self.location.pitch,
                flags: 0,
                teleport_id: 0,
            })
            .await
            .unwrap();
        self.broadcast_to_player_in_viewdistance(&C56EntityTeleport {
            entity_id: self.entity_id,
            x: self.location.x,
            y: self.location.y,
            z: self.location.z,
            yaw: self.location.yaw_angle(),
            pitch: self.location.pitch_angle(),
            on_ground: self.on_ground,
        })
        .await;
        self.broadcast_to_player_in_viewdistance(&C3AEntityHeadLook {
            entity_id: self.entity_id,
            head_yaw: self.location.yaw_angle(),
        })
        .await;
    }

    pub async fn update_player_entities(&mut self) {
        let view_distance2 = (self.server.read().await.view_distance as f64).powf(2.0);
        for (eid, a_player) in self.server.read().await.players.iter() {
            if self.entity_id == *eid {
                continue;
            };

            let a_player = a_player.read().await;
            let is_in_range = a_player.location.distance2(&self.location) < view_distance2;
            if is_in_range && !self.loaded_players.contains(&a_player.entity_id) {
                self.client
                    .lock()
                    .await
                    .spawn_player(&C04SpawnPlayer {
                        entity_id: a_player.entity_id,
                        uuid: a_player.uuid.clone(),
                        x: a_player.location.x,
                        y: a_player.location.y,
                        z: a_player.location.z,
                        yaw: a_player.location.yaw_angle(),
                        pitch: a_player.location.pitch_angle(),
                    })
                    .await
                    .unwrap();
                self.loaded_players.insert(a_player.entity_id);
            }
            if !is_in_range && self.loaded_players.contains(&a_player.entity_id) {
                self.client
                    .lock()
                    .await
                    .destroy_entities(vec![a_player.entity_id])
                    .await
                    .unwrap();
                self.loaded_players.remove(&a_player.entity_id);
            }
        }
    }
    pub async fn update_metadata(&self) {
        self.broadcast_to_player_in_viewdistance(&C44EntityMetadata {
            entity_id: self.entity_id,
            metadata: map! {
                0 => MetadataValue::Byte((self.is_sneaking as u8) * 0x02)
            }
        }).await;
    }
}

pub struct Server {
    pub players: HashMap<i32, Arc<RwLock<Player>>>,
    pub entity_id_counter: i32,
    pub max_players: u16,
    pub view_distance: i32,
}
impl Server {
    pub fn new() -> Self {
        Self {
            players: HashMap::new(),
            entity_id_counter: 0,
            max_players: 10,
            view_distance: 10,
        }
    }
}

pub async fn handle_client(server: Arc<RwLock<Server>>, socket: TcpStream) {
    let (client, mut event_receiver) = Client::new(socket);
    let client = Arc::new(Mutex::new(client));

    tokio::task::spawn(async move {
        let mut player: Option<Arc<RwLock<Player>>> = None;

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
                                "online": server.read().await.players.len(),
                                "sample": []
                            },
                            "description": "Hi"
                        }))
                        .unwrap();
                }
                ClientEvent::LoginStart { response, username } => {
                    let mut server_write = server.write().await;
                    if (server_write.max_players as usize) <= server_write.players.len() {
                        response
                            .send(LoginStartResult::Disconnect {
                                reason: "The server is full :(".to_string(),
                            })
                            .unwrap();
                    } else {
                        server_write.entity_id_counter += 1;
                        player = Some(Arc::new(RwLock::new(Player::new(
                            Arc::clone(&server),
                            Arc::clone(&client),
                            Uuid::new_v3(
                                &Uuid::new_v4(),
                                format!("OfflinePlayer;{}", username.clone()).as_bytes(),
                            ),
                            server_write.entity_id_counter,
                            username.clone(),
                            -1,
                            1,
                        ))));
                        server_write.players.insert(
                            player.as_ref().unwrap().read().await.entity_id,
                            Arc::clone(player.as_ref().unwrap()),
                        );

                        response
                            .send(LoginStartResult::Accept {
                                uuid: player.as_ref().unwrap().read().await.uuid.clone(),
                                username,
                            })
                            .unwrap();
                    }
                }
                ClientEvent::LoggedIn => {
                    let player = player.as_ref().unwrap();

                    client
                        .lock()
                        .await
                        .join_game(&{
                            let server = server.read().await;
                            let player = player.read().await;

                            C24JoinGame {
                                entity_id: 0,
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
                                view_distance: server.view_distance,
                                reduced_debug_info: false,
                                enable_respawn_screen: true,
                                is_debug: false,
                                is_flat: true,
                            }
                        })
                        .await
                        .unwrap();
                    player
                        .write()
                        .await
                        .teleport(Location {
                            x: 0.0,
                            y: 20.0,
                            z: 0.0,
                            yaw: 0.0,
                            pitch: 0.0,
                        })
                        .await;

                    player.read().await.send_brand("HMC".to_string()).await;

                    client
                        .lock()
                        .await
                        .send_player_info(&C32PlayerInfo {
                            players: {
                                let server = server.read().await;

                                let mut players = vec![];
                                for (.., player) in server.players.iter() {
                                    let player = player.read().await;
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
                            },
                        })
                        .await
                        .unwrap();

                    {
                        let self_player = player.read().await;
                        for a_player in server.read().await.players.values() {
                            if Arc::ptr_eq(player, a_player) {
                                continue;
                            };
                            a_player
                                .read()
                                .await
                                .client
                                .lock()
                                .await
                                .send_player_info(&C32PlayerInfo {
                                    players: vec![C32PlayerInfoPlayerUpdate::AddPlayer {
                                        uuid: self_player.uuid.clone(),
                                        name: self_player.username.clone(),
                                        properties: vec![],
                                        gamemode: self_player.gamemode as i32,
                                        ping: self_player.ping,
                                        display_name: None,
                                    }],
                                })
                                .await
                                .unwrap();
                        }
                    }

                    client
                        .lock()
                        .await
                        .send_window_items(&C13WindowItems {
                            window_id: 0,
                            slots: {
                                let mut slot_data = vec![];
                                for _ in 0..=44 {
                                    slot_data.push(Slot::NotPresent);
                                }
                                slot_data
                            },
                        })
                        .await
                        .unwrap();

                    client.lock().await.hold_item_change(0).await.unwrap();

                    client
                        .lock()
                        .await
                        .send_player_abilities(false, false, true, false, 0.05, 0.1)
                        .await
                        .unwrap();

                    {
                        let mut motion_blocking_heightmap = BitBuffer::create(9, 256);
                        for x in 0..16 {
                            for z in 0..16 {
                                motion_blocking_heightmap.set_entry((x * 16) + z, 10);
                            }
                        }
                        let mut section_blocks = BitBuffer::create(4, 4096);
                        for y in 0..16 {
                            for z in 0..16 {
                                for x in 0..16 {
                                    section_blocks.set_entry(x + (z * 16) + (y * 256), 0);
                                }
                            }
                        }
                        for x in 0..16 {
                            for z in 0..16 {
                                section_blocks.set_entry(x + (z * 16), 1);
                            }
                        }
                        let mut heightmaps = nbt::Blob::new();
                        heightmaps
                            .insert("MOTION_BLOCKING", motion_blocking_heightmap.into_buffer())
                            .unwrap();
                        let chunk_data = C20ChunkData {
                            chunk_x: 0,
                            chunk_z: 0,
                            full_chunk: true,
                            primary_bit_mask: 0b0000000000000010,
                            heightmaps,
                            biomes: Some(vec![0; 1024]),
                            chunk_sections: vec![C20ChunkDataSection {
                                block_count: 256,
                                bits_per_block: 4,
                                palette: Some(vec![0, 1]),
                                data_array: section_blocks.into_buffer(),
                            }],
                            block_entities: vec![],
                        };
                        for x in -2..=2 {
                            for z in -2..=2 {
                                let mut n_chunk_data = chunk_data.clone();
                                n_chunk_data.chunk_x = x;
                                n_chunk_data.chunk_z = z;
                                unsafe { client.lock().await.send_packet(&n_chunk_data) }
                                    .await
                                    .unwrap();
                            }
                        }
                    }
                }
                ClientEvent::Logout => {
                    let player = player.as_ref().unwrap();
                    server
                        .write()
                        .await
                        .players
                        .remove(&player.read().await.entity_id);

                    {
                        let self_player = player.read().await;
                        for a_player in server.read().await.players.values() {
                            if Arc::ptr_eq(player, a_player) {
                                continue;
                            };
                            a_player
                                .read()
                                .await
                                .client
                                .lock()
                                .await
                                .send_player_info(&C32PlayerInfo {
                                    players: vec![C32PlayerInfoPlayerUpdate::RemovePlayer {
                                        uuid: self_player.uuid.clone(),
                                    }],
                                })
                                .await
                                .unwrap();
                            a_player
                                .read()
                                .await
                                .client
                                .lock()
                                .await
                                .destroy_entities(vec![self_player.entity_id])
                                .await
                                .unwrap();
                        }
                    }

                    break;
                }

                ClientEvent::Ping { delay } => {
                    let player = player.as_ref().unwrap();
                    player.write().await.ping = delay as i32;
                    let uuid = player.read().await.uuid.clone();
                    for player in server.read().await.players.values() {
                        player
                            .read()
                            .await
                            .client
                            .lock()
                            .await
                            .send_player_info(&C32PlayerInfo {
                                players: vec![C32PlayerInfoPlayerUpdate::UpdateLatency {
                                    uuid: uuid.clone(),
                                    ping: delay as i32,
                                }],
                            })
                            .await
                            .unwrap();
                    }
                }

                ClientEvent::PlayerPosition { x, y, z, on_ground } => {
                    let player = player.as_ref().unwrap();

                    player.write().await.on_ground = on_ground;
                    player.write().await.set_position(x, y, z).await;
                    player.write().await.update_player_entities().await;
                }
                ClientEvent::PlayerPositionAndRotation {
                    x,
                    y,
                    z,
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    let player = player.as_ref().unwrap();
                    player.write().await.on_ground = on_ground;
                    player
                        .write()
                        .await
                        .set_position_and_rotation(x, y, z, yaw, pitch)
                        .await;
                    player.write().await.update_player_entities().await;
                }
                ClientEvent::PlayerRotation {
                    yaw,
                    pitch,
                    on_ground,
                } => {
                    let player = player.as_ref().unwrap();
                    player.write().await.on_ground = on_ground;
                    player.write().await.set_rotation(yaw, pitch).await;
                }
                ClientEvent::EntityAction {
                    entity_id,
                    action_id,
                    ..
                } => {
                    let player = player.as_ref().unwrap();
                    let mut player = player.write().await;
                    if entity_id == player.entity_id {
                        match action_id {
                            0 => {
                                player.is_sneaking = true;
                            },
                            1 => {
                                player.is_sneaking = false;
                            },
                            _ => unimplemented!()
                        }
                        player.update_metadata().await;
                    }
                }
            }
        }
    });
}

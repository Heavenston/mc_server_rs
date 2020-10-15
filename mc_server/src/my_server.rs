use mc_networking::client::{client_event::*, Client};
use mc_networking::map;

use crate::location::Location;
use log::*;
use mc_networking::data_types::bitbuffer::BitBuffer;
use mc_networking::data_types::Slot;
use mc_networking::packets::{client_bound::*, server_bound::*};
use serde_json::json;
use std::collections::HashMap;
use std::sync::Arc;
use tokio::net::TcpStream;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub struct Player {
    pub server: Arc<RwLock<Server>>,
    pub client: Arc<Mutex<Client>>,
    location: Location,
    pub uuid: Uuid,
    pub entity_id: i64,
    pub username: String,
    pub ping: i32,
    pub gamemode: u8,
}
impl Player {
    pub fn new(
        server: Arc<RwLock<Server>>,
        client: Arc<Mutex<Client>>,
        uuid: Uuid,
        entity_id: i64,
        username: String,
        ping: i32,
        gamemode: u8,
    ) -> Self {
        Self {
            server,
            client,
            location: Location::default(),
            uuid,
            entity_id,
            username,
            ping,
            gamemode,
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

    /// Change current location
    pub async fn set_location(&mut self, new_location: Location) {
        self.location = new_location;
    }
    // TODO: Add entity position packets
    /// Update location to all other players
    pub async fn update_location(&self) {
        /*let view_distance2 = (self.server.read().await.view_distance as f64).powf(2.0);
        for player in self.server.read().await.players.values() {
            let player = player.read().await;
            if player.location.distance2(&self.location) < view_distance2 {

            }
        }*/
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
    }
}

pub struct Server {
    pub players: HashMap<i64, Arc<RwLock<Player>>>,
    pub entity_id_counter: i64,
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
                            Uuid::new_v4(),
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

                _ => (),
            }
        }
    });
}

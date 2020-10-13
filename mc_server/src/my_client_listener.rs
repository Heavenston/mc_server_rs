use mc_networking::client::listener::{ClientListener, LoginStartResult};
use mc_networking::client::Client;
use mc_networking::map;
use mc_networking::packets::client_bound::{
    C13WindowItems, C17PluginMessage, C17PluginMessageBuilder, C20ChunkData, C20ChunkDataSection,
    C24JoinGame, C24JoinGameBiomeEffects, C24JoinGameBiomeEffectsMoodSound,
    C24JoinGameBiomeElement, C24JoinGameDimensionCodec, C24JoinGameDimensionElement, C32PlayerInfo,
    C32PlayerInfoPlayerUpdate, C34PlayerPositionAndLook,
};

use async_trait::async_trait;
use log::*;
use mc_networking::data_types::bitbuffer::BitBuffer;
use mc_networking::data_types::Slot;
use serde_json::json;
use serde_json::Value;
use std::cell::RefCell;
use std::sync::Arc;
use tokio::sync::{Mutex, RwLock};
use uuid::Uuid;

pub struct MyClientListener {
    client: Arc<RwLock<Client<MyClientListener>>>,
    uuid: Uuid,
    username: Mutex<String>,
}
impl MyClientListener {
    pub fn new(client: Arc<RwLock<Client<MyClientListener>>>) -> Self {
        Self {
            client,
            uuid: Uuid::new_v4(),
            username: Mutex::new(String::default()),
        }
    }
}
#[async_trait]
impl ClientListener for MyClientListener {
    async fn on_slp(&self) -> Value {
        info!("Server List Ping");
        json!({
            "version": {
                "name": "1.16.3",
                "protocol": 753
            },
            "players": {
                "max": 10,
                "online": 0,
                "sample": []
            },
            "description": "Hi"
        })
    }
    async fn on_login_start(&self, username: String) -> LoginStartResult {
        info!("Login request from {}", username);
        *self.username.lock().await = username.clone();
        LoginStartResult::Accept {
            uuid: self.uuid.clone(),
            username,
        }
    }
    async fn on_ready(&self) {
        info!("A player is ready !");
        let client = self.client.read().await;
        let username = self.username.lock().await.clone();

        client
            .join_game(&C24JoinGame {
                entity_id: 0,
                is_hardcore: false,
                gamemode: 1,
                previous_gamemode: 1,
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
                max_players: 10,
                view_distance: 10,
                reduced_debug_info: false,
                enable_respawn_screen: true,
                is_debug: false,
                is_flat: true,
            })
            .await
            .unwrap();

        let brand = {
            let mut builder = C17PluginMessageBuilder::new("minecraft:brand".to_string());
            builder.encoder.write_string("Heaven");
            builder.build()
        };
        client.send_plugin_message(&brand).await.unwrap();

        client
            .player_position_and_look(&C34PlayerPositionAndLook {
                x: 0.0,
                y: 17.0,
                z: 0.0,
                yaw: 0.0,
                pitch: 0.0,
                flags: 0,
                teleport_id: 0,
            })
            .await
            .unwrap();

        client
            .send_player_info(&C32PlayerInfo {
                players: vec![
                    C32PlayerInfoPlayerUpdate::AddPlayer {
                        uuid: self.uuid.clone(),
                        name: username.clone(),
                        properties: vec![],
                        gamemode: 1,
                        ping: 1000,
                        display_name: Some(r#"{"text":"Robert","color":"red"}"#.to_string()),
                    },
                    C32PlayerInfoPlayerUpdate::AddPlayer {
                        uuid: Uuid::new_v4(),
                        name: "Roberto".to_string(),
                        properties: vec![],
                        gamemode: 1,
                        ping: -1,
                        display_name: None,
                    },
                ],
            })
            .await
            .unwrap();

        client
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

        client.hold_item_change(0).await.unwrap();

        client.send_player_abilities(
            false,
            false,
            true,
            false,
            0.05,
            0.1
        ).await.unwrap();

        client.update_view_position(0, 0).await.unwrap();
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
                    unsafe { client.send_packet(&n_chunk_data) }.await.unwrap();
                }
            }
        }
    }

    async fn on_perform_respawn(&self) {
        info!("PERFORM RESPAWN");
    }
}

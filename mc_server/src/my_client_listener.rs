use mc_networking::client::listener::{ClientListener, LoginStartResult};
use mc_networking::client::Client;
use mc_networking::map;
use mc_networking::packets::client_bound::{C24JoinGame, C24JoinGameBiomeEffects, C24JoinGameBiomeEffectsMoodSound, C24JoinGameBiomeElement, C24JoinGameDimensionCodec, C24JoinGameDimensionElement, C17PluginMessage, C17PluginMessageBuilder};

use async_trait::async_trait;
use log::*;
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

pub struct MyClientListener(Arc<RwLock<Client<MyClientListener>>>);
impl MyClientListener {
    pub fn new(client: Arc<RwLock<Client<MyClientListener>>>) -> Self {
        Self(client)
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
        LoginStartResult::Accept {
            uuid: Uuid::new_v4(),
            username,
        }
    }
    async fn on_ready(&self) {
        info!("A player is ready !");
        let client = self.0.read().await;

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

    }

    async fn on_perform_respawn(&self) {
        info!("PERFORM RESPAWN");
    }
}

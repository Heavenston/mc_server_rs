use mc_networking::client::listener::{ClientListener, LoginStartResult};
use mc_networking::client::Client;
use mc_networking::map;
use mc_networking::packets::client_bound::{
    C24JoinGameBiomeEffects, C24JoinGameBiomeEffectsMoodSound, C24JoinGameBiomeElement,
    C24JoinGameDimensionCodec, C24JoinGameDimensionElement,
};

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

        let test_dimension = C24JoinGameDimensionElement {
            natural: 1,
            ambient_light: 0.0,
            has_ceiling: 0,
            has_skylight: 1,
            fixed_time: 0,
            shrunk: 0,
            ultrawarm: 0,
            has_raids: 0,
            respawn_anchor_works: 0,
            bed_works: 1,
            piglin_safe: 0,
            coordinate_scale: 1.0,
            logical_height: 256,
            infiniburn: "".to_string(),
        };
        let test_biome = C24JoinGameBiomeElement {
            depth: 0.1,
            temperature: 0.5,
            downfall: 0.5,
            precipitation: "none".to_string(),
            category: "none".to_string(),
            scale: 0.2,
            effects: C24JoinGameBiomeEffects {
                sky_color: 0x7BA4FF,
                water_fog_color: 0x050533,
                fog_color: 0xC0D8FF,
                water_color: 0x3F76E4,
                mood_sound: C24JoinGameBiomeEffectsMoodSound {
                    tick_delay: 6000,
                    offset: 2.0,
                    sound: "minecraft:ambient.cave".to_string(),
                    block_search_extent: 8,
                },
            },
        };

        client
            .join_game(
                0,
                false,
                1,
                vec!["minecraft:test".into()],
                C24JoinGameDimensionCodec {
                    dimensions: map!(
                        "minecraft:testdim".into() => test_dimension.clone()
                    ),
                    biomes: map!(
                        "minecraft:testbiome".into() => test_biome.clone()
                    ),
                },
                test_dimension.clone(),
                "minecraft:testdim".into(),
                0,
                10,
                false,
                true,
                false,
                true,
            )
            .await
            .unwrap();
    }
}

use mc_networking::data_types::command_data::{ArgumentNode, LiteralNode, Node};
use mc_server_lib::chat_manager::CommandExecutor;

use anyhow::Result;
use async_trait::async_trait;
use mc_networking::data_types::encoder::PacketEncoder;
use mc_server_lib::{entity::BoxedEntity, entity_manager::PlayerWrapper};
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

pub struct FlyCommand;
#[async_trait]
impl CommandExecutor for FlyCommand {
    fn names(&self) -> Vec<String> {
        vec!["fly".to_string()]
    }
    fn graph(&self) -> Vec<Arc<dyn Node>> {
        vec![Arc::new(LiteralNode {
            is_executable: false,
            children_nodes: vec![
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "enable".to_string(),
                }),
                Arc::new(LiteralNode {
                    is_executable: true,
                    children_nodes: vec![],
                    redirect_node: None,
                    name: "disable".to_string(),
                }),
                Arc::new(LiteralNode {
                    is_executable: false,
                    children_nodes: vec![Arc::new(ArgumentNode {
                        is_executable: true,
                        children_nodes: vec![],
                        redirect_node: None,
                        name: "speed".to_string(),
                        parser: "brigadier:float".to_string(),
                        properties: {
                            let mut encoder = PacketEncoder::new();
                            encoder.write_u8(0x01);
                            encoder.write_f32(0.0);
                            encoder.consume()
                        },
                        suggestions_type: None,
                    })],
                    redirect_node: None,
                    name: "speed".to_string(),
                }),
            ],
            redirect_node: None,
            name: self.names().first().unwrap().clone(),
        }) as Arc<dyn Node>]
    }

    async fn on_command(
        &self,
        executor: Arc<RwLock<BoxedEntity>>,
        _command: String,
        args: Vec<String>,
    ) -> Result<bool> {
        if let Some(player) = PlayerWrapper::new(executor).await {
            if args.len() < 1 {
                return Ok(false);
            }
            match &*args[0] {
                "enable" => {
                    if args.len() != 1 {
                        Ok(false)
                    }
                    else {
                        if player.read().await.as_player().unwrap().can_fly {
                            player
                                .send_message(json!({
                                    "text": "Fly is already enabled",
                                    "color": "red",
                                    "bold": "true"
                                }))
                                .await
                                .unwrap();
                        }
                        else {
                            player.write().await.as_player_mut().unwrap().can_fly = true;
                            player.update_abilities().await.unwrap();
                            player
                                .send_message(json!({
                                    "text": "Fly is now enabled",
                                    "color": "green",
                                    "bold": "true"
                                }))
                                .await
                                .unwrap();
                        }
                        Ok(true)
                    }
                }
                "disable" => {
                    if args.len() != 1 {
                        Ok(false)
                    }
                    else {
                        if !player.read().await.as_player().unwrap().can_fly {
                            player
                                .send_message(json!({
                                    "text": "Fly is already disabled",
                                    "color": "red",
                                    "bold": "true"
                                }))
                                .await
                                .unwrap();
                        }
                        else {
                            player.write().await.as_player_mut().unwrap().can_fly = false;
                            player.write().await.as_player_mut().unwrap().is_flying = false;
                            player.update_abilities().await.unwrap();
                            player
                                .send_message(json!({
                                    "text": "Fly is now disabled",
                                    "color": "green",
                                    "bold": "true"
                                }))
                                .await
                                .unwrap();
                        }
                        Ok(true)
                    }
                }
                "speed" => {
                    if args.len() != 2 {
                        Ok(false)
                    }
                    else {
                        let speed = args[1].parse::<f32>();
                        if speed.is_err() {
                            return Ok(false);
                        }
                        let speed = speed.unwrap();
                        player.write().await.as_player_mut().unwrap().flying_speed = 0.05 * speed;
                        player.update_abilities().await.unwrap();
                        player
                            .send_message(json!({
                                "text": format!("Flight speed set to x{}", speed),
                                "color": "green",
                                "bold": "true"
                            }))
                            .await
                            .unwrap();
                        Ok(true)
                    }
                }
                _ => Ok(false),
            }
        }
        else {
            Ok(false)
        }
    }
}
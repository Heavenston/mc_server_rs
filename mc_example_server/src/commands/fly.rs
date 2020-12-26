use mc_networking::data_types::command_data::{ArgumentNode, LiteralNode, Node};
use mc_server_lib::chat_manager::CommandExecutor;

use anyhow::Result;
use async_trait::async_trait;
use mc_networking::data_types::encoder::PacketEncoder;
use mc_server_lib::entity::{player::PlayerRef, BoxedEntity};
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
                        parser: "brigadier:float".into(),
                        properties: {
                            let mut encoder = PacketEncoder::default();
                            encoder.write_u8(0x01);
                            encoder.write_f32(0.0);
                            encoder.into_inner().freeze()
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
        if let Some(player_ref) = PlayerRef::new(executor).await {
            if args.is_empty() {
                return Ok(false);
            }
            match &*args[0] {
                "enable" => {
                    if args.len() != 1 {
                        Ok(false)
                    }
                    else {
                        if player_ref.entity.read().await.as_player().can_fly {
                            player_ref
                                .send_chat_message(json!({
                                    "text": "Fly is already enabled",
                                    "color": "red",
                                    "bold": "true"
                                }))
                                .await;
                        }
                        else {
                            player_ref.entity.write().await.as_player_mut().can_fly = true;
                            player_ref.update_abilities().await;
                            player_ref
                                .send_chat_message(json!({
                                    "text": "Fly is now enabled",
                                    "color": "green",
                                    "bold": "true"
                                }))
                                .await;
                        }
                        Ok(true)
                    }
                }
                "disable" => {
                    if args.len() != 1 {
                        Ok(false)
                    }
                    else {
                        if !player_ref.entity.read().await.as_player().can_fly {
                            player_ref
                                .send_chat_message(json!({
                                    "text": "Fly is already disabled",
                                    "color": "red",
                                    "bold": "true"
                                }))
                                .await;
                        }
                        else {
                            player_ref.entity.write().await.as_player_mut().can_fly = false;
                            player_ref.entity.write().await.as_player_mut().is_flying = false;
                            player_ref.update_abilities().await;
                            player_ref
                                .send_chat_message(json!({
                                    "text": "Fly is now disabled",
                                    "color": "green",
                                    "bold": "true"
                                }))
                                .await;
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
                        player_ref.entity.write().await.as_player_mut().flying_speed = 0.05 * speed;
                        player_ref.update_abilities().await;
                        player_ref
                            .send_chat_message(json!({
                                "text": format!("Flight speed set to x{}", speed),
                                "color": "green",
                                "bold": "true"
                            }))
                            .await;
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

use crate::{
    entity::{player::PlayerRef, BoxedEntity},
    entity_manager::PlayerManager,
};
use mc_networking::{
    data_types::command_data::{Node, RootNode},
    packets::client_bound::{C0EChatMessage, C10DeclareCommands},
};

use anyhow::Result;
use async_trait::async_trait;
use fxhash::FxHashMap;
use log::*;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;

#[async_trait]
pub trait CommandExecutor: Send + Sync {
    fn names(&self) -> Vec<String>;
    fn graph(&self) -> Vec<Arc<dyn Node>>;

    async fn on_command(
        &self,
        executor: Arc<RwLock<BoxedEntity>>,
        command: String,
        args: Vec<String>,
    ) -> Result<bool>;
}

pub struct ChatManager {
    commands_mapping: RwLock<FxHashMap<String, Arc<dyn CommandExecutor>>>,
    commands: RwLock<Vec<Arc<dyn CommandExecutor>>>,

    pub players: RwLock<PlayerManager>,
}
impl ChatManager {
    pub fn new() -> Self {
        Self {
            commands: RwLock::default(),
            commands_mapping: RwLock::default(),

            players: RwLock::new(PlayerManager::new()),
        }
    }

    async fn get_declare_commands(&self) -> C10DeclareCommands {
        C10DeclareCommands {
            root_node: Arc::new(RootNode {
                is_executable: false,
                children_nodes: self
                    .commands
                    .read()
                    .await
                    .iter()
                    .flat_map(|command| command.graph())
                    .collect(),
                redirect_node: None,
            }),
        }
    }

    /// Should be executed when a player join
    pub async fn declare_commands_to_player(&self, player_id: i32) {
        self.players
            .read()
            .await
            .get_entity(player_id)
            .unwrap()
            .send_packet(&self.get_declare_commands().await)
            .await
            .unwrap();
    }

    pub async fn register_command(&self, command: Arc<dyn CommandExecutor>) {
        let names = command.names();
        self.commands.write().await.push(Arc::clone(&command));
        for name in names {
            self.commands_mapping
                .write()
                .await
                .insert(name.to_lowercase(), Arc::clone(&command));
        }
    }

    /// Should be called when an entity sends a message
    /// It will parse commands and call the command_executor
    pub async fn player_message(&self, sender: PlayerRef, message: String) {
        if message.starts_with('/') {
            let mut args = message.trim_start_matches('/').split(' ');
            let command_name = args.next().unwrap_or("").to_lowercase();
            let args: Vec<_> = args.map(|s| s.to_string()).collect();

            let mut exist = true;
            match self.commands_mapping.read().await.get(&command_name) {
                Some(command) => {
                    match command
                        .on_command(sender.clone().into(), command_name.clone(), args)
                        .await
                    {
                        Ok(is_valid) => {
                            exist = is_valid;
                        }
                        Err(error) => {
                            self.send_message(sender.entity.read().await.entity_id(), json!({
                            "text": "An unexpected error occurred while executing command".to_string(),
                            "color": "red"
                        })).await;
                            error!("Error while executing command {}: {}", command_name, error);
                        }
                    }
                }
                None => {
                    exist = false;
                }
            }

            if !exist {
                self.send_message(
                    sender.entity.read().await.entity_id(),
                    json!({
                        "text": format!("Unknown command name '{}'", command_name),
                        "color": "red"
                    }),
                )
                .await;
            }
        }
        else {
            let username = sender.entity.read().await.as_player().username.clone();
            self.players
                .read()
                .await
                .broadcast(&C0EChatMessage {
                    json_data: json!({ "text": format!("<{}> {}", username, message) }),
                    position: 0, // Chat box
                    sender: Some(*sender.entity.read().await.uuid()),
                })
                .await
                .unwrap();
        }
    }

    /// Sends a message to all players
    pub async fn broadcast(&self, message: serde_json::Value) {
        self.players
            .read()
            .await
            .broadcast(&C0EChatMessage {
                json_data: message,
                position: 1, // System message
                sender: None,
            })
            .await
            .unwrap();
    }
    /// Sends a message to one player
    pub async fn send_message(&self, target: i32, message: serde_json::Value) {
        self.players
            .read()
            .await
            .get_entity(target)
            .unwrap()
            .send_packet(&C0EChatMessage {
                json_data: message,
                position: 1, // System message
                sender: None,
            })
            .await
            .unwrap();
    }
}

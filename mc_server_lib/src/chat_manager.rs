use crate::entity_manager::{PlayerManager, PlayerWrapper};
use crate::entity::BoxedEntity;
use mc_networking::packets::client_bound::C0EChatMessage;

use tokio::sync::RwLock;
use std::sync::Arc;
use async_trait::async_trait;
use serde_json::json;
use std::collections::HashMap;

#[async_trait]
pub trait CommandExecutor: Send + Sync {
    fn names(&self) -> Vec<String>;

    async fn on_command(&self, executor: Arc<RwLock<BoxedEntity>>, command: String, args: Vec<String>);
}

pub struct ChatManager {
    commands: RwLock<HashMap<String, Arc<dyn CommandExecutor>>>,
    pub players: RwLock<PlayerManager>,
}
impl ChatManager {
    pub fn new() -> Self {
        Self {
            commands: RwLock::default(),
            players: RwLock::new(PlayerManager::new())
        }
    }

    pub async fn register_command(&self, command: Arc<dyn CommandExecutor>) {
        let names = command.names();
        for name in names {
            self.commands.write().await.insert(name, Arc::clone(&command));
        }
    }

    /// Should be called when an entity sends a message
    /// It will interpret commands and call the command_executor
    pub async fn player_message(&self, sender: PlayerWrapper, message: String) {
        let username = sender.read().await.as_player().unwrap().username.clone();
        self.players.read().await.broadcast(&C0EChatMessage {
            json_data: json!({
                "text": format!("<{}> {}", username, message)
            }),
            position: 0, // Chat box
            sender: Some(sender.read().await.uuid().clone())
        }).await.unwrap();
    }

    /// Sends a message to all players
    pub async fn broadcast(&self, message: serde_json::Value) {
        self.players.read().await.broadcast(&C0EChatMessage {
            json_data: message,
            position: 1, // System message
            sender: None
        }).await.unwrap();
    }
    /// Sends a message to one player
    pub async fn send_message(&self, target: i32, message: serde_json::Value) {
        self.players.read().await.get_entity(target).unwrap().send_packet(&C0EChatMessage {
            json_data: message,
            position: 1, // System message
            sender: None
        }).await.unwrap();
    }
}

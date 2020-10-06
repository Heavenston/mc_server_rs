use mc_networking::client::listener::{ClientListener, LoginStartResult};

use async_trait::async_trait;
use log::*;
use mc_networking::client::Client;
use mc_networking::client::ClientState::Login;
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
        LoginStartResult::Disconnect {
            reason: "This server isn't ready yet".into()
        }
    }
}

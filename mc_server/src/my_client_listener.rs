use mc_networking::client::listener::{ClientListener, LoginStartResult};

use async_trait::async_trait;
use log::*;
use mc_networking::client::Client;
use serde_json::json;
use serde_json::Value;
use std::sync::Arc;
use tokio::sync::RwLock;
use tokio::task;
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
        println!("Hi");
    }
}

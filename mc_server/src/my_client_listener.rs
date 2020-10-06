use mc_networking::client::listener::ClientListener;

use serde_json::Value;
use log::*;
use async_trait::async_trait;
use serde_json::json;
use std::sync::Arc;
use tokio::sync::RwLock;
use mc_networking::client::Client;

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
}
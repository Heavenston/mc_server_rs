use async_trait::async_trait;
use uuid::Uuid;

pub enum LoginStartResult {
    Accept { uuid: Uuid, username: String },
    Disconnect { reason: String },
}

#[async_trait]
pub trait ClientListener: Send + Sync {
    async fn on_slp(&self) -> serde_json::Value;
    async fn on_login_start(&self, username: String) -> LoginStartResult;
    async fn on_ready(&self);

    async fn on_perform_respawn(&self) {}
    async fn on_request_stats(&self) {}
    #[allow(unused_variables)]
    async fn on_player_position(&self, x: f64, y: f64, z: f64, on_ground: bool) {}
}

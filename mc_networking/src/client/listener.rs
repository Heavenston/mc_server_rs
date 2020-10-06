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
}

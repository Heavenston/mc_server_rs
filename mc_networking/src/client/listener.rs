use async_trait::async_trait;

#[async_trait]
pub trait ClientListener: Send + Sync {
    async fn on_slp(&self) -> serde_json::Value;
}

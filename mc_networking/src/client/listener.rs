
pub trait ClientListener: Send + Sync {

    fn on_slp(&self) -> serde_json::Value;

}
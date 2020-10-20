use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug)]
pub enum LoginStartResult {
    Accept { uuid: Uuid, username: String },
    Disconnect { reason: String },
}

#[derive(Debug)]
pub enum ClientEvent {
    ServerListPing {
        response: oneshot::Sender<serde_json::Value>,
    },
    LoginStart {
        username: String,
        response: oneshot::Sender<LoginStartResult>,
    },
    LoggedIn,
    Logout,

    Ping {
        delay: u128,
    },

    ChatMessage {
        message: String,
    },
    PlayerPosition {
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    },
    PlayerRotation {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    PlayerPositionAndRotation {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    EntityAction {
        entity_id: i32,
        action_id: i32,
        jump_boost: i32,
    },
    PlayerAbilities {
        is_flying: bool,
    },
}

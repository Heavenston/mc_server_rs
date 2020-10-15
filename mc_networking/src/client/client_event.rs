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
}

use crate::{
    data_types::{Position, Slot},
    packets::server_bound::{S1BPlayerDiggingFace, S1BPlayerDiggingStatus},
};

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
    Animation {
        hand: i32,
    },
    PlayerDigging {
        status: S1BPlayerDiggingStatus,
        position: Position,
        face: S1BPlayerDiggingFace,
    },
    PlayerBlockPlacement {
        /// The hand from which the block is placed; 0: main hand, 1: off hand
        hand: i32,
        /// Block position
        position: Position,
        /// The face on which the block is placed
        face: S1BPlayerDiggingFace,
        /// The position of the crosshair on the block, from 0 to 1 increasing from west to east
        cursor_position_x: f32,
        /// The position of the crosshair on the block, from 0 to 1 increasing from bottom to top
        cursor_position_y: f32,
        /// The position of the crosshair on the block, from 0 to 1 increasing from north to south
        cursor_position_z: f32,
        /// True when the player's head is inside of a block.
        inside_block: bool,
    },
    CreativeInventoryAction {
        slot_id: i16,
        slot: Slot,
    },
}

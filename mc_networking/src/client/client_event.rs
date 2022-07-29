use crate::{
    data_types::{Position, Slot},
    packets::server_bound::*,
    packets::client_bound::*,
};

use tokio::sync::oneshot;
use uuid::Uuid;

#[derive(Debug)]
pub enum LoginStartResult {
    Accept {
        uuid: Uuid,
        username: String,
        encrypt: bool,
        compress: bool,
    },
    Disconnect {
        reason: String,
    },
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

    ChatMessage(S04ChatMessage),
    ClickContainer(S0AClickContainer),
    PluginMessage(S0CPluginMessage),
    SetPlayerPosition(S13SetPlayerPosition),
    SetPlayerPositionAndRotation(S13SetPlayerPositionAndRotation),
    SetPlayerRotation(S15SetPlayerRotation),
    PlayerCommand(S1DPlayerCommand),
    PlayerAbilities(S1BPlayerAbilities),
    PlayerAction(S1CPlayerAction),
    SetHeldItem(S27SetHeldItem),
    SetCreativeModeSlot(S2ASetCreativeModeSlot),
    SwingArm(S2ESwingArm),
    UseItemOn(S30UseItemOn),
}

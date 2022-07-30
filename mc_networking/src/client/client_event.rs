use crate::packets::server_bound::*;

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
    /// Sent after a client has been entered the "login" phase
    /// A respone in the respone channel will be awaited, after which
    /// the client will either be disconnected
    /// Or compression and encryption will be setup
    /// Before sending any play packets, wait for the LoggedIn client event
    LoginStart {
        username: String,
        response: oneshot::Sender<LoginStartResult>,
    },
    /// Sent when the client is ready for receiving play packets
    /// Usually, following this event, you'll want to send a C23Login packet.
    /// As well as the tab player list (using C32PlayerInfoPlayerUpdate), the player's inventory
    LoggedIn,
    /// Sent anytime the client has been disconnected
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

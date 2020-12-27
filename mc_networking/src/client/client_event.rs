use crate::{
    data_types::{Position, Slot},
    packets::server_bound::{S1BPlayerDiggingFace, S1BPlayerDiggingStatus},
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

    /// Correspond to packet S03ChatMessage
    ChatMessage {
        message: String,
    },
    /// This is sent by the player when it clicks on a slot in a window.
    ///
    /// <https://wiki.vg/Protocol#Click_Window>
    /// Correspond to packet S09ClickWindow
    ClickWindow {
        /// The ID of the window which was clicked. 0 for player inventory.
        window_id: u8,
        /// The clicked slot number, see wiki.vg
        slot_id: i16,
        /// The button used in the click, see below
        button: i8,
        /// A unique number for the action, implemented by Notchian as a counter,
        /// starting at 1 (different counter for every window ID).
        action_number: i16,
        /// Inventory operation mode, see wiki.vg
        mode: i32,
        /// The clicked slot. Has to be empty (item ID = -1) for drop mode.
        clicked_item: Slot,
    },
    /// Mods and plugins can use this to send their data.
    ///
    /// <https://wiki.vg/Protocol#Plugin_Message_.28serverbound.29>
    /// Correspond to the packet S0BPluginMessage
    PluginMessage {
        channel: String,
        data: Vec<u8>,
    },
    /// Correspond to packet S12PlayerPosition
    PlayerPosition {
        x: f64,
        y: f64,
        z: f64,
        on_ground: bool,
    },
    /// A combination of PlayerRotation and PlayerPositionAndRotation.
    /// Correspond to packet S13PlayerPositionAndRotation
    PlayerPositionAndRotation {
        x: f64,
        y: f64,
        z: f64,
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    /// Correspond to packet S14PlayerRotation
    PlayerRotation {
        yaw: f32,
        pitch: f32,
        on_ground: bool,
    },
    /// Sent by the client to indicate that it has performed certain actions:
    /// sneaking (crouching), sprinting, exiting a bed,
    /// jumping with a horse, and opening a horse's inventory while riding it.
    ///
    /// <https://wiki.vg/Protocol#Entity_Action>
    /// Correspond to the packet S1CEntityAction
    EntityAction {
        /// Player ID
        entity_id: i32,
        /// The ID of the action, see wiki.vg
        action_id: i32,
        /// Only used by the “start jump with horse” action, in which case it ranges from 0 to 100. In all other cases it is 0.
        jump_boost: i32,
    },
    /// The vanilla client sends this when the player starts/stops flying.
    ///
    /// <https://wiki.vg/Protocol#Player_Abilities_.28serverbound.29>
    /// Correspond to the packet S1APlayerAbilities
    PlayerAbilities {
        is_flying: bool,
    },
    /// Sent when the player mines a block.
    ///
    /// <https://wiki.vg/Protocol#Player_Digging>
    /// Correspond to the packet S1BPlayerDigging
    PlayerDigging {
        status: S1BPlayerDiggingStatus,
        position: Position,
        face: S1BPlayerDiggingFace,
    },
    /// Sent when the player changes the slot selection
    ///
    /// <https://wiki.vg/Protocol#Held_Item_Change_.28serverbound.29>
    /// Correspond to the packet S25HeldItemChange
    HeldItemChange {
        /// The slot which the player has selected (0–8)
        slot: i16,
    },
    /// While the user is in the standard inventory (i.e., not a crafting bench) in Creative mode,
    /// the player will send this packet.
    /// This action can be described as "set inventory slot".
    ///
    /// <https://wiki.vg/Protocol#Creative_Inventory_Action>
    /// Correspond to the packet S28CreativeInventoryAction
    CreativeInventoryAction {
        slot_id: i16,
        slot: Slot,
    },
    /// Sent when the player's arm swings.
    ///
    /// <https://wiki.vg/Protocol#Animation_.28serverbound.29>
    /// Correspond to the packet S2CAnimation
    Animation {
        hand: i32,
    },
    /// Upon placing a block, this packet is sent once.
    ///
    /// <https://wiki.vg/Protocol#Player_Block_Placement>
    /// Correspond to the packet S2EPlayerBlockPlacement
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
}

use crate::{data_types::encoder::PacketDecoder, packets::RawPacket};

use anyhow::{Error, Result};

pub trait ServerBoundPacket: Sized {
    fn packet_id() -> i32;
    fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self>;

    fn decode(raw_packet: RawPacket) -> Result<Self> {
        if raw_packet.packet_id != Self::packet_id() {
            return Err(Error::msg("Invalid packet id"));
        }
        let mut packet_decoder = PacketDecoder::new(raw_packet);
        let result = Self::run_decoder(&mut packet_decoder);
        if packet_decoder.remaining() > 0 {
            return Err(Error::msg("Packet not fully consumed"));
        }
        result
    }
}

mod handshake {
    use super::ServerBoundPacket;
    use crate::data_types::VarInt;

    use crate::data_types::encoder::PacketDecoder;
    use anyhow::Result;

    /// This causes the server to switch into the target state.
    ///
    /// https://wiki.vg/Protocol#Handshake
    #[derive(Clone, Debug)]
    pub struct S00Handshake {
        pub protocol_version: VarInt,
        pub server_addr: String,
        pub server_port: u16,
        pub next_state: VarInt,
    }
    impl ServerBoundPacket for S00Handshake {
        fn packet_id() -> i32 {
            0x00
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                protocol_version: decoder.read_varint()?,
                server_addr: decoder.read_string()?,
                server_port: decoder.read_u16()?,
                next_state: decoder.read_varint()?,
            })
        }
    }
}
pub use handshake::*;

mod status {
    use super::ServerBoundPacket;
    use crate::{data_types::encoder::PacketDecoder, packets::RawPacket};

    use anyhow::{Error, Result};
    use std::convert::{TryFrom, TryInto};

    /// Initiate SLP and should be responded with C00Response
    ///
    /// https://wiki.vg/Protocol#Request
    #[derive(Clone, Debug)]
    pub struct S00Request;
    impl ServerBoundPacket for S00Request {
        fn packet_id() -> i32 {
            0x00
        }

        fn run_decoder(_decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(S00Request)
        }
    }

    /// Should be responses with C01Pong with provided payload
    ///
    /// https://wiki.vg/Protocol#Ping
    #[derive(Clone, Debug)]
    pub struct S01Ping {
        pub payload: i64,
    }
    impl ServerBoundPacket for S01Ping {
        fn packet_id() -> i32 {
            0x01
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                payload: decoder.read_i64()?,
            })
        }
    }
    impl TryFrom<RawPacket> for S01Ping {
        type Error = Error;

        fn try_from(value: RawPacket) -> Result<Self, Self::Error> {
            if value.packet_id != Self::packet_id() {
                return Err(Error::msg("Invalid packet id"));
            }
            Ok(S01Ping {
                payload: i64::from_be_bytes(value.data.as_ref().try_into()?),
            })
        }
    }
}
pub use status::*;

mod login {
    use super::ServerBoundPacket;
    use crate::data_types::{encoder::PacketDecoder, VarInt};

    use anyhow::Result;

    /// Initiate login state
    ///
    /// https://wiki.vg/Protocol#Login_Start
    #[derive(Clone, Debug)]
    pub struct S00LoginStart {
        pub name: String,
    }
    impl ServerBoundPacket for S00LoginStart {
        fn packet_id() -> i32 {
            0x00
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                name: decoder.read_string()?,
            })
        }
    }

    /// Will succeed C01EncryptionRequest
    ///
    /// https://wiki.vg/Protocol#Encryption_Response
    #[derive(Clone, Debug)]
    pub struct S01EncryptionResponse {
        pub shared_secret: Vec<u8>,
        pub verify_token: Vec<u8>,
    }
    impl ServerBoundPacket for S01EncryptionResponse {
        fn packet_id() -> i32 {
            0x01
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let shared_secret_length = decoder.read_varint()? as usize;
            let shared_secret = decoder.read_bytes(shared_secret_length)?;

            let verify_token_length = decoder.read_varint()? as usize;
            let verify_token = decoder.read_bytes(verify_token_length)?;

            Ok(Self {
                shared_secret,
                verify_token,
            })
        }
    }

    /// Will succeed C02LoginPluginRequest
    ///
    /// https://wiki.vg/Protocol#Login_Plugin_Response
    #[derive(Clone, Debug)]
    pub struct S02LoginPluginResponse {
        pub message_id: VarInt,
        pub successful: bool,
        pub data: Option<Vec<u8>>,
    }
    impl ServerBoundPacket for S02LoginPluginResponse {
        fn packet_id() -> i32 {
            0x02
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let message_id = decoder.read_varint()?;
            let successful = decoder.read_bool()?;
            let data = if successful {
                Some(decoder.read_to_end()?)
            }
            else {
                None
            };
            Ok(Self {
                message_id,
                successful,
                data,
            })
        }
    }
}
pub use login::*;

mod play {
    use super::ServerBoundPacket;
    use crate::data_types::{Position, Slot, VarInt};

    use crate::data_types::encoder::PacketDecoder;
    use anyhow::Error;

    /// Sent by client as confirmation of C36PlayerPositionAndLook.
    ///
    /// https://wiki.vg/Protocol#Teleport_Confirm
    #[derive(Clone, Debug)]
    pub struct S00TeleportConfirm {
        pub teleport_id: VarInt,
    }
    impl ServerBoundPacket for S00TeleportConfirm {
        fn packet_id() -> i32 {
            0x00
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                teleport_id: decoder.read_varint()?,
            })
        }
    }

    /// Used to send a chat message to the server.
    /// The message may not be longer than 256 characters or else the server will kick the client.
    ///
    /// https://wiki.vg/Protocol#Chat_Message_.28serverbound.29
    #[derive(Clone, Debug)]
    pub struct S03ChatMessage {
        pub message: String,
    }
    impl ServerBoundPacket for S03ChatMessage {
        fn packet_id() -> i32 {
            0x03
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                message: decoder.read_string()?,
            })
        }
    }

    /// 0: Perform Respawn | Sent when the client is ready to complete login and when the client is ready to respawn after death.
    /// 1: Request Stats   | Sent when the client opens the Statistics menu
    ///
    /// https://wiki.vg/Protocol#Client_Status
    #[derive(Clone, Debug)]
    pub struct S04ClientStatus {
        pub action_id: VarInt,
    }
    impl ServerBoundPacket for S04ClientStatus {
        fn packet_id() -> i32 {
            0x04
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                action_id: decoder.read_varint()?,
            })
        }
    }

    /// Sent when the player connects, or when settings are changed.
    ///
    /// https://wiki.vg/Protocol#Client_Settings
    #[derive(Clone, Debug)]
    pub struct S05ClientSettings {
        /// e.g. en_GB
        pub local: String,
        /// Client-side render distance, in chunks
        pub view_distance: i8,
        /// 0: enabled, 1: commands only, 2: hidden. See processing chat for more information.
        pub chat_mode: VarInt,
        /// “Colors” multiplayer setting
        pub chat_colors: bool,
        /// Bit mask, see in https://wiki.vg/Protocol#Client_Settings
        pub displayed_skin_parts: u8,
        /// 0: Left, 1: Right
        pub main_hand: VarInt,
    }
    impl ServerBoundPacket for S05ClientSettings {
        fn packet_id() -> i32 {
            0x05
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                local: decoder.read_string()?,
                view_distance: decoder.read_i8()?,
                chat_mode: decoder.read_varint()?,
                chat_colors: decoder.read_bool()?,
                displayed_skin_parts: decoder.read_u8()?,
                main_hand: decoder.read_varint()?,
            })
        }
    }

    /// Sent by the client after C1FKeepAlive
    ///
    /// https://wiki.vg/Protocol#Keep_Alive_.28serverbound.29
    #[derive(Clone, Debug)]
    pub struct S10KeepAlive {
        pub id: i64,
    }
    impl ServerBoundPacket for S10KeepAlive {
        fn packet_id() -> i32 {
            0x10
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                id: decoder.read_i64()?,
            })
        }
    }

    /// Updates the player's XYZ position on the server.
    ///
    /// https://wiki.vg/Protocol#Player_Position
    #[derive(Clone, Debug)]
    pub struct S12PlayerPosition {
        /// Absolute position
        pub x: f64,
        /// Absolute feet position, normally Head Y - 1.62
        pub feet_y: f64,
        /// Absolute position
        pub z: f64,
        /// True if the client is on the ground, false otherwise
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S12PlayerPosition {
        fn packet_id() -> i32 {
            0x12
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                x: decoder.read_f64()?,
                feet_y: decoder.read_f64()?,
                z: decoder.read_f64()?,
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// A combination of S13PlayerRotation and S11PlayerPosition.
    ///
    /// https://wiki.vg/Protocol#Player_Position_And_Rotation_.28serverbound.29
    #[derive(Clone, Debug)]
    pub struct S13PlayerPositionAndRotation {
        /// Absolute position
        pub x: f64,
        /// Absolute feet position, normally Head Y - 1.62
        pub feet_y: f64,
        /// Absolute position
        pub z: f64,
        /// Absolute rotation on the X Axis, in degrees
        pub yaw: f32,
        /// Absolute rotation on the Y Axis, in degrees
        pub pitch: f32,
        /// True if the client is on the ground, false otherwise
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S13PlayerPositionAndRotation {
        fn packet_id() -> i32 {
            0x13
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                x: decoder.read_f64()?,
                feet_y: decoder.read_f64()?,
                z: decoder.read_f64()?,
                yaw: decoder.read_f32()?,
                pitch: decoder.read_f32()?,
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// Updates the direction the player is looking in.
    ///
    /// https://wiki.vg/Protocol#Player_Rotation
    #[derive(Clone, Debug)]
    pub struct S14PlayerRotation {
        /// Absolute rotation on the X Axis, in degrees
        pub yaw: f32,
        /// Absolute rotation on the Y Axis, in degrees
        pub pitch: f32,
        /// True if the client is on the ground, false otherwise
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S14PlayerRotation {
        fn packet_id() -> i32 {
            0x14
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                yaw: decoder.read_f32()?,
                pitch: decoder.read_f32()?,
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// This packet is used to indicate whether the player is on ground (walking/swimming), or airborne (jumping/falling).
    ///
    /// https://wiki.vg/Protocol#Player_Movement
    #[derive(Clone, Debug)]
    pub struct S15PlayerMovement {
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S15PlayerMovement {
        fn packet_id() -> i32 {
            0x15
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// The vanilla client sends this packet when the player starts/stops flying with the Flags parameter changed accordingly.
    /// All other parameters are ignored by the vanilla server.
    ///
    /// https://wiki.vg/Protocol#Player_Abilities_.28serverbound.29
    #[derive(Clone, Debug)]
    pub struct S1APlayerAbilities {
        /// 0x02: is flying
        pub flags: u8,
    }
    impl ServerBoundPacket for S1APlayerAbilities {
        fn packet_id() -> i32 {
            0x1A
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                flags: decoder.read_u8()?,
            })
        }
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    #[repr(u8)]
    pub enum S1BPlayerDiggingStatus {
        StartedDigging = 0,
        /// Sent when the player lets go of the Mine Block key (default: left click)
        CancelledDigging = 1,
        /// Sent when the client thinks it is finished
        FinishedDigging = 2,
        /// Triggered by using the Drop Item key (default: Q) with the modifier to drop the entire
        /// selected stack (default: depends on OS). Location is always set to 0/0/0,
        /// Face is always set to -Y.
        DropItemStack = 3,
        /// Triggered by using the Drop Item key (default: Q). Location is always set to 0/0/0,
        /// Face is always set to -Y.
        DropItem = 4,
        /// Indicates that the currently held item should have its state updated such as eating food,
        /// pulling back bows, using buckets, etc.
        /// Location is always set to 0/0/0, Face is always set to -Y.
        ShootArrowOrFinishEating = 5,
        /// Used to swap or assign an item to the second hand. Location is always set to 0/0/0,
        /// Face is always set to -Y.
        SwapItemInHand = 6,
    }

    #[derive(Clone, Debug, PartialEq, Eq)]
    #[repr(u8)]
    pub enum S1BPlayerDiggingFace {
        /// -Y
        Bottom,
        /// +Y
        Top,
        /// -Z
        North,
        /// +Z
        South,
        /// -X
        West,
        /// +X
        East,
    }

    /// Sent when the player mines a block.
    ///
    /// https://wiki.vg/Protocol#Player_Digging
    #[derive(Clone, Debug)]
    pub struct S1BPlayerDigging {
        /// The action the player is taking against the block
        pub status: S1BPlayerDiggingStatus,
        /// Block position
        pub position: Position,
        /// The face being hit
        pub face: S1BPlayerDiggingFace,
    }
    impl ServerBoundPacket for S1BPlayerDigging {
        fn packet_id() -> i32 {
            0x1B
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            let status = match decoder.read_varint()? {
                0 => S1BPlayerDiggingStatus::StartedDigging,
                1 => S1BPlayerDiggingStatus::CancelledDigging,
                2 => S1BPlayerDiggingStatus::FinishedDigging,
                3 => S1BPlayerDiggingStatus::DropItemStack,
                4 => S1BPlayerDiggingStatus::DropItem,
                5 => S1BPlayerDiggingStatus::ShootArrowOrFinishEating,
                6 => S1BPlayerDiggingStatus::SwapItemInHand,
                _ => return Err(Error::msg("invalid player digging status")),
            };
            let position = Position::decode(decoder.read_i64()?);
            let face = match decoder.read_u8()? {
                0 => S1BPlayerDiggingFace::Bottom,
                1 => S1BPlayerDiggingFace::Top,
                2 => S1BPlayerDiggingFace::North,
                3 => S1BPlayerDiggingFace::South,
                4 => S1BPlayerDiggingFace::West,
                5 => S1BPlayerDiggingFace::East,
                _ => return Err(Error::msg("invalid player digging face")),
            };

            Ok(Self {
                status,
                position,
                face,
            })
        }
    }

    /// Sent by the client to indicate that it has performed certain actions:
    /// sneaking (crouching), sprinting, exiting a bed,
    /// jumping with a horse, and opening a horse's inventory while riding it.
    ///
    /// https://wiki.vg/Protocol#Entity_Action
    #[derive(Clone, Debug)]
    pub struct S1CEntityAction {
        /// Player ID
        pub entity_id: VarInt,
        /// The ID of the action, see website
        pub action_id: VarInt,
        /// Only used by the “start jump with horse” action, in which case it ranges from 0 to 100. In all other cases it is 0.
        pub jump_boost: VarInt,
    }
    impl ServerBoundPacket for S1CEntityAction {
        fn packet_id() -> i32 {
            0x1C
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                entity_id: decoder.read_varint()?,
                action_id: decoder.read_varint()?,
                jump_boost: decoder.read_varint()?,
            })
        }
    }

    /// While the user is in the standard inventory (i.e., not a crafting bench) in Creative mode,
    /// the player will send this packet.
    /// This action can be described as "set inventory slot".
    ///
    /// https://wiki.vg/Protocol#Creative_Inventory_Action
    #[derive(Clone, Debug)]
    pub struct S28CreativeInventoryAction {
        pub slot_id: i16,
        pub slot: Slot,
    }
    impl ServerBoundPacket for S28CreativeInventoryAction {
        fn packet_id() -> i32 {
            0x28
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                slot_id: decoder.read_i16()?,
                slot: Slot::decode_sync(decoder)?,
            })
        }
    }

    /// Sent when the player changes the slot selection
    #[derive(Clone, Debug)]
    pub struct S25HeldItemChange {
        /// The slot which the player has selected (0–8)
        pub slot: i16,
    }
    impl ServerBoundPacket for S25HeldItemChange {
        fn packet_id() -> i32 {
            0x25
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                slot: decoder.read_i16()?,
            })
        }
    }

    /// Sent when the player's arm swings.
    ///
    /// https://wiki.vg/Protocol#Animation_.28serverbound.29
    #[derive(Clone, Debug)]
    pub struct S2CAnimation {
        /// Hand used for the animation. 0: main hand, 1: off hand.
        pub hand: VarInt,
    }
    impl ServerBoundPacket for S2CAnimation {
        fn packet_id() -> i32 {
            0x2C
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                hand: decoder.read_varint()?,
            })
        }
    }

    /// Upon placing a block, this packet is sent once.
    ///
    /// https://wiki.vg/Protocol#Player_Block_Placement
    #[derive(Clone, Debug)]
    pub struct S2EPlayerBlockPlacement {
        /// The hand from which the block is placed; 0: main hand, 1: off hand
        pub hand: VarInt,
        /// Block position
        pub position: Position,
        /// The face on which the block is placed
        pub face: S1BPlayerDiggingFace,
        /// The position of the crosshair on the block, from 0 to 1 increasing from west to east
        pub cursor_position_x: f32,
        /// The position of the crosshair on the block, from 0 to 1 increasing from bottom to top
        pub cursor_position_y: f32,
        /// The position of the crosshair on the block, from 0 to 1 increasing from north to south
        pub cursor_position_z: f32,
        /// True when the player's head is inside of a block.
        pub inside_block: bool,
    }
    impl ServerBoundPacket for S2EPlayerBlockPlacement {
        fn packet_id() -> i32 {
            0x2E
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                hand: decoder.read_varint()?,
                position: Position::decode(decoder.read_i64()?),
                face: match decoder.read_u8()? {
                    0 => S1BPlayerDiggingFace::Bottom,
                    1 => S1BPlayerDiggingFace::Top,
                    2 => S1BPlayerDiggingFace::North,
                    3 => S1BPlayerDiggingFace::South,
                    4 => S1BPlayerDiggingFace::West,
                    5 => S1BPlayerDiggingFace::East,
                    _ => return Err(Error::msg("invalid player digging face")),
                },
                cursor_position_x: decoder.read_f32()?,
                cursor_position_y: decoder.read_f32()?,
                cursor_position_z: decoder.read_f32()?,
                inside_block: decoder.read_bool()?,
            })
        }
    }
}
pub use play::*;

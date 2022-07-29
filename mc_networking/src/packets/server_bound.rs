use crate::{
    data_types::encoder::PacketDecoder, packets::RawPacket, DecodingError, DecodingResult,
};

pub trait ServerBoundPacket: Sized {
    const PACKET_ID: i32;
    fn run_decoder(decoder: &mut PacketDecoder) -> DecodingResult<Self>;

    fn decode(raw_packet: RawPacket) -> DecodingResult<Self> {
        if raw_packet.packet_id != Self::PACKET_ID {
            return Err(DecodingError::parse_error(
                &format!("packet 0x{:x}", Self::PACKET_ID),
                "invalid packet id",
            ));
        }
        let mut packet_decoder = PacketDecoder::new(raw_packet);
        let result = Self::run_decoder(&mut packet_decoder);
        if packet_decoder.remaining() > 0 {
            return Err(DecodingError::parse_error(
                &format!("packet 0x{:x}", Self::PACKET_ID),
                &format!(
                    "not all bytes have been read after decoding ({} remaining)",
                    packet_decoder.remaining()
                ),
            ));
        }
        result
    }
}

mod handshake {
    use super::ServerBoundPacket;
    use crate::{
        data_types::{encoder::PacketDecoder, VarInt},
        DecodingResult,
    };

    /// This causes the server to switch into the target state.
    ///
    /// <https://wiki.vg/Protocol#Handshake>
    #[derive(Clone, Debug)]
    pub struct S00Handshake {
        /// The protocol the client's expecting
        pub protocol_version: VarInt,
        /// Hostname or IP, e.g. localhost or 127.0.0.1, that was used to connect.
        /// The Notchian server does not use this information.
        /// Note that SRV records are a simple redirect,
        /// e.g. if _minecraft._tcp.example.com points to mc.example.org,
        /// users connecting to example.com will provide example.org as server address in addition to connecting to it.
        pub server_addr: String,
        /// Default is 25565. The Notchian server does not use this information.
        pub server_port: u16,
        /// 1 for Status, 2 for Login.
        pub next_state: VarInt,
    }
    impl ServerBoundPacket for S00Handshake {
        const PACKET_ID: i32 = 0x00;

        fn run_decoder(decoder: &mut PacketDecoder) -> DecodingResult<Self> {
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
    use crate::{data_types::encoder::PacketDecoder, DecodingResult as Result};

    /// Initiate SLP and should be responded with C00Response
    ///
    /// <https://wiki.vg/Protocol#Request>
    #[derive(Clone, Debug)]
    pub struct S00Request;
    impl ServerBoundPacket for S00Request {
        const PACKET_ID: i32 = 0x00;

        fn run_decoder(_decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(S00Request)
        }
    }

    /// Should be responded with C01Pong with the same payload value.
    ///
    /// <https://wiki.vg/Protocol#Ping>
    #[derive(Clone, Debug)]
    pub struct S01Ping {
        /// May be any number.
        /// Notchian clients use a system-dependent time value which is counted in milliseconds.
        pub payload: i64,
    }
    impl ServerBoundPacket for S01Ping {
        const PACKET_ID: i32 = 0x01;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                payload: decoder.read_i64()?,
            })
        }
    }
}
pub use status::*;

mod login {
    use super::ServerBoundPacket;
    use crate::{
        data_types::{encoder::PacketDecoder, VarInt},
        DecodingResult as Result
    };

    #[derive(Clone, Debug)]
    pub struct S00SigData {
        /// When the key data will expire.
        pub timestamp: i64,
        /// The encoded bytes of the public key the client received from Mojang.
        pub public_key: Vec<u8>,
        /// The bytes of the public key signature the client received from Mojang.
        pub signature: Vec<u8>,
    }

    /// Initiate login state
    ///
    /// <https://wiki.vg/Protocol#Login_Start>
    #[derive(Clone, Debug)]
    pub struct S00LoginStart {
        /// Player's username
        pub name: String,
        /// The player's signature
        pub sig_data: Option<S00SigData>,
    }
    impl ServerBoundPacket for S00LoginStart {
        const PACKET_ID: i32 = 0x00;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let name = decoder.read_string()?;
            let sig_data = decoder.read_bool()?.then(|| -> Result<_> {
                Ok(S00SigData {
                    timestamp: decoder.read_i64()?,
                    public_key: decoder.read_varint().and_then(|l| decoder.read_bytes(l as _))?,
                    signature: decoder.read_varint().and_then(|l| decoder.read_bytes(l as _))?,
                })
            }).transpose()?; // Transpose is to get the Result out of the Option to then use ?

            Ok(Self {
                name,
                sig_data,
            })
        }
    }

    #[derive(Clone, Debug)]
    pub enum S01VerifyToken {
        With {
            verify_token: Vec<u8>,
        },
        Without {
            salt: i64,
            message_signature: Vec<u8>,
        }
    }

    /// Sent in response to C01EncryptionRequest
    ///
    /// <https://wiki.vg/Protocol#Encryption_Response>
    #[derive(Clone, Debug)]
    pub struct S01EncryptionResponse {
        pub shared_secret: Vec<u8>,
        pub verify_token: S01VerifyToken,
    }
    impl ServerBoundPacket for S01EncryptionResponse {
        const PACKET_ID: i32 = 0x01;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let shared_secret = decoder.read_varint().and_then(|l|
                decoder.read_bytes(l as _)
            )?;

            let verify_token = if decoder.read_bool()? {
                S01VerifyToken::With {
                    verify_token: decoder.read_varint().and_then(|l|
                        decoder.read_bytes(l as _)
                    )?,
                }
            }
            else {
                S01VerifyToken::Without {
                    salt: decoder.read_i64()?,
                    message_signature: decoder.read_varint().and_then(|l|
                        decoder.read_bytes(l as _)
                    )?,
                }
            };

            Ok(Self {
                shared_secret,
                verify_token,
            })
        }
    }

    /// Sent in response to C02LoginPluginRequest
    ///
    /// <https://wiki.vg/Protocol#Login_Plugin_Response>
    #[derive(Clone, Debug)]
    pub struct S02LoginPluginResponse {
        /// Should match ID from server.
        pub message_id: VarInt,
        /// true if the client understood the request, false otherwise.
        /// When false, no payload follows.
        pub successful: bool,
        /// Any data, depending on the channel.
        /// The length of this array must be inferred from the packet length.
        pub data: Option<Vec<u8>>,
    }
    impl ServerBoundPacket for S02LoginPluginResponse {
        const PACKET_ID: i32 = 0x02;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let message_id = decoder.read_varint()?;
            let successful = decoder.read_bool()?;
            let data = successful.then(|| {
                decoder.read_to_end()
            }).transpose()?;

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
    use std::convert::TryFrom;

    use super::ServerBoundPacket;
    use crate::{
        data_types::{encoder::PacketDecoder, Position, Slot, VarInt, Identifier},
        DecodingError as Error, DecodingResult as Result,
    };

    use num_traits::{ ToPrimitive, FromPrimitive };
    use num_derive::{ ToPrimitive, FromPrimitive };

    /// Sent by client as confirmation of C37SynchronizePlayerPosition
    ///
    /// <https://wiki.vg/Protocol#Confirm_Teleportation>
    #[derive(Clone, Debug)]
    pub struct S00ConfirmTeleportation {
        /// The ID given by the Synchronize Player Position packet.
        pub teleport_id: VarInt,
    }
    impl ServerBoundPacket for S00ConfirmTeleportation {
        const PACKET_ID: i32 = 0x00;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                teleport_id: decoder.read_varint()?,
            })
        }
    }

    /// Used to send a chat message to the server.
    /// The message may not be longer than 256 characters or else the server will kick the client.
    ///
    /// The server will broadcast the same chat message to all players on the server 
    /// (including the player that sent the message), prepended with player's name.
    /// Specifically, it will respond with a translate chat component, 
    /// "chat.type.text" with the first parameter set to the display name of the player 
    /// (including some chat component logic to support clicking the name to send a PM)
    /// and the second parameter set to the message. See processing chat for more information.
    ///
    /// <https://wiki.vg/Protocol#Chat_Message>
    #[derive(Clone, Debug)]
    pub struct S04ChatMessage {
        pub message: String,
        pub timestamp: i64,
        /// The salt used to verify the signature hash.
        pub salt: i64,
        /// The signature used to verify the chat message's authentication.
        pub signature: Vec<u8>,
        pub signed_preview: bool,
    }
    impl ServerBoundPacket for S04ChatMessage {
        const PACKET_ID: i32 = 0x04;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                message: decoder.read_string()?,
                timestamp: decoder.read_i64()?,
                salt: decoder.read_i64()?,
                signature: decoder.read_varint().and_then(|l|
                    decoder.read_bytes(l as _)
                )?,
                signed_preview: decoder.read_bool()?,
            })
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
    #[repr(i32)]
    pub enum S06ActionId {
        PerformRespawn = 0,
        RequestStats = 1,
    }

    /// Sent by the client to notify the server of it's current state
    ///
    /// <https://wiki.vg/Protocol#Client_Command>
    #[derive(Clone, Debug)]
    pub struct S06ClientCommand {
        /// 0 (Perform respawn): 
        ///     Sent when the client is ready to complete login and when the client is ready to respawn after death.
        /// 1 (Request stats): 
        ///     Sent when the client opens the Statistics menu.
        pub action_id: S06ActionId,
    }
    impl ServerBoundPacket for S06ClientCommand {
        const PACKET_ID: i32 = 0x06;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let ai = decoder.read_varint()?;
            Ok(Self {
                action_id: S06ActionId::from_i32(ai)
                    .ok_or(Error::parse_error(
                        "packet 0x06",
                        format!("invalid action id (expected 0 or 1; received {ai})")
                    ))?,
            })
        }
    }

    /// Sent when the player connects, or when settings are changed.
    ///
    /// <https://wiki.vg/Protocol#Client_Information>
    #[derive(Clone, Debug)]
    pub struct S07ClientInformation {
        /// e.g. en_GB
        pub locale: String,
        /// Client-side render distance, in chunks
        pub view_distance: i8,
        /// 0: enabled, 1: commands only, 2: hidden. See processing chat for more information.
        pub chat_mode: VarInt,
        /// “Colors” multiplayer setting
        pub chat_colors: bool,
        /// Bit mask, see in <https://wiki.vg/Protocol#Client_Information>
        pub displayed_skin_parts: u8,
        /// 0: Left, 1: Right
        pub main_hand: VarInt,
        /// Enables filtering of text on signs and written book titles.
        /// Currently always false (i.e. the filtering is disabled)
        pub enable_text_filtering: bool,
        /// Servers usually list online players, this option should let you not show up in that list.
        pub allow_server_listings: bool,
    }
    impl ServerBoundPacket for S07ClientInformation {
        const PACKET_ID: i32 = 0x07;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                locale: decoder.read_string()?,
                view_distance: decoder.read_i8()?,
                chat_mode: decoder.read_varint()?,
                chat_colors: decoder.read_bool()?,
                displayed_skin_parts: decoder.read_u8()?,
                main_hand: decoder.read_varint()?,
                enable_text_filtering: decoder.read_bool()?,
                allow_server_listings: decoder.read_bool()?,
            })
        }
    }

    /// This packet is sent by the player when it clicks on a slot in a window.
    ///
    /// <https://wiki.vg/Protocol#Click_Container>
    #[derive(Clone, Debug)]
    pub struct S0AClickContainer {
        /// The ID of the window which was clicked. 0 for player inventory.
        pub window_id: u8,
        /// The last recieved State ID from either a Set Container Slot or a Set Container Content packet
        pub state_id: VarInt,
        /// The clicked slot number, see <https://wiki.vg/Protocol#Click_Container>.
        pub slot_id: i16,
        /// The button used in the click, see <https://wiki.vg/Protocol#Click_Container>.
        pub button: i8,
        /// Inventory operation mode, see <https://wiki.vg/Protocol#Click_Container>.
        pub mode: VarInt,
        /// List of slot ids with their new states
        pub slots: Vec<(i16, Slot)>,
        /// Item carried by the cursor.
        /// Has to be empty (item ID = -1) for drop mode, otherwise nothing will happen.
        pub carried_item: Slot,
    }
    impl ServerBoundPacket for S0AClickContainer {
        const PACKET_ID: i32 = 0x0A;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let window_id = decoder.read_u8()?;
            let state_id = decoder.read_varint()?;
            let slot_id = decoder.read_i16()?;
            let button = decoder.read_i8()?;
            let mode = decoder.read_varint()?;

            let slots_length = decoder.read_varint()?;
            let mut slots = Vec::with_capacity(slots_length.clamp(0, 2048) as _);
            for _ in 0..slots_length {
                slots.push((decoder.read_i16()?, Slot::decode_sync(decoder)?));
            }

            let carried_item = Slot::decode_sync(decoder)?;
            Ok(Self {
                window_id,
                state_id,
                slot_id,
                button,
                mode,
                slots,
                carried_item,
            })
        }
    }

    /// Mods and plugins can use this to send their data.
    ///
    /// <https://wiki.vg/Protocol#Plugin_Message_.28serverbound.29>
    #[derive(Clone, Debug)]
    pub struct S0CPluginMessage {
        pub channel: Identifier,
        pub data: Vec<u8>,
    }
    impl ServerBoundPacket for S0CPluginMessage {
        const PACKET_ID: i32 = 0x0C;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                channel: decoder.read_string()?.as_str().into(),
                data: decoder.read_to_end()?,
            })
        }
    }

    #[derive(Clone, Copy, Debug)]
    pub enum S0FInteractKind {
        /// 0
        Interact,
        /// 1
        Attack,
        /// 2
        InteractAt {
            target_x: f32,
            target_y: f32,
            target_z: f32,
            /// 0: main hand, 1: off hand.
            hand: VarInt,
        },
    }
    impl S0FInteractKind {
        pub fn is_interact(self) -> bool {
            match self {
                Self::Interact => true,
                _ => false,
            }
        }
        pub fn is_attack(self) -> bool {
            match self {
                Self::Attack => true,
                _ => false,
            }
        }
        pub fn is_interact_at(self) -> bool {
            match self {
                Self::InteractAt { .. } => true,
                _ => false,
            }
        }
    }

    /// This packet is sent from the client to the server when
    /// the client attacks or right-clicks another entity (a player, minecart, etc).
    /// A Notchian server only accepts this packet if the entity
    /// being attacked/used is visible without obstruction
    /// and within a 4-unit radius of the player's position.
    /// The target X, Y, and Z fields represent the difference between the
    /// vector location of the cursor at the time of the packet and the entity's position.
    /// Note that middle-click in creative mode is interpreted by
    /// the client and sent as a Set Creative Mode Slot packet instead.
    #[derive(Clone, Debug)]
    pub struct S0FInteract {
        pub entity_id: VarInt,
        pub kind: S0FInteractKind,
        pub sneaking: bool,
    }
    impl ServerBoundPacket for S0FInteract {
        const PACKET_ID: i32 = 0x0F;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let entity_id = decoder.read_varint()?;
            let kind = match decoder.read_varint()? {
                0 => S0FInteractKind::Interact,
                1 => S0FInteractKind::Attack,
                2 => S0FInteractKind::InteractAt {
                    target_x: decoder.read_f32()?,
                    target_y: decoder.read_f32()?,
                    target_z: decoder.read_f32()?,
                    hand: decoder.read_varint()?,
                },

                v => return Err(Error::parse_error(
                    "packet 0x0F",
                    format!("invalid kind id (expected 0, 1 or 2; received {v})"),
                )),
            };
            let sneaking = decoder.read_bool()?;
            Ok(Self {
                entity_id,
                kind,
                sneaking,
            })
        }
    }

    /// The server will frequently send out a keep-alive, 
    /// each containing a random ID.
    /// The client must respond with the same packet.
    ///
    /// <https://wiki.vg/Protocol#Keep_Alive_.28serverbound.29>
    #[derive(Clone, Debug)]
    pub struct S11KeepAlive {
        pub id: i64,
    }
    impl ServerBoundPacket for S11KeepAlive {
        const PACKET_ID: i32 = 0x11;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                id: decoder.read_i64()?,
            })
        }
    }

    /// Updates the player's XYZ position on the server.
    ///
    /// <https://wiki.vg/Protocol#Set_Player_Position>
    #[derive(Clone, Debug)]
    pub struct S13SetPlayerPosition {
        /// Absolute position
        pub x: f64,
        /// Absolute feet position, normally Head Y - 1.62
        pub feet_y: f64,
        /// Absolute position
        pub z: f64,
        /// True if the client is on the ground, false otherwise
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S13SetPlayerPosition {
        const PACKET_ID: i32 = 0x13;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                x: decoder.read_f64()?,
                feet_y: decoder.read_f64()?,
                z: decoder.read_f64()?,
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// A combination of S13PlayerRotation and S13PlayerPosition.
    ///
    /// <https://wiki.vg/Protocol#Set_Player_Position_and_Rotation>
    #[derive(Clone, Debug)]
    pub struct S13SetPlayerPositionAndRotation {
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
    impl ServerBoundPacket for S13SetPlayerPositionAndRotation {
        const PACKET_ID: i32 = 0x13;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
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
    /// <https://wiki.vg/Protocol#Set_Player_Rotation>
    #[derive(Clone, Debug)]
    pub struct S15SetPlayerRotation {
        /// Absolute rotation on the X Axis, in degrees
        pub yaw: f32,
        /// Absolute rotation on the Y Axis, in degrees
        pub pitch: f32,
        /// True if the client is on the ground, false otherwise
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S15SetPlayerRotation {
        const PACKET_ID: i32 = 0x15;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                yaw: decoder.read_f32()?,
                pitch: decoder.read_f32()?,
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// This packet is used to indicate whether the player is on ground (walking/swimming), or airborne (jumping/falling).
    ///
    /// <https://wiki.vg/Protocol#Set_Player_On_Ground>
    #[derive(Clone, Debug)]
    pub struct S16SetPlayerOnGround {
        /// True if the client is on the ground, false otherwise.
        pub on_ground: bool,
    }
    impl ServerBoundPacket for S16SetPlayerOnGround {
        const PACKET_ID: i32 = 0x15;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                on_ground: decoder.read_bool()?,
            })
        }
    }

    /// The vanilla client sends this packet when the player starts/stops
    /// flying with the Flags parameter changed accordingly.
    ///
    /// <https://wiki.vg/Protocol#Player_Abilities_.28serverbound.29>
    #[derive(Clone, Debug)]
    pub struct S1BPlayerAbilities {
        /// 0x02: is flying
        pub flags: u8,
    }
    impl ServerBoundPacket for S1BPlayerAbilities {
        const PACKET_ID: i32 = 0x1B;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                flags: decoder.read_u8()?,
            })
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
    #[repr(i32)]
    pub enum S1CStatus {
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

    #[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
    #[repr(i32)]
    pub enum S1CDiggingFace {
        /// -Y
        Bottom = 0,
        /// +Y
        Top = 1,
        /// -Z
        North = 2,
        /// +Z
        South = 3,
        /// -X
        West = 4,
        /// +X
        East = 5,
    }

    /// Sent when the player mines a block.
    ///
    /// <https://wiki.vg/Protocol#Player_Digging>
    #[derive(Clone, Debug)]
    pub struct S1CPlayerAction {
        /// The action the player is taking against the block
        pub status: S1CStatus,
        /// Block position
        pub position: Position,
        /// The face being hit
        pub face: S1CDiggingFace,
        pub sequence: VarInt,
    }
    impl ServerBoundPacket for S1CPlayerAction {
        const PACKET_ID: i32 = 0x1C;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let sid = decoder.read_varint()?;
            let status = S1CStatus::from_i32(sid)
                .ok_or(Error::parse_error(
                    "packet 0x1C",
                    format!("invalid player digging status (expected 0 through 6, received {sid}"),
                ))?;
            let position = Position::decode(decoder.read_i64()?);
            let fid = decoder.read_i8()?;
            let face = S1CDiggingFace::from_i8(fid)
                .ok_or(Error::parse_error(
                    "packet 0x1C",
                    format!("invalid player digging face (expected 0 through 5, received {fid})"),
                ))?;
            let sequence = decoder.read_varint()?;

            Ok(Self {
                status,
                position,
                face,
                sequence,
            })
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq, FromPrimitive, ToPrimitive)]
    #[repr(i32)]
    pub enum S1DActionId {
        StartSneaking = 0,
        StopSneaking = 1,
        /// Leave bed is only sent when the “Leave Bed” button is clicked on the sleep GUI,
        /// not when waking up due today time.
        LeaveBed = 2,
        StartSprinting = 3,
        StopSprinting = 4,
        StartJumpWithHorse = 5,
        StopJumpWithHorse = 6,
        /// Open horse inventory is only sent when pressing the inventory key (default: E)
        /// while on a horse — all other methods of opening a horse's inventory 
        /// (involving right-clicking or shift-right-clicking it) do not use this packet.
        OpenHorseInventory = 7,
        StartFlyingWithElytra = 8,
    }

    /// Sent by the client to indicate that it has performed certain actions:
    /// sneaking (crouching), sprinting, exiting a bed,
    /// jumping with a horse, and opening a horse's inventory while riding it.
    ///
    /// <https://wiki.vg/Protocol#Player_Command>
    #[derive(Clone, Debug)]
    pub struct S1DPlayerCommand {
        /// Player ID
        pub entity_id: VarInt,
        /// The ID of the action, see <https://wiki.vg/Protocol#Player_Command>
        pub action_id: S1DActionId,
        /// Only used by the “start jump with horse” action, in which case it ranges from 0 to 100.
        /// In all other cases it is 0.
        pub jump_boost: VarInt,
    }
    impl ServerBoundPacket for S1DPlayerCommand {
        const PACKET_ID: i32 = 0x1D;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            let entity_id = decoder.read_varint()?;
            let ai = decoder.read_varint()?;
            let action_id = S1DActionId::from_i32(decoder.read_varint()?).ok_or(Error::parse_error(
                "packet 0x1D",
                format!("invalid action id (expected 0 through 8, received {ai})"),
            ))?;
            let jump_boost = decoder.read_i32()?;

            Ok(Self {
                entity_id,
                action_id,
                jump_boost,
            })
        }
    }

    /// Sent when the player changes the slot selection
    ///
    /// <https://wiki.vg/Protocol#Set_Held_Item_.28serverbound.29>
    #[derive(Clone, Debug)]
    pub struct S27SetHeldItem {
        /// The slot which the player has selected (0–8)
        pub slot: i16,
    }
    impl ServerBoundPacket for S27SetHeldItem {
        const PACKET_ID: i32 = 0x27;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                slot: decoder.read_i16()?,
            })
        }
    }

    /// While the user is in the standard inventory (i.e., not a crafting bench) in Creative mode,
    /// the player will send this packet.
    /// This action can be described as "set inventory slot".
    /// Picking up an item sets the slot to item ID -1.
    ///
    /// <https://wiki.vg/Protocol#Set_Creative_Mode_Slot>
    #[derive(Clone, Debug)]
    pub struct S2ASetCreativeModeSlot {
        pub slot_id: i16,
        pub slot: Slot,
    }
    impl ServerBoundPacket for S2ASetCreativeModeSlot {
        const PACKET_ID: i32 = 0x2A;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                slot_id: decoder.read_i16()?,
                slot: Slot::decode_sync(decoder)?,
            })
        }
    }

    /// Sent when the player's arm swings.
    ///
    /// <https://wiki.vg/Protocol#Animation_.28serverbound.29>
    #[derive(Clone, Debug)]
    pub struct S2ESwingArm {
        /// Hand used for the animation. 0: main hand, 1: off hand.
        pub hand: VarInt,
    }
    impl ServerBoundPacket for S2ESwingArm {
        const PACKET_ID: i32 = 0x2E;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                hand: decoder.read_varint()?,
            })
        }
    }

    /// Upon placing a block, this packet is sent once.
    ///
    /// <https://wiki.vg/Protocol#Player_Block_Placement>
    #[derive(Clone, Debug)]
    pub struct S30UseItemOn {
        /// The hand from which the block is placed; 0: main hand, 1: off hand
        pub hand: VarInt,
        /// Block position
        pub position: Position,
        /// The face on which the block is placed
        pub face: S1CDiggingFace,
        /// The position of the crosshair on the block, from 0 to 1 increasing from west to east
        pub cursor_position_x: f32,
        /// The position of the crosshair on the block, from 0 to 1 increasing from bottom to top
        pub cursor_position_y: f32,
        /// The position of the crosshair on the block, from 0 to 1 increasing from north to south
        pub cursor_position_z: f32,
        /// True when the player's head is inside of a block.
        pub inside_block: bool,
        pub sequence: VarInt,
    }
    impl ServerBoundPacket for S30UseItemOn {
        const PACKET_ID: i32 = 0x2E;

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self> {
            Ok(Self {
                hand: decoder.read_varint()?,
                position: Position::decode(decoder.read_i64()?),
                face: {
                    let fid = decoder.read_i8()?;
                    S1CDiggingFace::from_i8(fid)
                        .ok_or(Error::parse_error(
                            "packet 0x2E",
                            format!("invalid player digging face (expected 0 though 5, received {fid})"),
                        ))?
                },
                cursor_position_x: decoder.read_f32()?,
                cursor_position_y: decoder.read_f32()?,
                cursor_position_z: decoder.read_f32()?,
                inside_block: decoder.read_bool()?,
                sequence: decoder.read_varint()?,
            })
        }
    }
}
pub use play::*;

use crate::data_types::encoder::PacketDecoder;
use crate::packets::RawPacket;

use anyhow::{Error, Result};

pub trait ServerBoundPacket: Sized {
    fn packet_id() -> i32;
    fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self>;

    fn decode(raw_packet: RawPacket) -> Result<Self> {
        if raw_packet.packet_id != Self::packet_id() {
            return Err(Error::msg("Invalid packet id"));
        }
        Self::run_decoder(&mut PacketDecoder::new(raw_packet))
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
    use crate::data_types::encoder::PacketDecoder;
    use crate::packets::RawPacket;

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
    use crate::data_types::encoder::PacketDecoder;
    use crate::data_types::VarInt;

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
            } else {
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
    use crate::data_types::VarInt;

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
            0x01
        }

        fn run_decoder(decoder: &mut PacketDecoder) -> Result<Self, Error> {
            Ok(Self {
                teleport_id: decoder.read_varint()?,
            })
        }
    }
}
pub use play::*;

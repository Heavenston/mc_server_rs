use crate::packets::RawPacket;

use std::convert::TryFrom;

pub trait ServerBoundPacket: TryFrom<RawPacket> {
    fn packet_id() -> i32;
}

mod handshake {
    use super::ServerBoundPacket;
    use crate::data_types::encoder;
    use crate::packets::RawPacket;

    use anyhow::Error;
    use byteorder::{BigEndian, ReadBytesExt};
    use std::convert::TryFrom;
    use std::io::Cursor;

    /// This causes the server to switch into the target state.
    ///
    /// https://wiki.vg/Protocol#Handshake
    #[derive(Clone, Debug)]
    pub struct S00Handshake {
        pub protocol_version: i32,
        pub server_addr: String,
        pub server_port: u16,
        pub next_state: i32,
    }
    impl ServerBoundPacket for S00Handshake {
        fn packet_id() -> i32 {
            0x00
        }
    }
    impl TryFrom<RawPacket> for S00Handshake {
        type Error = Error;

        fn try_from(raw_packet: RawPacket) -> Result<Self, Self::Error> {
            if Self::packet_id() != raw_packet.packet_id {
                return Err(Error::msg("Invalid packet id"));
            };
            let mut data = Cursor::new(&raw_packet.data);
            let protocol_version = encoder::varint::decode_sync(&mut data)?;
            let server_addr = encoder::string::decode_sync(&mut data)?;
            let server_port = data.read_u16::<BigEndian>()?;
            let next_state = encoder::varint::decode_sync(&mut data)?;

            Ok(Self {
                protocol_version,
                server_addr,
                server_port,
                next_state,
            })
        }
    }
}
pub use handshake::*;

mod status {
    use super::ServerBoundPacket;
    use crate::packets::RawPacket;

    use anyhow::Error;
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
    }
    impl TryFrom<RawPacket> for S00Request {
        type Error = Error;

        fn try_from(value: RawPacket) -> Result<Self, Self::Error> {
            if value.packet_id != Self::packet_id() {
                return Err(Error::msg("Invalid packet id"));
            }
            if value.data.len() != 0 {
                return Err(Error::msg("Invalid data"));
            }

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
    use crate::data_types::encoder;
    use crate::packets::RawPacket;

    use anyhow::Error;
    use byteorder::ReadBytesExt;
    use std::convert::TryFrom;
    use std::io::{Cursor, Read};

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
    }
    impl TryFrom<RawPacket> for S00LoginStart {
        type Error = Error;

        fn try_from(packet: RawPacket) -> Result<Self, Self::Error> {
            if packet.packet_id != Self::packet_id() {
                return Err(Error::msg("Invalid packet id"));
            };
            Ok(Self {
                name: encoder::string::decode_sync(&mut Cursor::new(packet.data.as_ref()))?,
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
    }
    impl TryFrom<RawPacket> for S01EncryptionResponse {
        type Error = Error;

        fn try_from(value: RawPacket) -> Result<Self, Self::Error> {
            if value.packet_id != Self::packet_id() {
                return Err(Error::msg("Invalid packet id"));
            }
            let mut data = Cursor::new(value.data.as_ref());

            let shared_secret_length = encoder::varint::decode_sync(&mut data)?;
            let mut shared_secret = Vec::with_capacity(shared_secret_length as usize);
            for _ in 0..shared_secret_length {
                shared_secret.push(data.read_u8()?);
            }

            let verify_token_length = encoder::varint::decode_sync(&mut data)?;
            let mut verify_token = Vec::with_capacity(verify_token_length as usize);
            for _ in 0..verify_token_length {
                verify_token.push(data.read_u8()?);
            }

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
        pub message_id: i32,
        pub successful: bool,
        pub data: Option<Vec<u8>>,
    }
    impl ServerBoundPacket for S02LoginPluginResponse {
        fn packet_id() -> i32 {
            0x02
        }
    }
    impl TryFrom<RawPacket> for S02LoginPluginResponse {
        type Error = Error;

        fn try_from(value: RawPacket) -> Result<Self, Self::Error> {
            if value.packet_id != Self::packet_id() {
                return Err(Error::msg("Invalid packet id"));
            }
            let mut data = Cursor::new(value.data.as_ref());

            let message_id = encoder::varint::decode_sync(&mut data)?;
            let successful = data.read_u8()? == 1;
            let resp_data = if successful {
                let mut resp_data = vec![];
                data.read_to_end(&mut resp_data)?;
                Some(resp_data)
            } else {
                None
            };

            Ok(Self {
                message_id,
                successful,
                data: resp_data,
            })
        }
    }
}
pub use login::*;

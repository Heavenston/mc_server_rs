use crate::packets::{RawPacket, encoder};
use std::convert::{TryFrom, TryInto};
use anyhow::Error;
use std::io::Cursor;
use byteorder::{ReadBytesExt, BigEndian};

pub trait ServerBoundPacket: TryFrom<RawPacket> {
    fn packet_id() -> i32;
}

pub struct HandshakePacket {
    pub protocol_version: i32,
    pub server_addr: String,
    pub server_port: u16,
    pub next_state: i32,
}
impl ServerBoundPacket for HandshakePacket {
    fn packet_id() -> i32 {
        0
    }
}
impl TryFrom<RawPacket> for HandshakePacket {
    type Error = Error;

    fn try_from(raw_packet: RawPacket) -> Result<Self, Self::Error> {
        if Self::packet_id() != raw_packet.packet_id {
            return Err(Error::msg("Invalid packet id"))
        };
        let mut data = Cursor::new(&raw_packet.data);
        let protocol_version = encoder::varint::decode_sync(&mut data)?;
        let server_addr = encoder::string::decode_string_sync(&mut data)?;
        let server_port = data.read_u16::<BigEndian>()?;
        let next_state = encoder::varint::decode_sync(&mut data)?;

        Ok(Self {
            protocol_version,
            server_addr,
            server_port,
            next_state
        })
    }
}

pub struct RequestPacket;
impl ServerBoundPacket for RequestPacket {
    fn packet_id() -> i32 {
        0
    }
}
impl TryFrom<RawPacket> for RequestPacket {
    type Error = Error;

    fn try_from(value: RawPacket) -> Result<Self, Self::Error> {
        if value.packet_id != Self::packet_id() {
            return Err(Error::msg("Invalid packet id"));
        }
        if value.data.len() != 0 {
            return Err(Error::msg("Invalid data"));
        }

        Ok(RequestPacket)
    }
}

pub struct PingPacket {
    pub payload: i64,
}
impl ServerBoundPacket for  PingPacket {
    fn packet_id() -> i32 {
        1
    }
}
impl TryFrom<RawPacket> for PingPacket {
    type Error = Error;

    fn try_from(value: RawPacket) -> Result<Self, Self::Error> {
        if value.packet_id != Self::packet_id() {
            return Err(Error::msg("Invalid packet id"));
        }
        Ok(PingPacket {
            payload: i64::from_be_bytes(value.data.as_ref().try_into()?),
        })
    }
}

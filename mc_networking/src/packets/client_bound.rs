use crate::packets::{encoder, RawPacket};

pub trait ClientBoundPacket: Into<RawPacket> {
    fn packet_id() -> i32;
}

#[derive(Clone, Debug)]
pub struct ResponsePacket {
    pub json_response: serde_json::Value,
}
impl ResponsePacket {
    pub fn new(json_response: serde_json::Value) -> Self {
        Self { json_response }
    }
}
impl ClientBoundPacket for ResponsePacket {
    fn packet_id() -> i32 {
        0
    }
}
impl Into<RawPacket> for ResponsePacket {
    fn into(self) -> RawPacket {
        RawPacket::new(
            Self::packet_id(),
            encoder::string::encode_string(&self.json_response.to_string()).into_boxed_slice(),
        )
    }
}

#[derive(Clone, Debug)]
pub struct PongPacket {
    pub payload: i64,
}
impl PongPacket {
    pub fn new(payload: i64) -> Self {
        Self { payload }
    }
}
impl ClientBoundPacket for PongPacket {
    fn packet_id() -> i32 {
        1
    }
}
impl Into<RawPacket> for PongPacket {
    fn into(self) -> RawPacket {
        RawPacket::new(
            Self::packet_id(),
            Box::new(self.payload.to_be_bytes()) as Box<[u8]>,
        )
    }
}

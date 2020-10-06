use crate::packets::{encoder, RawPacket};

pub trait ClientBoundPacket: Into<RawPacket> {
    fn packet_id() -> i32;
}

mod status {
    use crate::packets::{encoder, RawPacket};
    use super::ClientBoundPacket;

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
}
pub use status::*;

mod login {
    use uuid::Uuid;
    use crate::packets::{encoder, RawPacket};
    use super::ClientBoundPacket;

    #[derive(Clone, Debug)]
    pub struct LoginDisconnectPacket {
        pub reason: serde_json::Value,
    }
    impl LoginDisconnectPacket {
        pub fn new(reason: serde_json::Value) -> Self {
            Self { reason }
        }
    }
    impl ClientBoundPacket for LoginDisconnectPacket {
        fn packet_id() -> i32 {
            0x00
        }
    }
    impl Into<RawPacket> for LoginDisconnectPacket {
        fn into(self) -> RawPacket {
            RawPacket::new(
                Self::packet_id(),
                encoder::string::encode_string(&self.reason.to_string()).into_boxed_slice(),
            )
        }
    }

    #[derive(Clone, Debug)]
    pub struct EncryptionRequest {
        pub server_id: String,
        pub public_key: Vec<u8>,
        pub verify_token: Vec<u8>,
    }
    impl EncryptionRequest {
        pub fn new(server_id: String, public_key: Vec<u8>, verify_token: Vec<u8>) -> Self {
            Self {
                server_id,
                public_key,
                verify_token,
            }
        }
    }
    impl ClientBoundPacket for EncryptionRequest {
        fn packet_id() -> i32 {
            0x01
        }
    }
    impl Into<RawPacket> for EncryptionRequest {
        fn into(mut self) -> RawPacket {
            let mut data = vec![];

            data.append(&mut encoder::string::encode_string(&self.server_id));
            data.append(&mut encoder::varint::encode(self.public_key.len() as i32));
            data.append(&mut self.public_key);
            data.append(&mut encoder::varint::encode(self.verify_token.len() as i32));
            data.append(&mut self.verify_token);

            RawPacket::new(
                Self::packet_id(),
                data.into_boxed_slice(),
            )
        }
    }

    #[derive(Clone, Debug)]
    pub struct LoginSuccessPacket {
        pub uuid: Uuid,
        pub username: String,
    }
    impl LoginSuccessPacket {
        pub fn new(uuid: Uuid, username: String) -> Self {
            Self {
                uuid,
                username
            }
        }
    }
    impl ClientBoundPacket for LoginSuccessPacket {
        fn packet_id() -> i32 {
            0x02
        }
    }
    impl Into<RawPacket> for LoginSuccessPacket {
        fn into(self) -> RawPacket {
            let mut data = vec![];

            data.append(&mut self.uuid.as_bytes().to_vec());
            data.append(&mut encoder::string::encode_string(&self.username));

            RawPacket::new(
                Self::packet_id(),
                data.into_boxed_slice(),
            )
        }
    }

    #[derive(Clone, Debug)]
    pub struct SetCompressionPacket {
        pub threshold: i32,
    }
    impl SetCompressionPacket {
        pub fn new(threshold: i32) -> Self {
            Self { threshold }
        }
    }
    impl ClientBoundPacket for SetCompressionPacket {
        fn packet_id() -> i32 {
            0x03
        }
    }
    impl Into<RawPacket> for SetCompressionPacket {
        fn into(self) -> RawPacket {
            RawPacket::new(
                Self::packet_id(),
                encoder::varint::encode(self.threshold).into(),
            )
        }
    }

    #[derive(Clone, Debug)]
    pub struct LoginPluginRequest {
        pub message_id: i32,
        pub channel: String,
        pub data: Vec<u8>,
    }
    impl LoginPluginRequest {
        pub fn new(message_id: i32, channel: String, data: Vec<u8>) -> Self {
            Self {
                message_id, channel, data
            }
        }
    }
    impl ClientBoundPacket for LoginPluginRequest {
        fn packet_id() -> i32 {
            0x04
        }
    }
    impl Into<RawPacket> for LoginPluginRequest {
        fn into(mut self) -> RawPacket {
            let mut data = vec![];

            data.append(&mut encoder::varint::encode(self.message_id));
            data.append(&mut encoder::string::encode_string(&self.channel));
            data.append(&mut self.data);

            RawPacket::new(
                Self::packet_id(),
                data.into_boxed_slice()
            )
        }
    }
}
pub use login::*;
use crate::packets::RawPacket;
pub trait ClientBoundPacket: Into<RawPacket> {
    fn packet_id() -> i32;
}

mod status {
    use super::ClientBoundPacket;
    use crate::data_types::encoder;
    use crate::packets::RawPacket;

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
            0x00
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
            0x01
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
    use super::ClientBoundPacket;
    use crate::data_types::encoder;
    use crate::packets::RawPacket;
    use uuid::Uuid;

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

            RawPacket::new(Self::packet_id(), data.into_boxed_slice())
        }
    }

    #[derive(Clone, Debug)]
    pub struct LoginSuccessPacket {
        pub uuid: Uuid,
        pub username: String,
    }
    impl LoginSuccessPacket {
        pub fn new(uuid: Uuid, username: String) -> Self {
            Self { uuid, username }
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

            RawPacket::new(Self::packet_id(), data.into_boxed_slice())
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
                message_id,
                channel,
                data,
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

            RawPacket::new(Self::packet_id(), data.into_boxed_slice())
        }
    }
}
pub use login::*;

mod play {
    use super::ClientBoundPacket;
    use crate::data_types::encoder;
    use crate::packets::RawPacket;
    use crate::nbt_map::NBTMap;

    use anyhow::{Result};
    use serde::{Serialize};
    use std::collections::HashMap;

    #[derive(Clone, Debug, Serialize)]
    pub struct JoinGamePacketDimensionElement {
        pub natural: i8,
        pub ambient_light: f32,
        pub has_ceiling: i8,
        pub has_skylight: i8,
        pub fixed_time: i64,
        pub shrunk: i8,
        pub ultrawarm: i8,
        pub has_raids: i8,
        pub respawn_anchor_works: i8,
        pub bed_works: i8,
        pub piglin_safe: i8,
        pub coordinate_scale: f32,
        pub logical_height: i32,
        pub infiniburn: String,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct JoinGamePacketBiomeEffectsMoodSound {
        pub tick_delay: i32,
        pub offset: f32,
        pub sound: String,
        pub block_search_extent: i32,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct JoinGamePacketBiomeEffects {
        pub sky_color: i32,
        pub water_fog_color: i32,
        pub fog_color: i32,
        pub water_color: i32,
        pub mood_sound: JoinGamePacketBiomeEffectsMoodSound,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct JoinGamePacketBiomeElement {
        pub depth: f32,
        pub temperature: f32,
        pub downfall: f32,
        pub precipitation: String,
        pub category: String,
        pub scale: f32,
        pub effects: JoinGamePacketBiomeEffects,
    }

    #[derive(Clone, Debug)]
    pub struct JoinGamePacketDimensionCodec {
        pub dimensions: HashMap<String, JoinGamePacketDimensionElement>,
        pub biomes: HashMap<String, JoinGamePacketBiomeElement>,
    }

    #[derive(Clone, Debug, Serialize)]
    struct JoinGamePacketDimensionCodecInner {
        #[serde(rename = "minecraft:dimension_type")]
        pub dimensions: NBTMap<JoinGamePacketDimensionElement>,
        #[serde(rename = "minecraft:worldgen/biome")]
        pub biomes: NBTMap<JoinGamePacketBiomeElement>,
    }

    impl JoinGamePacketDimensionCodec {
        fn encode(self, buf: &mut Vec<u8>) -> Result<()> {
            let mut dimension_map = NBTMap::new("minecraft:dimension_type".into());
            for (name, element) in self.dimensions {
                dimension_map.push_element(name, element);
            }
            let mut biome_map = NBTMap::new("minecraft:worldgen/biome".into());
            for (name, element) in self.biomes {
                biome_map.push_element(name, element);
            }
            let codec = JoinGamePacketDimensionCodecInner {
                dimensions: dimension_map,
                biomes: biome_map,
            };
            nbt::ser::to_writer(buf, &codec, None)?;
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    pub struct JoinGamePacket {
        pub entity_id: i32,
        pub is_hardcore: bool,
        pub gamemode: u8,
        pub previous_gamemode: u8,
        pub world_names: Vec<String>,
        pub dimension_codec: JoinGamePacketDimensionCodec,
        pub dimension: JoinGamePacketDimensionElement,
        pub world_name: String,
        pub hashed_seed: u64,
        pub max_players: i32,
        pub view_distance: i32,
        pub reduced_debug_info: bool,
        pub enable_respawn_screen: bool,
        pub is_debug: bool,
        pub is_flat: bool,
    }
    impl JoinGamePacket {
        pub fn new(
            entity_id: i32,
            is_hardcore: bool,
            gamemode: u8,
            previous_gamemode: u8,
            world_names: Vec<String>,
            dimension_codec: JoinGamePacketDimensionCodec,
            dimension: JoinGamePacketDimensionElement,
            world_name: String,
            hashed_seed: u64,
            max_players: i32,
            view_distance: i32,
            reduced_debug_info: bool,
            enable_respawn_screen: bool,
            is_debug: bool,
            is_flat: bool,
        ) -> Self {
            Self {
                entity_id,
                is_hardcore,
                gamemode,
                previous_gamemode,
                world_names,
                dimension_codec,
                dimension,
                world_name,
                hashed_seed,
                max_players,
                view_distance,
                reduced_debug_info,
                enable_respawn_screen,
                is_debug,
                is_flat,
            }
        }
    }
    impl ClientBoundPacket for JoinGamePacket {
        fn packet_id() -> i32 {
            0x24
        }
    }
    impl Into<RawPacket> for JoinGamePacket {
        fn into(self) -> RawPacket {
            let mut data = vec![];

            data.extend_from_slice(&self.entity_id.to_be_bytes());
            data.push(if self.is_hardcore { 1 } else { 0 });
            data.push(self.gamemode);
            data.push(self.previous_gamemode);
            data.append(&mut encoder::varint::encode(self.world_names.len() as i32));
            for world_name in self.world_names.iter() {
                data.append(&mut encoder::string::encode_string(world_name));
            }
            self.dimension_codec.encode(&mut data).unwrap();
            nbt::ser::to_writer(&mut data, &self.dimension, None).unwrap();
            data.append(&mut encoder::string::encode_string(&self.world_name));
            data.extend_from_slice(&self.hashed_seed.to_be_bytes());
            data.append(&mut encoder::varint::encode(self.max_players));
            data.append(&mut encoder::varint::encode(self.view_distance));
            data.push(self.reduced_debug_info as u8);
            data.push(self.enable_respawn_screen as u8);
            data.push(self.is_debug as u8);
            data.push(self.is_flat as u8);

            RawPacket::new(Self::packet_id(), data.into_boxed_slice())
        }
    }
}
pub use play::*;

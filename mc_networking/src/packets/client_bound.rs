use crate::data_types::encoder::PacketEncoder;
use crate::packets::RawPacket;

pub trait ClientBoundPacket {
    fn packet_id() -> i32;
    fn encode(&self, encoder: &mut PacketEncoder);

    fn to_rawpacket(&self) -> RawPacket {
        let mut packet_encoder = PacketEncoder::new();
        self.encode(&mut packet_encoder);
        RawPacket::new(
            Self::packet_id(),
            packet_encoder.consume().into_boxed_slice(),
        )
    }
}

mod status {
    use super::ClientBoundPacket;
    use crate::data_types::encoder::PacketEncoder;

    #[derive(Clone, Debug)]
    pub struct ResponsePacket {
        pub json_response: serde_json::Value,
    }
    impl ClientBoundPacket for ResponsePacket {
        fn packet_id() -> i32 {
            0x00
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.json_response.to_string());
        }
    }

    #[derive(Clone, Debug)]
    pub struct PongPacket {
        pub payload: i64,
    }
    impl ClientBoundPacket for PongPacket {
        fn packet_id() -> i32 {
            0x01
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_i64(self.payload);
        }
    }
}
pub use status::*;

mod login {
    use super::ClientBoundPacket;
    use crate::data_types::encoder::PacketEncoder;
    use crate::data_types::VarInt;
    use uuid::Uuid;

    #[derive(Clone, Debug)]
    pub struct LoginDisconnectPacket {
        pub reason: serde_json::Value,
    }
    impl ClientBoundPacket for LoginDisconnectPacket {
        fn packet_id() -> i32 {
            0x00
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.reason.to_string());
        }
    }

    #[derive(Clone, Debug)]
    pub struct EncryptionRequest {
        pub server_id: String,
        pub public_key: Vec<u8>,
        pub verify_token: Vec<u8>,
    }
    impl ClientBoundPacket for EncryptionRequest {
        fn packet_id() -> i32 {
            0x01
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.server_id);
            encoder.write_varint(self.public_key.len() as VarInt);
            encoder.write_bytes(self.public_key.as_slice());
            encoder.write_varint(self.verify_token.len() as i32);
            encoder.write_bytes(self.verify_token.as_slice());
        }
    }

    #[derive(Clone, Debug)]
    pub struct LoginSuccessPacket {
        pub uuid: Uuid,
        pub username: String,
    }
    impl ClientBoundPacket for LoginSuccessPacket {
        fn packet_id() -> i32 {
            0x02
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_uuid(&self.uuid);
            encoder.write_string(&self.username);
        }
    }

    #[derive(Clone, Debug)]
    pub struct SetCompressionPacket {
        pub threshold: i32,
    }
    impl ClientBoundPacket for SetCompressionPacket {
        fn packet_id() -> i32 {
            0x03
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.threshold);
        }
    }

    #[derive(Clone, Debug)]
    pub struct LoginPluginRequest {
        pub message_id: i32,
        pub channel: String,
        pub data: Vec<u8>,
    }
    impl ClientBoundPacket for LoginPluginRequest {
        fn packet_id() -> i32 {
            0x04
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.message_id);
            encoder.write_string(&self.channel);
            encoder.write_bytes(self.data.as_slice());
        }
    }
}
pub use login::*;

mod play {
    use super::ClientBoundPacket;
    use crate::data_types::{Angle, MetadataValue, Position, VarInt};
    use crate::nbt_map::NBTMap;

    use crate::data_types::encoder::PacketEncoder;
    use anyhow::Result;
    use serde::Serialize;
    use std::collections::HashMap;
    use uuid::Uuid;

    #[derive(Clone, Debug)]
    pub struct C00SpawnEntity {
        pub entity_id: VarInt,
        pub object_uuid: Uuid,
        pub kind: VarInt,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub pitch: Angle,
        pub yaw: Angle,
        pub data: i32,
        pub velocity_x: i16,
        pub velocity_y: i16,
        pub velocity_z: i16,
    }
    impl ClientBoundPacket for C00SpawnEntity {
        fn packet_id() -> i32 {
            0x00
        }
        fn encode(&self, packet_encoder: &mut PacketEncoder) {
            packet_encoder.write_varint(self.entity_id);
            packet_encoder.write_uuid(&self.object_uuid);
            packet_encoder.write_f64(self.x);
            packet_encoder.write_f64(self.y);
            packet_encoder.write_f64(self.z);
            packet_encoder.write_i8(self.pitch);
            packet_encoder.write_i8(self.yaw);
            packet_encoder.write_i32(self.data);
            packet_encoder.write_i16(self.velocity_x);
            packet_encoder.write_i16(self.velocity_y);
            packet_encoder.write_i16(self.velocity_z);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C01SpawnExperienceOrb {
        pub entity_id: VarInt,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub count: i16,
    }
    impl ClientBoundPacket for C01SpawnExperienceOrb {
        fn packet_id() -> i32 {
            0x01
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_i16(self.count);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C02SpawnWeatherEntity {
        pub entity_id: VarInt,
        pub kind: i8,
        pub x: f64,
        pub y: f64,
        pub z: f64,
    }
    impl ClientBoundPacket for C02SpawnWeatherEntity {
        fn packet_id() -> i32 {
            0x02
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C03SpawnLivingEntity {
        pub entity_id: VarInt,
        pub entity_uuid: Uuid,
        pub kind: VarInt,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: Angle,
        pub pitch: Angle,
        pub head_pitch: Angle,
        pub velocity_x: i16,
        pub velocity_y: i16,
        pub velocity_z: i16,
    }
    impl ClientBoundPacket for C03SpawnLivingEntity {
        fn packet_id() -> i32 {
            0x03
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_uuid(&self.entity_uuid);
            encoder.write_varint(self.kind);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_i8(self.yaw);
            encoder.write_i8(self.pitch);
            encoder.write_i8(self.head_pitch);
            encoder.write_i16(self.velocity_x);
            encoder.write_i16(self.velocity_y);
            encoder.write_i16(self.velocity_z);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C04SpawnPainting {
        pub entity_id: VarInt,
        pub entity_uuid: Uuid,
        pub motive: VarInt,
        pub location: Position,
        pub direction: u8,
    }
    impl ClientBoundPacket for C04SpawnPainting {
        fn packet_id() -> i32 {
            0x04
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_uuid(&self.entity_uuid);
            encoder.write_varint(self.motive);
            encoder.write_u64(self.location.encode());
            encoder.write_u8(self.direction);
        }
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C24JoinGameDimensionElement {
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
    pub struct C24JoinGameBiomeEffectsMoodSound {
        pub tick_delay: i32,
        pub offset: f32,
        pub sound: String,
        pub block_search_extent: i32,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C24JoinGameBiomeEffects {
        pub sky_color: i32,
        pub water_fog_color: i32,
        pub fog_color: i32,
        pub water_color: i32,
        pub mood_sound: C24JoinGameBiomeEffectsMoodSound,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C24JoinGameBiomeElement {
        pub depth: f32,
        pub temperature: f32,
        pub downfall: f32,
        pub precipitation: String,
        pub category: String,
        pub scale: f32,
        pub effects: C24JoinGameBiomeEffects,
    }

    #[derive(Clone, Debug)]
    pub struct C24JoinGameDimensionCodec {
        pub dimensions: HashMap<String, C24JoinGameDimensionElement>,
        pub biomes: HashMap<String, C24JoinGameBiomeElement>,
    }

    #[derive(Clone, Debug, Serialize)]
    struct C24JoinGameDimensionCodecInner {
        #[serde(rename = "minecraft:dimension_type")]
        pub dimensions: NBTMap<C24JoinGameDimensionElement>,
        #[serde(rename = "minecraft:worldgen/biome")]
        pub biomes: NBTMap<C24JoinGameBiomeElement>,
    }
    impl C24JoinGameDimensionCodec {
        fn encode<T: std::io::Write>(&self, buf: &mut T) -> Result<()> {
            let mut dimension_map = NBTMap::new("minecraft:dimension_type".into());
            for (name, element) in self.dimensions.iter() {
                dimension_map.push_element(name.clone(), element.clone());
            }
            let mut biome_map = NBTMap::new("minecraft:worldgen/biome".into());
            for (name, element) in self.biomes.iter() {
                biome_map.push_element(name.clone(), element.clone());
            }
            let codec = C24JoinGameDimensionCodecInner {
                dimensions: dimension_map,
                biomes: biome_map,
            };
            nbt::ser::to_writer(buf, &codec, None)?;
            Ok(())
        }
    }

    #[derive(Clone, Debug)]
    pub struct C24JoinGame {
        pub entity_id: i32,
        pub is_hardcore: bool,
        pub gamemode: u8,
        pub previous_gamemode: u8,
        pub world_names: Vec<String>,
        pub dimension_codec: C24JoinGameDimensionCodec,
        pub dimension: C24JoinGameDimensionElement,
        pub world_name: String,
        pub hashed_seed: u64,
        pub max_players: i32,
        pub view_distance: i32,
        pub reduced_debug_info: bool,
        pub enable_respawn_screen: bool,
        pub is_debug: bool,
        pub is_flat: bool,
    }
    impl ClientBoundPacket for C24JoinGame {
        fn packet_id() -> i32 {
            0x24
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_i32(self.entity_id);
            encoder.write_bool(self.is_hardcore);
            encoder.write_u8(self.gamemode);
            encoder.write_u8(self.previous_gamemode);
            encoder.write_varint(self.world_names.len() as VarInt);
            for world_name in self.world_names.iter() {
                encoder.write_string(world_name);
            }
            self.dimension_codec.encode(encoder).unwrap();
            nbt::ser::to_writer(encoder, &self.dimension, None).unwrap();
            encoder.write_string(&self.world_name);
            encoder.write_u64(self.hashed_seed);
            encoder.write_varint(self.max_players);
            encoder.write_varint(self.view_distance);
            encoder.write_bool(self.reduced_debug_info);
            encoder.write_bool(self.enable_respawn_screen);
            encoder.write_bool(self.is_debug);
            encoder.write_bool(self.is_flat);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C36PlayerPositionAndLook {
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: f32,
        pub pitch: f32,
        pub flags: u8,
        pub teleport_id: i32,
    }
    impl ClientBoundPacket for C36PlayerPositionAndLook {
        fn packet_id() -> i32 {
            0x36
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_f32(self.yaw);
            encoder.write_f32(self.pitch);
            encoder.write_u8(self.flags);
            encoder.write_varint(self.teleport_id);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C44EntityMetadata {
        entity_id: i32,
        metadata: HashMap<u8, MetadataValue>,
    }
    impl ClientBoundPacket for C44EntityMetadata {
        fn packet_id() -> i32 {
            0x44
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            for (key, value) in self.metadata.iter() {
                encoder.write_u8(*key);
                encoder.write_bytes(value.encode().as_slice());
            }
            encoder.write_u8(0xFF);
        }
    }
}
pub use play::*;

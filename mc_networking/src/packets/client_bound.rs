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

    /// Response to S00Request with server ping infos
    ///
    /// https://wiki.vg/Protocol#Response
    #[derive(Clone, Debug)]
    pub struct C00Response {
        pub json_response: serde_json::Value,
    }
    impl ClientBoundPacket for C00Response {
        fn packet_id() -> i32 {
            0x00
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.json_response.to_string());
        }
    }

    /// Response to S01Ping with provided payload
    ///
    /// https://wiki.vg/Protocol#Pong
    #[derive(Clone, Debug)]
    pub struct C01Pong {
        pub payload: i64,
    }
    impl ClientBoundPacket for C01Pong {
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

    /// Disconnect the player with the specified message
    ///
    /// https://wiki.vg/Protocol#Disconnect_.28login.29
    #[derive(Clone, Debug)]
    pub struct C00LoginDisconnect {
        pub reason: serde_json::Value,
    }
    impl ClientBoundPacket for C00LoginDisconnect {
        fn packet_id() -> i32 {
            0x00
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.reason.to_string());
        }
    }

    /// Request packet encryption
    ///
    /// https://wiki.vg/Protocol#Encryption_Request
    #[derive(Clone, Debug)]
    pub struct C01EncryptionRequest {
        pub server_id: String,
        pub public_key: Vec<u8>,
        pub verify_token: Vec<u8>,
    }
    impl ClientBoundPacket for C01EncryptionRequest {
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

    /// Finishes Login stage
    ///
    /// https://wiki.vg/Protocol#Login_Success
    #[derive(Clone, Debug)]
    pub struct C02LoginSuccess {
        pub uuid: Uuid,
        pub username: String,
    }
    impl ClientBoundPacket for C02LoginSuccess {
        fn packet_id() -> i32 {
            0x02
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_uuid(&self.uuid);
            encoder.write_string(&self.username);
        }
    }

    /// Set packet compression
    ///
    /// https://wiki.vg/Protocol#Set_Compression
    #[derive(Clone, Debug)]
    pub struct C03SetCompression {
        pub threshold: i32,
    }
    impl ClientBoundPacket for C03SetCompression {
        fn packet_id() -> i32 {
            0x03
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.threshold);
        }
    }

    /// Used to implement a custom handshaking flow together with S02LoginPluginResponse.
    ///
    /// https://wiki.vg/Protocol#Login_Plugin_Request
    #[derive(Clone, Debug)]
    pub struct C04LoginPluginRequest {
        pub message_id: i32,
        pub channel: String,
        pub data: Vec<u8>,
    }
    impl ClientBoundPacket for C04LoginPluginRequest {
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

    /// Sent by the server when a vehicle or other non-living entity is created.
    ///
    /// https://wiki.vg/Protocol#Spawn_Entity
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

    /// Spawns one or more experience orbs.
    ///
    /// https://wiki.vg/Protocol#Spawn_Experience_Orb
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

    /// With this packet, the server notifies the client of thunderbolts striking within a 512 block radius around the player.
    /// The coordinates specify where exactly the thunderbolt strikes.
    ///
    /// https://wiki.vg/Protocol#Spawn_Weather_Entity
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

    /// Sent by the server when a living entity is spawned.
    ///
    /// https://wiki.vg/Protocol#Spawn_Living_Entity
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

    /// This packet shows location, name, and type of painting.
    ///
    /// https://wiki.vg/Protocol#Spawn_Painting
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

    /// Tells the client to unload a chunk column.
    /// It is legal to send this packet even if the given chunk is not currently loaded.
    ///
    /// https://wiki.vg/Protocol#Unload_Chunk
    #[derive(Clone, Debug)]
    pub struct C1EUnloadChunk {
        pub chunk_x: i32,
        pub chunk_z: i32,
    }
    impl ClientBoundPacket for C1EUnloadChunk {
        fn packet_id() -> i32 {
            0x1E
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_i32(self.chunk_x);
            encoder.write_i32(self.chunk_z);
        }
    }

    /// Used for a wide variety of game state things, from whether to bed use to gamemode to demo messages.
    ///
    /// https://wiki.vg/Protocol#Change_Game_State
    #[derive(Clone, Debug)]
    pub struct C1FChangeGameState {
        pub reason: u8,
        pub value: f32,
    }
    impl ClientBoundPacket for C1FChangeGameState {
        fn packet_id() -> i32 {
            0x1F
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_u8(self.reason);
            encoder.write_f32(self.value);
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

    /// This packet is sent by the server when an entity moves less then 8 blocks;
    /// if an entity moves more than 8 blocks C57EntityTeleport should be sent instead.
    ///
    /// https://wiki.vg/Protocol#Entity_Position
    #[derive(Clone, Debug)]
    pub struct C29EntityPosition {
        pub entity_id: VarInt,
        /// Change in X position as `(currentX * 32 - prevX * 32) * 128`
        pub delta_x: i16,
        /// Change in Y position as `(currentY * 32 - prevY * 32) * 128`
        pub delta_y: i16,
        /// Change in Z position as `(currentZ * 32 - prevZ * 32) * 128`
        pub delta_z: i16,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C29EntityPosition {
        fn packet_id() -> i32 {
            0x29
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_i16(self.delta_x);
            encoder.write_i16(self.delta_y);
            encoder.write_i16(self.delta_z);
            encoder.write_bool(self.on_ground);
        }
    }

    /// This packet is sent by the server when an entity moves less then 8 blocks;
    /// if an entity moves more than 8 blocks C57EntityTeleport should be sent instead.
    ///
    /// https://wiki.vg/Protocol#Entity_Position_and_Rotation
    #[derive(Clone, Debug)]
    pub struct C2AEntityPositionAndRotation {
        pub entity_id: VarInt,
        /// Change in X position as `(currentX * 32 - prevX * 32) * 128`
        pub delta_x: i16,
        /// Change in Y position as `(currentY * 32 - prevY * 32) * 128`
        pub delta_y: i16,
        /// Change in Z position as `(currentZ * 32 - prevZ * 32) * 128`
        pub delta_z: i16,
        /// New angle, not a delta
        pub yaw: Angle,
        /// New angle, not a delta
        pub pitch: Angle,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C2AEntityPositionAndRotation {
        fn packet_id() -> i32 {
            0x2A
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_i16(self.delta_x);
            encoder.write_i16(self.delta_y);
            encoder.write_i16(self.delta_z);
            encoder.write_i8(self.yaw);
            encoder.write_i8(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }

    /// This packet is sent by the server when an entity rotates.
    ///
    /// https://wiki.vg/Protocol#Entity_Rotation
    #[derive(Clone, Debug)]
    pub struct C2BEntityRotation {
        pub entity_id: VarInt,
        /// New angle, not a delta
        pub yaw: Angle,
        /// New angle, not a delta
        pub pitch: Angle,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C2BEntityRotation {
        fn packet_id() -> i32 {
            0x2A
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_i8(self.yaw);
            encoder.write_i8(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }

    /// Send information about the game
    ///
    /// https://wiki.vg/Pre-release_protocol#Join_Game
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

    /// Updates the player's position on the server.
    ///
    /// https://wiki.vg/Protocol#Player_Position_And_Look_.28clientbound.29
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

    /// Updates the client's location.
    /// This is used to determine what chunks should remain loaded and if a chunk load should be ignored;
    /// chunks outside of the view distance may be unloaded.
    ///
    /// https://wiki.vg/Protocol#Update_View_Position
    #[derive(Clone, Debug)]
    pub struct C41UpdateViewPosition {
        pub chunk_x: i32,
        pub chunk_z: i32,
    }
    impl ClientBoundPacket for C41UpdateViewPosition {
        fn packet_id() -> i32 {
            0x41
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.chunk_z);
            encoder.write_varint(self.chunk_x);
        }
    }

    /// Updates one or more metadata properties for an existing entity.
    /// Any properties not included in the Metadata field are left unchanged.
    ///
    /// https://wiki.vg/Protocol#Entity_Metadata
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

    /// Sent by the server after login to specify the coordinates of the spawn point
    /// (the point at which players spawn at, and which the compass points to).
    /// It can be sent at any time to update the point compasses point at.
    ///
    /// https://wiki.vg/Protocol#Spawn_Position
    #[derive(Clone, Debug)]
    pub struct C4ESpawnPosition {
        pub location: Position,
    }
    impl ClientBoundPacket for C4ESpawnPosition {
        fn packet_id() -> i32 {
            0x4E
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_u64(self.location.encode());
        }
    }

    /// This packet is sent by the server when an entity moves more than 8 blocks.
    ///
    /// https://wiki.vg/Protocol#Entity_Teleport
    #[derive(Clone, Debug)]
    pub struct C57EntityTeleport {
        pub entity_id: VarInt,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: Angle,   // New angle, not a delta
        pub pitch: Angle, // New angle, not a delta
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C57EntityTeleport {
        fn packet_id() -> i32 {
            0x57
        }
        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_i8(self.yaw);
            encoder.write_i8(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }
}
pub use play::*;

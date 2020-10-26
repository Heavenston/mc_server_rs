use crate::{data_types::encoder::PacketEncoder, packets::RawPacket};

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
        fn packet_id() -> i32 { 0x00 }

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
        fn packet_id() -> i32 { 0x01 }

        fn encode(&self, encoder: &mut PacketEncoder) { encoder.write_i64(self.payload); }
    }
}
pub use status::*;

mod login {
    use super::ClientBoundPacket;
    use crate::data_types::{encoder::PacketEncoder, VarInt};
    use uuid::Uuid;

    /// Disconnect the player with the specified message
    ///
    /// https://wiki.vg/Protocol#Disconnect_.28login.29
    #[derive(Clone, Debug)]
    pub struct C00LoginDisconnect {
        pub reason: serde_json::Value,
    }
    impl ClientBoundPacket for C00LoginDisconnect {
        fn packet_id() -> i32 { 0x00 }

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
        fn packet_id() -> i32 { 0x01 }

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
        fn packet_id() -> i32 { 0x02 }

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
        fn packet_id() -> i32 { 0x03 }

        fn encode(&self, encoder: &mut PacketEncoder) { encoder.write_varint(self.threshold); }
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
        fn packet_id() -> i32 { 0x04 }

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
    use crate::{
        data_types::{Angle, MetadataValue, Position, Slot, VarInt, command_data},
        nbt_map::NBTMap,
    };

    use crate::data_types::encoder::PacketEncoder;
    use anyhow::Result;
    use serde::Serialize;
    use std::collections::HashMap;
    use uuid::Uuid;
    use std::rc::Rc;

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
        fn packet_id() -> i32 { 0x00 }

        fn encode(&self, packet_encoder: &mut PacketEncoder) {
            packet_encoder.write_varint(self.entity_id);
            packet_encoder.write_uuid(&self.object_uuid);
            packet_encoder.write_f64(self.x);
            packet_encoder.write_f64(self.y);
            packet_encoder.write_f64(self.z);
            packet_encoder.write_angle(self.pitch);
            packet_encoder.write_angle(self.yaw);
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
        fn packet_id() -> i32 { 0x01 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_i16(self.count);
        }
    }

    /// Sent by the server when a living entity is spawned.
    ///
    /// https://wiki.vg/Protocol#Spawn_Living_Entity
    #[derive(Clone, Debug)]
    pub struct C02SpawnLivingEntity {
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
    impl ClientBoundPacket for C02SpawnLivingEntity {
        fn packet_id() -> i32 { 0x02 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_uuid(&self.entity_uuid);
            encoder.write_varint(self.kind);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.pitch);
            encoder.write_angle(self.head_pitch);
            encoder.write_i16(self.velocity_x);
            encoder.write_i16(self.velocity_y);
            encoder.write_i16(self.velocity_z);
        }
    }

    /// This packet shows location, name, and type of painting.
    ///
    /// https://wiki.vg/Protocol#Spawn_Painting
    #[derive(Clone, Debug)]
    pub struct C03SpawnPainting {
        pub entity_id: VarInt,
        pub entity_uuid: Uuid,
        pub motive: VarInt,
        pub location: Position,
        pub direction: u8,
    }
    impl ClientBoundPacket for C03SpawnPainting {
        fn packet_id() -> i32 { 0x03 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_uuid(&self.entity_uuid);
            encoder.write_varint(self.motive);
            encoder.write_u64(self.location.encode());
            encoder.write_u8(self.direction);
        }
    }

    /// This packet is sent by the server when a player comes into visible range.
    ///
    /// https://wiki.vg/Protocol#Spawn_Player
    #[derive(Clone, Debug)]
    pub struct C04SpawnPlayer {
        pub entity_id: VarInt,
        pub uuid: Uuid,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: Angle,
        pub pitch: Angle,
    }
    impl ClientBoundPacket for C04SpawnPlayer {
        fn packet_id() -> i32 { 0x04 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_uuid(&self.uuid);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.pitch);
        }
    }

    /// Sent whenever an entity should change animation.
    ///
    /// https://wiki.vg/Protocol#Entity_Animation_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C05EntityAnimation {
        pub entity_id: VarInt,
        pub animation: u8,
    }
    impl ClientBoundPacket for C05EntityAnimation {
        fn packet_id() -> i32 { 0x05 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_u8(self.animation);
        }
    }

    /// Identifying the difference between Chat/System Message is important as
    /// it helps respect the user's chat visibility options.
    /// See processing chat for more info about these positions.
    ///
    /// https://wiki.vg/Pre-release_protocol#Chat_Message_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C0EChatMessage {
        /// Limited to 32767 bytes
        pub json_data: serde_json::Value,
        /// 0: chat (chat box), 1: system message (chat box), 2: game info (above hotbar).
        pub position: u8,
        /// Used by the Notchian client for the disableChat launch option. Setting to `None` will always display the message regardless of the setting.
        pub sender: Option<uuid::Uuid>,
    }
    impl ClientBoundPacket for C0EChatMessage {
        fn packet_id() -> i32 { 0x0E }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.json_data.to_string());
            encoder.write_u8(self.position);
            match self.sender.as_ref() {
                Some(uuid) => encoder.write_uuid(uuid),
                None => {
                    encoder.write_u64(0);
                    encoder.write_u64(0);
                }
            }
        }
    }

    /// Lists all of the commands on the server, and how they are parsed.
    /// This is a directed graph, with one root node.
    /// Each redirect or child node must refer only to nodes that have already been declared.
    ///
    /// https://wiki.vg/Protocol#Declare_Commands
    #[derive(Clone)]
    pub struct C10DeclareCommands {
        pub root_node: Rc<command_data::RootNode>,
    }
    impl ClientBoundPacket for C10DeclareCommands {
        fn packet_id() -> i32 { 0x10 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            let mut graph_encoder = command_data::GraphEncoder::new();
            let root_index = graph_encoder.add_node(self.root_node.clone());
            let nodes = graph_encoder.encode();

            encoder.write_varint(nodes.len() as VarInt);
            for node in nodes.iter() {
                encoder.write_bytes(node);
            }
            encoder.write_varint(root_index);
        }
    }

    /// Sent by the server when items in multiple slots (in a window) are added/removed.
    /// This includes the main inventory, equipped armour and crafting slots.
    ///
    /// https://wiki.vg/Protocol#Window_Items
    #[derive(Clone, Debug)]
    pub struct C13WindowItems {
        pub window_id: u8,
        pub slots: Vec<Slot>,
    }
    impl ClientBoundPacket for C13WindowItems {
        fn packet_id() -> i32 { 0x13 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_u8(self.window_id);
            encoder.write_i16(self.slots.len() as i16);
            for slot in self.slots.iter() {
                encoder.write_bytes(slot.encode().as_slice());
            }
        }
    }

    pub struct C17PluginMessageBuilder {
        pub channel: String,
        pub encoder: PacketEncoder,
    }
    impl C17PluginMessageBuilder {
        pub fn new(channel: String) -> Self {
            Self {
                channel,
                encoder: PacketEncoder::new(),
            }
        }

        pub fn build(self) -> C17PluginMessage {
            C17PluginMessage {
                channel: self.channel,
                data: self.encoder.consume(),
            }
        }
    }

    /// Tells the client to unload a chunk column.
    /// It is legal to send this packet even if the given chunk is not currently loaded.
    ///
    /// https://wiki.vg/Protocol#Unload_Chunk
    #[derive(Clone, Debug)]
    pub struct C17PluginMessage {
        pub channel: String,
        pub data: Vec<u8>,
    }
    impl ClientBoundPacket for C17PluginMessage {
        fn packet_id() -> i32 { 0x17 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.channel);
            encoder.write_bytes(self.data.as_slice());
        }
    }

    /// Tells the client to unload a chunk column.
    /// It is legal to send this packet even if the given chunk is not currently loaded.
    ///
    /// https://wiki.vg/Protocol#Unload_Chunk
    #[derive(Clone, Debug)]
    pub struct C1CUnloadChunk {
        pub chunk_x: i32,
        pub chunk_z: i32,
    }
    impl ClientBoundPacket for C1CUnloadChunk {
        fn packet_id() -> i32 { 0x1C }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_i32(self.chunk_x);
            encoder.write_i32(self.chunk_z);
        }
    }

    /// Used for a wide variety of game state things, from whether to bed use to gamemode to demo messages.
    ///
    /// https://wiki.vg/Protocol#Change_Game_State
    #[derive(Clone, Debug)]
    pub struct C1DChangeGameState {
        pub reason: u8,
        pub value: f32,
    }
    impl ClientBoundPacket for C1DChangeGameState {
        fn packet_id() -> i32 { 0x1D }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_u8(self.reason);
            encoder.write_f32(self.value);
        }
    }

    /// The server will frequently send out a keep-alive, each containing a random ID.
    /// The client must respond with the same packet.
    /// If the client does not respond to them for over 30 seconds, the server kicks the client.
    /// Vice versa, if the server does not send any keep-alives for 20 seconds,
    /// the client will disconnect and yields a "Timed out" exception.
    ///
    /// https://wiki.vg/Protocol#Keep_Alive_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C1FKeepAlive {
        pub id: i64,
    }
    impl ClientBoundPacket for C1FKeepAlive {
        fn packet_id() -> i32 { 0x1F }

        fn encode(&self, encoder: &mut PacketEncoder) { encoder.write_i64(self.id); }
    }

    #[derive(Clone, Debug)]
    pub struct C20ChunkDataSection {
        pub block_count: i16,
        pub bits_per_block: u8,
        pub palette: Option<Vec<i32>>,
        pub data_array: Vec<i64>,
    }

    /// https://wiki.vg/Pre-release_protocol#Chunk_Data
    #[derive(Clone, Debug)]
    pub struct C20ChunkData {
        /// Chunk coordinate (block coordinate divided by 16, rounded down)
        pub chunk_x: i32,
        /// Chunk coordinate (block coordinate divided by 16, rounded down)
        pub chunk_z: i32,
        /// MUST be false if biomes is None, true otherwise
        pub full_chunk: bool,
        /// Bitmask with bits set to 1 for every 16×16×16 chunk section whose data is included in Data.
        /// The least significant bit represents the chunk section at the bottom of the chunk column (from y=0 to y=15).
        pub primary_bit_mask: VarInt,
        /// Compound containing one long array named MOTION_BLOCKING,
        /// which is a heightmap for the highest solid block at each position in the chunk
        /// (as a compacted long array with 256 entries at 9 bits per entry totaling 36 longs).
        pub heightmaps: nbt::Blob,
        /// See website
        pub biomes: Option<Vec<VarInt>>,
        /// The number of elements in the array is equal to the number of bits set in Primary Bit Mask.
        /// Sections are sent bottom-to-top, i.e. the first section, if sent, extends from Y=0 to Y=15.
        pub chunk_sections: Vec<C20ChunkDataSection>,
        /// All block entities in the chunk.
        /// Use the x, y, and z tags in the NBT to determine their positions.
        pub block_entities: Vec<nbt::Blob>,
    }
    impl ClientBoundPacket for C20ChunkData {
        fn packet_id() -> i32 { 0x20 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_i32(self.chunk_x);
            encoder.write_i32(self.chunk_z);
            encoder.write_bool(self.full_chunk && self.biomes.is_some());
            encoder.write_varint(self.primary_bit_mask);
            self.heightmaps.to_writer(encoder).unwrap();
            if self.full_chunk && self.biomes.is_some() {
                let biomes = self.biomes.as_ref().unwrap();
                encoder.write_varint(biomes.len() as VarInt);
                for biome in biomes.iter() {
                    encoder.write_varint(*biome);
                }
            }
            let mut data_encoder = PacketEncoder::new();
            for chunk_section in self.chunk_sections.iter() {
                data_encoder.write_i16(chunk_section.block_count);
                data_encoder.write_u8(chunk_section.bits_per_block);
                if let Some(palette) = chunk_section.palette.as_ref() {
                    data_encoder.write_varint(palette.len() as i32);
                    for palette_entry in palette {
                        data_encoder.write_varint(*palette_entry);
                    }
                }
                data_encoder.write_varint(chunk_section.data_array.len() as i32);
                for long in chunk_section.data_array.iter() {
                    data_encoder.write_i64(*long);
                }
            }
            let data = data_encoder.consume();
            encoder.write_varint(data.len() as i32);
            encoder.write_bytes(data.as_slice());
            encoder.write_varint(self.block_entities.len() as i32);
            for block_entity in self.block_entities.iter() {
                block_entity.to_writer(encoder).unwrap();
            }
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
        fn packet_id() -> i32 { 0x24 }

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

    /// This packet is sent by the server when an entity moves less then 8 blocks;
    /// if an entity moves more than 8 blocks C57EntityTeleport should be sent instead.
    ///
    /// https://wiki.vg/Protocol#Entity_Position
    #[derive(Clone, Debug)]
    pub struct C27EntityPosition {
        pub entity_id: VarInt,
        /// Change in X position as `(currentX * 32 - prevX * 32) * 128`
        pub delta_x: i16,
        /// Change in Y position as `(currentY * 32 - prevY * 32) * 128`
        pub delta_y: i16,
        /// Change in Z position as `(currentZ * 32 - prevZ * 32) * 128`
        pub delta_z: i16,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C27EntityPosition {
        fn packet_id() -> i32 { 0x27 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_i16(self.delta_x);
            encoder.write_i16(self.delta_y);
            encoder.write_i16(self.delta_z);
            encoder.write_bool(self.on_ground);
        }
    }

    /// This packet is sent by the server when an entity moves less then 8 blocks;
    /// if an entity moves more than 8 blocks C56EntityTeleport should be sent instead.
    ///
    /// https://wiki.vg/Protocol#Entity_Position_and_Rotation
    #[derive(Clone, Debug)]
    pub struct C28EntityPositionAndRotation {
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
    impl ClientBoundPacket for C28EntityPositionAndRotation {
        fn packet_id() -> i32 { 0x28 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_i16(self.delta_x);
            encoder.write_i16(self.delta_y);
            encoder.write_i16(self.delta_z);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }

    /// This packet is sent by the server when an entity rotates.
    ///
    /// https://wiki.vg/Protocol#Entity_Rotation
    #[derive(Clone, Debug)]
    pub struct C29EntityRotation {
        pub entity_id: VarInt,
        /// New angle, not a delta
        pub yaw: Angle,
        /// New angle, not a delta
        pub pitch: Angle,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C29EntityRotation {
        fn packet_id() -> i32 { 0x29 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }

    /// This packet is sent by the server when an entity doesn't move
    ///
    /// https://wiki.vg/Protocol#Entity_Movement
    #[derive(Clone, Debug)]
    pub struct C2AEntityMovement {
        pub entity_id: VarInt,
    }
    impl ClientBoundPacket for C2AEntityMovement {
        fn packet_id() -> i32 { 0x2A }

        fn encode(&self, encoder: &mut PacketEncoder) { encoder.write_varint(self.entity_id); }
    }

    /// https://wiki.vg/Protocol#Player_Abilities_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C30PlayerAbilities {
        pub flags: u8,
        pub flying_speed: f32,
        pub fov_modifier: f32,
    }
    impl ClientBoundPacket for C30PlayerAbilities {
        fn packet_id() -> i32 { 0x30 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_u8(self.flags);
            encoder.write_f32(self.flying_speed);
            encoder.write_f32(self.fov_modifier);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C32PlayerInfoAddPlayerProperty {
        name: String,
        value: String,
        signature: Option<String>,
    }

    #[derive(Clone, Debug)]
    pub enum C32PlayerInfoPlayerUpdate {
        AddPlayer {
            uuid: Uuid,
            name: String,
            properties: Vec<C32PlayerInfoAddPlayerProperty>,
            gamemode: VarInt,
            ping: VarInt,
            display_name: Option<String>,
        },
        UpdateGamemode {
            uuid: Uuid,
            gamemode: VarInt,
        },
        UpdateLatency {
            uuid: Uuid,
            ping: VarInt,
        },
        UpdateDisplayName {
            uuid: Uuid,
            display_name: Option<String>,
        },
        RemovePlayer {
            uuid: Uuid,
        },
    }

    /// Sent by the server to update the user list (<tab> in the client).
    ///
    /// https://wiki.vg/Protocol#Player_Info
    #[derive(Clone, Debug)]
    pub struct C32PlayerInfo {
        /// List of players, must all be of the same type
        pub players: Vec<C32PlayerInfoPlayerUpdate>,
    }
    impl ClientBoundPacket for C32PlayerInfo {
        fn packet_id() -> i32 { 0x32 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            let action = match self.players.first() {
                Some(C32PlayerInfoPlayerUpdate::AddPlayer { .. }) => 0,
                Some(C32PlayerInfoPlayerUpdate::UpdateGamemode { .. }) => 1,
                Some(C32PlayerInfoPlayerUpdate::UpdateLatency { .. }) => 2,
                Some(C32PlayerInfoPlayerUpdate::UpdateDisplayName { .. }) => 3,
                Some(C32PlayerInfoPlayerUpdate::RemovePlayer { .. }) => 4,
                _ => 0,
            };
            encoder.write_varint(action);
            encoder.write_varint(self.players.len() as VarInt);
            for player in self.players.iter() {
                match player {
                    C32PlayerInfoPlayerUpdate::AddPlayer {
                        name,
                        properties,
                        gamemode,
                        ping,
                        display_name,
                        uuid,
                    } => {
                        if action != 0 {
                            panic!("Invalid action (all player update s must be of the same types");
                        }
                        encoder.write_bytes(uuid.as_bytes());
                        encoder.write_string(name);
                        encoder.write_varint(properties.len() as VarInt);
                        /*for property in properties.iter() {
                        TODO: Add properties
                        }*/
                        encoder.write_varint(*gamemode);
                        encoder.write_varint(*ping);
                        encoder.write_bool(display_name.is_some());
                        if let Some(display_name) = display_name {
                            encoder.write_string(display_name);
                        }
                    }

                    C32PlayerInfoPlayerUpdate::UpdateGamemode { uuid, gamemode } => {
                        if action != 1 {
                            panic!("Invalid action (all player update s must be of the same types");
                        }
                        encoder.write_bytes(uuid.as_bytes());
                        encoder.write_varint(*gamemode);
                    }

                    C32PlayerInfoPlayerUpdate::UpdateLatency { uuid, ping } => {
                        if action != 2 {
                            panic!("Invalid action (all player update s must be of the same types");
                        }
                        encoder.write_bytes(uuid.as_bytes());
                        encoder.write_varint(*ping);
                    }

                    C32PlayerInfoPlayerUpdate::UpdateDisplayName { uuid, display_name } => {
                        if action != 3 {
                            panic!("Invalid action (all player update s must be of the same types");
                        }
                        encoder.write_bytes(uuid.as_bytes());
                        encoder.write_bool(display_name.is_some());
                        if let Some(display_name) = display_name {
                            encoder.write_string(display_name);
                        }
                    }

                    C32PlayerInfoPlayerUpdate::RemovePlayer { uuid } => {
                        if action != 4 {
                            panic!("Invalid action (all player update s must be of the same types");
                        }
                        encoder.write_bytes(uuid.as_bytes());
                    }
                }
            }
        }
    }

    /// Updates the player's position on the server.
    ///
    /// https://wiki.vg/Protocol#Player_Position_And_Look_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C34PlayerPositionAndLook {
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: f32,
        pub pitch: f32,
        pub flags: u8,
        pub teleport_id: i32,
    }
    impl ClientBoundPacket for C34PlayerPositionAndLook {
        fn packet_id() -> i32 { 0x34 }

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

    /// Sent by the server when a list of entities is to be destroyed on the client.
    ///
    /// https://wiki.vg/Protocol#Destroy_Entities
    #[derive(Clone, Debug)]
    pub struct C36DestroyEntities {
        pub entities: Vec<VarInt>,
    }
    impl ClientBoundPacket for C36DestroyEntities {
        fn packet_id() -> i32 { 0x36 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entities.len() as i32);
            for eid in self.entities.iter() {
                encoder.write_varint(*eid);
            }
        }
    }

    /// Changes the direction an entity's head is facing.
    /// While sending the Entity Look packet changes the vertical rotation of the head,
    /// sending this packet appears to be necessary to rotate the head horizontally.
    ///
    /// https://wiki.vg/Protocol#Held_Item_Change_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C3AEntityHeadLook {
        pub entity_id: VarInt,
        /// New angle, not a delta
        pub head_yaw: Angle,
    }
    impl ClientBoundPacket for C3AEntityHeadLook {
        fn packet_id() -> i32 { 0x3A }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_angle(self.head_yaw);
        }
    }

    /// Sent to change the player's slot selection.
    ///
    /// https://wiki.vg/Protocol#Held_Item_Change_.28clientbound.29
    #[derive(Clone, Debug)]
    pub struct C3FHoldItemChange {
        /// The slot which the player has selected (0–8)
        pub slot: i8,
    }
    impl ClientBoundPacket for C3FHoldItemChange {
        fn packet_id() -> i32 { 0x3F }

        fn encode(&self, encoder: &mut PacketEncoder) { encoder.write_i8(self.slot); }
    }

    /// Updates the client's location.
    /// This is used to determine what chunks should remain loaded and if a chunk load should be ignored;
    /// chunks outside of the view distance may be unloaded.
    ///
    /// https://wiki.vg/Protocol#Update_View_Position
    #[derive(Clone, Debug)]
    pub struct C40UpdateViewPosition {
        pub chunk_x: i32,
        pub chunk_z: i32,
    }
    impl ClientBoundPacket for C40UpdateViewPosition {
        fn packet_id() -> i32 { 0x40 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.chunk_z);
            encoder.write_varint(self.chunk_x);
        }
    }

    /// Sent by the server after login to specify the coordinates of the spawn point
    /// (the point at which players spawn at, and which the compass points to).
    /// It can be sent at any time to update the point compasses point at.
    ///
    /// https://wiki.vg/Protocol#Spawn_Position
    #[derive(Clone, Debug)]
    pub struct C42SpawnPosition {
        pub location: Position,
    }
    impl ClientBoundPacket for C42SpawnPosition {
        fn packet_id() -> i32 { 0x42 }

        fn encode(&self, encoder: &mut PacketEncoder) { encoder.write_u64(self.location.encode()); }
    }

    /// Updates one or more metadata properties for an existing entity.
    /// Any properties not included in the Metadata field are left unchanged.
    ///
    /// https://wiki.vg/Protocol#Entity_Metadata
    #[derive(Clone, Debug)]
    pub struct C44EntityMetadata {
        pub entity_id: i32,
        pub metadata: HashMap<u8, MetadataValue>,
    }
    impl ClientBoundPacket for C44EntityMetadata {
        fn packet_id() -> i32 { 0x44 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            for (key, value) in self.metadata.iter() {
                encoder.write_u8(*key);
                encoder.write_bytes(value.encode().as_slice());
            }
            encoder.write_u8(0xFF);
        }
    }

    /// This packet may be used by custom servers to display additional information above/below the player list.
    /// It is never sent by the Notchian server.
    ///
    /// https://wiki.vg/Protocol#Player_List_Header_And_Footer
    #[derive(Clone, Debug)]
    pub struct C53PlayerListHeaderAndFooter {
        /// To remove the header, send a empty text component: {"text":""}
        pub header: serde_json::Value,
        /// To remove the footer, send a empty text component: {"text":""}
        pub footer: serde_json::Value,
    }
    impl ClientBoundPacket for C53PlayerListHeaderAndFooter {
        fn packet_id() -> i32 { 0x53 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_string(&self.header.to_string());
            encoder.write_string(&self.footer.to_string());
        }
    }

    /// This packet is sent by the server when an entity moves more than 8 blocks.
    ///
    /// https://wiki.vg/Protocol#Entity_Teleport
    #[derive(Clone, Debug)]
    pub struct C56EntityTeleport {
        pub entity_id: VarInt,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: Angle,   // New angle, not a delta
        pub pitch: Angle, // New angle, not a delta
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C56EntityTeleport {
        fn packet_id() -> i32 { 0x56 }

        fn encode(&self, encoder: &mut PacketEncoder) {
            encoder.write_varint(self.entity_id);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }
}
pub use play::*;

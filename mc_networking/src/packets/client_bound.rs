use super::RawPacket;
use crate::data_types::encoder::PacketEncoder;

pub trait ClientBoundPacket {
    const PACKET_ID: i32;
    fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>);

    fn to_rawpacket(&self) -> RawPacket {
        let mut packet_encoder = PacketEncoder::default();
        self.encode(&mut packet_encoder);
        RawPacket::new(Self::PACKET_ID, packet_encoder.into_inner().freeze())
    }

    fn to_rawpacket_in<'a>(&self, bytes: &mut BytesMut) -> RawPacket {
        assert!(bytes.is_empty());
        let mut packet_encoder = PacketEncoder::new(bytes);
        self.encode(&mut packet_encoder);
        RawPacket::new(Self::PACKET_ID, packet_encoder.into_inner().split().freeze())
    }
}

mod status {
    use bytes::BufMut;

    use super::ClientBoundPacket;
    use crate::data_types::encoder::PacketEncoder;

    /// Response to S00StatusRequest with server ping infos
    ///
    /// <https://wiki.vg/Protocol#Status_Response>
    #[derive(Clone, Debug)]
    pub struct C00StatusResponse {
        /// See [Server List Ping](https://wiki.vg/Server_List_Ping#Response);
        pub json_response: serde_json::Value,
    }
    impl ClientBoundPacket for C00StatusResponse {
        const PACKET_ID: i32 = 0x00;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.json_response.to_string());
        }
    }

    /// Response to S01Ping with provided payload
    ///
    /// <https://wiki.vg/Protocol#Ping_Response>
    #[derive(Clone, Debug)]
    pub struct C01Pong { // Sticking with Pong even if wiki.vg is using Ping Response...
        /// Should be the same as sent by the client.
        pub payload: i64,
    }
    impl ClientBoundPacket for C01Pong {
        const PACKET_ID: i32 = 0x01;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i64(self.payload);
        }
    }
}
use bytes::{BufMut, BytesMut};
pub use status::*;

mod login {
    use super::ClientBoundPacket;
    use crate::data_types::{encoder::PacketEncoder, Identifier, VarInt};
    use bytes::BufMut;
    use uuid::Uuid;

    /// Disconnect the player with the specified message
    ///
    /// <https://wiki.vg/Protocol#Disconnect_.28login.29>
    #[derive(Clone, Debug)]
    pub struct C00LoginDisconnect {
        /// The reason why the player was disconnected.
        pub reason: serde_json::Value,
    }
    impl ClientBoundPacket for C00LoginDisconnect {
        const PACKET_ID: i32 = 0x00;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.reason.to_string());
        }
    }

    /// Request packet encryption
    ///
    /// <https://wiki.vg/Protocol#Encryption_Request>
    #[derive(Clone, Debug)]
    pub struct C01EncryptionRequest {
        pub server_id: String,
        pub public_key: Vec<u8>,
        pub verify_token: Vec<u8>,
    }
    impl ClientBoundPacket for C01EncryptionRequest {
        const PACKET_ID: i32 = 0x01;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.server_id);
            encoder.write_varint(self.public_key.len() as VarInt);
            encoder.write_bytes(self.public_key.as_slice());
            encoder.write_varint(self.verify_token.len() as VarInt);
            encoder.write_bytes(self.verify_token.as_slice());
        }
    }

    /// Finishes the Login stage
    ///
    /// <https://wiki.vg/Protocol#Login_Success>
    #[derive(Clone, Debug)]
    pub struct C02LoginSuccess {
        pub uuid: Uuid,
        pub username: String,
    }
    impl ClientBoundPacket for C02LoginSuccess {
        const PACKET_ID: i32 = 0x02;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_uuid(&self.uuid);
            encoder.write_string(&self.username);

            encoder.write_varint(0); // No properties
                                     // TODO: Know what they are for and implement them
        }
    }

    /// Enables compression. If compression is enabled,
    /// all following packets are encoded in the compressed packet format.
    /// Negative or zero values will disable compression,
    /// meaning the packet format should remain in the uncompressed packet format.
    /// However, this packet is entirely optional, and if not sent,
    /// compression will also not be enabled 
    /// (the notchian server does not send the packet when compression is disabled).
    ///
    /// <https://wiki.vg/Protocol#Set_Compression>
    #[derive(Clone, Debug)]
    pub struct C03SetCompression {
        pub threshold: VarInt,
    }
    impl ClientBoundPacket for C03SetCompression {
        const PACKET_ID: i32 = 0x03;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.threshold);
        }
    }

    /// Used to implement a custom handshaking flow together with S02LoginPluginResponse.
    /// Unlike plugin messages in "play" mode, these messages follow a lock-step request/response scheme,
    /// where the client is expected to respond to a request indicating whether it understood.
    /// The notchian client always responds that it hasn't understood, and sends an empty payload.
    ///
    /// <https://wiki.vg/Protocol#Login_Plugin_Request>
    #[derive(Clone, Debug)]
    pub struct C04LoginPluginRequest {
        pub message_id: VarInt,
        pub channel: Identifier,
        pub data: Vec<u8>,
    }
    impl ClientBoundPacket for C04LoginPluginRequest {
        const PACKET_ID: i32 = 0x04;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
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
        data_types::{
            command_data, encoder::PacketEncoder, Angle, Identifier, MetadataValue, Position, Slot,
            VarInt, bitset::BitSet
        },
        nbt_map::NBTMap,
        DecodingResult as Result,
    };

    use bytes::{BufMut, Bytes};
    use serde::Serialize;
    use std::{collections::HashMap, sync::Arc};
    use uuid::Uuid;

    /// Sent by the server when a vehicle or other non-living entity is created.
    ///
    /// <https://wiki.vg/Protocol#Spawn_Entity>
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
        pub head_yaw: Angle,
        pub data: i32,
        pub velocity_x: i16,
        pub velocity_y: i16,
        pub velocity_z: i16,
    }
    impl ClientBoundPacket for C00SpawnEntity {
        const PACKET_ID: i32 = 0x00;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_uuid(&self.object_uuid);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_angle(self.pitch);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.head_yaw);
            encoder.write_i32(self.data);
            encoder.write_i16(self.velocity_x);
            encoder.write_i16(self.velocity_y);
            encoder.write_i16(self.velocity_z);
        }
    }

    /// Spawns one or more experience orbs.
    ///
    /// <https://wiki.vg/Protocol#Spawn_Experience_Orb>
    #[derive(Clone, Debug)]
    pub struct C01SpawnExperienceOrb {
        pub entity_id: VarInt,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub count: i16,
    }
    impl ClientBoundPacket for C01SpawnExperienceOrb {
        const PACKET_ID: i32 = 0x01;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_i16(self.count);
        }
    }

    /// This packet is sent by the server when a player comes into visible range.
    ///
    /// <https://wiki.vg/Protocol#Spawn_Player>
    #[derive(Clone, Debug)]
    pub struct C02SpawnPlayer {
        pub entity_id: VarInt,
        pub uuid: Uuid,
        pub x: f64,
        pub y: f64,
        pub z: f64,
        pub yaw: Angle,
        pub pitch: Angle,
    }
    impl ClientBoundPacket for C02SpawnPlayer {
        const PACKET_ID: i32 = 0x02;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
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
    /// <https://wiki.vg/Protocol#Entity_Animation_.28clientbound.29>
    #[derive(Clone, Debug)]
    pub struct C03EntityAnimation {
        pub entity_id: VarInt,
        pub animation: u8,
    }
    impl ClientBoundPacket for C03EntityAnimation {
        const PACKET_ID: i32 = 0x03;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_u8(self.animation);
        }
    }

    /// This is used for properly syncing block changes to the client after interactions.
    ///
    /// <https://wiki.vg/Protocol#Acknowledge_Block_Change>
    #[derive(Clone, Debug)]
    pub struct C05AcknowledgeBlockChange {
        /// Represents the sequence to acknowledge,
        /// this is used for properly syncing block changes to the client after interactions.
        pub seq_id: VarInt,
    }
    impl ClientBoundPacket for C05AcknowledgeBlockChange {
        const PACKET_ID: i32 = 0x05;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.seq_id);
        }
    }

    /// <https://wiki.vg/Protocol#Set_Block_Destroy_Stage>
    #[derive(Clone, Debug)]
    pub struct C06SetBlockDestroyStage {
        /// The ID of the entity breaking the block.
        pub entity_id: VarInt,
        /// Block Position.
        pub position: Position,
        /// 0–9 to set it, any other value to remove it
        pub destroy_stage: i8,
    }
    impl ClientBoundPacket for C06SetBlockDestroyStage {
        const PACKET_ID: i32 = 0x07;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_u64(self.position.encode());
            encoder.write_i8(self.destroy_stage);
        }
    }

    /// Fired whenever a block is changed within the render distance.
    ///
    /// <https://wiki.vg/Protocol#Block_Change>
    #[derive(Clone, Debug)]
    pub struct C09BlockChange {
        pub position: Position,
        pub block_id: VarInt,
    }
    impl ClientBoundPacket for C09BlockChange {
        const PACKET_ID: i32 = 0x09;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u64(self.position.encode());
            encoder.write_varint(self.block_id);
        }
    }

    /// Sets the message to preview on the client.
    ///
    /// <https://wiki.vg/Protocol#Chat_Preview_.28clientbound.29>
    #[derive(Clone, Debug)]
    pub struct COCChatPreview {
        query_id: i32,
        component_is_present: bool,
        message_to_preview: serde_json::Value,
    }
    impl ClientBoundPacket for COCChatPreview {
        const PACKET_ID: i32 = 0x0C;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i32(self.query_id);
            encoder.write_bool(self.component_is_present);
            encoder.write_string(&self.message_to_preview.to_string());
        }
    }

    /// Lists all of the commands on the server, and how they are parsed.
    /// This is a directed graph, with one root node.
    /// Each redirect or child node must refer only to nodes that have already been declared.
    ///
    /// <https://wiki.vg/Protocol#Commands>
    #[derive(Clone)]
    pub struct C0FCommands {
        pub root_node: Arc<command_data::RootNode>,
    }
    impl ClientBoundPacket for C0FCommands {
        const PACKET_ID: i32 = 0x0F;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            let mut graph_encoder = command_data::GraphEncoder::new();
            let root_node = self.root_node.clone() as Arc<dyn command_data::Node>;
            let root_index = graph_encoder.add_node(&root_node);
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
    /// This packet with Window ID set to "0" is sent during
    /// the player joining sequence to initialise the player's inventory.
    ///
    /// <https://wiki.vg/Protocol#Set_Container_Content>
    #[derive(Clone, Debug)]
    pub struct C11SetContainerContent {
        /// The ID of window which items are being sent for. 0 for player inventory.
        pub window_id: u8,
        /// See <https://wiki.vg/Protocol#Click_Container>
        pub state_id: VarInt,
        /// The container's content
        pub slots: Vec<Slot>,
        /// Item held by player.
        pub carried_item: Slot,
    }
    impl ClientBoundPacket for C11SetContainerContent {
        const PACKET_ID: i32 = 0x11;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u8(self.window_id);
            encoder.write_varint(self.state_id);

            encoder.write_varint(self.slots.len() as _);
            for slot in &self.slots {
                encoder.write_bytes(&slot.encode());
            }

            encoder.write_bytes(&self.carried_item.encode());
        }
    }

    /// Sent by the server when an item in a slot (in a window) is added/removed.
    ///
    /// <https://wiki.vg/Protocol#Set_Container_Slot>
    #[derive(Clone, Debug)]
    pub struct C13SetContainerSlot {
        /// The window which is being updated. 0 for player inventory.
        /// Note that all known window types include the player inventory.
        /// This packet will only be sent for the currently opened window while the player is performing actions,
        /// even if it affects the player inventory.
        /// After the window is closed, a number of these packets
        /// are sent to update the player's inventory window (0).
        pub window_id: u8,
        /// See <https://wiki.vg/Protocol#Click_Container>
        pub state_id: VarInt,
        /// The slot that should be updated
        pub slot: i16,
        pub slot_data: Slot,
    }
    impl ClientBoundPacket for C13SetContainerSlot {
        const PACKET_ID: i32 = 0x13;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u8(self.window_id);
            encoder.write_varint(self.state_id);
            encoder.write_i16(self.slot);
            encoder.write_bytes(&self.slot_data.encode());
        }
    }

    pub struct C15PluginMessageBuilder {
        pub channel: Identifier,
        pub encoder: PacketEncoder,
    }
    impl C15PluginMessageBuilder {
        pub fn new(channel: Identifier) -> Self {
            Self {
                channel,
                encoder: PacketEncoder::default(),
            }
        }

        pub fn build(self) -> C15PluginMessage {
            C15PluginMessage {
                channel: self.channel,
                data: self.encoder.into_inner().freeze(),
            }
        }
    }

    /// Mods and plugins can use this to send their data.
    /// Minecraft itself uses several plugin channels.
    /// These internal channels are in the minecraft namespace.
    /// More documentation on this: https://dinnerbone.com/blog/2012/01/13/minecraft-plugin-channels-messaging/
    ///
    /// <https://wiki.vg/Protocol#Plugin_Message_.28clientbound.29>
    #[derive(Clone, Debug)]
    pub struct C15PluginMessage {
        pub channel: Identifier,
        pub data: Bytes,
    }
    impl ClientBoundPacket for C15PluginMessage {
        const PACKET_ID: i32 = 0x15;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.channel);
            encoder.write_bytes(&self.data);
        }
    }

    /// Sent by the server before it disconnects a client.
    /// The client assumes that the server has already closed the connection by the time the packet arrives.
    #[derive(Clone, Debug)]
    pub struct C17Disconnect {
        /// Displayed to the client when the connection terminates.
        pub reason: serde_json::Value,
    }
    impl ClientBoundPacket for C17Disconnect {
        const PACKET_ID: i32 = 0x17;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.reason.to_string());
        }
    }

    /// Tells the client to unload a chunk column.
    /// It is legal to send this packet even if the given chunk is not currently loaded.
    ///
    /// <https://wiki.vg/Protocol#Unload_Chunk>
    #[derive(Clone, Debug)]
    pub struct C1AUnloadChunk {
        pub chunk_x: i32,
        pub chunk_z: i32,
    }
    impl ClientBoundPacket for C1AUnloadChunk {
        const PACKET_ID: i32 = 0x1A;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i32(self.chunk_x);
            encoder.write_i32(self.chunk_z);
        }
    }

    /// Used for a wide variety of game state things, from whether to bed use to gamemode to demo messages.
    ///
    /// <https://wiki.vg/Protocol#Game_Event>
    #[derive(Clone, Debug)]
    pub struct C1BGameEvent {
        pub event: u8,
        pub value: f32,
    }
    impl ClientBoundPacket for C1BGameEvent {
        const PACKET_ID: i32 = 0x1B;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u8(self.event);
            encoder.write_f32(self.value);
        }
    }

    /// The server will frequently send out a keep-alive, each containing a random ID.
    /// The client must respond with the same packet.
    /// If the client does not respond to them for over 30 seconds, the server kicks the client.
    /// Vice versa, if the server does not send any keep-alives for 20 seconds,
    /// the client will disconnect and yields a "Timed out" exception.
    ///
    /// <https://wiki.vg/Protocol#Keep_Alive_.28clientbound.29>
    #[derive(Clone, Debug)]
    pub struct C1EKeepAlive {
        pub id: i64,
    }
    impl ClientBoundPacket for C1EKeepAlive {
        const PACKET_ID: i32 = 0x1E;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i64(self.id);
        }
    }

    /// A Paletted Container is a palette-based storage of entries.
    /// Paletted Containers have an associated global palette (either block states or biomes as of now),
    /// where values are mapped from.
    ///
    /// <https://wiki.vg/Chunk_Format#Paletted_Container_structure>
    #[derive(Clone, Debug)]
    pub enum C1FPalettedContainer {
        Single(VarInt),
        Indirect {
            /// Must be within [4; 8] for blocks
            /// And in [0; 3] for biomes
            bits_per_entry: u8,
            palette: Vec<VarInt>,
            data_array: Vec<i64>,
        },
        Direct {
            /// Must be >= 9 for blocks
            /// And >= 4 for biomes
            bits_per_entry: u8,
            data_array: Vec<i64>,
        },
    }
    impl C1FPalettedContainer {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            match self {
                Self::Single(v) => {
                    encoder.write_u8(0);
                    encoder.write_varint(*v);
                    encoder.write_varint(0);
                }
                Self::Indirect {
                    bits_per_entry, palette, data_array
                } => {
                    encoder.write_u8(*bits_per_entry);
                    encoder.write_varint(palette.len() as _);
                    encoder.write_varint(data_array.len() as _);
                    for x in data_array {
                        encoder.write_i64(*x);
                    }
                }
                Self::Direct {
                    bits_per_entry, data_array
                } => {
                    encoder.write_u8(*bits_per_entry);
                    encoder.write_varint(data_array.len() as _);
                    for x in data_array {
                        encoder.write_i64(*x);
                    }
                }
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct C1FSection {
        pub block_count: i16,
        pub block_states: C1FPalettedContainer,
        pub biomes: C1FPalettedContainer,
    }
    impl C1FSection {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i16(self.block_count);
            self.block_states.encode(encoder);
            self.biomes.encode(encoder);
        }
    }

    /// TODO: Add block entities
    /// The server only sends skylight information for chunk pillars in the Overworld,
    /// it's up to the client to know in which dimension the player is currently located.
    /// You can also infer this information from the primary bitmask and the amount of uncompressed bytes sent.
    /// This packet also sends all block entities in the chunk (though sending them is not required;
    /// it is still legal to send them with Block Entity Data later).
    ///
    /// <https://wiki.vg/Protocol#Chunk_Data_and_Update_Light>
    #[derive(Clone, Debug)]
    pub struct C1FChunkDataAndUpdateLight {
        /// Chunk coordinate (block coordinate divided by 16, rounded down)
        pub chunk_x: i32,
        /// Chunk coordinate (block coordinate divided by 16, rounded down)
        pub chunk_z: i32,
        /// Compound containing one long array named MOTION_BLOCKING,
        /// which is a heightmap for the highest solid block at each position in the chunk 
        /// (as a compacted long array with 256 entries, with the number of bits per
        /// entry varying depending on the world's height, defined by the formula ceil(log2(height + 1))).
        /// The Notchian server also adds a WORLD_SURFACE long array,
        /// the purpose of which is unknown, but it's not required for the chunk to be accepted.
        pub heightmaps: nbt::Blob,
        /// The number of elements in the array is calculated based on the world's height.
        /// Sections are sent bottom-to-top.
        pub chunk_sections: Vec<C1FSection>,
        /// TODO: Implement
        pub block_entities: Vec<()>,
        /// If edges should be trusted for light updates.
        pub trust_edges: bool,
        /// BitSet containing bits for each section in the world + 2.
        /// Each set bit indicates that the corresponding 16×16×16 chunk section has data in the Sky Light array below.
        /// The least significant bit is for blocks 16 blocks to 1 block below the min world height 
        /// (one section below the world), while the most significant bit covers blocks 1 to
        /// 16 blocks above the max world height (one section above the world).
        pub sky_light_mask: BitSet,
        /// BitSet containing bits for each section in the world + 2.
        /// Each set bit indicates that the corresponding 16×16×16 chunk
        /// section has data in the Block Light array below.
        /// The order of bits is the same as in Sky Light Mask.
        pub block_light_mask: BitSet,
        /// BitSet containing bits for each section in the world + 2.
        /// Each set bit indicates that the corresponding 16×16×16 chunk section
        /// has all zeros for its Sky Light data. The order of bits is the same as in Sky Light Mask.
        pub empty_sky_light_mask: BitSet,
        /// BitSet containing bits for each section in the world + 2.
        /// Each set bit indicates that the corresponding 16×16×16 chunk
        /// section has all zeros for its Block Light data.
        /// The order of bits is the same as in Sky Light Mask.
        pub empty_block_light_mask: BitSet,
        /// There is 1 array for each bit set to true in the sky light mask,
        /// starting with the lowest value. Half a byte per light value.
        /// Indexed ((y<<8) | (z<<4) | x) / 2 If there's a remainder, masked 0xF0 else 0x0F.
        pub sky_light_array: Vec<Box<[u8; 2048]>>,
        /// There is 1 array for each bit set to true in the block light mask,
        /// starting with the lowest value. Half a byte per light value.
        /// Indexed ((y<<8) | (z<<4) | x) / 2 If there's a remainder, masked 0xF0 else 0x0F.
        pub block_light_array: Vec<Box<[u8; 2048]>>,
    }
    impl ClientBoundPacket for C1FChunkDataAndUpdateLight {
        const PACKET_ID: i32 = 0x1F;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i32(self.chunk_x);
            encoder.write_i32(self.chunk_z);
            nbt::ser::to_writer(encoder, &self.heightmaps, None).expect("No error from packet encoder");
            let chunk_data = {
                let mut encoder = PacketEncoder::default();
                for section in &self.chunk_sections
                { section.encode(&mut encoder); }
                encoder.into_inner().freeze()
            };
            encoder.write_varint(chunk_data.len() as _);
            encoder.write_bytes(&chunk_data);
            encoder.write_varint(0); // TODO: Real block entities
            encoder.write_bool(self.trust_edges);
            // Note: For loop is just to avoid repeating the same code 4 times
            for bitset in [
                &self.sky_light_mask, &self.block_light_mask,
                &self.empty_sky_light_mask, &self.empty_block_light_mask
            ] {
                encoder.write_varint(bitset.longs.len() as _);
                for x in &bitset.longs {
                    encoder.write_u64(*x);
                }
            }

            encoder.write_varint(self.sky_light_array.len() as _);
            for b in &self.sky_light_array {
                encoder.write_bytes(b.as_ref());
            }

            encoder.write_varint(self.block_light_array.len() as _);
            for b in &self.block_light_array {
                encoder.write_bytes(b.as_ref());
            }
        }
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23DimensionElement {
        /// Whether piglins shake and transform to zombified piglins.
        ///
        /// 1: true, 0: false.
        pub piglin_safe: i8,
        /// Whether players with the Bad Omen effect can cause a raid.
        ///
        /// 1: true, 0: false.
        pub has_raids: i8,
        pub monster_spawn_light_level: i32,
        pub monster_spawn_block_light_limit: i32,
        /// When false, compasses spin randomly. When true, nether portals can spawn zombified piglins.	
        ///
        /// 1: true, 0: false.
        pub natural: i8,
        /// How much light the dimension has.	
        ///
        /// 0.0 to 1.0.
        pub ambient_light: f32,
        /// If set, the time of the day is the specified value.	
        ///
        /// If set, 0 to 24000.
        pub fixed_time: Option<i64>,
        /// A resource location defining what block tag to use for infiniburn.	
        ///
        /// "#" or minecraft resource "#minecraft:...".
        pub infiniburn: String,
        /// Whether players can charge and use respawn anchors.	
        ///
        /// 1: true, 0: false.
        pub respawn_anchor_works: i8,
        /// Whether the dimension has skylight access or not.	
        ///
        /// 1: true, 0: false.
        pub has_skylight: i8,
        /// Whether players can use a bed to sleep.	
        ///
        /// 1: true, 0: false.
        pub bed_works: i8,
        /// ?
        ///
        /// "minecraft:overworld", "minecraft:the_nether", "minecraft:the_end" or something else.
        pub effects: String,
        /// The minimum Y level.	
        ///
        /// A multiple of 16. Example: -64
        pub min_y: i32,
        /// The maximum height.	
        ///
        /// A multiple of 16. Example: 256
        pub height: i32,
        /// The maximum height to which chorus fruits and nether portals can bring players within this dimension.	
        ///
        /// 0-384.
        pub logical_height: i32,
        /// The multiplier applied to coordinates when traveling to the dimension.	
        ///
        /// 0.00001 - 30000000.0
        pub coordinate_scale: f64,
        /// Whether the dimensions behaves like the nether (water evaporates and sponges dry) or not.
        /// Also causes lava to spread thinner.	
        ///
        /// 1: true, 0: false.
        pub ultrawarm: i8,
        /// Whether the dimension has a bedrock ceiling or not. When true, causes lava to spread faster.
        ///
        /// 1: true, 0: false.
        pub shrunk: i8,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeElement {
        /// The type of precipitation in the biome.	
        ///
        /// "rain", "snow", or "none".
        pub precipitation: String,
        /// The depth factor of the biome.	
        ///
        /// The default values vary between 1.5 and -1.8.
        pub depth: f32,
        /// The temperature factor of the biome.	
        ///
        /// The default values vary between 2.0 and -0.5.
        pub temperature: f32,
        /// ?
        ///
        /// The default values vary between 1.225 and 0.0.
        pub scale: f32,
        /// ?
        ///
        /// The default values vary between 1.0 and 0.0.
        pub downfall: f32,
        /// The category of the biome.	
        ///
        /// Known values are "ocean", "plains", "desert", "forest", "extreme_hills",
        /// "taiga", "swamp", "river", "nether", "the_end", "icy", "mushroom", "beach", "jungle",
        /// "mesa", "savanna", and "none".
        pub category: String,
        /// The only known value is "frozen".
        pub temperature_modifier: Option<String>,
        /// Various biome effects
        pub effects: C23BiomeEffects,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeEffects {
        /// The color of the sky.	
        ///
        /// Example: 8364543, which is #7FA1FF in RGB.
        pub sky_color: i32,
        /// Possibly the tint color when swimming.	
        ///
        /// Example: 8364543, which is #7FA1FF in RGB.
        pub water_fog_color: i32,
        /// Possibly the color of the fog effect when looking past the view distance.	
        ///
        /// Example: 8364543, which is #7FA1FF in RGB.
        pub fog_color: i32,
        /// The tint color of the water blocks.	
        ///
        /// Example: 8364543, which is #7FA1FF in RGB.
        pub water_color: i32,
        /// The tint color of the grass.	
        ///
        /// Example: 8364543, which is #7FA1FF in RGB.
        pub folliage_color: Option<i32>,
        /// ?
        ///
        /// Example: 8364543, which is #7FA1FF in RGB.
        pub grass_color: Option<i32>,
        /// Unknown, likely affects foliage color.	
        ///
        /// If set, known values are "swamp" and "dark_forest".
        pub grass_color_modifier: Option<String>,
        /// Music properties for the biome.	
        pub music: Option<C23BiomeMusic>,
        /// Ambient soundtrack.	
        pub ambient_sound: Option<String>,
        /// Additional ambient sound that plays randomly.	
        pub additions_sound: Option<C23BiomeAdditionsSound>,
        /// Additional ambient sound that plays at an interval.	
        pub mood_sound: Option<C23BiomeMoodSound>,
        /// Particles that appear randomly in the biome.
        pub particle: Option<C23BiomeParticle>
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeMusic {
        pub replace_current_music: i8,
        pub sound: String,
        pub max_delay: i32,
        pub min_delay: i32,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeAdditionsSound {
        pub sound: String,
        pub tick_chance: f64,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeMoodSound {
        pub sound: String,
        pub tick_delay: i32,
        pub offset: f32,
        pub block_search_extent: i32,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeParticle {
        /// Possibly the probability of spawning the particle.
        pub probability: f32,
        /// The properties of the particle to spawn.	
        pub options: C23BiomeParticleOptions,
    }

    #[derive(Clone, Debug, Serialize)]
    pub struct C23BiomeParticleOptions {
        /// Particle's type
        #[serde(rename = "type")]
        pub kind: String,
    }

    #[derive(Clone, Debug)]
    pub struct C23RegistryCodec {
        /// The dimension type registry
        pub dimension_types: Vec<(Identifier, C23DimensionElement)>,
        /// The biome registry
        pub biomes: Vec<(Identifier, C23BiomeElement)>,
        /// The chat type registry
        /// TODO: Implement (currenly lacks documentation)
        pub chat_types: (),
    }

    #[derive(Clone, Debug, Serialize)]
    struct C24RegistryCodecInner {
        /*
        #[serde(rename = "minecraft:dimension_type")]
        pub dimensions: NBTMap<C23DimensionElement>,
        #[serde(rename = "minecraft:worldgen/biome")]
        pub biomes: NBTMap<C23BiomeElement>,
        #[serde(rename = "minecraft:chat_type")]
        pub chat_types: NBTMap<C23BiomeElement>,
        */
    }
    impl C23RegistryCodec {
        fn encode<T: std::io::Write>(&self, buf: &mut T) -> Result<()> {
            /*
            let mut dimension_map = NBTMap::new("minecraft:dimension_type".into());
            for (name, element) in self.dimension_types.iter() {
                dimension_map.push_element(name.to_string(), element.clone());
            }
            let mut biome_map = NBTMap::new("minecraft:worldgen/biome".into());
            for (name, element) in self.biomes.iter() {
                biome_map.push_element(name.to_string(), element.clone());
            }
            let /*mut*/ chat_map = NBTMap::new("minecraft:chat_type".into());
            /*for (name, element) in self.chat_types.iter() {
                chat_map.push_element(name.to_string(), element.clone());
            }*/
            let codec = C24RegistryCodecInner {
                dimensions: dimension_map,
                biomes: biome_map,
                chat_types: chat_map,
            };
            */
            let codec = C24RegistryCodecInner {};
            nbt::ser::to_writer(buf, &codec, None)?;
            Ok(())
        }
    }

    /// Send information about the game
    ///
    /// <https://wiki.vg/Protocol#Login_.28play.29>
    #[derive(Clone, Debug)]
    pub struct C23Login {
        /// The player's Entity ID (EID).
        pub entity_id: i32,
        /// Probably changes the player's hearts
        pub is_hardcore: bool,
        /// 0: Survival, 1: Creative,
        /// 2: Adventure, 3: Spectator.
        pub gamemode: i8,
        /// 0: survival, 1: creative, 2: adventure, 3: spectator.
        /// The hardcore flag is not included. The previous gamemode.
        /// Defaults to -1 if there is no previous gamemode. (More information needed)
        pub previous_gamemode: i8,
        /// Identifiers for all dimensions on the server.
        pub dimension_names: Vec<Identifier>,
        /// Represents certain registries that are sent from the server and are applied on the client.
        pub registry_codec: C23RegistryCodec,
        /// Name of the dimension type being spawned into.
        pub dimension_type: Identifier,
        /// Name of the dimension being spawned into.
        pub dimension_name: Identifier,
        /// First 8 bytes of the SHA-256 hash of the world's seed. Used client side for biome noise
        pub hashed_seed: u64,
        /// Was once used by the client to draw the player list, but now is ignored.
        pub max_players: VarInt,
        /// Render distance (2-32).
        pub view_distance: VarInt,
        /// The distance that the client will process specific things, such as entities.
        pub simulation_distance: VarInt,
        /// If true, a Notchian client shows reduced information on the debug screen.
        /// For servers in development, this should almost always be false.
        pub reduced_debug_info: bool,
        /// Set to false when the doImmediateRespawn gamerule is true.
        pub enable_respawn_screen: bool,
        /// True if the world is a debug mode world;
        /// debug mode worlds cannot be modified and have predefined blocks.
        pub is_debug: bool,
        /// True if the world is a superflat world;
        /// flat worlds have different void fog and a horizon at y=0 instead of y=63.
        pub is_flat: bool,
        /// Name of the dimension and location the player died in.
        pub death_location: Option<(Identifier, Position)>,

    }
    impl ClientBoundPacket for C23Login {
        const PACKET_ID: i32 = 0x23;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i32(self.entity_id);
            encoder.write_bool(self.is_hardcore);
            encoder.write_i8(self.gamemode);
            encoder.write_i8(self.previous_gamemode);
            encoder.write_varint(self.dimension_names.len() as _);
            for name in &self.dimension_names
            { encoder.write_string(&name); }
            self.registry_codec.encode(encoder).expect("Unexpected encode error");
            encoder.write_string(&self.dimension_type);
            encoder.write_string(&self.dimension_name);
            encoder.write_u64(self.hashed_seed);
            encoder.write_varint(self.max_players);
            encoder.write_varint(self.view_distance);
            encoder.write_varint(self.simulation_distance);
            encoder.write_bool(self.reduced_debug_info);
            encoder.write_bool(self.enable_respawn_screen);
            encoder.write_bool(self.is_debug);
            encoder.write_bool(self.is_flat);

            encoder.write_bool(self.death_location.is_some());
            if let Some((dimension, location)) = &self.death_location {
                encoder.write_string(&dimension);
                encoder.write_u64(location.encode());
            }
        }
    }

    /// This packet is sent by the server when an entity moves less then 8 blocks;
    /// if an entity moves more than 8 blocks C57EntityTeleport should be sent instead.
    ///
    /// <https://wiki.vg/Protocol#Update_Entity_Position>
    #[derive(Clone, Debug)]
    pub struct C26UpdateEntityPosition {
        pub entity_id: VarInt,
        /// Change in X position as `(currentX * 32 - prevX * 32) * 128`
        pub delta_x: i16,
        /// Change in Y position as `(currentY * 32 - prevY * 32) * 128`
        pub delta_y: i16,
        /// Change in Z position as `(currentZ * 32 - prevZ * 32) * 128`
        pub delta_z: i16,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C26UpdateEntityPosition {
        const PACKET_ID: i32 = 0x26;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
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
    /// <https://wiki.vg/Protocol#Update_Entity_Position_and_Rotation>
    #[derive(Clone, Debug)]
    pub struct C27UpdateEntityPositionAndRotation {
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
    impl ClientBoundPacket for C27UpdateEntityPositionAndRotation {
        const PACKET_ID: i32 = 0x27;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
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
    /// <https://wiki.vg/Protocol#Entity_Rotation>
    #[derive(Clone, Debug)]
    pub struct C28UpdateEntityRotation {
        pub entity_id: VarInt,
        /// New angle, not a delta
        pub yaw: Angle,
        /// New angle, not a delta
        pub pitch: Angle,
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C28UpdateEntityRotation {
        const PACKET_ID: i32 = 0x28;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_angle(self.yaw);
            encoder.write_angle(self.pitch);
            encoder.write_bool(self.on_ground);
        }
    }

    /// Use the new method for easier use
    ///
    /// <https://wiki.vg/Protocol#Player_Abilities_.28clientbound.29>
    #[derive(Clone, Debug)]
    pub struct C2FPlayerAbilities {
        /// Bit field (use the new method for ease of use)
        pub flags: u8,
        /// 0.05 by default.
        pub flying_speed: f32,
        /// Modifies the field of view, like a speed potion.
        /// A Notchian server will use the same value as the movement
        /// speed sent in the Update Attributes packet,
        /// which defaults to 0.1 for players.
        pub fov_modifier: f32,
    }
    impl C2FPlayerAbilities {
        pub fn new(
            invulnerable: bool,
            flying: bool,
            allow_flying: bool,
            creative_mode: bool,
            flying_speed: f32,
            fov_modifier: f32,
        ) -> Self {
            C2FPlayerAbilities {
                flags: ((invulnerable as u8) * 0x01)
                    | ((flying as u8) * 0x02)
                    | ((allow_flying as u8) * 0x04)
                    | ((creative_mode as u8) * 0x08),
                flying_speed,
                fov_modifier,
            }
        }
    }
    impl ClientBoundPacket for C2FPlayerAbilities {
        const PACKET_ID: i32 = 0x2F;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u8(self.flags);
            encoder.write_f32(self.flying_speed);
            encoder.write_f32(self.fov_modifier);
        }
    }


    /// Identifying the difference between Chat/System Message is important as it helps respect the user's chat visibility options.
    ///
    /// <https://wiki.vg/Protocol#Player_Chat_Message>
    #[derive(Clone, Debug)]
    pub struct C30PlayerChatMessage {
        /// Player message, this could either be the player typed message or the 
        /// server previewed message if it was signed by the client.
        /// Not modifiable by the server without invalidating the sigature.
        pub signed_chat_content: serde_json::Value,
        /// Server modifiable message.
        /// The client will prefer to choose this UNLESS show only secure chat 
        /// is enabled on the client or has unsigned chat content is false. 
        /// If these conditions are met, it will instead use the signed chat content
        pub unsigned_chat_content: Option<serde_json::Value>,
        /// Registered on the server, sent to the client.
        /// Default: 0: chat (chat box),
        /// 1: system message (chat box),
        /// 2: game info (above hotbar),
        /// 3: say command,
        /// 4: msg command,
        /// 5: team msg command,
        /// 6: emote command,
        /// 7: tellraw command
        pub kind: VarInt,
        /// Used by the Notchian client for the disableChat launch option.
        /// Setting to None will always display the message regardless of the setting.
        pub sender_uuid: Option<Uuid>,
        /// This can be modified without disrupting the message signature.
        pub sender_display_name: serde_json::Value,
        /// Used in the team_name decorator parameter.
        pub sender_team_name: Option<serde_json::Value>,
        /// Represents the time the message was signed,
        /// used to check if the message was received within 2 minutes of it being sent.
        pub timestamp: i64,
        /// Cryptography, used for validating the message signature.
        pub salt: i64,
        /// Cryptography, the signature consists of the Sender UUID, Timestamp and Original Chat Content.
        /// Modifying any of these values in the packet will cause this signature to fail.
        pub message_signature: Bytes,
    }
    impl ClientBoundPacket for C30PlayerChatMessage {
        const PACKET_ID: i32 = 0x30;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.signed_chat_content.to_string());

            encoder.write_bool(self.unsigned_chat_content.is_some());
            if let Some(c) = &self.unsigned_chat_content {
                encoder.write_string(&c.to_string())
            }
            
            encoder.write_varint(self.kind);
            encoder.write_uuid(&self.sender_uuid
                .unwrap_or(Uuid::from_u128(0)));

            encoder.write_string(&self.sender_display_name.to_string());

            encoder.write_bool(self.sender_team_name.is_some());
            if let Some(c) = &self.sender_team_name {
                encoder.write_string(&c.to_string())
            }
            
            encoder.write_i64(self.timestamp);
            encoder.write_i64(self.salt);
            encoder.write_varint(self.message_signature.len() as _);
            encoder.write_bytes(&self.message_signature);
        }
    }


    #[derive(Clone, Debug)]
    pub struct C32PlayerInfoAddPlayerProperty {
        pub name: String,
        pub value: String,
        pub signature: Option<String>,
    }
    #[derive(Clone, Debug)]
    pub struct C34AddPlayer {
        pub uuid: Uuid,
        pub name: String,
        pub properties: Vec<C32PlayerInfoAddPlayerProperty>,
        pub gamemode: VarInt,
        /// Measured in milliseconds.
        pub ping: VarInt,
        pub display_name: Option<String>,
        /// TODO: Implement
        pub sig_data: (),
    }
    impl C34AddPlayer {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_uuid(&self.uuid);
            encoder.write_string(&self.name);
            for prop in &self.properties {
                encoder.write_string(&prop.name);
                encoder.write_string(&prop.value);
                encoder.write_bool(prop.signature.is_some());
                if let Some(sig) = &prop.signature {
                    encoder.write_string(sig);
                }
            }
            encoder.write_varint(self.gamemode);
            encoder.write_varint(self.ping);
            encoder.write_bool(self.display_name.is_some());
            if let Some(dm) = &self.display_name {
                encoder.write_string(dm);
            }
            encoder.write_bool(false); // No sig data
        }
    }
    #[derive(Clone, Debug)]
    pub struct C34UpdateGamemode {
        pub uuid: Uuid,
        pub gamemode: VarInt,
    }
    impl C34UpdateGamemode {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_uuid(&self.uuid);
            encoder.write_varint(self.gamemode);
        }
    }
    #[derive(Clone, Debug)]
    pub struct C34UpdateLatency {
        pub uuid: Uuid,
        /// Measured in milliseconds.
        pub ping: VarInt,
    }
    impl C34UpdateLatency {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_uuid(&self.uuid);
            encoder.write_varint(self.ping);
        }
    }
    #[derive(Clone, Debug)]
    pub struct C34UpdateDisplayName {
        pub uuid: Uuid,
        pub display_name: Option<String>,
    }
    impl C34UpdateDisplayName {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_uuid(&self.uuid);
            encoder.write_bool(self.display_name.is_some());
            if let Some(dm) = &self.display_name {
                encoder.write_string(dm);
            }
        }
    }
    #[derive(Clone, Debug)]
    pub struct C34RemovePlayer {
        pub uuid: Uuid,
    }
    impl C34RemovePlayer {
        pub fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_uuid(&self.uuid);
        }
    }

    /// Sent by the server to update the user list (<tab> in the client).
    ///
    /// <https://wiki.vg/Protocol#Player_Info>
    #[derive(Clone, Debug)]
    pub enum C34PlayerInfo {
        AddPlayers {
            players: Vec<C34AddPlayer>,
        },
        UpdateGamemodes {
            players: Vec<C34UpdateGamemode>,
        },
        UpdateLatencies {
            players: Vec<C34UpdateLatency>,
        },
        UpdateDisplayNames {
            players: Vec<C34UpdateDisplayName>,
        },
        RemovePlayers {
            players: Vec<C34RemovePlayer>,
        }
    }
    impl ClientBoundPacket for C34PlayerInfo {
        const PACKET_ID: i32 = 0x34;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            // A macro because each action has a different type, so to avoid repeating the same
            // 7 lines 5 times, let's use this
            macro_rules! create_encoder {
                ($($a: path => $action: expr),*) => {
                    match self {
                        $(
                        $a { players } => {
                            encoder.write_varint($action);
                            encoder.write_varint(players.len() as _);
                            for player in players {
                                player.encode(encoder);
                            }
                        }
                        ),*
                    }
                }
            }

            create_encoder!(
                Self::AddPlayers => 0,
                Self::UpdateGamemodes => 1,
                Self::UpdateLatencies => 2,
                Self::UpdateDisplayNames => 3,
                Self::RemovePlayers => 4
            );
        }
    }

    /// Updates the player's position on the server.
    ///
    /// <https://wiki.vg/Protocol#Synchronize_Player_Position>
    #[derive(Clone, Debug)]
    pub struct C36SynchronizePlayerPosition {
        /// Absolute or relative position, depending on Flags.
        pub x: f64,
        /// Absolute or relative position, depending on Flags.
        pub y: f64,
        /// Absolute or relative position, depending on Flags.
        pub z: f64,
        /// Absolute or relative rotation on the X axis, in degrees.
        pub yaw: f32,
        /// Absolute or relative rotation on the Y axis, in degrees.
        pub pitch: f32,
        /// Bit field for relativity of coordinates:
        /// <Dinnerbone> It's a bitfield, X/Y/Z/Y_ROT/X_ROT.
        /// <Dinnerbone> If X is set, the x value is relative and not absolute.
        pub flags: u8,
        /// Client should confirm this packet with Accept Teleportation containing the same Teleport ID.
        pub teleport_id: i32,
        /// True if the player should dismount their vehicle.
        pub dismount_vehicle: bool,
    }
    impl ClientBoundPacket for C36SynchronizePlayerPosition {
        const PACKET_ID: i32 = 0x36;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_f64(self.x);
            encoder.write_f64(self.y);
            encoder.write_f64(self.z);
            encoder.write_f32(self.yaw);
            encoder.write_f32(self.pitch);
            encoder.write_u8(self.flags);
            encoder.write_varint(self.teleport_id);
            encoder.write_bool(self.dismount_vehicle);
        }
    }

    /// Sent by the server when an entity is to be destroyed on the client.
    ///
    /// <https://wiki.vg/Protocol#Remove_Entities>
    #[derive(Clone, Debug)]
    pub struct C38RemoveEntities {
        /// The list of entities to destroy.
        pub entities: Vec<VarInt>,
    }
    impl ClientBoundPacket for C38RemoveEntities {
        const PACKET_ID: i32 = 0x38;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entities.len() as _);
            for eid in self.entities.iter() {
                encoder.write_varint(*eid);
            }
        }
    }

    /// Changes the direction an entity's head is facing.
    /// While sending the Entity Look packet changes the vertical rotation of the head,
    /// sending this packet appears to be necessary to rotate the head horizontally.
    ///
    /// <https://wiki.vg/Protocol#Set_Head_Rotation>
    #[derive(Clone, Debug)]
    pub struct C3CSetHeadRotation {
        pub entity_id: VarInt,
        /// New angle, not a delta
        pub head_yaw: Angle,
    }
    impl ClientBoundPacket for C3CSetHeadRotation {
        const PACKET_ID: i32 = 0x3C;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_angle(self.head_yaw);
        }
    }

    #[derive(Clone, Debug)]
    pub struct C3DBlockChange {
        pub x: u8,
        pub y: u8,
        pub z: u8,
        pub block_id: i32,
    }

    /// Fired whenever 2 or more blocks are changed within the same chunk on the same tick.
    ///
    /// <https://wiki.vg/Prrotocol#Multi_Block_Change>
    #[derive(Clone, Debug)]
    pub struct C3DUpdateSectionBlocks {
        pub section_x: i32,
        pub section_y: i32,
        pub section_z: i32,
        pub inverted_trust_edges: bool,
        pub blocks: Vec<C3DBlockChange>,
    }
    impl ClientBoundPacket for C3DUpdateSectionBlocks {
        const PACKET_ID: i32 = 0x3D;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u64(
                ((self.section_x as u64 & 0x3FFFFF) << 42)
              | ((self.section_z as u64 & 0x3FFFFF) << 20)
              |  (self.section_y as u64 & 0x0FFFFF),
            );
            encoder.write_bool(self.inverted_trust_edges);
            encoder.write_varint(self.blocks.len() as VarInt);
            for block_change in self.blocks.iter() {
                encoder.write_varlong(
                    (block_change.block_id as i64      ) << 12
                  | (block_change.x        as i64 & 0xF) << 8
                  | (block_change.z        as i64 & 0xF) << 4
                  | (block_change.y        as i64 & 0xF),
                );
            }
        }
    }

    /// Sent to change the player's slot selection.
    ///
    /// <https://wiki.vg/Protocol#Set_Held_Item_.28clientbound.29>
    #[derive(Clone, Debug)]
    pub struct C47SetHeldItem {
        /// The slot which the player has selected (0–8)
        pub slot: i8,
    }
    impl ClientBoundPacket for C47SetHeldItem {
        const PACKET_ID: i32 = 0x47;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_i8(self.slot);
        }
    }

    /// Updates the client's location. This is used to determine what chunks should
    /// remain loaded and if a chunk load should be ignored;
    /// chunks outside of the view distance may be unloaded.
    /// Sent whenever the player moves across a chunk border horizontally,
    /// and also (according to testing) for any integer change in the vertical axis,
    /// even if it doesn't go across a chunk section border.
    ///
    /// <https://wiki.vg/Protocol#Set_Center_Chunk>
    #[derive(Clone, Debug)]
    pub struct C48SetCenterChunk {
        pub chunk_x: i32,
        pub chunk_z: i32,
    }
    impl ClientBoundPacket for C48SetCenterChunk {
        const PACKET_ID: i32 = 0x40;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.chunk_x);
            encoder.write_varint(self.chunk_z);
        }
    }

    /// Sent by the server after login to specify the coordinates of the spawn point
    /// (the point at which players spawn at, and which the compass points to).
    /// It can be sent at any time to update the point compasses point at.
    ///
    /// <https://wiki.vg/Protocol#Set_Default_Spawn_Position>
    #[derive(Clone, Debug)]
    pub struct C4ASetDefaultSpawnPosition {
        /// Spawn location.
        pub location: Position,
        /// The angle at which to respawn at.
        pub angle: f32,
    }
    impl ClientBoundPacket for C4ASetDefaultSpawnPosition {
        const PACKET_ID: i32 = 0x4A;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_u64(self.location.encode());
            encoder.write_f32(self.angle);
        }
    }

    /// Updates one or more metadata properties for an existing entity.
    /// Any properties not included in the Metadata field are left unchanged.
    ///
    /// <https://wiki.vg/Protocol#Set_Entity_Metadata>
    #[derive(Clone, Debug)]
    pub struct C4DSetEntityMetadata {
        pub entity_id: VarInt,
        pub metadata: HashMap<u8, MetadataValue>,
    }
    impl ClientBoundPacket for C4DSetEntityMetadata {
        const PACKET_ID: i32 = 0x4D;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            for (key, value) in self.metadata.iter() {
                encoder.write_u8(*key);
                encoder.write_bytes(&value.encode());
            }
            encoder.write_u8(0xFF);
        }
    }

    /// Velocity is believed to be in units of 1/8000 of a block per server tick (50ms);
    /// for example, -1343 would move (-1343 / 8000) = −0.167875 blocks per tick (or −3,3575 blocks per second).
    /// 
    /// <https://wiki.vg/Protocol#Set_Entity_Velocity>
    #[derive(Clone, Debug)]
    pub struct C4FSetEntityVelocity {
        pub entity_id: VarInt,
        /// Velocity on the X axis
        pub vel_x: i16,
        /// Velocity on the Y axis
        pub vel_y: i16,
        /// Velocity on the Z axis
        pub vel_z: i16,
    }
    impl ClientBoundPacket for C4FSetEntityVelocity {
        const PACKET_ID: i32 = 0x4F;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            encoder.write_i16(self.vel_x);
            encoder.write_i16(self.vel_y);
            encoder.write_i16(self.vel_z);
        }
    }

    #[derive(Clone, Copy, Debug)]
    #[repr(u8)]
    pub enum C47EntityEquipmentSlot {
        MainHand = 0,
        OffHand = 1,
        Feet = 2,
        Legs = 3,
        Chest = 4,
        Head = 5,
    }

    /// Change one or more slots of an entity's equipment
    ///
    /// <https://wiki.vg/Protocol#Set_Equipment>
    #[derive(Clone, Debug)]
    pub struct C50EntityEquipment {
        pub entity_id: VarInt,
        pub equipment: Vec<(C47EntityEquipmentSlot, Slot)>,
    }
    impl ClientBoundPacket for C50EntityEquipment {
        const PACKET_ID: i32 = 0x50;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_varint(self.entity_id);
            for (i, (slot_pos, slot)) in self.equipment.iter().enumerate() {
                encoder.write_u8(
                    *slot_pos as u8
                        | if i == self.equipment.len() - 1 {
                            0
                        }
                        else {
                            !(!0 >> 1)
                        },
                );
                encoder.write_bytes(&slot.encode());
            }
        }
    }

    /// This packet may be used by custom servers to display additional information above/below the player list.
    /// It is never sent by the Notchian server.
    ///
    /// <https://wiki.vg/Protocol#Player_List_Header_And_Footer>
    #[derive(Clone, Debug)]
    pub struct C60SetTabListHeaderAndFooter {
        /// To remove the header, send a empty text component: {"text":""}
        pub header: serde_json::Value,
        /// To remove the footer, send a empty text component: {"text":""}
        pub footer: serde_json::Value,
    }
    impl ClientBoundPacket for C60SetTabListHeaderAndFooter {
        const PACKET_ID: i32 = 0x60;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
            encoder.write_string(&self.header.to_string());
            encoder.write_string(&self.footer.to_string());
        }
    }

    /// This packet is sent by the server when an entity moves more than 8 blocks.
    ///
    /// <https://wiki.vg/Protocol#Teleport_Entity>
    #[derive(Clone, Debug)]
    pub struct C63TeleportEntity {
        pub entity_id: VarInt,
        /// X Axis position of the entity
        pub x: f64,
        /// Y Axis position of the entity
        pub y: f64,
        /// Z Axis position of the entity
        pub z: f64,
        /// Y Rot
        pub yaw: Angle,
        /// X Rot
        pub pitch: Angle,
        /// Wether the entity is touching the ground
        pub on_ground: bool,
    }
    impl ClientBoundPacket for C63TeleportEntity {
        const PACKET_ID: i32 = 0x63;

        fn encode<D: BufMut>(&self, encoder: &mut PacketEncoder<D>) {
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

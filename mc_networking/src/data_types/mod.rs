use crate::{data_types::encoder::PacketEncoder, DecodingError, DecodingResult};

use byteorder::ReadBytesExt;
use bytes::Bytes;
use serde::{Deserialize, Serialize};
use std::io::{Cursor, Read};
use tokio::io::{AsyncRead, AsyncReadExt};
use uuid::Uuid;

pub mod bitbuffer;
pub mod bitset;
pub mod command_data;
pub mod encoder;
mod identifier;

pub use identifier::*;

pub type VarInt = i32;
pub type VarLong = i64;
pub type Angle = u8;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Slot {
    NotPresent,
    Present {
        item_id: i32,
        item_count: u8,
        nbt: nbt::Blob,
    },
}
impl Slot {
    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> DecodingResult<Self> {
        if stream.read_u8()? == 1 {
            let item_id = encoder::varint::decode_sync(stream)?;
            let item_count = stream.read_u8()?;
            let remaining = {
                let mut remaining = vec![];
                stream.read_to_end(&mut remaining)?;
                remaining
            };
            Ok(Slot::Present {
                item_id,
                item_count,
                nbt: if remaining[0] == 0 {
                    nbt::Blob::new()
                } else {
                    nbt::Blob::from_reader(&mut Cursor::new(remaining))
                        .map_err(std::io::Error::from)?
                },
            })
        } else {
            Ok(Slot::NotPresent)
        }
    }

    pub fn decode<T: AsRef<[u8]>>(buffer: &T) -> DecodingResult<Self> {
        Self::decode_sync(&mut Cursor::new(buffer.as_ref()))
    }

    pub fn encode(&self) -> Bytes {
        let mut encoder = PacketEncoder::default();
        match self {
            Slot::NotPresent => encoder.write_bool(false),
            Slot::Present {
                item_id,
                item_count,
                nbt,
            } => {
                encoder.write_bool(true);
                encoder.write_varint(*item_id);
                encoder.write_u8(*item_count);
                nbt::ser::to_writer(&mut encoder, nbt, None).unwrap();
            }
        }
        encoder.into_inner().freeze()
    }

    pub fn is_present(&self) -> bool {
        match self {
            Slot::NotPresent => false,
            Slot::Present { .. } => true,
        }
    }
}
impl Default for Slot {
    fn default() -> Self {
        Self::NotPresent
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl Position {
    pub fn encode(&self) -> u64 {
        ((self.x as u64 & 0x3FFFFFF) << 38) |
        ((self.z as u64 & 0x3FFFFFF) << 12) |
         (self.y as u64 & 0xFFF)
    }

    pub fn decode(bytes: i64) -> Self {
        let x = bytes >> 38;
        let y = bytes & 0xFFF;
        let z = (bytes << 26) >> 38;
        Self {
            x: x as i32,
            y: y as i32,
            z: z as i32,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Particle {
    pub id: i32,
    pub data: i32,
}
impl Particle {
    pub fn encode(&self) -> Bytes {
        let mut data = PacketEncoder::default();
        data.write_varint(self.id);
        data.write_varint(self.data);
        data.into_inner().freeze()
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> DecodingResult<Self> {
        Ok(Self {
            id: encoder::varint::decode_async(stream).await?,
            data: encoder::varint::decode_async(stream).await?,
        })
    }

    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> DecodingResult<Self> {
        Ok(Self {
            id: encoder::varint::decode_sync(stream)?,
            data: encoder::varint::decode_sync(stream)?,
        })
    }

    pub fn decode<T: AsRef<[u8]>>(buffer: &T) -> DecodingResult<Self> {
        Self::decode_sync(&mut Cursor::new(buffer.as_ref()))
    }
}

#[repr(u8)]
#[derive(Clone, Copy, Debug)]
pub enum Pose {
    Standing = 0,
    FallFlying = 1,
    Sleeping = 2,
    Swimming = 3,
    SpinAttack = 4,
    Sneaking = 5,
    Dying = 6,
}
impl Pose {
    pub fn encode(&self) -> u8 {
        *self as u8
    }

    pub fn decode(data: u8) -> Self {
        if data > 6 {
            panic!("Invalid enum")
        }
        unsafe { std::mem::transmute::<u8, Self>(data) }
    }
}

#[derive(Clone, Debug)]
pub enum MetadataValue {
    Byte(u8),
    VarInt(i32),
    Float(f32),
    String(String),
    Chat(serde_json::Value),
    OptChat(Option<serde_json::Value>),
    Slot(Slot),
    Boolean(bool),
    Rotation(f32, f32, f32),
    Position(Position),
    Direction(VarInt),
    OptUUID(Option<Uuid>),
    OptPosition(Option<Position>),
    OptBlockID(Option<VarInt>),
    NBT(nbt::Value),
    Particle(Particle),
    OptVarInt(Option<VarInt>),
    Pose(Pose),
    VillagerData {
        kind: VarInt,
        profession: VarInt,
        level: VarInt,
    },
}
impl MetadataValue {
    pub fn encode(&self) -> Bytes {
        let mut data = PacketEncoder::default();
        match self {
            MetadataValue::Byte(b) => {
                data.write_u8(0);
                data.write_u8(*b);
            }
            MetadataValue::VarInt(v) => {
                data.write_u8(1);
                data.write_varint(*v);
            }
            MetadataValue::Float(v) => {
                data.write_u8(2);
                data.write_f32(*v);
            }
            MetadataValue::String(v) => {
                data.write_u8(3);
                data.write_string(v);
            }
            MetadataValue::Chat(v) => {
                data.write_u8(4);
                data.write_string(&v.to_string());
            }
            MetadataValue::OptChat(v) => {
                data.write_u8(5);
                match v {
                    Some(v) => {
                        data.write_bool(true);
                        data.write_string(&v.to_string());
                    }
                    None => {
                        data.write_bool(false);
                    }
                }
            }
            MetadataValue::Slot(s) => {
                data.write_u8(6);
                data.write_bytes(&s.encode());
            }
            MetadataValue::Boolean(b) => {
                data.write_u8(7);
                data.write_u8(*b as u8);
            }
            MetadataValue::Rotation(x, y, z) => {
                data.write_u8(8);
                data.write_f32(*x);
                data.write_f32(*y);
                data.write_f32(*z);
            }
            MetadataValue::Position(pos) => {
                data.write_u8(9);
                data.write_u64(pos.encode());
            }
            MetadataValue::OptPosition(p_pos) => {
                data.write_u8(10);
                match p_pos {
                    Some(pos) => {
                        data.write_bool(true);
                        data.write_u64(pos.encode());
                    }
                    None => {
                        data.write_bool(false);
                    }
                }
            }
            MetadataValue::Direction(dir) => {
                data.write_u8(11);
                data.write_i32(*dir);
            }
            MetadataValue::OptUUID(p_uuid) => {
                data.write_u8(12);
                match p_uuid {
                    Some(uuid) => {
                        data.write_u8(1);
                        data.write_uuid(uuid);
                    }
                    None => {
                        data.write_u8(0);
                    }
                }
            }
            MetadataValue::OptBlockID(p_id) => {
                data.write_u8(13);
                match p_id {
                    Some(id) => {
                        data.write_u8(1);
                        data.write_varint(*id);
                    }
                    None => {
                        data.write_u8(0);
                    }
                }
            }
            MetadataValue::NBT(nbt) => {
                data.write_u8(14);
                nbt.to_writer(&mut data).unwrap();
            }
            MetadataValue::Particle(particle) => {
                data.write_u8(15);
                data.write_bytes(&particle.encode());
            }
            MetadataValue::VillagerData {
                kind,
                level,
                profession,
            } => {
                data.write_u8(16);
                data.write_varint(*kind);
                data.write_varint(*level);
                data.write_varint(*profession);
            }
            MetadataValue::OptVarInt(p_varint) => {
                data.write_u8(17);
                match p_varint {
                    Some(varint) => {
                        data.write_bool(true);
                        data.write_varint(*varint);
                    }
                    None => {
                        data.write_bool(false);
                    }
                }
            }
            MetadataValue::Pose(pos) => {
                data.write_u8(18);
                data.write_varint(pos.encode() as i32);
            }
        };
        data.into_inner().freeze()
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> DecodingResult<Self> {
        let kind = encoder::varint::decode_async(stream).await?;

        #[allow(overlapping_range_endpoints)]
        match kind {
            0 => Ok(Self::Byte(stream.read_u8().await?)),
            1 => Ok(Self::VarInt(encoder::varint::decode_async(stream).await?)),
            2 => Ok(Self::Float(f32::from_bits(stream.read_u32().await?))),
            3 => Ok(Self::String(encoder::string::decode_async(stream).await?)),
            // 4  Chat
            // 5  OptChat
            // 6  Slot
            7 => Ok(Self::Boolean(stream.read_u8().await? == 1)),
            // 8  Rotation
            // 9  Position
            // 10 OptPosition
            // 11 Direction
            // 12 OptUUID
            // 13 OptBlockID
            // 14 NBT
            // 15 Particle
            // 16 Villager Data
            // 17 OptVarInt
            // 18 Pose
            0..=18 => unimplemented!(), // TODO: Implement everything
            _ => Err(DecodingError::parse_error("metadata value", "invalid type")),
        }
    }
}

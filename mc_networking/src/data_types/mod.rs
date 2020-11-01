use anyhow::{Error, Result};
use std::io::{Cursor, Read};
use tokio::prelude::{io::AsyncReadExt, AsyncRead};
use uuid::Uuid;
use crate::data_types::encoder::PacketEncoder;

pub mod bitbuffer;
pub mod command_data;
pub mod encoder;

pub type VarInt = i32;
pub type VarLong = i64;
pub type Angle = u8;

#[derive(Clone, Debug)]
pub enum Slot {
    NotPresent,
    Present {
        item_id: i32,
        item_count: u8,
        nbt: nbt::Value,
    },
}
impl Slot {
    pub fn encode(&self) -> Vec<u8> {
        let mut encoder = PacketEncoder::new();
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
        encoder.consume()
    }
}
impl Default for Slot {
    fn default() -> Self {
        Self::NotPresent
    }
}

#[derive(Clone, Debug)]
pub struct Position {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}
impl Position {
    pub fn encode(&self) -> u64 {
        let x = u32::from_ne_bytes(self.x.to_ne_bytes()) as u64;
        let y = u32::from_ne_bytes(self.y.to_ne_bytes()) as u64;
        let z = u32::from_ne_bytes(self.z.to_ne_bytes()) as u64;

        ((x & 0x3FFFFFF) << 38) | ((z & 0x3FFFFFF) << 12) | (y & 0xFFF)
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

#[derive(Clone, Debug)]
pub struct Particle {
    pub id: i32,
    pub data: i32,
}
impl Particle {
    pub fn encode(&self) -> Vec<u8> {
        let mut data = vec![];
        data.append(&mut encoder::varint::encode(self.id));
        data.append(&mut encoder::varint::encode(self.data));
        data
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> Result<Self> {
        Ok(Self {
            id: encoder::varint::decode_async(stream).await?,
            data: encoder::varint::decode_async(stream).await?,
        })
    }

    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> Result<Self> {
        Ok(Self {
            id: encoder::varint::decode_sync(stream)?,
            data: encoder::varint::decode_sync(stream)?,
        })
    }

    pub fn decode<T: AsRef<[u8]>>(buffer: &T) -> Result<Self> {
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
    Direction(i32), // VarInt
    OptUUID(Option<Uuid>),
    OptPosition(Option<Position>),
    OptBlockID(Option<i32>), // VarInt
    NBT(nbt::Value),
    Particle(Particle),
    OptVarInt(Option<i32>), // VarInt
    Pose(Pose),
    VillagerData {
        kind: i32,       // VarInt
        profession: i32, // VarInt
        level: i32,      // VarInt
    },
}
impl MetadataValue {
    pub fn encode(&self) -> Vec<u8> {
        let mut data = vec![];
        match self {
            MetadataValue::Byte(b) => {
                data.push(0);
                data.push(*b);
            }
            MetadataValue::VarInt(v) => {
                data.push(1);
                data.append(&mut encoder::varint::encode(*v));
            }
            MetadataValue::Float(v) => {
                data.push(2);
                data.extend_from_slice(&v.to_be_bytes());
            }
            MetadataValue::String(v) => {
                data.push(3);
                data.append(&mut encoder::string::encode(v));
            }
            MetadataValue::Chat(v) => {
                data.push(4);
                data.append(&mut encoder::string::encode(&v.to_string()));
            }
            MetadataValue::OptChat(v) => {
                data.push(5);
                match v {
                    Some(v) => {
                        data.push(1); // true
                        data.append(&mut encoder::string::encode(&v.to_string()));
                    }
                    None => {
                        data.push(0); // false
                    }
                }
            }
            MetadataValue::Slot(s) => {
                data.push(6);
                data.append(&mut s.encode());
            }
            MetadataValue::Boolean(b) => {
                data.push(7);
                data.push(*b as u8);
            }
            MetadataValue::Rotation(x, y, z) => {
                data.push(8);
                data.extend_from_slice(&x.to_be_bytes());
                data.extend_from_slice(&y.to_be_bytes());
                data.extend_from_slice(&z.to_be_bytes());
            }
            MetadataValue::Position(pos) => {
                data.push(9);
                data.extend_from_slice(&pos.encode().to_be_bytes());
            }
            MetadataValue::OptPosition(p_pos) => {
                data.push(10);
                match p_pos {
                    Some(pos) => {
                        data.push(1); // true
                        data.extend_from_slice(&pos.encode().to_be_bytes());
                    }
                    None => {
                        data.push(0); // false
                    }
                }
            }
            MetadataValue::Direction(dir) => {
                data.push(11);
                data.extend_from_slice(&dir.to_be_bytes());
            }
            MetadataValue::OptUUID(p_uuid) => {
                data.push(12);
                match p_uuid {
                    Some(uuid) => {
                        data.push(1); // true
                        data.extend_from_slice(uuid.as_bytes());
                    }
                    None => {
                        data.push(0); // false
                    }
                }
            }
            MetadataValue::OptBlockID(p_id) => {
                data.push(13);
                match p_id {
                    Some(id) => {
                        data.push(1); // true
                        data.append(&mut encoder::varint::encode(*id));
                    }
                    None => {
                        data.push(0); // false
                    }
                }
            }
            MetadataValue::NBT(nbt) => {
                data.push(14);
                nbt.to_writer(&mut data).unwrap();
            }
            MetadataValue::Particle(particle) => {
                data.push(15);
                data.append(&mut particle.encode());
            }
            MetadataValue::VillagerData {
                kind,
                level,
                profession,
            } => {
                data.push(16);
                data.append(&mut encoder::varint::encode(*kind));
                data.append(&mut encoder::varint::encode(*profession));
                data.append(&mut encoder::varint::encode(*level));
            }
            MetadataValue::OptVarInt(p_varint) => {
                data.push(17);
                match p_varint {
                    Some(varint) => {
                        data.push(1); // true
                        data.append(&mut encoder::varint::encode(*varint));
                    }
                    None => {
                        data.push(0); // false
                    }
                }
            }
            MetadataValue::Pose(pos) => {
                data.push(18);
                data.append(&mut encoder::varint::encode(pos.encode() as i32));
            }
        };
        data
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> Result<Self> {
        let kind = encoder::varint::decode_async(stream).await?;

        #[allow(overlapping_patterns)]
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
            _ => Err(Error::msg("Invalid type")),
        }
    }
}

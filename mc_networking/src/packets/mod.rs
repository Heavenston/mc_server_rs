pub mod client_bound;
pub mod server_bound;

use crate::{data_types::encoder::varint, DecodingResult};

use byteorder::ReadBytesExt;
use flate2::{
    read::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use std::{
    fmt::Debug,
    io::{Cursor, Read},
    ops::Deref,
};
use tokio::prelude::{io::AsyncReadExt, AsyncRead};

#[derive(Debug, Clone, Copy)]
pub struct PacketCompression(i32);
impl PacketCompression {
    pub fn new(threshold: i32) -> Self {
        Self(threshold)
    }
}
impl Default for PacketCompression {
    fn default() -> Self {
        Self(-1)
    }
}
impl Deref for PacketCompression {
    type Target = i32;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub struct RawPacket {
    pub packet_id: i32,
    pub data: Box<[u8]>,
}
impl RawPacket {
    pub fn new(packet_id: i32, data: Box<[u8]>) -> Self {
        Self { packet_id, data }
    }

    pub fn will_compress(&self, compression: PacketCompression) -> bool {
        *compression > 0
            && self.data.len() as i32 + (self.packet_id / 2i32.pow(7) + 1) >= *compression
    }

    pub fn encode(&self, compression: PacketCompression) -> Box<[u8]> {
        // PacketID + Data
        let mut data_buffer = vec![];
        let data_buffer_length = data_buffer.len();
        data_buffer.append(&mut varint::encode(self.packet_id));
        data_buffer.extend_from_slice(self.data.as_ref());

        if *compression > 0 {
            let (mut compressed_data_buffer, compressed) =
                if (data_buffer_length as i32) >= *compression {
                    let mut data = vec![];
                    ZlibEncoder::new(Cursor::new(data_buffer), Compression::new(9))
                        .read_to_end(&mut data)
                        .unwrap();
                    (data, true)
                }
                else {
                    (data_buffer, false)
                };
            let data_length = &mut varint::encode(if compressed {
                data_buffer_length as i32
            }
            else {
                0
            });

            let mut buffer = vec![];
            buffer.append(&mut varint::encode(
                (data_length.len() + compressed_data_buffer.len()) as i32,
            ));
            buffer.append(data_length);
            buffer.append(&mut compressed_data_buffer);
            buffer.into_boxed_slice()
        }
        else {
            let mut buffer = vec![];
            buffer.append(&mut varint::encode(data_buffer.len() as i32));
            buffer.append(&mut data_buffer);
            buffer.into_boxed_slice()
        }
    }

    /// Decodes the data part of a Packet (packet_id + data)
    fn decode<T: Read + Unpin>(stream: &mut T) -> DecodingResult<Self> {
        let packet_id = varint::decode_sync(stream)?;
        let mut data = vec![];
        while let Ok(b) = stream.read_u8() {
            data.push(b);
        }
        Ok(Self {
            packet_id,
            data: data.into_boxed_slice(),
        })
    }
    pub async fn decode_async<T: AsyncRead + Unpin>(
        stream: &mut T,
        compression: PacketCompression,
    ) -> DecodingResult<Self> {
        if *compression > 0 {
            let packet_length = varint::decode_async(stream).await?;
            let mut taker = stream.take(packet_length as u64);
            let is_compressed = varint::decode_async(&mut taker).await? != 0;

            let mut data = {
                let mut data = vec![];
                while let Ok(b) = taker.read_u8().await {
                    data.push(b);
                }
                Cursor::new(data)
            };

            if is_compressed {
                let mut data_decoder = ZlibDecoder::new(data);
                Self::decode(&mut data_decoder)
            }
            else {
                Self::decode(&mut data)
            }
        }
        else {
            let length = varint::decode_async(stream).await?;
            let mut taker = stream.take(length as u64);
            let mut data = vec![];
            while let Ok(b) = taker.read_u8().await {
                data.push(b);
            }
            Self::decode(&mut Cursor::new(data))
        }
    }
    pub fn decode_sync<T: Read + Unpin>(
        stream: &mut T,
        compression: PacketCompression,
    ) -> DecodingResult<Self> {
        if *compression > 0 {
            let packet_length = varint::decode_sync(stream)?;
            let mut taker = stream.take(packet_length as u64);
            let _data_length = varint::decode_sync(&mut taker)?;

            let mut data_decoder = ZlibDecoder::new({
                let mut data = vec![];
                while let Ok(b) = taker.read_u8() {
                    data.push(b);
                }
                Cursor::new(data)
            });

            Self::decode(&mut data_decoder)
        }
        else {
            let length = varint::decode_sync(stream)?;
            let mut taker = stream.take(length as u64);
            Self::decode(&mut taker)
        }
    }
}

impl Debug for RawPacket {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawPacket")
            .field("packet_id", &self.packet_id)
            .field("data_length", &self.data.len())
            .finish()
    }
}

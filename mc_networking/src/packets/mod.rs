pub mod client_bound;
pub mod server_bound;

use crate::{data_types::encoder::varint, DecodingError, DecodingResult};

use byteorder::{ReadBytesExt, WriteBytesExt};
use bytes::{Buf, BufMut, Bytes, BytesMut};
use flate2::{
    write::{ZlibDecoder, ZlibEncoder},
    Compression,
};
use openssl::{
    aes::{aes_ige, AesKey},
    symm::{Cipher, Crypter, Mode},
};
use std::{
    fmt::{self, Debug},
    io::{Cursor, Read, Write},
    ops::Deref,
};
use tokio::prelude::{io::AsyncReadExt, AsyncRead};

#[derive(Debug, Clone, Copy)]
pub struct PacketCompression(i32);
impl PacketCompression {
    pub fn new(threshold: i32) -> Self {
        Self(threshold)
    }

    pub fn is_enabled(&self) -> bool {
        self.0 > 0
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
    pub data: Bytes,
}
impl RawPacket {
    pub fn new(packet_id: i32, data: Bytes) -> Self {
        Self { packet_id, data }
    }

    pub fn will_compress(&self, compression: PacketCompression) -> bool {
        *compression > 0
            && self.data.len() as i32 + (self.packet_id / 2i32.pow(7) + 1) >= *compression
    }
    pub fn encode(&self, compression: PacketCompression, dst: &mut BytesMut) {
        assert!(dst.is_empty());
        dst.reserve(varint::MAX_BYTE_SIZE * 2 + self.data.len());
        dst.extend_from_slice(&[0; varint::MAX_BYTE_SIZE]);
        let mut packet_length_bytes = dst.split_to(varint::MAX_BYTE_SIZE);

        let mut packet_id_varint_buffer = [0u8; varint::MAX_BYTE_SIZE];
        let packet_id_varint_length =
            varint::encode_into(self.packet_id, &mut &mut packet_id_varint_buffer[..]);
        let packet_id_varint_buffer = &packet_id_varint_buffer[0..packet_id_varint_length];

        if compression.is_enabled() {
            let uncompressed_length = packet_id_varint_length + self.data.len();
            if uncompressed_length as i32 >= *compression {
                varint::encode_into(uncompressed_length as i32, dst);

                let mut compressor = ZlibEncoder::new(dst.writer(), Compression::fast());
                compressor.write_all(packet_id_varint_buffer).unwrap();
                compressor.write_all(&self.data).unwrap();
                compressor.flush_finish().unwrap();
            }
            else {
                dst.put_u8(0); // 0 VarInt, no compression
                dst.extend_from_slice(packet_id_varint_buffer);
                dst.extend_from_slice(&self.data);
            }
        }
        else {
            dst.extend_from_slice(packet_id_varint_buffer);
            dst.extend_from_slice(&self.data);
        }

        let mut packet_length_buffer = [0u8; varint::MAX_BYTE_SIZE];
        let packet_length_varint_len =
            varint::encode_into(dst.len() as i32, &mut &mut packet_length_buffer[..]);
        packet_length_bytes.advance(varint::MAX_BYTE_SIZE - packet_length_varint_len);
        packet_length_bytes.clear();
        packet_length_bytes.extend_from_slice(&packet_length_buffer[0..packet_length_varint_len]);
        std::mem::swap(dst, &mut packet_length_bytes);
        dst.unsplit(packet_length_bytes);
    }

    /// Decodes the content part of a Packet (packet_id + data)
    fn decode_content(stream: &mut BytesMut, size: usize) -> DecodingResult<Self> {
        let packet_id = varint::decode_buf(stream)?;
        Ok(Self {
            packet_id,
            data: stream.split_to(size - 1).freeze(),
        })
    }

    pub fn decode(bytes: &mut BytesMut, compression: PacketCompression) -> DecodingResult<Self> {
        let mut taker = bytes.take(varint::MAX_BYTE_SIZE);
        let packet_length = varint::decode_buf(&mut taker)?;
        taker.get_mut().reserve(packet_length as usize);
        taker.set_limit(packet_length as usize);
        if taker.remaining() < packet_length as usize {
            return Err(DecodingError::NotEnoughBytes);
        }
        if *compression > 0 {
            let content_length = varint::decode_buf(&mut taker)?;
            // No compression
            if content_length == 0 {
                let content_length = taker.remaining();
                Self::decode_content(taker.into_inner(), content_length)
            }
            else {
                let mut uncompressed = BytesMut::with_capacity(content_length as usize);
                let mut decoder = flate2::write::ZlibDecoder::new((&mut uncompressed).writer());
                std::io::copy(&mut (&mut taker).reader(), &mut decoder)?;
                decoder.finish()?;
                if uncompressed.len() != content_length as usize {
                    return Err(DecodingError::ParseError {
                        data_type: "raw packet".to_string(),
                        message: "content length does not match".to_string(),
                    });
                }
                let content_length = uncompressed.len();
                Self::decode_content(&mut uncompressed, content_length)
            }
        }
        else {
            let content_length = taker.remaining();
            Self::decode_content(taker.into_inner(), content_length)
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

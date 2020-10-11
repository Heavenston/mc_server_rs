use crate::data_types::VarInt;
use uuid::Uuid;
use std::io::Write;

pub struct PacketEncoder {
    data: Vec<u8>,
}
impl PacketEncoder {
    pub fn new() -> Self {
        Self {
            data: Vec::new()
        }
    }
    pub fn consume(self) -> Vec<u8> {
        self.data
    }

    pub fn write_u8(&mut self, v: u8) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_i8(&mut self, v: i8) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_u16(&mut self, v: u16) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_i16(&mut self, v: i16) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_u32(&mut self, v: u32) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_i32(&mut self, v: i32) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_u64(&mut self, v: u64) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_i64(&mut self, v: i64) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_f32(&mut self, v: f32) {
        self.write_bytes(&v.to_be_bytes());
    }
    pub fn write_f64(&mut self, v: f64) {
        self.write_bytes(&v.to_be_bytes());
    }

    pub fn write_bool(&mut self, v: bool) {
        self.write_u8(if v { 1 } else { 0 });
    }
    pub fn write_varint(&mut self, v: VarInt) {
        self.data.append(&mut varint::encode(v));
    }
    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }
    pub fn write_string(&mut self, text: &str) {
        self.write_bytes(string::encode(text).as_slice());
    }
    pub fn write_uuid(&mut self, uuid: &Uuid) {
        self.write_bytes(uuid.as_bytes());
    }
}
impl Write for PacketEncoder {
    fn write(&mut self, buf: &[u8]) -> Result<usize, std::io::Error> {
        self.write_bytes(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> Result<(), std::io::Error> {
        Ok(())
    }
}

pub mod varint {
    use anyhow::{Error, Result};
    use byteorder::ReadBytesExt;
    use std::io::{Cursor, Read};
    use tokio::prelude::io::AsyncReadExt;
    use tokio::prelude::AsyncRead;
    use crate::data_types::VarInt;

    pub fn encode(int: VarInt) -> Vec<u8> {
        let mut val: u32 = int as u32;
        let mut buf = Vec::new();
        loop {
            let mut temp = (val & 0b0111_1111) as u8;
            val >>= 7;
            if val != 0 {
                temp |= 0b1000_0000;
            }
            buf.push(temp);
            if val == 0 {
                return buf;
            }
        }
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> Result<VarInt> {
        let mut num_read: i32 = 0;
        let mut result = 0i32;
        let mut read;
        loop {
            read = stream.read_u8().await?;
            let value = (read & 0b0111_1111) as i32;
            result |= value << (7 * num_read);

            num_read += 1;
            if num_read > 5 {
                return Err(Error::msg("VarInt is too big!"));
            }
            if read & 0b1000_0000 == 0 {
                break;
            }
        }
        Ok(result)
    }
    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> Result<VarInt> {
        let mut num_read = 0;
        let mut result = 0i32;
        let mut read;
        loop {
            read = stream.read_u8()?;
            let value = (read & 0b0111_1111) as i32;
            result |= value << (7 * num_read);

            num_read += 1;
            if num_read > 5 {
                return Err(Error::msg("VarInt is too big!"));
            }
            if read & 0b1000_0000 == 0 {
                break;
            }
        }
        Ok(result)
    }
    pub fn decode<T: AsRef<[u8]>>(buffer: &T) -> Result<VarInt> {
        decode_sync(&mut Cursor::new(buffer.as_ref()))
    }
}
pub mod string {
    use super::varint;

    use anyhow::Result;
    use byteorder::ReadBytesExt;
    use std::io::Read;
    use tokio::prelude::io::AsyncReadExt;
    use tokio::prelude::AsyncRead;

    pub fn encode(string: &str) -> Vec<u8> {
        let mut data = vec![];
        let text = string.as_bytes();
        data.append(&mut varint::encode(text.len() as i32));
        data.extend_from_slice(text);
        data
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> Result<String> {
        let size = varint::decode_async(stream).await?;
        let mut data: Vec<u8> = Vec::with_capacity(size as usize);

        for _ in 0..size {
            data.push(stream.read_u8().await?);
        }

        return Ok(String::from_utf8_lossy(&data).into());
    }
    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> Result<String> {
        let size = varint::decode_sync(stream)?;
        let mut data: Vec<u8> = Vec::with_capacity(size as usize);

        for _ in 0..size {
            data.push(stream.read_u8()?);
        }

        return Ok(String::from_utf8_lossy(&data).into());
    }
}

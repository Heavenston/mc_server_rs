
pub mod varint {
    use anyhow::{Error, Result};
    use byteorder::ReadBytesExt;
    use std::io::{Cursor, Read};
    use tokio::prelude::io::AsyncReadExt;
    use tokio::prelude::AsyncRead;

    pub fn encode(int: i32) -> Vec<u8> {
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

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> Result<i32> {
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
    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> Result<i32> {
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
    pub fn decode<T: AsRef<[u8]>>(buffer: &T) -> Result<i32> {
        decode_sync(&mut Cursor::new(buffer.as_ref()))
    }
}

pub mod string {
    use anyhow::Result;
    use tokio::prelude::io::AsyncReadExt;
    use tokio::prelude::AsyncRead;
    use byteorder::ReadBytesExt;
    use super::varint;
    use std::io::Read;

    pub fn encode_string(string: &str) -> Vec<u8> {
        let mut data = vec![];
        let text = string.as_bytes();
        data.append(&mut varint::encode(text.len() as i32));
        data.extend_from_slice(text);
        data
    }

    pub async fn decode_string_async<T: AsyncRead + Unpin>(stream: &mut T) -> Result<String> {
        let size = varint::decode_async(stream).await?;
        let mut data: Vec<u8> = Vec::with_capacity(size as usize);

        for _ in 0..size {
            data.push(stream.read_u8().await?);
        }

        return Ok(String::from_utf8_lossy(&data).into());
    }
    pub fn decode_string_sync<T: Read + Unpin>(stream: &mut T) -> Result<String> {
        let size = varint::decode_sync(stream)?;
        let mut data: Vec<u8> = Vec::with_capacity(size as usize);

        for _ in 0..size {
            data.push(stream.read_u8()?);
        }

        return Ok(String::from_utf8_lossy(&data).into());
    }
}

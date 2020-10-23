pub mod client_bound;
pub mod server_bound;

use crate::data_types::encoder::varint;

use anyhow::Result;
use byteorder::ReadBytesExt;
use std::io::Read;
use tokio::prelude::{io::AsyncReadExt, AsyncRead};

pub struct RawPacket {
    pub packet_id: i32,
    pub data: Box<[u8]>,
}
impl RawPacket {
    pub fn new(packet_id: i32, data: Box<[u8]>) -> Self { Self { packet_id, data } }

    pub fn encode(&self) -> Box<[u8]> {
        let packet_id = &mut varint::encode(self.packet_id);
        let length = &mut varint::encode((self.data.len() + packet_id.len()) as i32);

        let mut buffer = vec![];
        buffer.append(length);
        buffer.append(packet_id);
        buffer.extend_from_slice(self.data.as_ref());

        buffer.into_boxed_slice()
    }

    pub async fn decode_async<T: AsyncRead+Unpin>(stream: &mut T) -> Result<Self> {
        let length = varint::decode_async(stream).await?;
        let mut taker = stream.take(length as u64);
        let packet_id = varint::decode_async(&mut taker).await?;
        let mut data = vec![];
        while let Ok(b) = taker.read_u8().await {
            data.push(b);
        }
        Ok(Self {
            packet_id,
            data: data.into_boxed_slice(),
        })
    }

    pub fn decode_sync<T: Read+Unpin>(stream: &mut T) -> Result<Self> {
        let length = varint::decode_sync(stream)?;
        let mut taker = stream.take(length as u64);
        let packet_id = varint::decode_sync(&mut taker)?;
        let mut data = vec![];
        while let Ok(b) = taker.read_u8() {
            data.push(b);
        }
        Ok(Self {
            packet_id,
            data: data.into_boxed_slice(),
        })
    }
}

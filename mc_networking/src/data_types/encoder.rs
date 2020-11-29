use crate::{
    data_types::{Angle, VarInt, VarLong},
    packets::RawPacket,
    DecodingResult,
};

use byteorder::{ReadBytesExt, BE};
use bytes::{Buf, Bytes, BytesMut};
use std::io::{Cursor, Read, Result as IoResult, Write};
use uuid::Uuid;

pub struct PacketEncoder {
    data: BytesMut,
}
impl PacketEncoder {
    pub fn new() -> Self {
        Self {
            data: BytesMut::new(),
        }
    }

    pub fn consume(self) -> Bytes {
        self.data.freeze()
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

    pub fn write_angle(&mut self, v: Angle) {
        self.write_bytes(&v.to_be_bytes());
    }

    pub fn write_bool(&mut self, v: bool) {
        self.write_u8(if v { 1 } else { 0 });
    }

    pub fn write_varint(&mut self, v: VarInt) {
        varint::encode_into(v, &mut self.data);
    }
    pub fn write_varlong(&mut self, v: VarLong) {
        varlong::encode_into(v, &mut self.data);
    }

    pub fn write_bytes(&mut self, bytes: &[u8]) {
        self.data.extend_from_slice(bytes);
    }

    pub fn write_string(&mut self, text: &str) {
        string::encode_into(text, &mut self.data);
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

pub struct PacketDecoder {
    data: Cursor<Bytes>,
}
impl PacketDecoder {
    pub fn new(raw_packet: RawPacket) -> Self {
        Self {
            data: Cursor::new(raw_packet.data),
        }
    }

    pub fn remaining(&self) -> usize {
        self.data.remaining()
    }

    pub fn read_u8(&mut self) -> DecodingResult<u8> {
        Ok(self.data.read_u8()?)
    }

    pub fn read_i8(&mut self) -> DecodingResult<i8> {
        Ok(self.data.read_i8()?)
    }

    pub fn read_u16(&mut self) -> DecodingResult<u16> {
        Ok(self.data.read_u16::<BE>()?)
    }

    pub fn read_i16(&mut self) -> DecodingResult<i16> {
        Ok(self.data.read_i16::<BE>()?)
    }

    pub fn read_u32(&mut self) -> DecodingResult<u32> {
        Ok(self.data.read_u32::<BE>()?)
    }

    pub fn read_i32(&mut self) -> DecodingResult<i32> {
        Ok(self.data.read_i32::<BE>()?)
    }

    pub fn read_u64(&mut self) -> DecodingResult<u64> {
        Ok(self.data.read_u64::<BE>()?)
    }

    pub fn read_i64(&mut self) -> DecodingResult<i64> {
        Ok(self.data.read_i64::<BE>()?)
    }

    pub fn read_f32(&mut self) -> DecodingResult<f32> {
        Ok(self.data.read_f32::<BE>()?)
    }

    pub fn read_f64(&mut self) -> DecodingResult<f64> {
        Ok(self.data.read_f64::<BE>()?)
    }

    pub fn read_bool(&mut self) -> DecodingResult<bool> {
        Ok(self.read_u8()? == 1)
    }

    pub fn read_varint(&mut self) -> DecodingResult<VarInt> {
        Ok(varint::decode_sync(&mut self.data)?)
    }

    pub fn read_varlong(&mut self) -> DecodingResult<VarLong> {
        Ok(varlong::decode_sync(&mut self.data)?)
    }

    pub fn read_bytes(&mut self, amount: usize) -> DecodingResult<Vec<u8>> {
        let mut bytes = vec![0; amount];
        self.data.read_exact(bytes.as_mut_slice())?;
        Ok(bytes)
    }

    pub fn read_to_end(&mut self) -> DecodingResult<Vec<u8>> {
        let mut bytes = vec![];
        self.data.read_to_end(&mut bytes)?;
        Ok(bytes)
    }

    pub fn read_string(&mut self) -> DecodingResult<String> {
        Ok(string::decode_sync(&mut self.data)?)
    }

    pub fn read_uuid(&mut self) -> DecodingResult<Uuid> {
        Ok(Uuid::from_slice(self.read_bytes(16)?.as_slice())?)
    }
}
impl Read for PacketDecoder {
    fn read(&mut self, buf: &mut [u8]) -> IoResult<usize> {
        self.data.read(buf)
    }
}

macro_rules! create_varint_decoder {
    ($(async $(@$async:tt)?)? fn $func_name: ident ($stream: ident: $stream_type: ty), $read_expr: expr, output_type: $output_type: ty, max_byte_size: $bytes_limit: expr) => {
        pub $(async $($async)?)? fn $func_name($stream: $stream_type) -> DecodingResult<$output_type> {
            let mut num_read: usize = 0;
            let mut result = 0;
            loop {
                let read = $read_expr;
                let value = <$output_type>::from(read & 0b0111_1111);
                result |= value.overflowing_shl(7 * num_read as u32).0;

                num_read += 1;
                if num_read > $bytes_limit {
                    return Err(DecodingError::parse_error("varint or varlong", "too many bytes"));
                }
                if read & 0b1000_0000 == 0 {
                    break;
                }
            }
            Ok(result)
        }
    };
}
macro_rules! create_varint_decoders {
    (output_type: $output_type: ty, max_byte_size: $bytes_limit: expr) => {
        create_varint_decoder!(
            async fn decode_async(stream: &mut (impl AsyncRead + Unpin)),
            stream.read_u8().await?,
            output_type: $output_type,
            max_byte_size: $bytes_limit
        );
        create_varint_decoder!(
            fn decode_sync(stream: &mut (impl Read + Unpin)),
            stream.read_u8()?,
            output_type: $output_type,
            max_byte_size: $bytes_limit
        );
        create_varint_decoder!(
            fn decode_buf(buffer: &mut impl Buf),
            {
                if !buffer.has_remaining() {
                    return Err(DecodingError::NotEnoughBytes);
                }
                buffer.get_u8()
            },
            output_type: $output_type,
            max_byte_size: $bytes_limit
        );
    };
}

macro_rules! create_varint_encoders {
    (input_type: $input_type: ty, unsigned_type: $unsigned_type: ty) => {
        pub fn encode_into(val: $input_type, bytes: &mut impl BufMut) -> usize {
            let mut val = val as $unsigned_type;
            let mut written = 0;
            loop {
                let mut temp = (val & 0b0111_1111) as u8;
                val >>= 7;
                if val != 0 {
                    temp |= 0b10000000;
                }
                bytes.put_u8(temp);
                written += 1;
                if val == 0 {
                    break;
                }
            }
            written
        }
        pub fn encode(int: $input_type) -> Bytes {
            let mut bytes = BytesMut::with_capacity(MAX_BYTE_SIZE);
            encode_into(int, &mut bytes);
            bytes.freeze()
        }
    };
}

pub mod varint {
    use crate::{data_types::VarInt, DecodingError, DecodingResult};

    use byteorder::ReadBytesExt;
    use bytes::{Buf, BufMut, Bytes, BytesMut};
    use std::io::Read;
    use tokio::prelude::{io::AsyncReadExt, AsyncRead};

    pub const MAX_BYTE_SIZE: usize = 5;

    create_varint_encoders!(input_type: VarInt, unsigned_type: u32);
    create_varint_decoders!(output_type: VarInt, max_byte_size: MAX_BYTE_SIZE);
}
pub mod varlong {
    use crate::{data_types::VarLong, DecodingError, DecodingResult};

    use byteorder::ReadBytesExt;
    use bytes::{Buf, BufMut, Bytes, BytesMut};
    use std::io::Read;
    use tokio::prelude::{io::AsyncReadExt, AsyncRead};

    pub const MAX_BYTE_SIZE: usize = 10;

    create_varint_encoders!(input_type: VarLong, unsigned_type: u64);
    create_varint_decoders!(output_type: VarLong, max_byte_size: MAX_BYTE_SIZE);
}
pub mod string {
    use super::varint;
    use crate::DecodingResult;

    use bytes::{BufMut, Bytes, BytesMut};
    use std::io::Read;
    use tokio::prelude::{io::AsyncReadExt, AsyncRead};

    pub fn encode_into(string: &str, bytes: &mut impl BufMut) {
        let text = string.as_bytes();
        varint::encode_into(text.len() as i32, bytes);
        bytes.put(text);
    }
    pub fn encode(string: &str) -> Bytes {
        let mut bytes = BytesMut::new();
        encode_into(string, &mut bytes);
        bytes.freeze()
    }

    pub async fn decode_async<T: AsyncRead + Unpin>(stream: &mut T) -> DecodingResult<String> {
        let size = varint::decode_async(stream).await?;
        let mut data = BytesMut::with_capacity(size as usize);
        for _ in 0..size {
            data.put_u8(stream.read_u8().await?);
        }
        return Ok(String::from_utf8_lossy(&data).into());
    }
    pub fn decode_sync<T: Read + Unpin>(stream: &mut T) -> DecodingResult<String> {
        let size = varint::decode_sync(stream)?;
        let mut data = BytesMut::with_capacity(size as usize).writer();
        std::io::copy(&mut stream.take(size as u64), &mut data)?;
        Ok(String::from_utf8_lossy(&data.into_inner()).into())
    }
}

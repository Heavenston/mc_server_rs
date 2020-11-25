pub mod client;
pub mod data_types;
pub mod nbt_map;
pub mod packets;

use thiserror::Error;
use tokio::io;

#[derive(Error, Debug)]
pub enum DecodingError {
    #[error("io error {0}")]
    IoError(io::Error),
    #[error("not enough bytes")]
    NotEnoughBytes,
    #[error("could not parse {data_type}: {message}")]
    ParseError { data_type: String, message: String },
}
impl DecodingError {
    pub fn parse_error(data_type: &str, message: &str) -> Self {
        Self::ParseError {
            data_type: data_type.to_string(),
            message: message.to_string(),
        }
    }
}
impl From<uuid::Error> for DecodingError {
    fn from(error: uuid::Error) -> Self {
        Self::ParseError {
            data_type: "uuid".to_string(),
            message: error.to_string(),
        }
    }
}
impl From<io::Error> for DecodingError {
    fn from(error: io::Error) -> Self {
        match error.kind() {
            io::ErrorKind::UnexpectedEof => Self::NotEnoughBytes,
            _ => Self::IoError(error),
        }
    }
}
impl From<nbt::Error> for DecodingError {
    fn from(error: nbt::Error) -> Self {
        Self::ParseError {
            data_type: "uuid".to_string(),
            message: format!("{}", error),
        }
    }
}

pub type DecodingResult<T> = Result<T, DecodingError>;

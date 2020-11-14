#![feature(backtrace)]

pub mod client;
pub mod data_types;
pub mod nbt_map;
pub mod packets;

use std::backtrace::Backtrace;
use thiserror::Error;
use tokio::io;

#[derive(Error, Debug)]
pub enum DecodingError {
    #[error("{source}")]
    IoError {
        #[from]
        source: io::Error,
        backtrace: Backtrace,
    },
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
impl From<nbt::Error> for DecodingError {
    fn from(error: nbt::Error) -> Self {
        Self::from(std::io::Error::from(error))
    }
}

pub type DecodingResult<T> = Result<T, DecodingError>;

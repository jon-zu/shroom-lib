use std::{fmt::Display, str::Utf8Error};

use nt_time::error::FileTimeRangeError;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};

use crate::analyzer::PacketDataAnalytics;
use thiserror::Error;

#[derive(Debug)]
pub struct EOFErrorData {
    pub analytics: PacketDataAnalytics,
    pub type_name: &'static str,
}

impl Display for EOFErrorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "eof packet(type={}): {}", self.type_name, self.analytics)
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("string utf8 error")]
    StringUtf8(#[from] Utf8Error),
    #[error("EOF error: {0}")]
    EOF(Box<EOFErrorData>),
    #[error("String limit {0} exceeed")]
    StringLimit(usize),
    #[error("Invalid enum discriminant {0}")]
    InvalidEnumDiscriminant(usize),
    #[error("Invalid enum primitive {0}")]
    InvalidEnumPrimitive(u32),
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(u64),
    #[error("Invalid time: {0}")]
    InvalidTime(#[from] FileTimeRangeError),
    #[error("Invalid opcode: {0:X}")]
    InvalidOpCode(u16),
    #[error("Out of capacity")]
    OutOfCapacity,
    #[error("No opcode)")]
    NoOpCode,
    #[error("Invalid all bits")]
    InvalidAllBits
}

impl Error {
    //TODO disable diagnostic for release builds
    pub fn eof<T>(data: &[u8], read_len: usize) -> Self {
        let type_name = std::any::type_name::<T>();
        let pos = data.len().saturating_sub(read_len);
        Self::EOF(Box::new(EOFErrorData {
            analytics: PacketDataAnalytics::from_data(data, pos, read_len, read_len * 5),
            type_name,
        }))
    }
}

impl<E> From<TryFromPrimitiveError<E>> for Error
where
    E: TryFromPrimitive,
    E::Primitive: Into<u32>,
{
    fn from(value: TryFromPrimitiveError<E>) -> Self {
        Error::InvalidEnumPrimitive(value.number.into())
    }
}

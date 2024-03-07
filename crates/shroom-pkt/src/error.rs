use std::{fmt::Display, str::Utf8Error};

use nt_time::error::FileTimeRangeError;
use num_enum::{TryFromPrimitive, TryFromPrimitiveError};
use thiserror::Error;

use crate::analyzer::PacketAnalyzer;

#[derive(Debug)]
#[cfg(feature = "eof_ext")]
pub struct EOFExtraData {
    type_name: &'static str,
    read_len: usize,
}

#[derive(Debug)]
#[cfg(not(feature = "eof_ext"))]
pub struct EOFExtraData;

impl EOFExtraData {
    #[cfg(feature = "eof_ext")]
    pub fn from_type<T>(read_len: usize) -> Self {
        let type_name = std::any::type_name::<T>();

        EOFExtraData {
            type_name,
            read_len,
        }
    }

    #[cfg(not(feature = "eof_ext"))]
    pub fn from_type<T>(_read_len: usize) -> Self {
        EOFExtraData
    }
}

#[derive(Debug)]
pub struct EOFErrorData {
    pub pos: usize,
    #[cfg(feature = "eof_ext")]
    pub extra: Box<EOFExtraData>,
}

impl EOFErrorData {
    pub fn from_type<T>(pos: usize, read_len: usize) -> Self {
        let extra = EOFExtraData::from_type::<T>(read_len);
        EOFErrorData {
            pos,
            extra: Box::new(extra),
        }
    }

    pub fn analytics<'a>(&'a self, data: &'a [u8]) -> PacketAnalyzer<'a> {
        PacketAnalyzer::new(self, data)
    }

    #[cfg(not(feature = "eof_ext"))]
    pub fn read_len(&self) -> usize {
        // Holy number for context
        4
    }

    #[cfg(feature = "eof_ext")]
    pub fn read_len(&self) -> usize {
        self.extra.read_len
    }

    #[cfg(not(feature = "eof_ext"))]
    pub fn type_name(&self) -> &str {
        "unknown"
    }

    #[cfg(feature = "eof_ext")]
    pub fn type_name(&self) -> &str {
        self.extra.type_name
    }
}

impl Display for EOFErrorData {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if cfg!(feature = "eof_ext") {
            write!(
                f,
                "eof packet(type={}): {}",
                self.extra.type_name, self.extra.read_len
            )
        } else {
            write!(f, "eof packet: {}", self.pos)
        }
    }
}

#[derive(Debug, Error)]
pub enum Error {
    #[error("string utf8 error")]
    StringUtf8(#[from] Utf8Error),
    #[error("EOF error: {0}")]
    EOF(EOFErrorData),
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
    InvalidAllBits,
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

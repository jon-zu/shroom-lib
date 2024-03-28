use std::{io, str::Utf8Error};

use shroom_pkt::Error;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum NetError {
    #[error("IO")]
    IO(#[from] io::Error),
    #[error("Websocket")]
    Websocket(#[from] tokio_websockets::Error),
    #[error("Packet")]
    Packet(#[from] Error),
    #[error("string utf8 error")]
    StringUtf8(#[from] Utf8Error),
    #[error("String limit {0} exceeed")]
    StringLimit(usize),
    #[error("invalid header")]
    InvalidHeader(#[from] shroom_crypto::net::header::InvalidHeaderError),
    #[error("Invalid enum discriminant {0}")]
    InvalidEnumDiscriminant(usize),
    #[error("Invalid enum primitive {0}")]
    InvalidEnumPrimitive(u32),
    #[error("Frame of length {0} is too large.")]
    FrameSize(usize),
    #[error("Handshake of length {0} is too large.")]
    HandshakeSize(usize),
    #[error("Unable to read handshake")]
    InvalidHandshake,
    #[error("Invalid AES key")]
    InvalidAESKey,
    #[error("Invalid timestamp: {0}")]
    InvalidTimestamp(i64),
    #[error("Invalid opcode: {0:X}")]
    InvalidOpCode(u16),
}

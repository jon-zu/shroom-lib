#![recursion_limit = "256"]

pub mod analyzer;
pub mod error;
pub mod pkt;
pub mod proto;
pub mod reader;
pub mod test_util;
pub mod util;
pub mod writer;
pub mod opcode;

pub use error::Error;
pub use util::SizeHint;

pub type PacketResult<T> = Result<T, error::Error>;

/// Export the reader and writer here
pub use reader::PacketReader;
pub use writer::PacketWriter;

// Re-export proto
pub use proto::*;

pub use opcode::ShroomOpCode;
pub use pkt::Packet;
pub use shroom_pkt_derive::*;

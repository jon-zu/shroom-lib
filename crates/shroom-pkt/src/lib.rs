#![recursion_limit = "256"]
#![allow(
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc
)]

pub mod analyzer;
pub mod error;
pub mod opcode;
pub mod pkt;
pub mod proto;
pub mod reader;
pub mod test_util;
pub mod util;
pub mod writer;

pub use error::Error;
pub use util::SizeHint;

pub type PacketResult<T> = Result<T, error::Error>;

/// Export the reader and writer here
pub use reader::PacketReader;
pub use writer::PacketWriter;

// Re-export proto
pub use proto::*;

pub use opcode::{HasOpCode, ShroomOpCode};
pub use pkt::Packet;
pub use shroom_pkt_derive::*;


#[doc(hidden)]
/// Default check function for the conditional proc macro
pub fn default_check(val: &bool) -> bool {
    *val
}

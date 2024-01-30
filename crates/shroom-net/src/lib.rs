pub mod codec;
pub mod error;
pub mod stream;

pub use error::NetError;
pub use shroom_crypto::{CryptoContext, SharedCryptoContext};
pub use shroom_pkt::Packet;
pub use stream::ShroomStream;

pub type NetResult<T> = Result<T, error::NetError>;

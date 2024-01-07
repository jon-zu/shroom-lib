pub mod codec;
pub mod stream;
pub mod error;

pub use shroom_pkt::Packet;
pub use error::NetError;
pub use stream::ShroomStream;
pub use shroom_crypto::{CryptoContext, SharedCryptoContext};

pub type NetResult<T> = Result<T, error::NetError>;

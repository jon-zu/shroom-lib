pub mod ig_cipher;
pub mod str;
pub mod version;

pub mod default_keys {
    pub const DEFAULT_IG_SHUFFLE: &[u8; 256] = include_bytes!("default_keys/ig_shuffle.bin");
    pub const DEFAULT_IG_SEED: u32 =
        u32::from_le_bytes(*include_bytes!("default_keys/ig_seed.bin"));

    pub mod net {
        pub const DEFAULT_AES_KEY: &[u8; crate::AES_KEY_LEN] =
            include_bytes!("default_keys/net/aes_key.bin");
    }

    pub mod wz {
        pub const DEFAULT_WZ_OFFSET_MAGIC: u32 =
            u32::from_be_bytes(*include_bytes!("default_keys/wz/offset_magic.bin"));
        pub const GLOBAL_WZ_IV: &[u8; 16] = include_bytes!("default_keys/wz/global_iv.bin");
        pub const SEA_WZ_IV: &[u8; 16] = include_bytes!("default_keys/wz/sea_iv.bin");
        pub const DEFAULT_WZ_IV: &[u8; 16] = include_bytes!("default_keys/wz/default_iv.bin");
        pub const DEFAULT_WZ_AES_KEY: &[u8; crate::AES_KEY_LEN] =
            include_bytes!("default_keys/wz/aes_key.bin");
    }
}

pub mod net {
    pub mod header;
    pub mod net_cipher;
    pub mod packet_cipher;
    pub mod round_key;
    pub mod shanda_cipher;
}

pub mod wz {
    pub mod wz_data_cipher;
    pub mod wz_offset_cipher;
}

// Re-exports
pub use ig_cipher::IgCipher;
pub use net::packet_cipher::ShroomPacketCipher;
pub use net::round_key::RoundKey;
pub use net::shanda_cipher::ShandaCipher;
pub use version::ShroomVersion;

use std::sync::Arc;

use self::ig_cipher::{IgContext, DEFAULT_IG_CONTEXT};

pub const ROUND_KEY_LEN: usize = 4;
pub const AES_KEY_LEN: usize = 32;
pub const AES_BLOCK_LEN: usize = 16;
pub const PACKET_HEADER_LEN: usize = 4;

pub type AesKey = [u8; AES_KEY_LEN];
pub type ShuffleKey = [u8; 256];
pub type PacketHeader = [u8; PACKET_HEADER_LEN];

pub type SharedIgContext = Arc<IgContext>;

/// Crypto Context providing all keys for this crypto
/// Should be used via `SharedCryptoContext` to avoid
/// re-allocating this for every crypto
#[derive(Debug)]
pub struct CryptoContext {
    pub aes_key: AesKey,
    pub ig_ctx: IgContext,
}

impl Default for CryptoContext {
    fn default() -> Self {
        Self {
            aes_key: *default_keys::net::DEFAULT_AES_KEY,
            ig_ctx: DEFAULT_IG_CONTEXT,
        }
    }
}

/// Alias for a shared context
pub type SharedCryptoContext = Arc<CryptoContext>;

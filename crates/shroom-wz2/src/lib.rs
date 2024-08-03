#![allow(
    clippy::must_use_candidate,
    clippy::cast_possible_truncation,
    clippy::module_name_repetitions,
    clippy::missing_errors_doc,
    clippy::missing_panics_doc,
    clippy::cast_sign_loss,
    clippy::cast_possible_wrap
)]

pub mod canvas;
pub mod crypto;
pub mod ctx;
pub mod file;
pub mod l0;
pub mod l1;
pub mod ty;
pub mod util;
//pub mod val;
//pub mod version;

use crypto::WzCryptoContext;
#[cfg(feature = "mmap")]
pub use file::mmap::{WzReaderMmap, WzReaderSharedMmap};
pub use file::WzArchiveReader;

use shroom_crypto::ShroomVersion;

#[derive(Debug, Clone, Copy)]
pub enum WzRegion {
    Global,
    Sea,
    Other,
    Server,
}

impl From<WzRegion> for WzCryptoContext {
    fn from(region: WzRegion) -> Self {
        use shroom_crypto::default_keys::wz;
        match region {
            WzRegion::Global => Self {
                initial_iv: *wz::GLOBAL_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: false,
            },
            WzRegion::Sea => Self {
                initial_iv: *wz::SEA_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: false,
            },
            WzRegion::Other => Self {
                initial_iv: *wz::DEFAULT_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: false,
            },
            WzRegion::Server => Self {
                initial_iv: *wz::DEFAULT_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: true,
            },
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WzConfig {
    pub(crate) region: WzRegion,
    pub(crate) version: ShroomVersion,
}

impl WzConfig {
    /// Creates a config with the given region and version
    #[must_use]
    pub const fn new(region: WzRegion, version: ShroomVersion) -> Self {
        Self { region, version }
    }

    /// Creates a config for the global region
    #[must_use]
    pub const fn global(version: ShroomVersion) -> Self {
        Self {
            region: WzRegion::Global,
            version,
        }
    }
}

pub const GMS95: WzConfig = WzConfig::global(ShroomVersion::new(95));

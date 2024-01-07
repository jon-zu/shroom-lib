use std::num::Wrapping;

use aes::cipher::{inout::InOutBuf, KeyIvInit};
use shroom_crypto::{
    wz::{wz_data_cipher::WzDataCipher, wz_offset_cipher::WzOffsetCipher},
    ShroomVersion,
};

use crate::{WzConfig, WzRegion};

#[derive(Debug)]
pub struct WzCryptoContext {
    pub initial_iv: [u8; 16],
    pub key: [u8; 32],
    pub offset_magic: u32,
    pub no_crypto: bool,
}

impl From<WzRegion> for WzCryptoContext {
    fn from(region: WzRegion) -> Self {
        use shroom_crypto::default_keys::wz;
        match region {
            WzRegion::Global => Self {
                initial_iv: *wz::GLOBAL_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: false
            },
            WzRegion::Sea => Self {
                initial_iv: *wz::SEA_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: false
            },
            WzRegion::Other => Self {
                initial_iv: *wz::DEFAULT_WZ_IV,
                key: *wz::DEFAULT_WZ_AES_KEY,
                offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                no_crypto: false
            },
            WzRegion::Server => {
                Self {
                    initial_iv: *wz::DEFAULT_WZ_IV,
                    key: *wz::DEFAULT_WZ_AES_KEY,
                    offset_magic: wz::DEFAULT_WZ_OFFSET_MAGIC,
                    no_crypto: true
                }
            }
        }
    }
}

fn xor_mask_ascii(data: &mut [u8]) {
    let mut mask = Wrapping(0xAAu8);
    for b in data.iter_mut() {
        *b ^= mask.0;
        mask += 1;
    }
}

fn xor_mask_unicode(data: &mut [u16]) {
    let mut mask = Wrapping(0xAAAA);
    for b in data.iter_mut() {
        *b ^= mask.0;
        mask += 1;
    }
}

#[derive(Clone)]
pub struct WzCipher {
    cipher: WzDataCipher,
    offset_cipher: WzOffsetCipher,
    data_offset: u32,
    no_crypt: bool,
}

impl std::fmt::Debug for WzCipher {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WzCrypto")
            .field("data_offset", &self.data_offset)
            .field("no_crypt", &self.no_crypt)
            .finish()
    }
}

impl WzCipher {
    pub fn new(
        ctx: &WzCryptoContext,
        version: ShroomVersion,
        data_offset: u32,
    ) -> Self {
        Self {
            cipher: WzDataCipher::new(&ctx.key.into(), &ctx.initial_iv.into()),
            offset_cipher: WzOffsetCipher::new(ctx.offset_magic, version.wz_hash()),
            data_offset,
            no_crypt: ctx.no_crypto,
        }
    }

    pub fn from_cfg(cfg: WzConfig, data_offset: u32) -> Self {
        Self::new(
            &cfg.region.into(),
            cfg.version,
            data_offset,
        )
    }

    pub fn crypt_inout(&self, buf: InOutBuf<u8>) {
        if self.no_crypt {
            return;
        }

        self.cipher.crypt_inout(buf)
    }

    pub fn crypt(&self, buf: &mut [u8]) {
        self.crypt_inout(buf.into())
    }

    pub fn decode_str8(&self, buf: &mut [u8]) {
        xor_mask_ascii(buf);
        self.crypt(buf);
    }

    pub fn encode_str8(&self, buf: &mut [u8]) {
        self.crypt(buf);
        xor_mask_ascii(buf);
    }

    pub fn decode_str16(&self, buf: &mut [u16]) {
        xor_mask_unicode(buf);
        self.crypt(bytemuck::cast_slice_mut(buf));
    }

    pub fn encode_str16(&self, buf: &mut [u16]) {
        self.crypt(bytemuck::cast_slice_mut(buf));
        xor_mask_unicode(buf);
    }

    pub fn decrypt_offset(&self, enc_off: u32, pos: u32) -> u32 {
        self.offset_cipher
            .decrypt_offset(self.data_offset, enc_off, pos)
    }

    pub fn encrypt_offset(&self, off: u32, pos: u32) -> u32 {
        self.offset_cipher
            .encrypt_offset(self.data_offset, off, pos)
    }

    pub fn offset_link(&self, off: u32) -> u64 {
        self.data_offset as u64 + off as u64
    }
}

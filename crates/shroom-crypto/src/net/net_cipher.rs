use cipher::{inout::InOutBuf, KeyIvInit, StreamCipher};

use crate::{
    PacketHeader, RoundKey, ShandaCipher, SharedCryptoContext, ShroomPacketCipher, ShroomVersion,
};

use super::header;

pub const CRYPT_NONE: u8 = 0;
pub const CRYPT_SHANDA: u8 = 1;
pub const CRYPT_AES: u8 = 2;
pub const CRYPT_ALL: u8 = CRYPT_SHANDA | CRYPT_AES;

#[derive(Clone)]
pub struct NetCipher<const CRYPT: u8 = CRYPT_ALL> {
    cipher: ShroomPacketCipher,
    ctx: SharedCryptoContext,
    version: ShroomVersion,
}

impl<const CRYPT: u8> NetCipher<CRYPT> {
    /// Creates a new crypto used en/decoding packets
    /// with the given context, initial `RoundKey`and version
    pub fn new(ctx: SharedCryptoContext, round_key: RoundKey, version: ShroomVersion) -> Self {
        Self {
            cipher: ShroomPacketCipher::new(&ctx.aes_key.into(), &round_key.expand()),
            ctx,
            version,
        }
    }

    /// Decodes and verifies a header from the given bytes
    pub fn encode_header(&self, length: u16) -> PacketHeader {
        if CRYPT & CRYPT_AES == 0 {
            return header::encode_header_no_crypt(length);
        }
        header::encode_header(self.cipher.round_key(), length, self.version.raw())
    }

    /// Decodes and verifies a header from the given bytes
    pub fn decode_header(&self, hdr: PacketHeader) -> Result<u16, header::InvalidHeaderError> {
        if CRYPT & CRYPT_AES == 0 {
            return Ok(header::decode_header_no_crypt(hdr));
        }
        header::decode_header(hdr, self.cipher.round_key(), self.version.raw())
    }

    /// Decrypt a block of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn encrypt_inout(&mut self, mut data: InOutBuf<u8>) {
        if CRYPT & CRYPT_SHANDA != 0 {
            ShandaCipher::encrypt_inout(data.reborrow());
        }
        if CRYPT & CRYPT_AES != 0 {
            self.cipher.apply_keystream_inout(data);
            self.cipher.update_round_key_ig(&self.ctx.ig_ctx);
        }
    }

    /// Encrypts a block of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn encrypt(&mut self, data: &mut [u8]) {
        self.encrypt_inout(data.into());
    }

    /// Decrypts a chunk of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn decrypt_inout(&mut self, mut data: InOutBuf<u8>) {
        if CRYPT & CRYPT_AES != 0 {
            self.cipher.apply_keystream_inout(data.reborrow());
            self.cipher.update_round_key_ig(&self.ctx.ig_ctx);
        }
        if CRYPT & CRYPT_SHANDA != 0 {
            ShandaCipher::decrypt_inout(data);
        }
    }

    /// Decrypts a chunk of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn decrypt(&mut self, data: &mut [u8]) {
        self.decrypt_inout(data.into());
    }
}

#[cfg(test)]
mod tests {
    use crate::{net::net_cipher::{NetCipher, CRYPT_ALL}, RoundKey};

    use super::{SharedCryptoContext, ShroomVersion};
    const V: ShroomVersion = ShroomVersion::new(95);
    #[test]
    fn en_dec() {
        let key = RoundKey::from([1, 2, 3, 4]);

        let mut enc = NetCipher::<CRYPT_ALL>::new(SharedCryptoContext::default(), key, V);
        let mut dec = NetCipher::<CRYPT_ALL>::new(SharedCryptoContext::default(), key, V);
        let data = b"abcdef";

        let mut data_enc = *data;
        enc.encrypt(data_enc.as_mut_slice());
        dec.decrypt(data_enc.as_mut_slice());

        assert_eq!(*data, data_enc);
        assert_eq!(enc.cipher.round_key(), dec.cipher.round_key());
    }

    #[test]
    fn en_dec_100() {
        let key = RoundKey::from([1, 2, 3, 4]);

        let mut enc = NetCipher::<CRYPT_ALL>::new(SharedCryptoContext::default(), key, V);
        let mut dec = NetCipher::<CRYPT_ALL>::new(SharedCryptoContext::default(), key, V);
        let data = b"abcdef".to_vec();

        for _ in 0..100 {
            let mut data_enc = data.clone();
            enc.encrypt(data_enc.as_mut_slice());
            dec.decrypt(data_enc.as_mut_slice());

            assert_eq!(*data, data_enc);
        }
    }
}

use cipher::{inout::InOutBuf, KeyIvInit, StreamCipher};

use crate::{
    PacketHeader, RoundKey, ShandaCipher, SharedCryptoContext, ShroomPacketCipher, ShroomVersion,
};

use super::header;

#[derive(Clone)]
pub struct NetCipher<const SHANDA: bool = true> {
    cipher: ShroomPacketCipher,
    ctx: SharedCryptoContext,
    version: ShroomVersion,
}

impl<const SHANDA: bool> NetCipher<SHANDA> {
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
        header::encode_header(self.cipher.round_key(), length, self.version.raw())
    }

    /// Decodes and verifies a header from the given bytes
    pub fn decode_header(&self, hdr: PacketHeader) -> Result<u16, header::InvalidHeaderError> {
        header::decode_header(hdr, self.cipher.round_key(), self.version.raw())
    }

    /// Decrypt a block of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn encrypt_inout(&mut self, mut data: InOutBuf<u8>) {
        if SHANDA {
            ShandaCipher::encrypt_inout(data.reborrow());
        }
        self.cipher.apply_keystream_inout(data);
        self.cipher.update_round_key_ig(&self.ctx.ig_ctx);
    }

    /// Encrypts a block of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn encrypt(&mut self, data: &mut [u8]) {
        self.encrypt_inout(data.into());
    }

    /// Decrypts a chunk of data
    /// IMPORTANT: only call this with a full block of data, because of the internal state updates
    pub fn decrypt_inout(&mut self, mut data: InOutBuf<u8>) {
        self.cipher.apply_keystream_inout(data.reborrow());
        self.cipher.update_round_key_ig(&self.ctx.ig_ctx);
        if SHANDA {
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
    use crate::{net::net_cipher::NetCipher, RoundKey};

    use super::{SharedCryptoContext, ShroomVersion};
    const V: ShroomVersion = ShroomVersion::new(95);
    #[test]
    fn en_dec() {
        let key = RoundKey::from([1, 2, 3, 4]);

        let mut enc = NetCipher::<true>::new(SharedCryptoContext::default(), key, V);
        let mut dec = NetCipher::<true>::new(SharedCryptoContext::default(), key, V);
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

        let mut enc = NetCipher::<true>::new(SharedCryptoContext::default(), key, V);
        let mut dec = NetCipher::<true>::new(SharedCryptoContext::default(), key, V);
        let data = b"abcdef".to_vec();

        for _ in 0..100 {
            let mut data_enc = data.clone();
            enc.encrypt(data_enc.as_mut_slice());
            dec.decrypt(data_enc.as_mut_slice());

            assert_eq!(*data, data_enc);
        }
    }
}

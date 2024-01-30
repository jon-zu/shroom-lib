use std::ops::Add;

use aes::Aes256;
use cipher::{
    generic_array::GenericArray,
    inout::InOutBuf,
    typenum::{U1000, U16, U32, U460},
    AlgorithmName, InnerIvInit, IvSizeUser, KeyInit, KeyIvInit, KeySizeUser, StreamCipher,
};
use ofb::OfbCore;

use crate::{default_keys::net::DEFAULT_AES_KEY, ig_cipher::IgContext, RoundKey};

type Aes256Ofb<'a> = ofb::Ofb<&'a aes::Aes256>;

const BLOCK_LEN: usize = 1460;
const FIRST_BLOCK_LEN: usize = BLOCK_LEN - 4;

type InnerBlockLen = <U1000 as Add<U460>>::Output;

#[derive(Clone)]
pub struct ShroomPacketCipher {
    aes: Aes256,
    iv: GenericArray<u8, U16>,
}

impl Default for ShroomPacketCipher {
    fn default() -> Self {
        Self::new(DEFAULT_AES_KEY.into(), &RoundKey::zero().expand())
    }
}

impl AlgorithmName for ShroomPacketCipher {
    fn write_alg_name(f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str("ShroomPacketCipher<")?;
        OfbCore::<aes::Aes256>::write_alg_name(f)?;
        f.write_str(">")
    }
}

impl KeySizeUser for ShroomPacketCipher {
    type KeySize = U32;
}

impl IvSizeUser for ShroomPacketCipher {
    type IvSize = U16;
}

impl KeyIvInit for ShroomPacketCipher {
    fn new(key: &GenericArray<u8, Self::KeySize>, iv: &GenericArray<u8, Self::IvSize>) -> Self {
        Self {
            aes: <Aes256 as KeyInit>::new(key),
            iv: *iv,
        }
    }
}

impl From<RoundKey> for ShroomPacketCipher {
    fn from(value: RoundKey) -> Self {
        Self::new(DEFAULT_AES_KEY.into(), &value.expand())
    }
}

impl StreamCipher for ShroomPacketCipher {
    fn try_apply_keystream_inout(
        &mut self,
        buf: InOutBuf<'_, '_, u8>,
    ) -> Result<(), cipher::StreamCipherError> {
        let mut ofb = Aes256Ofb::from_core(ofb::OfbCore::inner_iv_init(&self.aes, &self.iv));

        // Fast path for small packets
        if buf.len() < FIRST_BLOCK_LEN {
            ofb.apply_keystream_inout(buf);
            return Ok(());
        }

        // De crypt first block
        let (first_block, buf) = buf.split_at(FIRST_BLOCK_LEN);
        ofb.clone().apply_keystream_inout(first_block);

        // Decrypt inner blocks
        let (blocks, tail_block) = buf.into_chunks::<InnerBlockLen>();
        for block in blocks {
            ofb.clone().apply_keystream_inout(block.into_buf());
        }

        // Decrypt tail
        if !tail_block.is_empty() {
            ofb.apply_keystream_inout(tail_block);
        }
        Ok(())
    }
}

impl ShroomPacketCipher {
    /// Updates the current round key
    pub fn update_round_key<F: FnOnce(RoundKey) -> RoundKey>(&mut self, f: F) {
        self.iv = f(self.round_key()).expand();
    }

    /// Updates the current round key
    pub fn update_round_key_ig(&mut self, ig_ctx: &IgContext) {
        self.update_round_key(|rk| rk.update(ig_ctx))
    }

    /// Gets a copy of the current round key
    pub fn round_key(&self) -> RoundKey {
        self.iv.into()
    }
}

#[cfg(test)]
mod tests {
    use cipher::StreamCipher;

    use super::ShroomPacketCipher;

    fn enc_dec(cipher: &mut ShroomPacketCipher, data: &mut [u8]) {
        cipher.apply_keystream(data);
        cipher.apply_keystream(data);
    }

    #[test]
    fn en_dec_aes() {
        let mut aes = ShroomPacketCipher::default();
        let data = b"abcdef";

        let mut data_enc = *data;
        enc_dec(&mut aes, data_enc.as_mut());
        assert_eq!(*data, data_enc);
    }

    #[test]
    fn en_dec_aes2() {
        let mut aes = ShroomPacketCipher::default();
        let data = &[1, 2, 3, 4, 5, 6];

        let mut data_enc = *data;
        enc_dec(&mut aes, data_enc.as_mut());
        assert_eq!(*data, data_enc);
    }

    #[test]
    fn en_dec_aes3() {
        let mut aes = ShroomPacketCipher::default();
        let data = &[0u8; 4096];

        let mut data_enc = *data;
        enc_dec(&mut aes, data_enc.as_mut());
        assert_eq!(*data, data_enc);
    }
}

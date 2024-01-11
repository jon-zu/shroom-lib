use aes::Aes256;
use cipher::{
    generic_array::GenericArray,
    inout::InOutBuf,
    typenum::{U16, U32},
    InnerIvInit, IvSizeUser, IvState, KeyInit, KeyIvInit, KeySizeUser, StreamCipher,
};

type Aes256Ofb<'a> = ofb::Ofb<&'a aes::Aes256>;

pub const DEFAULT_WZ_CIPHER_CACHE: usize = 4096 / 16;

#[derive(Clone)]
/// Cipher used to encrypt data in wz files
/// This is a wrapper around a AES256-OFB cipher, but It
/// pre-caches N U16 blocks to accelerate the encryption
pub struct WzDataCipher<const N: usize = DEFAULT_WZ_CIPHER_CACHE> {
    aes: Aes256,
    iv: GenericArray<u8, U16>,
    cached_key: [[u8; 16]; N],
}

impl<const N: usize> KeySizeUser for WzDataCipher<N> {
    type KeySize = U32;
}

impl<const N: usize> IvSizeUser for WzDataCipher<N> {
    type IvSize = U16;
}

impl<const N: usize> KeyIvInit for WzDataCipher<N> {
    fn new(key: &GenericArray<u8, Self::KeySize>, iv: &GenericArray<u8, Self::IvSize>) -> Self {
        let aes = Aes256::new(key);
        let mut cache_ofb = Aes256Ofb::from_core(ofb::OfbCore::inner_iv_init(&aes, iv));
        let cached_key = Self::calc_cache_key(&mut cache_ofb);
        let iv = cache_ofb.get_core().iv_state();

        Self {
            aes,
            iv,
            cached_key,
        }
    }
}

impl<const N: usize> WzDataCipher<N> {
    /// Precache the key, since ofb uses xor
    /// we can just xor It with a 0 block to get the key
    fn calc_cache_key(ofb: &mut Aes256Ofb) -> [[u8; 16]; N] {
        let mut cache = [[0; 16]; N];
        for cache in cache.iter_mut().take(N) {
            ofb.apply_keystream(cache);
        }
        cache
    }

    /// Crypts all given blocks with the cache
    /// It will panic If the given buffer is larger than N * 16
    fn crypt_cached(&self, buf: InOutBuf<'_, '_, u8>) {
        let (blocks, mut tail) = buf.into_chunks::<U16>();
        let n = blocks.len();
        for (ix, mut block) in blocks.into_iter().enumerate() {
            block.xor_in2out(&self.cached_key[ix].into());
        }

        if !tail.is_empty() {
            tail.xor_in2out(&self.cached_key[n][..tail.len()]);
        }
    }

    /// Crypts all given blocks with the cache
    /// It will panic If the given buffer is larger than N * 16
    fn crypt_cached_at(&self, mut buf: InOutBuf<'_, '_, u8>, at: usize) {
        let key_slice: &[u8] = bytemuck::cast_slice(&self.cached_key);
        buf.xor_in2out(&key_slice[at..at + buf.len()]);
    }

    /// Crypts an in out buffer
    pub fn crypt_inout(&self, buf: InOutBuf<u8>) {
        // If the buffer is smaller use the cache
        if buf.len() <= N * 16 {
            self.crypt_cached(buf);
            return;
        }

        // Else split at N*16, use the cache and then re-use the cipher
        let (first, second) = buf.split_at(N * 16);
        self.crypt_cached(first);

        // Use the cipher again
        Aes256Ofb::from_core(ofb::OfbCore::inner_iv_init(&self.aes, &self.iv))
            .apply_keystream_inout(second);
    }

    /// Crypts a slice
    pub fn crypt(&self, buf: &mut [u8]) {
        self.crypt_inout(buf.into());
    }

    /// Creates a data crypt stream
    pub fn stream(&self) -> WzDataCryptStream<'_, N> {
        WzDataCryptStream::new(self)
    }
}

pub struct WzDataCryptStream<'a, const N: usize = DEFAULT_WZ_CIPHER_CACHE> {
    cipher: &'a WzDataCipher<N>,
    ix: usize,
    ofb: Aes256Ofb<'a>,
}

impl<'a, const N: usize> WzDataCryptStream<'a, N> {
    fn new(cipher: &'a WzDataCipher<N>) -> Self {
        Self {
            cipher,
            ix: 0,
            ofb: Aes256Ofb::from_core(ofb::OfbCore::inner_iv_init(&cipher.aes, &cipher.iv)),
        }
    }

    /// Resets the stream
    pub fn reset(&mut self) {
        self.ix = 0;
        self.ofb = Aes256Ofb::from_core(ofb::OfbCore::inner_iv_init(&self.cipher.aes, &self.cipher.iv));
    }

    /// Crypts an in out buffer
    pub fn crypt_inout(&mut self, mut buf: InOutBuf<u8>) {
        let c = N * 16;
        let n = buf.len();
        // Crypt first bytes with the cache
        if self.ix < c {
            // We crypt at most N bytes total with the cache
            let first = n.min(c - self.ix);
            let (block, rest) = buf.split_at(first);
            buf = rest;

            // Crypt at ix
            self.cipher.crypt_cached_at(block, self.ix);
            self.ix += first;
        }

        // Crypt the rest with the cipher
        let n = buf.len();
        self.ofb.apply_keystream_inout(buf);
        self.ix += n;
    }

    /// Crypts a slice
    pub fn crypt(&mut self, buf: &mut [u8]) {
        self.crypt_inout(buf.into());
    }
}

#[cfg(test)]
mod tests {
    use crate::default_keys::wz::{DEFAULT_WZ_AES_KEY, DEFAULT_WZ_IV};

    use super::*;

    const N: usize = 256;
    const BLOCKS: usize = N / 16;

    fn en_de_crypt(cipher: &WzDataCipher<BLOCKS>, data: &mut [u8]) {
        let old = data.to_vec();
        cipher.crypt(data);
        cipher.crypt(data);
        assert_eq!(data, old);
    }

    #[test]
    fn wz_crypt() {
        let wz_cipher =
            WzDataCipher::<BLOCKS>::new(DEFAULT_WZ_AES_KEY.into(), DEFAULT_WZ_IV.into());

        for i in 0..=N + 1 {
            let mut data = vec![1; i];
            en_de_crypt(&wz_cipher, &mut data);
        }

        const LARGE: usize = N * 16;
        for i in [LARGE - 1, LARGE, LARGE + 1] {
            let mut data = vec![1; i];
            en_de_crypt(&wz_cipher, &mut data);
        }
    }

    #[test]
    fn wz_crypt_stream() {
        let wz_cipher =
            WzDataCipher::<BLOCKS>::new(DEFAULT_WZ_AES_KEY.into(), DEFAULT_WZ_IV.into());

        const M: usize = N * 2;

        for n in [0, 1, 2, 3, M, M-1, M, M+1] {
            // Crypt in one pass
            let mut data = vec![1; n];
            wz_cipher.crypt(&mut data);

            // Crypt N bytes one by one
            let mut stream_data = vec![1; n];
            let mut stream = wz_cipher.stream();
            for i in 0..n {
                stream.crypt(&mut stream_data[i..i + 1]);
            }

            assert_eq!(data, stream_data, "{n}");
        }
    }
}

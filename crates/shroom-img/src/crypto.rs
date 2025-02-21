use std::{io, ops::BitXorAssign};

use aes::cipher::{KeyIvInit, inout::InOutBuf};
use shroom_crypto::{
    default_keys,
    wz::data_cipher::{WzDataCipher, WzDataCryptStream},
};

use crate::util::array_chunks::as_chunks;

pub struct XorMask<T>(T);

pub trait XorMaskAble:
    BitXorAssign<Self> + Copy + Clone + Sized + bytemuck::AnyBitPattern + bytemuck::NoUninit
{
    const INITIAL: Self;
    const ZERO: Self;

    fn next(self) -> Self;

    fn append_chunk<const N: usize>(s: &mut String, chunk: &[Self]) -> io::Result<()>;
    fn append_slice(s: &mut String, slice: &[Self]) -> io::Result<()>;
}

fn char_decode_latin1(b: u8) -> char {
    // UTF8/ISO 8859-1 is a superset of latin1
    b as char
}

impl XorMaskAble for u8 {
    const INITIAL: Self = 0xAA;
    const ZERO: Self = 0;

    fn next(self) -> Self {
        self.wrapping_add(1)
    }

    fn append_chunk<const N: usize>(s: &mut String, chunk: &[Self]) -> io::Result<()> {
        s.extend(chunk.iter().copied().map(char_decode_latin1));
        Ok(())
    }

    fn append_slice(s: &mut String, slice: &[Self]) -> io::Result<()> {
        s.extend(slice.iter().copied().map(char_decode_latin1));
        Ok(())
    }
}

impl XorMaskAble for u16 {
    const INITIAL: Self = 0xAAAA;
    const ZERO: Self = 0;

    fn next(self) -> Self {
        self.wrapping_add(1)
    }

    fn append_chunk<const N: usize>(s: &mut String, chunk: &[Self]) -> io::Result<()> {
        s.reserve(N);

        for c in char::decode_utf16(chunk.iter().copied()) {
            let c = c.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            s.push(c);
        }

        Ok(())
    }

    fn append_slice(s: &mut String, slice: &[Self]) -> io::Result<()> {
        s.reserve(slice.len());

        for c in char::decode_utf16(slice.iter().copied()) {
            let c = c.map_err(|e| io::Error::new(io::ErrorKind::InvalidData, e))?;
            s.push(c);
        }

        Ok(())
    }
}

impl<T: XorMaskAble> XorMask<T> {
    pub fn new() -> Self {
        Self(T::INITIAL)
    }
    pub fn apply(&mut self, data: &mut [T]) {
        for b in data.iter_mut() {
            *b ^= self.0;
            self.0 = self.0.next();
        }
    }
}

#[derive(Clone)]
pub struct ImgCrypto {
    cipher: Option<WzDataCipher>,
    chunked_cipher: Option<WzDataCipher>,
}

impl std::fmt::Debug for ImgCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WzCrypto")
            .field("no_crypt", &self.cipher.is_none())
            .finish_non_exhaustive()
    }
}

impl ImgCrypto {
    pub fn new(cipher: WzDataCipher) -> Self {
        Self {
            cipher: Some(cipher),
            chunked_cipher: None,
        }
    }

    pub fn with_chunked(cipher: WzDataCipher, chunked_cipher: WzDataCipher) -> Self {
        Self {
            cipher: Some(cipher),
            chunked_cipher: Some(chunked_cipher),
        }
    }


    pub fn default_shroom() -> Self {
        Self::new(
            WzDataCipher::new_from_slices(
                default_keys::wz::DEFAULT_WZ_AES_KEY,
                default_keys::wz::DEFAULT_WZ_IV,
            )
            .unwrap(),
        )
    }

    pub fn kms() -> Self {
        Self::with_chunked(
            WzDataCipher::from_iv(&[
                0x45, 0x50, 0x33, 0x01, 0x45, 0x50, 0x33, 0x01, 0x45, 0x50, 0x33, 0x01, 0x45, 0x50,
                0x33, 0x01,
            ]),
            WzDataCipher::europe(),
        )
    }

    pub fn europe() -> Self {
        Self::new(WzDataCipher::europe())
    }

    pub fn global() -> Self {
        Self::new(WzDataCipher::global())
    }

    pub fn none() -> Self {
        Self {
            cipher: None,
            chunked_cipher: None,
        }
    }


    pub fn chunked_cipher(&self) -> Option<&WzDataCipher> {
        self.chunked_cipher
            .as_ref()
            .or_else(|| self.cipher.as_ref())
    }

    pub fn crypt_stream(&self) -> Option<WzDataCryptStream<'_>> {
        self.cipher.as_ref().map(|c| c.stream())
    }

    pub fn chunked_crypt_stream(&self) -> Option<WzDataCryptStream<'_>> {
        self.chunked_cipher().map(|c| c.stream())
    }

    pub fn crypt_inout(&self, buf: InOutBuf<u8>) {
        if let Some(cipher) = &self.cipher {
            cipher.crypt_inout(buf);
        }
    }

    pub fn crypt(&self, buf: &mut [u8]) {
        self.crypt_inout(buf.into());
    }

    fn read_str_inner<T: XorMaskAble, R: io::Read>(
        &self,
        mut r: R,
        len: usize,
    ) -> io::Result<String> {
        const CHUNK_LEN: usize = 16;
        let mut res = String::with_capacity(len);
        let chunks = len / CHUNK_LEN;
        let tail = len % CHUNK_LEN;
        let mut crypt = self.crypt_stream().unwrap();
        let mut xor = XorMask::<T>::new();

        // Read each chunk
        for _ in 0..chunks {
            let mut chunk = [T::ZERO; CHUNK_LEN];
            r.read_exact(bytemuck::cast_slice_mut(&mut chunk))?;
            xor.apply(&mut chunk);
            crypt.crypt(bytemuck::cast_slice_mut(&mut chunk));

            T::append_chunk::<CHUNK_LEN>(&mut res, &chunk)?;
        }

        if tail > 0 {
            let mut chunk = [T::ZERO; CHUNK_LEN];
            r.read_exact(bytemuck::cast_slice_mut(&mut chunk[..tail]))?;
            xor.apply(chunk.as_mut_slice());
            let chunk = &mut chunk[..tail];
            crypt.crypt(bytemuck::cast_slice_mut(chunk));

            T::append_slice(&mut res, chunk)?;
        }

        Ok(res)
    }

    fn write_str_inner<T: XorMaskAble, W: io::Write>(&self, mut w: W, buf: &[T]) -> io::Result<()> {
        let Some(mut crypt) = self.crypt_stream() else {
            return w.write_all(bytemuck::cast_slice(buf));
        };

        const CHUNK_LEN: usize = 16;
        let mut xor = XorMask::<T>::new();
        let (chunks, tail) = as_chunks::<CHUNK_LEN, T>(buf);

        // Write chunks
        for chunk in chunks {
            let mut chunk = *chunk;
            crypt.crypt(bytemuck::cast_slice_mut(&mut chunk));
            xor.apply(&mut chunk);
            w.write_all(bytemuck::cast_slice(&chunk))?;
        }

        // Write the tail block
        let mut chunk = [T::ZERO; CHUNK_LEN];
        let n = tail.len();
        chunk[..n].copy_from_slice(tail);
        let chunk = &mut chunk[..n];
        crypt.crypt(bytemuck::cast_slice_mut(chunk));
        xor.apply(chunk);
        w.write_all(bytemuck::cast_slice(chunk))?;

        Ok(())
    }

    fn decode_str_inner<T: XorMaskAble>(&self, buf: &mut [T]) {
        XorMask::<T>::new().apply(buf);
        if let Some(cipher) = &self.cipher {
            cipher.crypt(bytemuck::cast_slice_mut(buf));
        }
    }

    fn encode_str_inner<T: XorMaskAble>(&self, buf: &mut [T]) {
        if let Some(cipher) = &self.cipher {
            cipher.crypt(bytemuck::cast_slice_mut(buf));
        }
        XorMask::<T>::new().apply(buf);
    }

    pub fn decode_str8(&self, buf: &mut [u8]) {
        self.decode_str_inner::<u8>(buf);
    }

    pub fn encode_str8(&self, buf: &mut [u8]) {
        self.encode_str_inner::<u8>(buf);
    }

    pub fn read_str8(&self, r: impl io::Read, len: usize) -> io::Result<String> {
        self.read_str_inner::<u8, _>(r, len)
    }

    pub fn write_str8(&self, w: impl io::Write, buf: &[u8]) -> io::Result<()> {
        self.write_str_inner::<u8, _>(w, buf)
    }

    pub fn decode_str16(&self, buf: &mut [u16]) {
        self.decode_str_inner::<u16>(buf);
    }

    pub fn encode_str16(&self, buf: &mut [u16]) {
        self.encode_str_inner::<u16>(buf);
    }

    pub fn read_str16(&self, r: impl io::Read, len: usize) -> io::Result<String> {
        self.read_str_inner::<u16, _>(r, len)
    }

    pub fn write_str16(&self, w: impl io::Write, s: &[u16]) -> io::Result<()> {
        self.write_str_inner::<u16, _>(w, s)
    }
}

#[cfg(test)]
mod tests {
    use std::{io::Cursor, iter};

    use super::*;

    #[test]
    fn crypto_string() {
        let s = [
            "",
            "a",
            "abc",
            &iter::once('😀').take(4096).collect::<String>(),
        ];
        let cipher = ImgCrypto::global();

        for s in s {
            // EncDec8 full
            let mut b = s.as_bytes().to_vec();
            cipher.encode_str8(&mut b);
            cipher.decode_str8(&mut b);
            assert_eq!(s.as_bytes(), b.as_slice());

            // EncDec 8 with chunked write
            let mut rw = Cursor::new(Vec::new());
            cipher.write_str8(&mut rw, s.as_bytes()).unwrap();

            let mut b = rw.into_inner();
            cipher.decode_str8(&mut b);
            assert_eq!(s.as_bytes(), b.as_slice());

            // EncDec16 full
            let mut b = s.encode_utf16().collect::<Vec<_>>();
            cipher.encode_str16(&mut b);
            cipher.decode_str16(&mut b);
            let b = String::from_utf16(&b).unwrap();
            assert_eq!(s, b);

            // EncDec16 with chunked write
            let b = s.encode_utf16().collect::<Vec<_>>();
            let mut rw: Cursor<Vec<u8>> = Cursor::new(Vec::new());
            cipher.write_str16(&mut rw, &b).unwrap();
            let mut b = rw.into_inner();

            let mut b = if b.is_empty() {
                [].as_mut_slice()
            } else {
                bytemuck::cast_slice_mut(&mut b)
            };
            cipher.decode_str16(&mut b);
            let b = String::from_utf16(&b).unwrap();
            assert_eq!(s, b);
        }
    }
}

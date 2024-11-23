use std::{char::DecodeUtf16Error, io, num::Wrapping};

use aes::cipher::{inout::InOutBuf, KeyIvInit};
use image::EncodableLayout;
use shroom_crypto::{
    default_keys,
    wz::data_cipher::{WzDataCipher, WzDataCryptStream},
};

use crate::util::array_chunks::as_chunks;

fn xor_mask_str8(data: &mut [u8]) {
    let mut mask = Wrapping(0xAAu8);
    for b in data.iter_mut() {
        *b ^= mask.0;
        mask += 1;
    }
}

pub struct Str8XorMaskIter(Wrapping<u8>);

impl Str8XorMaskIter {
    pub fn apply_array<const N: usize>(&mut self, data: &mut [u8; N]) {
        for b in data.iter_mut() {
            *b ^= self.0 .0;
            self.0 += 1;
        }
    }

    pub fn apply_slice(&mut self, data: &mut [u8]) {
        for b in data.iter_mut() {
            *b ^= self.0 .0;
            self.0 += 1;
        }
    }
}

impl Default for Str8XorMaskIter {
    fn default() -> Self {
        Self(Wrapping(0xAA))
    }
}

impl Iterator for Str8XorMaskIter {
    type Item = u8;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.0 .0;
        self.0 += 1;
        Some(ret)
    }
}

fn xor_mask_unicode(data: &mut [u16]) {
    let mut mask = Wrapping(0xAAAA);
    for b in data.iter_mut() {
        *b ^= mask.0;
        mask += 1;
    }
}

pub struct Str16XorMaskIter(Wrapping<u16>);

impl Str16XorMaskIter {
    pub fn apply_array<const N: usize>(&mut self, data: &mut [u16; N]) {
        for b in data.iter_mut() {
            *b ^= self.0 .0;
            self.0 += 1;
        }
    }

    pub fn apply_slice(&mut self, data: &mut [u16]) {
        for b in data.iter_mut() {
            *b ^= self.0 .0;
            self.0 += 1;
        }
    }
}

impl Default for Str16XorMaskIter {
    fn default() -> Self {
        Self(Wrapping(0xAAAA))
    }
}

impl Iterator for Str16XorMaskIter {
    type Item = u16;

    fn next(&mut self) -> Option<Self::Item> {
        let ret = self.0 .0;
        self.0 += 1;
        Some(ret)
    }
}

#[derive(Clone)]
pub struct ImgCrypto {
    cipher: Option<WzDataCipher>,
}

impl std::fmt::Debug for ImgCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WzCrypto")
            .field("no_crypt", &self.cipher.is_none())
            .finish_non_exhaustive()
    }
}

fn append_chunk_str16<const N: usize>(
    s: &mut String,
    v: &[u16; N],
) -> Result<(), DecodeUtf16Error> {
    s.reserve(N);

    for c in char::decode_utf16(v.iter().copied()) {
        s.push(c?);
    }

    Ok(())
}

fn append_slice_str16(s: &mut String, v: &[u16]) -> Result<(), DecodeUtf16Error> {
    s.reserve(v.len());

    for c in char::decode_utf16(v.iter().copied()) {
        s.push(c?);
    }

    Ok(())
}

fn char_decode_latin1(b: u8) -> char {
    // UTF8/ISO 8859-1 is a superset of latin1
    b as char
}

fn append_chunk_str8<const N: usize>(s: &mut String, src: &[u8; N]) -> anyhow::Result<()> {
    s.extend(src.iter().copied().map(char_decode_latin1));
    Ok(())
}

fn append_slice_str8(s: &mut String, src: &[u8]) {
    s.extend(src.iter().copied().map(char_decode_latin1));
}

impl ImgCrypto {
    pub fn new(cipher: Option<WzDataCipher>) -> Self {
        Self { cipher }
    }

    pub fn default_shroom() -> Self {
        Self::new(Some(
            WzDataCipher::new_from_slices(
                default_keys::wz::DEFAULT_WZ_AES_KEY,
                default_keys::wz::DEFAULT_WZ_IV,
            )
            .unwrap(),
        ))
    }

    pub fn kms() -> Self {
        Self::new(Some(
            WzDataCipher::new_from_slices(
                default_keys::wz::DEFAULT_WZ_AES_KEY,
                &[0x45,0x50,0x33,0x01,0x45,0x50,0x33,0x01,0x45,0x50,0x33,0x01,0x45,0x50,0x33,0x01]
            )
            .unwrap(),
        ))
    }

    pub fn europe() -> Self {
        Self::new(Some(
            WzDataCipher::new_from_slices(
                default_keys::wz::DEFAULT_WZ_AES_KEY,
                default_keys::wz::SEA_WZ_IV,
            )
            .unwrap(),
        ))
    }

    pub fn global() -> Self {
        Self::new(Some(
            WzDataCipher::new_from_slices(
                default_keys::wz::DEFAULT_WZ_AES_KEY,
                default_keys::wz::GLOBAL_WZ_IV,
            )
            .unwrap(),
        ))
    }

    pub fn none() -> Self {
        Self::new(None)
    }

    pub fn crypt_stream(&self) -> Option<WzDataCryptStream<'_>> {
        self.cipher.as_ref().map(|c| c.stream())
    }

    pub fn crypt_inout(&self, buf: InOutBuf<u8>) {
        if let Some(cipher) = &self.cipher {
            cipher.crypt_inout(buf);
        }
    }

    pub fn crypt(&self, buf: &mut [u8]) {
        self.crypt_inout(buf.into());
    }

    pub fn decode_str8(&self, buf: &mut [u8]) {
        xor_mask_str8(buf);
        if self.cipher.is_some() {
            self.crypt(buf);
        }
    }

    pub fn encode_str8_mut(&self, buf: &mut [u8]) {
        self.crypt(buf);
        xor_mask_str8(buf);
    }

    pub fn read_str8(&self, mut r: impl io::Read, len: usize) -> anyhow::Result<String> {
        const CHUNK_LEN: usize = 16;
        let mut res = String::with_capacity(len);
        let chunks = len / CHUNK_LEN;
        let tail = len % CHUNK_LEN;
        let mut crypt = self.crypt_stream().unwrap();
        let mut xor = Str8XorMaskIter::default();

        // Read each chunk
        for _ in 0..chunks {
            let mut chunk = [0; CHUNK_LEN];
            r.read_exact(bytemuck::cast_slice_mut(&mut chunk))?;
            xor.apply_array(&mut chunk);
            crypt.crypt(bytemuck::cast_slice_mut(&mut chunk));

            append_chunk_str8(&mut res, &chunk)?;
        }

        if tail > 0 {
            let mut chunk = [0; CHUNK_LEN];
            r.read_exact(bytemuck::cast_slice_mut(&mut chunk[..tail]))?;
            xor.apply_array(&mut chunk);
            let chunk = &mut chunk[..tail];
            crypt.crypt(bytemuck::cast_slice_mut(chunk));

            append_slice_str8(&mut res, chunk);
        }

        Ok(res)
    }

    pub fn write_str8(&self, mut w: impl io::Write, buf: &[u8]) -> io::Result<()> {
        let Some(mut crypt) = self.crypt_stream() else {
            return w.write_all(buf);
        };

        const CHUNK_LEN: usize = 16;
        let mut xor = Str8XorMaskIter::default();
        let (chunks, tail) = as_chunks::<CHUNK_LEN, u8>(buf);

        // Write chunks
        for chunk in chunks {
            let mut chunk = *chunk;
            crypt.crypt(bytemuck::cast_slice_mut(&mut chunk));
            xor.apply_array(&mut chunk);
            w.write_all(bytemuck::cast_slice(&chunk))?;
        }

        // Write the tail block
        let mut chunk = [0; CHUNK_LEN];
        let n = tail.len();
        chunk[..n].copy_from_slice(tail);
        let chunk = &mut chunk[..n];
        crypt.crypt(bytemuck::cast_slice_mut(chunk));
        xor.apply_slice(chunk);
        w.write_all(bytemuck::cast_slice(chunk))?;

        Ok(())
    }

    pub fn decode_str16(&self, buf: &mut [u16]) {
        if self.cipher.is_some() {
            xor_mask_unicode(buf);
            self.crypt(bytemuck::cast_slice_mut(buf));
        }
    }

    pub fn encode_str16_mut(&self, buf: &mut [u16]) {
        self.crypt(bytemuck::cast_slice_mut(buf));
        xor_mask_unicode(buf);
    }

    pub fn read_str16(&self, mut r: impl io::Read, len: usize) -> anyhow::Result<String> {
        const CHUNK_LEN: usize = 16;
        let mut res = String::with_capacity(len);
        let chunks = len / CHUNK_LEN;
        let tail = len % CHUNK_LEN;
        let mut crypt = self.crypt_stream().unwrap();
        let mut xor = Str16XorMaskIter::default();

        // Read each chunk
        for _ in 0..chunks {
            let mut chunk = [0; CHUNK_LEN];
            r.read_exact(bytemuck::cast_slice_mut(&mut chunk))?;
            xor.apply_array(&mut chunk);
            crypt.crypt(bytemuck::cast_slice_mut(&mut chunk));
            append_chunk_str16(&mut res, &chunk)?;
        }

        if tail > 0 {
            let mut chunk = [0; CHUNK_LEN];
            r.read_exact(bytemuck::cast_slice_mut(&mut chunk[..tail]))?;
            xor.apply_array(&mut chunk);
            let chunk = &mut chunk[..tail];
            crypt.crypt(bytemuck::cast_slice_mut(chunk));
            append_slice_str16(&mut res, chunk)?;
        }

        Ok(res)
    }

    pub fn write_str16(&self, mut w: impl io::Write, s: &[u16]) -> io::Result<()> {
        let Some(mut crypt) = self.crypt_stream() else {
            return w.write_all(s.as_bytes());
        };
        
        const CHUNK_LEN: usize = 16;
        let mut xor = Str16XorMaskIter::default();
        let (chunks, tail) = as_chunks::<CHUNK_LEN, u16>(s);

        // Write chunks
        for chunk in chunks {
            let mut chunk = *chunk;
            crypt.crypt(bytemuck::cast_slice_mut(&mut chunk));
            xor.apply_array(&mut chunk);
            w.write_all(bytemuck::cast_slice(&chunk))?;
        }

        // Write the tail block
        let mut chunk = [0; CHUNK_LEN];
        let n = tail.len();
        chunk[..n].copy_from_slice(tail);
        let chunk = &mut chunk[..n];
        crypt.crypt(bytemuck::cast_slice_mut(chunk));
        xor.apply_slice(chunk);
        w.write_all(bytemuck::cast_slice(chunk))?;

        Ok(())
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
            cipher.encode_str8_mut(&mut b);
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
            cipher.encode_str16_mut(&mut b);
            cipher.decode_str16(&mut b);
            let b = String::from_utf16(&b).unwrap();
            assert_eq!(s, b);

            // EncDec16 with chunked write
            let b = s.encode_utf16().collect::<Vec<_>>();
            let mut rw = Cursor::new(Vec::new());
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

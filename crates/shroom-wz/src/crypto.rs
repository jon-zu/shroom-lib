use std::{char::DecodeUtf16Error, io, num::Wrapping};

use aes::cipher::{inout::InOutBuf, KeyIvInit};
use shroom_crypto::{
    wz::{
        wz_data_cipher::{WzDataCipher, WzDataCryptStream},
        wz_offset_cipher::WzOffsetCipher,
    },
    ShroomVersion,
};

use crate::{util::array_chunks::as_chunks, WzConfig};

#[derive(Debug)]
pub struct WzCryptoContext {
    pub initial_iv: [u8; 16],
    pub key: [u8; 32],
    pub offset_magic: u32,
    pub no_crypto: bool,
}

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
pub struct WzCrypto {
    cipher: WzDataCipher,
    offset_cipher: WzOffsetCipher,
    data_offset: u32,
    no_crypt: bool,
}

impl std::fmt::Debug for WzCrypto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WzCrypto")
            .field("data_offset", &self.data_offset)
            .field("no_crypt", &self.no_crypt)
            .finish()
    }
}

fn append_chunk_str16<const N: usize>(
    s: &mut String,
    v: &[u16; N],
) -> Result<(), DecodeUtf16Error> {
    s.reserve(N);

    for c in char::decode_utf16(v.iter().cloned()) {
        s.push(c?);
    }

    Ok(())
}

fn append_slice_str16(s: &mut String, v: &[u16]) -> Result<(), DecodeUtf16Error> {
    s.reserve(v.len());

    for c in char::decode_utf16(v.iter().cloned()) {
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

fn append_slice_str8(s: &mut String, src: &[u8]) -> anyhow::Result<()> {
    s.extend(src.iter().copied().map(char_decode_latin1));
    Ok(())
}

impl WzCrypto {
    pub fn new(ctx: &WzCryptoContext, version: ShroomVersion, data_offset: u32) -> Self {
        Self {
            cipher: WzDataCipher::new(&ctx.key.into(), &ctx.initial_iv.into()),
            offset_cipher: WzOffsetCipher::new(version, ctx.offset_magic),
            data_offset,
            no_crypt: ctx.no_crypto,
        }
    }

    pub fn from_cfg(cfg: WzConfig, data_offset: u32) -> Self {
        Self::new(&cfg.region.into(), cfg.version, data_offset)
    }

    pub fn crypt_stream(&self) -> WzDataCryptStream<'_> {
        self.cipher.stream()
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
        xor_mask_str8(buf);
        self.crypt(buf);
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
        let mut crypt = self.crypt_stream();
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

            append_slice_str8(&mut res, chunk)?;
        }

        Ok(res)
    }

    pub fn write_str8(&self, mut w: impl io::Write, buf: &[u8]) -> io::Result<()> {
        const CHUNK_LEN: usize = 16;
        let mut crypt = self.crypt_stream();
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
        xor_mask_unicode(buf);
        self.crypt(bytemuck::cast_slice_mut(buf));
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
        let mut crypt = self.crypt_stream();
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
        const CHUNK_LEN: usize = 16;
        let mut crypt = self.crypt_stream();
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

#[cfg(test)]
mod tests {
    use std::{io::Cursor, iter};

    use crate::GMS95;

    use super::*;

    #[test]
    fn crypto_string() {
        let s = [
            "",
            "a",
            "abc",
            &iter::once('😀').take(4096).collect::<String>(),
        ];
        let cipher = WzCrypto::from_cfg(GMS95, 2);

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

use std::io::{self, BufRead, Read, Write};

use shroom_crypto::wz::data_cipher::WzDataCryptStream;

use crate::crypto::ImgCrypto;

const MAX_CHUNK_SIZE: usize = 4096 * 8;
const CHUNK_BUF_LEN: usize = 512;

/// Writer for chunked data
pub struct ChunkWriter<'a, W> {
    inner: W,
    cipher: &'a ImgCrypto,
}

fn is_eof(e: &io::Error) -> bool {
    e.kind() == io::ErrorKind::UnexpectedEof
}

impl<'a, W: Write> ChunkWriter<'a, W> {
    /// Create a new chunked writer
    pub fn new(w: W, cipher: &'a ImgCrypto) -> Self {
        Self { inner: w, cipher }
    }

    /// Write a single chunk
    pub fn write_chunk(&mut self, chunk: &mut [u8]) -> io::Result<()> {
        if chunk.len() > MAX_CHUNK_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "chunk size too large",
            ));
        }
        self.cipher.crypt(chunk);

        let len = chunk.len() as u32;
        self.inner.write_all(&len.to_le_bytes()[..])?;
        self.inner.write_all(chunk)?;
        Ok(())
    }

    /// Write multiple chunks
    pub fn write_chunk_iter<T: AsMut<[u8]>>(
        &mut self,
        mut chunks: impl Iterator<Item = T>,
    ) -> io::Result<()> {
        chunks.try_for_each(|mut chunk| self.write_chunk(chunk.as_mut()))?;
        Ok(())
    }
}
/// Reader for chunked data
pub struct ChunkedReader<'a, R> {
    crypto_stream: WzDataCryptStream<'a>,
    inner: R,
    buf: [u8; CHUNK_BUF_LEN],
    buf_len: usize,
    ix: usize,
    remaining_chunk: usize,
}

impl<'a, R: Read> ChunkedReader<'a, R> {
    /// Create a new chunked reader
    pub fn new(r: R, cipher: &'a ImgCrypto) -> Self {
        Self {
            inner: r,
            ix: 0,
            buf: [0; CHUNK_BUF_LEN],
            crypto_stream: cipher.chunked_crypt_stream().unwrap(),
            remaining_chunk: 0,
            buf_len: 0,
        }
    }

    fn remaining_buf(&self) -> usize {
        self.buf_len - self.ix
    }

    /// Reads the chunk len for the next chunk
    fn refill_chunk(&mut self) -> io::Result<()> {
        let mut data = [0; 4];
        self.inner.read_exact(&mut data)?;
        let ln = u32::from_le_bytes(data) as usize;

        if ln > MAX_CHUNK_SIZE {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("chunk size {ln} too large"),
            ));
        }

        if ln == 0 {
            return Err(io::Error::new(io::ErrorKind::Other, "chunk size 0"));
        }

        self.crypto_stream.reset();
        self.remaining_chunk = ln;
        Ok(())
    }

    /// Consumes a chunk f
    fn cosume_chunk(&mut self) -> io::Result<usize> {
        let n = self.remaining_chunk.min(CHUNK_BUF_LEN);
        self.inner.read_exact(&mut self.buf[..n])?;
        self.crypto_stream.crypt(&mut self.buf[..n]);
        self.remaining_chunk -= n;

        self.ix = 0;
        self.buf_len = n;
        Ok(n)
    }

    /// Refill the buffer
    fn refill_buf(&mut self) -> io::Result<usize> {
        // Try to check for remaining buffer
        let rem = self.remaining_buf();
        if rem > 0 {
            return Ok(rem);
        }

        // Read a new chunk if required
        if self.remaining_chunk == 0 {
            if let Err(err) = self.refill_chunk() {
                if is_eof(&err) {
                    return Ok(0);
                }

                return Err(err);
            }
        }

        self.cosume_chunk()
    }
}

impl<R: Read> Read for ChunkedReader<'_, R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.refill_buf()?.min(buf.len());
        let ix = self.ix;
        buf[..n].copy_from_slice(&self.buf[ix..ix + n]);
        self.consume(n);
        Ok(n)
    }
}

impl<R: Read> BufRead for ChunkedReader<'_, R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        let n = self.refill_buf()?;
        let ix = self.ix;
        Ok(&self.buf[ix..ix + n])
    }

    fn consume(&mut self, amt: usize) {
        debug_assert!(amt <= self.remaining_buf());
        self.ix += amt;
    }
}

#[cfg(test)]
mod tests {
    use std::io::{Cursor, Seek};

    use super::*;

    fn enc_dec_chunkes(data: &[u8], chunk_len: usize) {
        let mut rw = Cursor::new(Vec::new());
        let crypto = ImgCrypto::global();

        let mut inp = data.to_vec();

        let mut w = ChunkWriter::new(&mut rw, &crypto);
        w.write_chunk_iter(inp.chunks_mut(chunk_len)).unwrap();

        // Read chunks back
        rw.rewind().unwrap();

        let mut out = Vec::new();
        let mut r = ChunkedReader::new(&mut rw, &crypto);
        r.read_to_end(&mut out).unwrap();
        assert_eq!(out, data);

        rw.rewind().unwrap();
        let out = ChunkedReader::new(&mut rw, &crypto)
            .bytes()
            .collect::<Result<Vec<_>, _>>()
            .unwrap();

        assert_eq!(out, data);
    }

    #[test]
    fn chunked_1() {
        enc_dec_chunkes(&[1], 1);
        enc_dec_chunkes(&[1, 2, 3, 4], 2);
        enc_dec_chunkes(&[1, 2, 3], 2);
        enc_dec_chunkes(&[0x1; 4096], 128);
    }

    #[test]
    fn chunked() {
        let mut rw = Cursor::new(Vec::new());
        let crypto = ImgCrypto::global();
        const DATA: [u8; 4096] = [0x1; 4096];

        let mut data = DATA;

        let mut w = ChunkWriter::new(&mut rw, &crypto);
        w.write_chunk_iter(data.chunks_mut(128)).unwrap();

        // Check buffer len
        assert_eq!(rw.get_ref().len(), 4096 + (4096 / 128) * 4);

        // Read chunks back
        rw.rewind().unwrap();

        let mut r = ChunkedReader::new(&mut rw, &crypto);
        r.read_exact(&mut data).unwrap();
        assert_eq!(data, DATA);
    }

    #[test]
    fn decode_chunked() {
        const ONES: [u8; 128] = [0x1; 128];
        let crypto = ImgCrypto::global();

        let mut rw = Cursor::new(Vec::new());
        let mut w = ChunkWriter::new(&mut rw, &crypto);
        let mut ones = ONES;
        w.write_chunk_iter(ones.chunks_mut(128)).unwrap();

        rw.rewind().unwrap();

        let r = ChunkedReader::new(rw, &crypto);
        let data = r.bytes().collect::<Result<Vec<_>, _>>().unwrap();
        assert_eq!(data, ONES);
    }
}

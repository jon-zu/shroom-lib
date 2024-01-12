use crate::crypto::WzCrypto;
use std::fmt::{Debug, Display};
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

#[cfg(test)]
pub(crate) mod test_util;

pub mod array_chunks;
pub mod chunked;
pub mod path;
pub mod str_table;

//pub mod animation;

pub fn custom_binrw_error<
    R: std::io::Read + std::io::Seek,
    E: Debug + Display + Sync + Send + 'static,
>(
    mut r: R,
    err: E,
) -> binrw::Error {
    binrw::Error::Custom {
        pos: r.stream_position().unwrap_or(0),
        err: Box::new(err),
    }
}

/// Calculate the checksum with a seed and a single byte
pub fn wz_checksum_step(seed: i32, b: u8) -> i32 {
    seed.wrapping_add(b as i32)
}

/// Calculates the checksum of the data with the given seed
pub fn wz_checksum_seed(seed: i32, data: &[u8]) -> i32 {
    data.iter().fold(seed, |acc, &b| wz_checksum_step(acc, b))
}

/// Calculates the checksum of the data
pub fn wz_checksum(data: &[u8]) -> i32 {
    wz_checksum_seed(0, data)
}

pub trait PeekExt: BufReadExt + Seek {
    fn peek_n<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let buf = self.fill_buf()?;

        if buf.len() < N {
            // Fallback incase the buffer is not filled enough
            let pos = self.stream_position()?;
            let buf = self.read_n();
            // We seek back regardless of the result
            self.seek(SeekFrom::Start(pos))?;
            return buf;
        }

        Ok(buf[..N].try_into().unwrap())
    }

    fn peek_u16(&mut self) -> io::Result<u16> {
        self.peek_n::<2>().map(u16::from_le_bytes)
    }
}

impl<T: BufRead + Seek> PeekExt for T {}

pub trait BufReadExt: BufRead {
    /// Calculates the checksum of the next n bytes
    fn wz_checksum(&mut self, n: u64) -> io::Result<i32> {
        self.bytes()
            .take(n as usize)
            .try_fold(0, |acc, b| b.map(|b| wz_checksum_step(acc, b)))
    }

    /// Calculates the checksum of the next n bytes
    fn wz_checksum_eof(&mut self) -> io::Result<i32> {
        self.bytes()
            .try_fold(0, |acc, b| b.map(|b| wz_checksum_step(acc, b)))
    }

    /// Reads a u32 in little endian order
    fn read_u32_le(&mut self) -> io::Result<u32> {
        self.read_n().map(u32::from_le_bytes)
    }

    /// Reads n bytes as array
    fn read_n<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buf = [0; N];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    /// Decompress the stream into n bytes
    fn decompress_flate_size_to(&mut self, mut w: impl Write, n: u64) -> io::Result<u64> {
        // We have to limit the decoder with the known size
        std::io::copy(&mut flate2::bufread::ZlibDecoder::new(self).take(n), &mut w)
    }
}

impl<T: BufRead> BufReadExt for T {}

pub trait WriteExt: Write {
    /// Writes a u32 in little endian order
    fn write_u32_le(&mut self, n: u32) -> io::Result<()> {
        self.write_all(n.to_le_bytes().as_slice())
    }

    /// Encrypts the chunk in-place and writes it to the writer.
    fn write_wz_chunk(&mut self, crypto: &WzCrypto, chunk: &mut [u8]) -> io::Result<usize> {
        let n = chunk.len();
        self.write_u32_le(n as u32)?;
        crypto.crypt(chunk);
        self.write_all(chunk)?;
        Ok(n)
    }

    /// Encrypts each chunk of the iterator and writes it to the writer.
    fn write_wz_chunks<'a>(
        &mut self,
        crypto: &WzCrypto,
        mut chunks: impl Iterator<Item = &'a mut [u8]>,
    ) -> io::Result<usize> {
        chunks.try_fold(0, |written, chunk| {
            self.write_wz_chunk(crypto, chunk).map(|n| written + n)
        })
    }

    /// Writes the buffer as compressed chunked out
    // TODO: should this weird format even be supported
    // essentially It is 2 chunks
    // 1st 2 byte with zlib hdr, 2nd with the whole compressed chunk
    /*fn write_wz_chunked_compressed(&mut self, data: &[u8]) -> io::Result<u64> {
        // TODO
        // Write Header as 2 byte chunk
        // Write the rest as a single chunk
    }*/

    /// Writes the buffer compressed
    fn write_wz_compressed(&mut self, data: &[u8]) -> io::Result<u64> {
        let mut enc = flate2::write::ZlibEncoder::new(self, flate2::Compression::best());
        enc.write_all(data)?;
        enc.try_finish()?;
        Ok(enc.total_out())
    }
}

impl<T: Write> WriteExt for T {}

// TODO: ensure the reader respects the limits

/// Creates a at the given offset and limited by size
/// such that position 0 is the offset
pub struct SubReader<'a, R> {
    inner: &'a mut R,
    offset: u64,
    size: u64,
}

impl<'a, R> SubReader<'a, R>
where
    R: Read + Seek,
{
    pub fn new(r: &'a mut R, offset: u64, size: u64) -> Self {
        Self {
            inner: r,
            offset,
            size,
        }
    }
}

impl<'a, R> Read for SubReader<'a, R>
where
    R: Read,
{
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.inner.read(buf)
    }
}

impl<'a, R> BufRead for SubReader<'a, R>
where
    R: BufRead,
{
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.inner.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.inner.consume(amt)
    }
}

// TODO this MUST be tested
impl<'a, R> Seek for SubReader<'a, R>
where
    R: Seek,
{
    fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
        let pos = match pos {
            SeekFrom::Current(p) => SeekFrom::Current(p),
            SeekFrom::End(p) => SeekFrom::End((self.offset + self.size) as i64 + p),
            SeekFrom::Start(p) => SeekFrom::Start(p + self.offset),
        };
        self.inner.seek(pos).map(|p| p - self.offset)
    }
}

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use super::*;

    #[test]
    fn read_peek() {
        let mut r = BufReader::new(Cursor::new([0x1, 0x2, 0x3, 0x4]));
        assert_eq!(r.peek_n::<2>().unwrap(), [1, 2]);
        assert!(r.peek_n::<5>().is_err());

        assert_eq!(r.read_n().unwrap(), [1, 2]);
        assert_eq!(r.peek_n::<2>().unwrap(), [3, 4]);
        assert_eq!(r.read_n().unwrap(), [3, 4]);
        assert!(r.peek_n::<1>().is_err());
    }

    #[test]
    fn checksum() {
        const N: usize = 4096 * 2 + 3;
        let data = [0x1; N];
        // We add up N * 1s up to i32::MAX then it wraps around
        assert_eq!(Cursor::new(&data).wz_checksum(N as u64).unwrap(), N as i32);
    }
}

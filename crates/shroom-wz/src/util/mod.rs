use crate::cipher::WzCipher;
use std::io::{self, BufRead, Read, Seek, SeekFrom, Write};

pub mod animation;

pub fn custom_binrw_error<R: std::io::Read + std::io::Seek>(
    r: &mut R,
    err: anyhow::Error,
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
    fn wz_checksum(&mut self, n: u64) -> io::Result<i32> {
        self.bytes()
            .take(n as usize)
            .try_fold(0, |acc, b| b.map(|b| wz_checksum_step(acc, b)))
    }

    fn read_u32_le(&mut self) -> io::Result<u32> {
        self.read_n().map(u32::from_le_bytes)
    }

    fn read_n<const N: usize>(&mut self) -> io::Result<[u8; N]> {
        let mut buf = [0; N];
        self.read_exact(&mut buf)?;
        Ok(buf)
    }

    fn read_chunk(&mut self, crypto: &WzCipher, buf: &mut [u8]) -> io::Result<usize> {
        let chunk_len = self.read_u32_le()? as usize;
        if buf.len() < chunk_len {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("Buffer too small for chunk: {} < {}", buf.len(), chunk_len),
            ));
        }

        self.read_exact(&mut buf[..chunk_len])?;
        crypto.crypt(&mut buf[..chunk_len]);
        Ok(chunk_len)
    }

    fn read_chunked_data(
        &mut self,
        crypto: &WzCipher,
        mut buf: &mut [u8],
        chunked_len: usize,
    ) -> io::Result<usize> {
        let mut read = 0;
        while read < chunked_len {
            let chunk_size = self.read_chunk(crypto, buf)?;
            buf = &mut buf[chunk_size..];
            read += chunk_size;
        }

        Ok(read)
    }
    /*
    fn decompress_flate(&mut self, buf: &mut Vec<u8>) -> io::Result<usize> {
        flate2::bufread::ZlibDecoder::new(self).read_to_end(buf)
    }*/

    fn decompress_flate_size(&mut self, buf: &mut Vec<u8>, size: usize) -> io::Result<usize> {
        buf.resize(size, 0);
        flate2::bufread::ZlibDecoder::new(self).read_exact(buf)?;
        Ok(size)
    }
}

impl<T: BufRead> BufReadExt for T {}

pub trait WriteExt: Write {
    /// Writes a u32 in little endian order
    fn write_u32_le(&mut self, n: u32) -> io::Result<()> {
        self.write_all(n.to_le_bytes().as_slice())
    }

    /// Encrypts the chunk in-place and writes it to the writer.
    fn write_wz_chunk(&mut self, crypto: &WzCipher, chunk: &mut [u8]) -> io::Result<usize> {
        let n = chunk.len();
        self.write_u32_le(n as u32)?;
        crypto.crypt(chunk);
        self.write_all(chunk)?;
        Ok(n)
    }

    /// Encrypts each chunk of the iterator and writes it to the writer.
    fn write_wz_chunks<'a>(
        &mut self,
        crypto: &WzCipher,
        mut chunks: impl Iterator<Item = &'a mut [u8]>,
    ) -> io::Result<usize> {
        chunks.try_fold(0, |written, chunk| {
            self.write_wz_chunk(crypto, chunk).map(|n| written + n)
        })
    }

    /// Writes the buffer compressed
    fn write_wz_compressed(&mut self, data: &[u8]) -> io::Result<u64> {
        let mut enc = flate2::write::ZlibEncoder::new(self, flate2::Compression::best());
        enc.write_all(data)?;
        enc.try_finish()?;
        Ok(enc.total_out())
    }
}

impl<T: Write> WriteExt for T {}

pub struct SubReader<'a, R> {
    inner: &'a mut R,
    offset: u64,
    size: u64,
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

#[cfg(test)]
mod tests {
    use std::io::{BufReader, Cursor};

    use crate::GMS95;

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
    fn chunked() {
        let mut rw = Cursor::new(Vec::new());
        let crypto = WzCipher::from_cfg(GMS95, 1337);

        const DATA: [u8; 4096] = [0x1; 4096];

        let mut data = DATA;

        // Write chunks
        rw.write_wz_chunks(&crypto, data.chunks_mut(128)).unwrap();

        // Check buffer len
        assert_eq!(rw.get_ref().len(), 4096 + (4096 / 128) * 4);

        // Read chunks back
        let n = data.len();
        rw.set_position(0);
        rw.read_chunked_data(&crypto, &mut data, n).unwrap();
        assert_eq!(data, DATA);
    }

    #[test]
    fn checksum() {
        const N: usize = 4096 * 2 + 3;
        let data = [0x1; N];
        // We add up N * 1s up to i32::MAX then it wraps around
        assert_eq!(
            Cursor::new(&data).wz_checksum(N as u64).unwrap(),
            N as i32
        );
    }
}

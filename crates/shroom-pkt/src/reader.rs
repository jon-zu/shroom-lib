use std::io::Cursor;

use bytes::Buf;

use super::{error::Error, ShroomOpCode, PacketResult};

use super::shroom128_from_bytes;

/// Packet Reader for reading data
#[derive(Debug)]
pub struct PacketReader<'a> {
    inner: Cursor<&'a [u8]>,
}

impl<'a, T: AsRef<[u8]>> From<&'a T> for PacketReader<'a>
{
    fn from(v: &'a T) -> Self {
        Self::new(v.as_ref())
    }
}

impl<'a> PacketReader<'a> {
    /// Create a new Pacekt reader from a slice
    pub fn new(inner: &'a [u8]) -> Self {
        Self {
            inner: Cursor::new(inner),
        }
    }

    /// Consume the reader as slice
    pub fn into_inner(self) -> &'a [u8] {
        self.inner.into_inner()
    }

    /// Gets a reference to the data
    pub fn get_ref(&self) -> &[u8] {
        self.inner.get_ref()
    }

    /// Helper function to check if there's enough bytes to read `T`
    /// the size `n` still has to be passed as the T is just used for the Error context
    fn check_size_typed<T>(&self, n: usize) -> PacketResult<()> {
        if self.inner.remaining() >= n {
            Ok(())
        } else {
            Err(Error::eof::<T>(self.inner.get_ref(), n))
        }
    }

    /// Check if there's enough remaining bytes
    fn check_size(&self, n: usize) -> PacketResult<()> {
        self.check_size_typed::<()>(n)
    }

    #[inline]
    fn read_bytes_inner<T>(&mut self, n: usize) -> PacketResult<&'a [u8]> {
        self.check_size_typed::<T>(n)?;
        let p = self.inner.position() as usize;
        // Size is already checked here
        let by = &self.inner.get_ref()[p..p + n];
        self.inner.advance(n);
        Ok(by)
    }

    /// Advances this reader by n bytes
    pub fn advance(&mut self, n: usize) -> PacketResult<()> {
        self.check_size(n)?;
        self.inner.advance(n);
        Ok(())
    }

    ///Get the reamining slice
    pub fn remaining_slice(&self) -> &'a [u8] {
        let len = self.inner.position().min(self.inner.get_ref().len() as u64);
        &self.inner.get_ref()[(len as usize)..]
    }

    /// Gets the remaining bytes
    pub fn remaining(&self) -> usize {
        self.inner.remaining()
    }

    /// Create a sub reader based on this slice
    pub fn sub_reader(&self) -> Self {
        Self::new(self.remaining_slice())
    }

    /// Commit a sub reader
    /// as in advancing the position of this reader
    pub fn commit_sub_reader(&mut self, sub_reader: Self) -> PacketResult<()> {
        self.advance(sub_reader.inner.position() as usize)
    }

    /// Read the given OpCode `T`
    pub fn read_opcode<T: ShroomOpCode>(&mut self) -> PacketResult<T> {
        let v = self.read_u16()?;
        T::get_opcode(v)
    }

    pub fn read_u8(&mut self) -> PacketResult<u8> {
        self.check_size_typed::<u8>(1)?;
        Ok(self.inner.get_u8())
    }

    pub fn read_i8(&mut self) -> PacketResult<i8> {
        self.check_size_typed::<i8>(1)?;
        Ok(self.inner.get_i8())
    }

    pub fn read_bool(&mut self) -> PacketResult<bool> {
        self.check_size_typed::<bool>(1)?;
        Ok(self.read_u8()? != 0)
    }

    pub fn read_u16(&mut self) -> PacketResult<u16> {
        self.check_size_typed::<u16>(2)?;
        Ok(self.inner.get_u16_le())
    }

    pub fn read_i16(&mut self) -> PacketResult<i16> {
        self.check_size_typed::<i16>(2)?;
        Ok(self.inner.get_i16_le())
    }

    pub fn read_u32(&mut self) -> PacketResult<u32> {
        self.check_size_typed::<u32>(4)?;
        Ok(self.inner.get_u32_le())
    }

    pub fn read_i32(&mut self) -> PacketResult<i32> {
        self.check_size_typed::<i32>(4)?;
        Ok(self.inner.get_i32_le())
    }

    pub fn read_u64(&mut self) -> PacketResult<u64> {
        self.check_size_typed::<u64>(8)?;
        Ok(self.inner.get_u64_le())
    }

    pub fn read_i64(&mut self) -> PacketResult<i64> {
        self.check_size_typed::<i64>(8)?;
        Ok(self.inner.get_i64_le())
    }

    pub fn read_u128(&mut self) -> PacketResult<u128> {
        Ok(shroom128_from_bytes(self.read_array()?))
    }

    pub fn read_i128(&mut self) -> PacketResult<i128> {
        Ok(self.read_u128()? as i128)
    }

    pub fn read_f32(&mut self) -> PacketResult<f32> {
        self.check_size_typed::<f32>(4)?;
        Ok(self.inner.get_f32_le())
    }

    pub fn read_f64(&mut self) -> PacketResult<f64> {
        self.check_size_typed::<f64>(8)?;
        Ok(self.inner.get_f64_le())
    }

    pub fn read_string(&mut self) -> PacketResult<&'a str> {
        let n = self.read_u16()? as usize;
        let str_inner = self.read_bytes_inner::<&'a str>(n)?;
        Ok(std::str::from_utf8(str_inner)?)
    }

    /// Read string but limit the max length in bytes
    pub fn read_string_limited(&mut self, limit: usize) -> PacketResult<&'a str> {
        let n = self.read_u16()? as usize;
        if n > limit {
            return Err(Error::StringLimit(limit));
        }

        let str_inner = self.read_bytes_inner::<&'a str>(n)?;
        Ok(std::str::from_utf8(str_inner)?)
    }

    pub fn read_bytes(&mut self, n: usize) -> PacketResult<&'a [u8]> {
        self.read_bytes_inner::<&'a [u8]>(n)
    }

    pub fn read_array<const N: usize>(&mut self) -> PacketResult<[u8; N]> {
        Ok(self.read_bytes_inner::<[u8; N]>(N)?.try_into().unwrap())
    }
}

#[cfg(test)]
mod tests {
    use crate::PacketReader;

    #[test]
    fn remaining() {
        let b = [1u8, 2, 3, 4, 5, 6, 7, 8, 9, 10];
        let mut r = PacketReader::from(&b);

        assert_eq!(r.read_u8().unwrap(), 1);
        assert_eq!(r.remaining(), 9);
        assert_eq!(r.remaining_slice(), &b[1..]);
    }
}

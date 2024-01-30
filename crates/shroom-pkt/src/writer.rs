use bytes::{BufMut, BytesMut};

use crate::{Error, Packet, PacketResult, ShroomOpCode};

use super::{packet_str_len, shroom128_to_bytes};

/// Writer to encode a packet onto a Buffer `T`
#[derive(Debug)]
pub struct PacketWriter<T = BytesMut> {
    pub buf: T,
}

// Default implementation for `BytesMut`
impl Default for PacketWriter<BytesMut> {
    fn default() -> Self {
        Self {
            buf: Default::default(),
        }
    }
}

impl<T> PacketWriter<T> {
    /// Consume the inner buffer
    pub fn into_inner(self) -> T {
        self.buf
    }

    pub fn get_mut(&mut self) -> &mut T {
        &mut self.buf
    }

    pub fn get_ref(&mut self) -> &T {
        &self.buf
    }
}

impl PacketWriter<BytesMut> {
    /// Create a Writer with the given capacity
    pub fn with_capacity(cap: usize) -> Self {
        Self::new(BytesMut::with_capacity(cap))
    }

    /// Lenght
    pub fn len(&self) -> usize {
        self.buf.len()
    }

    /// Check empty
    pub fn is_empty(&self) -> bool {
        self.buf.is_empty()
    }

    /// Consume the buffer into a packet
    pub fn into_packet(self) -> Packet {
        Packet::from(self.buf)
    }
}

impl<T> PacketWriter<T>
where
    T: BufMut,
{
    /// Create a new PacketWriter from any BufMut
    pub fn new(buf: T) -> Self {
        Self { buf }
    }

    /// Check if n bytes still fit in the buffer
    #[inline]
    fn check_capacity(&self, n: usize) -> PacketResult<()> {
        if self.buf.remaining_mut() < n {
            Err(Error::OutOfCapacity)
        } else {
            Ok(())
        }
    }

    /// Writes an opcode onto the buffer
    pub fn write_opcode(&mut self, op: impl ShroomOpCode) -> PacketResult<()> {
        self.write_u16(op.into())
    }

    /// Write an `u8`
    pub fn write_u8(&mut self, v: u8) -> PacketResult<()> {
        self.check_capacity(1)?;
        self.buf.put_u8(v);
        Ok(())
    }

    /// Write an `i8`
    pub fn write_i8(&mut self, v: i8) -> PacketResult<()> {
        self.check_capacity(1)?;
        self.buf.put_i8(v);
        Ok(())
    }

    /// Write `bool`
    pub fn write_bool(&mut self, v: bool) -> PacketResult<()> {
        self.check_capacity(1)?;
        self.write_u8(v.into())
    }

    /// Write an `i17`
    pub fn write_i16(&mut self, v: i16) -> PacketResult<()> {
        self.check_capacity(2)?;
        self.buf.put_i16_le(v);
        Ok(())
    }

    /// Write an `i32`
    pub fn write_i32(&mut self, v: i32) -> PacketResult<()> {
        self.check_capacity(4)?;
        self.buf.put_i32_le(v);
        Ok(())
    }

    /// Write an `i64`
    pub fn write_i64(&mut self, v: i64) -> PacketResult<()> {
        self.check_capacity(8)?;
        self.buf.put_i64_le(v);
        Ok(())
    }

    /// Write an `i128`
    pub fn write_i128(&mut self, v: i128) -> PacketResult<()> {
        self.check_capacity(16)?;
        self.write_u128(v as u128)
    }

    /// Write an `u16`
    pub fn write_u16(&mut self, v: u16) -> PacketResult<()> {
        self.check_capacity(2)?;
        self.buf.put_u16_le(v);
        Ok(())
    }

    /// Write an `u32`
    pub fn write_u32(&mut self, v: u32) -> PacketResult<()> {
        self.check_capacity(4)?;
        self.buf.put_u32_le(v);
        Ok(())
    }

    /// Write an `u64`
    pub fn write_u64(&mut self, v: u64) -> PacketResult<()> {
        self.check_capacity(8)?;
        self.buf.put_u64_le(v);
        Ok(())
    }

    /// Write an `u128`
    pub fn write_u128(&mut self, v: u128) -> PacketResult<()> {
        self.write_array(&shroom128_to_bytes(v))
    }

    /// Write a `f32`
    pub fn write_f32(&mut self, v: f32) -> PacketResult<()> {
        self.check_capacity(4)?;
        self.buf.put_f32_le(v);
        Ok(())
    }

    /// Write a `f64`
    pub fn write_f64(&mut self, v: f64) -> PacketResult<()> {
        self.check_capacity(8)?;
        self.buf.put_f64_le(v);
        Ok(())
    }

    /// Write a bytes slice
    pub fn write_bytes(&mut self, v: &[u8]) -> PacketResult<()> {
        self.check_capacity(v.len())?;
        self.buf.put(v);
        Ok(())
    }

    /// Write an array of bytes
    pub fn write_array<const N: usize>(&mut self, v: &[u8; N]) -> PacketResult<()> {
        self.check_capacity(N)?;
        self.buf.put(v.as_slice());
        Ok(())
    }

    /// Write a str
    pub fn write_str(&mut self, v: &str) -> PacketResult<()> {
        self.check_capacity(packet_str_len(v))?;
        let b = v.as_bytes();
        self.buf.put_u16_le(b.len() as u16);
        self.buf.put_slice(b);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::PacketWriter;

    #[test]
    fn write() {
        let mut pw = PacketWriter::with_capacity(64);
        pw.write_u8(0).unwrap();
        pw.write_bytes(&[1, 2, 3, 4]).unwrap();

        assert_eq!(pw.len(), 5);
    }
}

use crate::{pkt::EncodeMessage, PacketResult};

/// Buffer to allow to encode multiple packets onto one buffer
/// while still allowing to iterate over the encoded packets
#[derive(Debug, Default)]
pub struct PacketBuf {
    buf: Vec<u8>
}

pub struct PacketIter<'a> {
    buf: &'a [u8],
    ix: usize
}

impl<'a> Iterator for PacketIter<'a> {
    type Item = &'a [u8];

    fn next(&mut self) -> Option<Self::Item> {
        if self.ix < self.buf.len() {
            let ix = self.ix;
            let ln = u16::from_ne_bytes(self.buf[ix..ix + 2].try_into().unwrap()) as usize;
            self.ix += 2 + ln;
            Some(&self.buf[ix+2..self.ix])
        } else {
            None
        }
    }
}

impl PacketBuf {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap)
        }
    }

    /// Encode a packet onto the buffer
    pub fn encode<T: EncodeMessage>(&mut self, pkt: T) -> PacketResult<()> {
        // Store the previous index
        let ix = self.buf.len();

        // Write a dummy length
        self.buf.extend_from_slice(&[0, 0]);

        // If an error occurs reset the index
        if let Err(err) = pkt.encode_message(&mut self.buf) {
            self.buf.truncate(ix);
            return Err(err);
        }

        // Get the actual length
        let len = self.buf.len() - ix - 2;
        self.buf[ix..ix + 2].copy_from_slice(&(len as u16).to_ne_bytes());
        Ok(())
    }

    /// Iterator over the written packet frames
    pub fn packets(&self) -> PacketIter<'_> {
        PacketIter {
            buf: &self.buf,
            ix: 0
        }
    }

    /// Clears the buffer
    pub fn clear(&mut self) {
        self.buf.truncate(0);
    }
}

#[cfg(test)]
mod tests {
    use derive_more::{From, Into};
    use crate::{opcode::HasOpCode, packet_wrap};
    use super::PacketBuf;

    #[derive(Debug, Copy, Clone, From, Into)]
    pub struct V(u8);
    packet_wrap!(V<>, u8, u8);

    impl HasOpCode for V {
        const OPCODE: u16 = 1;

        type OpCode = u16;
    }

    #[test]
    fn packet_buf() -> anyhow::Result<()> {
        let mut buf = PacketBuf::default();
        buf.encode(V(1))?;
        buf.encode(V(2))?;
        buf.encode(V(3))?;
        itertools::assert_equal(buf.packets(), [[1, 0, 1], [1, 0, 2], [1, 0, 3]]);

        buf.clear();
        assert_eq!(buf.packets().count(), 0);

        buf.encode(V(1))?;
        itertools::assert_equal(buf.packets(), [[1, 0, 1]]);

        Ok(())
    }
}

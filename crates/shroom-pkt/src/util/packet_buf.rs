use itertools::Itertools;
use std::iter;

use crate::{pkt::EncodeMessage, PacketResult};

/// Buffer to allow to encode multiple packets onto one buffer
/// while still allowing to iterate over the encoded packets
#[derive(Debug, Default)]
pub struct PacketBuf {
    buf: Vec<u8>,
    ix: Vec<usize>,
}

impl PacketBuf {
    pub fn with_capacity(cap: usize) -> Self {
        Self {
            buf: Vec::with_capacity(cap),
            ix: Vec::new(),
        }
    }

    /// Encode a packet onto the buffer
    pub fn encode<T: EncodeMessage>(&mut self, pkt: T) -> PacketResult<()> {
        // Store the previous index
        let ix = self.buf.len();

        // If an error occurs reset the index
        if let Err(err) = pkt.encode_message(&mut self.buf) {
            self.buf.truncate(ix);
            return Err(err);
        }

        // Store the ix of the current packet
        self.ix.push(ix);
        Ok(())
    }

    /// Iterator over the written packet frames
    pub fn packets(&self) -> impl Iterator<Item = &[u8]> + '_ {
        self.ix
            .iter()
            .cloned()
            .chain(iter::once(self.buf.len()))
            .tuple_windows()
            .map(|(l, r)| &self.buf[l..r])
    }

    /// Clears the buffer
    pub fn clear(&mut self) {
        self.buf.truncate(0);
        self.ix.clear();
    }
}

#[cfg(test)]
mod tests {
    use derive_more::{Into, From};

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

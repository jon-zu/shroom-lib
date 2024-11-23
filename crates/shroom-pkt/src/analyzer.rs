use std::{fmt::Display, ops::Range};

use crate::error::EOFErrorData;

/// Data analytics to get some more info about an error during reading a packet
#[derive(Debug)]
pub struct PacketAnalyzer<'a> {
    eof: &'a EOFErrorData,
    data: &'a [u8],
}

pub struct HexString<'a, const SPACE: bool>(&'a [u8]);

impl<const SPACE: bool> std::fmt::Display for HexString<'_, SPACE> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        for byte in self.0 {
            if !first && SPACE {
                write!(f, " ")?;
            }
            write!(f, "{:02x}", byte)?;
            first = false;
        }
        Ok(())
    }
}

impl<const SPACE: bool> HexString<'_, SPACE> {
    const fn size_per_byte() -> usize {
        if SPACE {
            3
        } else {
            2
        }
    }

    pub fn map_index(&self, ix: usize) -> usize {
        ix * Self::size_per_byte()
    }

    pub fn map_range(&self, range: Range<usize>) -> Range<usize> {
        let start = range.start;
        let l = self.map_index(start);
        let end = self.map_index(start + range.count()).saturating_sub(1);
        l..end
    }

    pub fn str_len(&self) -> usize {
        self.map_index(self.0.len())
    }
}

impl<'a> PacketAnalyzer<'a> {
    pub fn new(eof: &'a EOFErrorData, data: &'a [u8]) -> Self {
        Self { eof, data }
    }
    /// Get the relevant data with the surrounding context bytes
    pub fn get_relevant_data(&'a self) -> &'a [u8] {
        let ctx = self.eof.read_len() * 2;
        let left = self.eof.pos.saturating_sub(ctx);
        let right = (self.eof.pos + ctx).min(self.data.len());

        &self.data[left..right]
    }

    /// Get the eof marker range
    pub fn eof_range(&self) -> Range<usize> {
        let p = self.eof.pos;
        let right = (p + self.eof.read_len()).min(self.data.len());
        p..right
    }

    /// Write the hex string
    pub fn hex_string(&self) -> HexString<'a, true> {
        HexString(self.data)
    }

    /// Write the hex string
    pub fn relevant_data_hex_string(&'a self) -> HexString<'a, true> {
        HexString(self.get_relevant_data())
    }

    /// Eof hex string
    pub fn eof_hex_string(&self) -> HexString<'a, true> {
        HexString(&self.data[self.eof_range()])
    }

    /// Marker
    pub fn eof_marker(&self, mut f: impl std::fmt::Write) -> std::fmt::Result {
        let eof_range = self.hex_string().map_range(self.eof_range());
        for _ in 0..eof_range.start {
            write!(f, " ")?;
        }
        for _ in eof_range {
            write!(f, "^")?;
        }
        writeln!(
            f,
            " (read_len={}, type={})",
            self.eof.read_len(),
            self.eof.type_name()
        )?;
        Ok(())
    }
}

impl Display for PacketAnalyzer<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // Write the hex string
        let hx = self.hex_string();
        writeln!(f, "{}", hx)?;
        // Write the eof marker and type
        if cfg!(feature = "eof_ext") {
            self.eof_marker(f)?;
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "eof_ext")]
    fn eof() {
        let data = [1, 2, 3, 4];
        let eof = EOFErrorData::from_type::<u8>(2, 1);
        let a = PacketAnalyzer::new(&eof, &data);
        let txt = a.to_string();
        assert_eq!(a.eof_range(), 2..3);
        assert_eq!(txt, "01 02 03 04\n      ^^ (read_len=1, type=u8)\n");
    }
}

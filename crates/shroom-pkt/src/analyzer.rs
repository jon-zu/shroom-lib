use std::fmt::Display;

use bytes::Bytes;
use pretty_hex::PrettyHex;

/// Data analytics to get some more info about an error during reading a packet
#[derive(Debug)]
pub struct PacketDataAnalytics {
    data: Bytes,
    pos: usize,
    read_len: usize,
    context: usize,
}

impl PacketDataAnalytics {
    /// Create analytics data by copying the byte slice
    pub fn from_data(data: &[u8], pos: usize, read_len: usize, context: usize) -> Self {
        Self {
            data: Bytes::from(data.to_vec()),
            pos,
            read_len,
            context,
        }
    }

    /// Get the relevant data with the surrounding context bytes
    pub fn get_relevant_data(&self) -> &[u8] {
        let left = self.pos.saturating_sub(self.context);
        let right = (self.pos + self.read_len + self.context).min(self.data.len());

        &self.data[left..right]
    }
}

impl Display for PacketDataAnalytics {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let hex = self.get_relevant_data().hex_dump();
        write!(f, "{hex}")
    }
}

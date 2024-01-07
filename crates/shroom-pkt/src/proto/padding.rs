use bytes::BufMut;

use crate::{PacketReader, PacketResult, PacketWriter, SizeHint};

use super::{DecodePacket, EncodePacket};

/// Special padding type for N bytes
#[cfg(debug_assertions)]
#[derive(Debug)]
pub struct Padding<const N: usize>(pub [u8; N]);

/// Special padding type for N bytes
#[cfg(not(debug_assertions))]
#[derive(Debug)]
pub struct Padding<const N: usize>();

// In Debug configuration the padding is is stored
// elsewise It's just dropped and replaced by N zeros

impl<const N: usize> Padding<N> {
    #[cfg(debug_assertions)]
    pub fn from_data(data: [u8; N]) -> Self {
        Self(data)
    }

    #[cfg(not(debug_assertions))]
    pub fn from_data(_data: [u8; N]) -> Self {
        Self()
    }
}

impl<const N: usize> EncodePacket for Padding<N> {
    fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        #[cfg(debug_assertions)]
        return pw.write_array(&self.0);

        #[cfg(not(debug_assertions))]
        return pw.write_array(&[0; N]);
    }

    const SIZE_HINT: SizeHint = SizeHint::new(N);

    fn encode_len(&self) -> usize {
        N
    }
}

impl<'de, const N: usize> DecodePacket<'de> for Padding<N> {
    fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        Ok(Self::from_data(pr.read_array()?))
    }
}

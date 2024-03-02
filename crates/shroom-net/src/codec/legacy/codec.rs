use std::ops::Deref;

use bytes::{BufMut, BytesMut};
use shroom_crypto::{net::net_cipher::NetCipher, PacketHeader, PACKET_HEADER_LEN};
use shroom_pkt::Packet;
use tokio_util::codec::{Decoder, Encoder};

use crate::{NetError, NetResult};

use super::MAX_PACKET_LEN;

/// Check the packet length
fn check_packet_len(len: usize) -> NetResult<usize> {
    if len > MAX_PACKET_LEN {
        return Err(NetError::FrameSize(len));
    }

    Ok(len)
}

pub struct LegacyDecoder<const S: bool> {
    crypto: NetCipher<S>,
    len: Option<usize>,
}

impl<const S: bool> LegacyDecoder<S> {
    pub fn new(crypto: NetCipher<S>) -> Self {
        Self { crypto, len: None }
    }

    pub fn read_packet_len(
        &mut self,
        src: &mut bytes::BytesMut,
    ) -> Result<Option<usize>, NetError> {
        if let Some(len) = self.len.take() {
            return Ok(Some(len));
        }

        if src.len() < PACKET_HEADER_LEN {
            return Ok(None);
        }
        let hdr: PacketHeader = src
            .split_to(PACKET_HEADER_LEN)
            .deref()
            .try_into()
            .expect("Packet header");

        let length = self.crypto.decode_header(hdr)? as usize;

        // Verify the packet is not greater than the maximum limit
        check_packet_len(length).map(|_| Some(length))
    }
}

impl<const S: bool> Decoder for LegacyDecoder<S> {
    type Item = Packet;
    type Error = NetError;

    fn decode(&mut self, src: &mut bytes::BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        let Some(packet_len) = self.read_packet_len(src)? else {
            return Ok(None);
        };

        // Check if we can read the packet
        if src.len() < packet_len {
            src.reserve(packet_len - src.len());
            self.len = Some(packet_len);
            return Ok(None);
        }

        // Read the packet payload
        let mut packet = src.split_to(packet_len);
        self.crypto.decrypt(&mut packet);

        Ok(Some(packet.freeze().into()))
    }
}

pub struct LegacyEncoder<const S: bool>(NetCipher<S>);

impl<const S: bool> LegacyEncoder<S> {
    pub fn new(crypto: NetCipher<S>) -> Self {
        Self(crypto)
    }
}

// SAFETY the caller must ensure, there's enough spare capacity to store src
unsafe fn copy_crypt<F>(dst: &mut BytesMut, src: &[u8], crypt: F)
where
    F: FnOnce(&mut [u8]),
{
    let cnt = src.len();

    // Copy the data over
    let data = unsafe {
        // Copy src into the buffer
        let dst = dst.spare_capacity_mut();
        debug_assert!(dst.len() >= cnt);
        std::ptr::copy_nonoverlapping(src.as_ptr(), dst.as_mut_ptr().cast(), cnt);

        std::slice::from_raw_parts_mut(dst.as_mut_ptr().cast(), cnt)
    };

    // Crypt the data
    crypt(data);

    // Advance the buffer
    unsafe {
        dst.advance_mut(cnt);
    }
}

impl<'a, const S: bool> Encoder<&'a [u8]> for LegacyEncoder<S> {
    type Error = NetError;

    fn encode(&mut self, item: &'a [u8], dst: &mut bytes::BytesMut) -> Result<(), Self::Error> {
        let cnt = check_packet_len(item.len())?;
        // Reserve enough bytes
        dst.reserve(PACKET_HEADER_LEN + cnt);
        // Doing a further check in case the Packet header was changed
        // to ensure the unsafe code works as expected
        assert!(PACKET_HEADER_LEN == std::mem::size_of::<PacketHeader>());
        // Write the header
        dst.put_slice(&self.0.encode_header(cnt as u16));
        unsafe { copy_crypt(dst, item, |b| self.0.encrypt(b)) }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn buf_copy_crypt() {
        let plus_one = |b: &mut [u8]| b.iter_mut().for_each(|b| *b += 1);

        let mut buf = BytesMut::with_capacity(4);
        unsafe { copy_crypt(&mut buf, &[0; 4], plus_one) };
        assert_eq!(buf[..], [1, 1, 1, 1]);

        let mut buf = BytesMut::with_capacity(4);
        unsafe { copy_crypt(&mut buf, &[0; 2], plus_one) };
        assert_eq!(buf[..], [1, 1]);

        unsafe { copy_crypt(&mut buf, &[0; 1], plus_one) };
        assert_eq!(buf[..], [1, 1, 1]);

        unsafe { copy_crypt(&mut buf, &[0; 0], plus_one) };
        assert_eq!(buf[..], [1, 1, 1]);
    }
}

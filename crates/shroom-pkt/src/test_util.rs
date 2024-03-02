use crate::{DecodePacket, DecodePacketOwned, EncodePacket, PacketReader, PacketWriter};

/// Helper function to test If encoding matches decoding
#[allow(clippy::needless_pass_by_value)]
pub fn test_enc_dec<T>(val: T)
where
    T: EncodePacket + DecodePacketOwned + PartialEq + std::fmt::Debug,
{
    let data = val.to_packet().expect("encode");
    let mut pr = data.into_reader();
    let decoded = T::decode(&mut pr).expect("decode");

    assert_eq!(val, decoded);
}

/// Helper function to test If encoding matches decoding
pub fn test_enc_dec_all<T>(vals: impl IntoIterator<Item = T>)
where
    T: EncodePacket + DecodePacketOwned + PartialEq + std::fmt::Debug,
{
    for val in vals {
        test_enc_dec(val);
    }
}

// Helper to test with a lifetime
#[allow(clippy::needless_pass_by_value)]
pub fn enc_dec_lf<'de, T>(data: T, buf: &'de mut Vec<u8>)
where
    T: EncodePacket + DecodePacket<'de> + PartialEq + std::fmt::Debug + 'de,
{
    let mut pw = PacketWriter::new(buf);
    data.encode(&mut pw).expect("must encode");
    let cmp = T::decode_complete(&mut PacketReader::new(pw.into_inner())).expect("must decode");
    assert_eq!(data, cmp);
}

/// Helper macro to allow
#[macro_export]
macro_rules! test_enc_dec_borrow {
    ($d:expr) => {
        let mut data = Vec::new();
        $crate::test_util::enc_dec_lf($d, &mut data);
    };
}

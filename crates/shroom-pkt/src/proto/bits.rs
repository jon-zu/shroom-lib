use bitflags::Flags;
use packed_struct::PackedStruct;

pub fn to_bit_flags<T: Flags>(v: &T) -> T::Bits {
    v.bits()
}

pub fn from_bit_flags<T: Flags>(v: T::Bits) -> T {
    T::from_bits_truncate(v)
}

/// Mark the given `BitFlags` by implementing a Wrapper
/// The trait has to be explicitely implemented due to Trait rules
#[macro_export]
macro_rules! mark_shroom_bitflags {
    ($bits_ty:ty) => {
        impl $crate::EncodePacket for $bits_ty {
            const SIZE_HINT: $crate::SizeHint = <$bits_ty as bitflags::Flags>::Bits::SIZE_HINT;

            fn encode<B: bytes::BufMut>(
                &self,
                pw: &mut $crate::PacketWriter<B>,
            ) -> $crate::PacketResult<()> {
                $crate::proto::bits::to_bit_flags(self).encode(pw)
            }
        }

        impl<'de> $crate::DecodePacket<'de> for $bits_ty {
            fn decode(pr: &mut $crate::PacketReader<'de>) -> $crate::PacketResult<Self> {
                Ok($crate::proto::bits::from_bit_flags(
                    <$bits_ty as bitflags::Flags>::Bits::decode(pr)?,
                ))
            }
        }
    };
}

pub fn pack_struct<T: PackedStruct>(v: &T) -> T::ByteArray {
    v.pack().expect("pack")
}

pub fn unpack_struct<T: PackedStruct>(v: T::ByteArray) -> T {
    T::unpack(&v).expect("unpack")
}

/// Mark the given `PacketStruct` by implementing a Wrapper
#[macro_export]
macro_rules! mark_shroom_packed_struct {
    ($packed_strct_ty:ty) => {
        impl $crate::EncodePacket for $packed_strct_ty {
            const SIZE_HINT: $crate::SizeHint =
                <$packed_strct_ty as packed_struct::PackedStruct>::ByteArray::SIZE_HINT;

            fn encode<B: bytes::BufMut>(
                &self,
                pw: &mut $crate::PacketWriter<B>,
            ) -> $crate::PacketResult<()> {
                $crate::proto::bits::pack_struct(self).encode(pw)
            }
        }

        impl<'de> $crate::DecodePacket<'de> for $packed_strct_ty {
            fn decode(pr: &mut $crate::PacketReader<'de>) -> $crate::PacketResult<Self> {
                Ok($crate::proto::bits::unpack_struct(pr.read_array()?))
            }
        }
    };
}

#[cfg(test)]
mod tests {
    use bitflags::bitflags;

    use crate::test_util::{test_enc_dec, test_enc_dec_all};

    #[test]
    fn bits() {
        bitflags! {
            #[repr(transparent)]
            #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
            struct Flags: u32 {
                const A = 1;
                const B = 2;
                const C = 4;
            }
        }

        mark_shroom_bitflags!(Flags);

        test_enc_dec_all([Flags::A | Flags::B, Flags::all(), Flags::empty()]);
    }

    #[test]
    fn packet_struct() {
        use packed_struct::prelude::*;

        #[derive(PackedStruct, Clone, PartialEq, Debug)]
        #[packed_struct(bit_numbering = "msb0")]
        pub struct TestPack {
            #[packed_field(bits = "0..=2")]
            tiny_int: Integer<u8, packed_bits::Bits<3>>,
            #[packed_field(bits = "3")]
            enabled: bool,
            #[packed_field(bits = "4..=7")]
            tail: Integer<u8, packed_bits::Bits<4>>,
        }

        mark_shroom_packed_struct!(TestPack);

        test_enc_dec(TestPack {
            tiny_int: 5.into(),
            enabled: true,
            tail: 7.into(),
        });
    }
}

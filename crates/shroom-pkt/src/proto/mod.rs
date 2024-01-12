pub mod bits;
pub mod conditional;
pub mod list;
pub mod option;
pub mod padding;
pub mod partial;
pub mod primitive;
pub mod r#enum;
pub mod string;
pub mod time;
pub mod euclid;

use bytes::BufMut;

pub use conditional::{CondEither, CondOption, PacketConditional};
pub use list::{
    ShroomIndexList, ShroomIndexList16, ShroomIndexList32, ShroomIndexList64, ShroomIndexList8,
    ShroomIndexListZ, ShroomIndexListZ16, ShroomIndexListZ32, ShroomIndexListZ64,
    ShroomIndexListZ8, ShroomList, ShroomList16, ShroomList32, ShroomList64, ShroomList8,
};
pub use option::{
    ShroomOption, ShroomOption8, ShroomOptionBool, ShroomOptionR8, ShroomOptionRBool,
};
pub use padding::Padding;
pub use time::{ShroomDurationMs16, ShroomDurationMs32, ShroomExpirationTime, ShroomTime};
use crate::{PacketReader, PacketResult, PacketWriter, Packet, SizeHint};

#[macro_export]
macro_rules! packet_wrap {
    (
        $name:ident
        <
            $($gen_ty:ident),*
        >,
        $into_ty:ty,
        $from_ty:ty
    ) => {
        impl<$($gen_ty: $crate::EncodePacket),*> $crate::EncodePacket for $name<$($gen_ty,)*> {
            const SIZE_HINT: $crate::SizeHint = <$into_ty>::SIZE_HINT;

            fn encode_len(&self) -> usize {
                <$into_ty>::from(self.clone()).encode_len()
            }

            fn encode<B: bytes::BufMut>(&self, pw: &mut $crate::PacketWriter<B>) -> $crate::PacketResult<()> {
                <$into_ty>::from(self.clone()).encode(pw)
            }
        }

        impl<'de, $($gen_ty: $crate::DecodePacket<'de>),*> $crate::DecodePacket<'de> for $name<$($gen_ty,)*> {
            fn decode(pr: &mut $crate::PacketReader<'de>) -> $crate::PacketResult<Self> {
                Ok(<$into_ty>::decode(pr)?.into())
            }
        }
    };
}

/// Decode a `u128` from the given byte array
pub(crate) fn shroom128_from_bytes(data: [u8; 16]) -> u128 {
    // u128 are stored as 4 u32 little endian encoded blocks
    // but the blocks are itself in LE order aswell
    // so we have to reverse it
    let mut data: [u32; 4] = bytemuck::cast(data);
    data.reverse();
    u128::from_le_bytes(bytemuck::cast(data))
}

/// Encode a `u128` into a byte array
pub(crate) fn shroom128_to_bytes(v: u128) -> [u8; 16] {
    let mut blocks: [u32; 4] = bytemuck::cast(v.to_le_bytes());
    blocks.reverse();
    bytemuck::cast(blocks)
}

/// Required length to encode this string
pub(crate) fn packet_str_len(s: &str) -> usize {
    // len(u16) + data
    2 + s.len()
}

/// Decodes this type from a packet reader
pub trait DecodePacket<'de>: Sized {
    /// Decodes the packet
    fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self>;

    /// Decodes the packet n times
    fn decode_n(pr: &mut PacketReader<'de>, n: usize) -> PacketResult<Vec<Self>> {
        (0..n)
            .map(|_| Self::decode(pr))
            .collect::<PacketResult<_>>()
    }

    /// Attempts to decode the packet
    /// If EOF is reached None is returned elsewise the Error is returned
    /// This is useful for reading an optional tail
    fn try_decode(pr: &mut PacketReader<'de>) -> PacketResult<Option<Self>> {
        let mut sub_reader = pr.sub_reader();
        Ok(match Self::decode(&mut sub_reader) {
            Ok(item) => {
                pr.commit_sub_reader(sub_reader)?;
                Some(item)
            }
            Err(crate::Error::EOF { .. }) => None,
            Err(err) => return Err(err),
        })
    }

    /// Decodes from the given byte slice and ensures
    /// every byte was read
    fn decode_complete(pr: &mut PacketReader<'de>) -> anyhow::Result<Self> {
        let res = Self::decode(pr)?;
        if !pr.remaining_slice().is_empty() {
            anyhow::bail!("Still remaining data: {:?}", pr.remaining_slice());
        }
        Ok(res)
    }
}

/// Encodes this type on a packet writer
pub trait EncodePacket: Sized {
    /// Size Hint for types with a known type at compile time
    const SIZE_HINT: SizeHint;

    /// Get the encoded length of this type
    fn encode_len(&self) -> usize {
        Self::SIZE_HINT.0.expect("encode_len")
    }

    /// Encodes this packet
    fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()>;

    /// Encodes n packets
    fn encode_n<B: BufMut>(items: &[Self], pw: &mut PacketWriter<B>) -> PacketResult<()> {
        for item in items.iter() {
            item.encode(pw)?;
        }

        Ok(())
    }

    /// Encodes the type on a writer and returns the data
    fn to_data(&self) -> PacketResult<bytes::Bytes> {
        let mut pw = PacketWriter::default();
        self.encode(&mut pw)?;
        Ok(pw.into_inner().freeze())
    }

    /// Encodes this type as a packet
    fn to_packet(&self) -> PacketResult<Packet> {
        Ok(self.to_data()?.into())
    }
}

/// Decodes a container with the given size
pub trait DecodePacketSized<'de, T>: Sized {
    fn decode_sized(pr: &mut PacketReader<'de>, size: usize) -> PacketResult<Self>;
}

impl<'de, T> DecodePacketSized<'de, T> for Vec<T>
where
    T: DecodePacket<'de>,
{
    fn decode_sized(pr: &mut PacketReader<'de>, size: usize) -> PacketResult<Self> {
        T::decode_n(pr, size)
    }
}

/// Helper trait to remove the lifetime from types without one
pub trait DecodePacketOwned: for<'de> DecodePacket<'de> {}
impl<T> DecodePacketOwned for T where T: for<'de> DecodePacket<'de> {}

/// Tuple support helper
macro_rules! impl_packet {
    // List of idents splitted by names or well tuple types here
    ( $($name:ident)* ) => {
        // Expand tuples and add a generic bound
        impl<$($name,)*> $crate::EncodePacket for ($($name,)*)
            where $($name: $crate::EncodePacket,)* {
                fn encode<T: BufMut>(&self, pw: &mut PacketWriter<T>) -> PacketResult<()> {
                    #[allow(non_snake_case)]
                    let ($($name,)*) = self;
                    $($name.encode(pw)?;)*
                    Ok(())
                }

                const SIZE_HINT: $crate::SizeHint = $crate::util::SizeHint::ZERO
                        $(.add($name::SIZE_HINT))*;

                fn encode_len(&self) -> usize {
                    #[allow(non_snake_case)]
                    let ($($name,)*) = self;

                    $($name.encode_len() +)*0
                }
            }


            impl<'de, $($name,)*> $crate::DecodePacket<'de> for ($($name,)*)
            where $($name: $crate::DecodePacket<'de>,)* {
                fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
                    Ok((
                        ($($name::decode(pr)?,)*)
                    ))
                }
            }
    }
}

// Implement the tuples here
macro_rules! impl_for_tuples {
    ($apply_macro:ident) => {
        $apply_macro! { A }
        $apply_macro! { A B }
        $apply_macro! { A B C }
        $apply_macro! { A B C D }
        $apply_macro! { A B C D E }
        $apply_macro! { A B C D E F }
        $apply_macro! { A B C D E F G }
        $apply_macro! { A B C D E F G H }
        $apply_macro! { A B C D E F G H I }
        $apply_macro! { A B C D E F G H I J }
        $apply_macro! { A B C D E F G H I J K }
        $apply_macro! { A B C D E F G H I J K L }
    };
}

impl_for_tuples!(impl_packet);

#[cfg(test)]
mod tests {
    use crate::EncodePacket;

    #[test]
    fn tuple_size() {
        assert_eq!(<((), (),)>::SIZE_HINT.0, Some(0));
        assert_eq!(<((), u32,)>::SIZE_HINT.0, Some(4));
        assert_eq!(<((), u32, String)>::SIZE_HINT.0, None);
    }

    use super::*;
    use proptest::prelude::*;

    proptest! {
        #[test]
        fn test_shroom128(v: u128) {
            let bytes = shroom128_to_bytes(v);
            let v2 = shroom128_from_bytes(bytes);
            assert_eq!(v, v2);
        }
    }
}

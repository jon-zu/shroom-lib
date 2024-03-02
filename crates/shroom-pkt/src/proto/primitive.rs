use std::sync::Arc;

use array_init::try_array_init;
use bytes::BufMut;
use either::Either;

use crate::{PacketReader, PacketResult, PacketWriter, SizeHint};

use super::{DecodePacket, EncodePacket};

macro_rules! impl_ref_wrapped {
    ($ty:ty) => {
        impl<T: EncodePacket> EncodePacket for $ty {
            const SIZE_HINT: SizeHint = T::SIZE_HINT;

            fn encode_len(&self) -> usize {
                self.as_ref().encode_len()
            }

            fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
                self.as_ref().encode(pw)
            }
        }

        impl<'de, T: DecodePacket<'de>> DecodePacket<'de> for $ty {
            fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
                Ok(Self::new(T::decode(pr)?))
            }
        }
    };
}

impl_ref_wrapped!(Arc<T>);
impl_ref_wrapped!(std::rc::Rc<T>);
impl_ref_wrapped!(Box<T>);

impl EncodePacket for () {
    const SIZE_HINT: SizeHint = SizeHint::ZERO;

    fn encode<B: BufMut>(&self, _pw: &mut PacketWriter<B>) -> PacketResult<()> {
        Ok(())
    }

    fn encode_len(&self) -> usize {
        0
    }
}

impl<'de> DecodePacket<'de> for () {
    fn decode(_pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        Ok(())
    }
}

impl<L, R> EncodePacket for Either<L, R>
where
    L: EncodePacket,
    R: EncodePacket,
{
    const SIZE_HINT: SizeHint = SizeHint::NONE;

    fn encode<T: BufMut>(&self, pw: &mut PacketWriter<T>) -> PacketResult<()> {
        either::for_both!(self, v => v.encode(pw))
    }

    fn encode_len(&self) -> usize {
        either::for_both!(self, v => v.encode_len())
    }
}

macro_rules! impl_dec_enc {
    ($ty:ty, $dec:path, $enc:path) => {
        impl EncodePacket for $ty {
            const SIZE_HINT: SizeHint = $crate::SizeHint::new(std::mem::size_of::<$ty>());

            fn encode<B: bytes::BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
                $enc(pw, *self)
            }

            fn encode_len(&self) -> usize {
                std::mem::size_of::<$ty>()
            }
        }

        impl<'de> DecodePacket<'de> for $ty {
            fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
                $dec(pr)
            }
        }
    };
}

impl_dec_enc!(bool, PacketReader::read_bool, PacketWriter::write_bool);
impl_dec_enc!(u8, PacketReader::read_u8, PacketWriter::write_u8);
impl_dec_enc!(i8, PacketReader::read_i8, PacketWriter::write_i8);
impl_dec_enc!(u16, PacketReader::read_u16, PacketWriter::write_u16);
impl_dec_enc!(u32, PacketReader::read_u32, PacketWriter::write_u32);
impl_dec_enc!(u64, PacketReader::read_u64, PacketWriter::write_u64);
impl_dec_enc!(u128, PacketReader::read_u128, PacketWriter::write_u128);
impl_dec_enc!(i16, PacketReader::read_i16, PacketWriter::write_i16);
impl_dec_enc!(i32, PacketReader::read_i32, PacketWriter::write_i32);
impl_dec_enc!(i64, PacketReader::read_i64, PacketWriter::write_i64);
impl_dec_enc!(i128, PacketReader::read_i128, PacketWriter::write_i128);
impl_dec_enc!(f32, PacketReader::read_f32, PacketWriter::write_f32);
impl_dec_enc!(f64, PacketReader::read_f64, PacketWriter::write_f64);

// Arrays

impl<'de, const N: usize, T: DecodePacket<'de>> DecodePacket<'de> for [T; N] {
    fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        try_array_init(|_| T::decode(pr))
    }
}

impl<const N: usize, T: EncodePacket> EncodePacket for [T; N] {
    const SIZE_HINT: SizeHint = T::SIZE_HINT.mul_n(N);

    fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        T::encode_all(self.as_slice(), pw)
    }

    fn encode_len(&self) -> usize {
        self.iter().map(EncodePacket::encode_len).sum()
    }
}

impl<T: EncodePacket> EncodePacket for Vec<T> {
    fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        T::encode_all(self, pw)
    }

    const SIZE_HINT: SizeHint = SizeHint::NONE;

    fn encode_len(&self) -> usize {
        self.iter().map(EncodePacket::encode_len).sum()
    }
}

impl<D: EncodePacket> EncodePacket for Option<D> {
    const SIZE_HINT: SizeHint = SizeHint::NONE;

    fn encode<T: BufMut>(&self, pw: &mut PacketWriter<T>) -> PacketResult<()> {
        if let Some(ref v) = self {
            v.encode(pw)?;
        }

        Ok(())
    }

    fn encode_len(&self) -> usize {
        self.as_ref().map_or(0, EncodePacket::encode_len)
    }
}

#[cfg(test)]
mod tests {
    use crate::test_util::test_enc_dec_all;

    #[test]
    fn prim_num() {
        macro_rules! test_num {
            ($ty:ty) => {
                let min = <$ty>::MIN;
                let max = <$ty>::MAX;
                let half = (min + max) / (2 as $ty);
                test_enc_dec_all([min, max, half])
            };
            ($($ty:ty,)*) => {
                $(test_num!($ty);)*
            };
        }

        test_num!(u8, i8, u16, i16, u32, i32, u64, i64, u128, i128, f32, f64,);
    }

    #[test]
    fn bool() {
        test_enc_dec_all([false, true]);
    }
}

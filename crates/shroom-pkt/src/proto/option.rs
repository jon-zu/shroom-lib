use std::marker::PhantomData;

use derive_more::{Deref, DerefMut, From, Into};

use crate::{PacketReader, PacketResult, PacketWriter, SizeHint};

use super::{DecodePacket, DecodePacketOwned, EncodePacket};

/// Discriminant for Option
pub trait ShroomOptionDiscriminant: EncodePacket + DecodePacketOwned{
    const NONE_VALUE: Self;
    const SOME_VALUE: Self;
    fn has_value(&self) -> bool;

    fn reverse(&self) -> Self {
        if self.has_value() {
            Self::NONE_VALUE
        } else {
            Self::SOME_VALUE
        }
    }
}

impl ShroomOptionDiscriminant for u8 {
    const NONE_VALUE: Self = 0;
    const SOME_VALUE: Self = 1;
    fn has_value(&self) -> bool {
        *self != 0
    }
}

impl ShroomOptionDiscriminant for bool {
    const NONE_VALUE: Self = false;
    const SOME_VALUE: Self = true;
    fn has_value(&self) -> bool {
        *self
    }
}

/// Reversed Option Discriminant
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RevShroomOptionDiscriminant<Opt>(Opt);

impl<Opt: ShroomOptionDiscriminant> From<Opt> for RevShroomOptionDiscriminant<Opt> {
    fn from(value: Opt) -> Self {
        Self(value.reverse())
    }
}

impl<Opt: ShroomOptionDiscriminant> EncodePacket for RevShroomOptionDiscriminant<Opt> {
    const SIZE_HINT: SizeHint = Opt::SIZE_HINT;
    fn encode_len(&self) -> usize {
        self.0.reverse().encode_len()
    }

    fn encode<B: bytes::BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        self.0.reverse().encode(pw)
    }
}

impl<'de, Opt: ShroomOptionDiscriminant> DecodePacket<'de> for RevShroomOptionDiscriminant<Opt> {
    fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        Ok(Self::from(Opt::decode(pr)?))
    }
}

//packet_wrap!(RevShroomOptionDiscriminant<Opt>, Opt, Opt);

impl<Opt> ShroomOptionDiscriminant for RevShroomOptionDiscriminant<Opt>
where
    Opt: ShroomOptionDiscriminant + Copy + 'static,
{
    const NONE_VALUE: Self = RevShroomOptionDiscriminant(Opt::SOME_VALUE);
    const SOME_VALUE: Self = RevShroomOptionDiscriminant(Opt::NONE_VALUE);

    fn has_value(&self) -> bool {
        !self.0.has_value()
    }
}

/// Optional type, first read the discriminant `D`
/// and then reads the value If D is some
#[derive(Debug, Clone, Copy, PartialEq, Into, Deref, DerefMut)]
pub struct ShroomOption<T, D> {
    #[into]
    #[deref]
    #[deref_mut]
    pub opt: Option<T>,
    _t: PhantomData<D>,
}

impl<T, D> ShroomOption<T, D> {
    pub fn from_opt(opt: Option<T>) -> Self {
        Self {
            opt,
            _t: PhantomData,
        }
    }
}

impl<T, Opt> From<Option<T>> for ShroomOption<T, Opt> {
    fn from(value: Option<T>) -> Self {
        Self::from_opt(value)
    }
}

impl<T, Opt> EncodePacket for ShroomOption<T, Opt>
where
    T: EncodePacket,
    Opt: ShroomOptionDiscriminant,
{
    fn encode<B: bytes::BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        match self.as_ref() {
            Some(v) => {
                Opt::SOME_VALUE.encode(pw)?;
                v.encode(pw)
            }
            None => Opt::NONE_VALUE.encode(pw),
        }
    }

    const SIZE_HINT: SizeHint = SizeHint::NONE;

    fn encode_len(&self) -> usize {
        match self.as_ref() {
            Some(v) => Opt::SOME_VALUE.encode_len() + v.encode_len(),
            None => Opt::NONE_VALUE.encode_len(),
        }
    }
}

impl<'de, T, Opt> DecodePacket<'de> for ShroomOption<T, Opt>
where
    T: DecodePacket<'de>,
    Opt: ShroomOptionDiscriminant,
{
    fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        let d = Opt::decode(pr)?;
        Ok(if d.has_value() {
            Some(T::decode(pr)?)
        } else {
            None
        }
        .into())
    }
}

/// Optional with u8 as discriminator, 0 signaling None, otherwise en/decode `T`
pub type ShroomOption8<T> = ShroomOption<T, u8>;
/// Optional with reversed u8 as discriminator, 0 signaling en/decode `T`
pub type ShroomOptionR8<T> = ShroomOption<T, RevShroomOptionDiscriminant<u8>>;
/// Optional with `bool` as discriminator, false signaling None, otherwise en/decode `T`
pub type ShroomOptionBool<T> = ShroomOption<T, bool>;
/// Optional with reversed `bool` as discriminator, false signaling en/decode `T`
pub type ShroomOptionRBool<T> = ShroomOption<T, RevShroomOptionDiscriminant<bool>>;

#[cfg(test)]
mod tests {
    use crate::test_util::test_enc_dec_all;

    use super::*;

    #[test]
    fn option() {
        test_enc_dec_all([
            ShroomOption8::from_opt(Some("abc".to_string())),
            ShroomOption8::from_opt(None),
        ]);
        test_enc_dec_all([
            ShroomOptionR8::from_opt(Some("abc".to_string())),
            ShroomOptionR8::from_opt(None),
        ]);
        test_enc_dec_all([
            ShroomOptionBool::from_opt(Some("abc".to_string())),
            ShroomOptionBool::from_opt(None),
        ]);
        test_enc_dec_all([
            ShroomOptionRBool::from_opt(Some("abc".to_string())),
            ShroomOptionRBool::from_opt(None),
        ]);
    }
}

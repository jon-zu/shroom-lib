use bytes::BufMut;
use derive_more::{Deref, DerefMut, From, Into};
use either::Either;

use crate::{PacketReader, PacketResult, PacketWriter, SizeHint};

use super::{DecodePacket, EncodePacket};

/// Helper trait for dealing with conditional En/decoding
pub trait PacketConditional<'de>: Sized {
    /// Encode if if the cond evaluates to true
    fn encode_cond<B: BufMut>(&self, cond: bool, pw: &mut PacketWriter<B>) -> PacketResult<()>;
    /// Decode if the cond evaluates to true
    fn decode_cond(cond: bool, pr: &mut PacketReader<'de>) -> PacketResult<Self>;
    /// Length based on cond
    fn encode_len_cond(&self, cond: bool) -> usize;
}

/// Conditional Option
#[derive(Debug, PartialEq, Eq, Clone, Copy, From, Into, Deref, DerefMut)]
pub struct CondOption<T>(pub Option<T>);

impl<T> Default for CondOption<T> {
    fn default() -> Self {
        Self(None)
    }
}

impl<T: EncodePacket> EncodePacket for CondOption<T> {
    fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        self.0.as_ref().map_or(Ok(()), |p| p.encode(pw))
    }

    const SIZE_HINT: SizeHint = SizeHint::NONE;

    fn encode_len(&self) -> usize {
        self.0.as_ref().map_or(0, EncodePacket::encode_len)
    }
}

impl<'de, T> PacketConditional<'de> for CondOption<T>
where
    T: EncodePacket + DecodePacket<'de>,
{
    fn encode_cond<B: BufMut>(&self, _cond: bool, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        if let Some(ref p) = self.0 {
            p.encode(pw)?;
        }
        Ok(())
    }

    fn decode_cond(cond: bool, pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        Ok(Self(if cond { Some(T::decode(pr)?) } else { None }))
    }

    fn encode_len_cond(&self, cond: bool) -> usize {
        cond.then(|| self.as_ref().expect("Must have value").encode_len())
            .unwrap_or(0)
    }
}

/// Conditional either type, cond false => Left, true => Right
#[derive(Debug, PartialEq, Eq, Clone, Copy, From, Into, Deref, DerefMut)]
pub struct CondEither<L, R>(pub Either<L, R>);

impl<'de, L, R> PacketConditional<'de> for CondEither<L, R>
where
    L: EncodePacket + DecodePacket<'de>,
    R: EncodePacket + DecodePacket<'de>,
{
    fn encode_cond<B: BufMut>(&self, _cond: bool, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        either::for_both!(self.0.as_ref(), v => v.encode(pw))
    }

    fn decode_cond(cond: bool, pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        Ok(Self(if cond {
            Either::Left(L::decode(pr)?)
        } else {
            Either::Right(R::decode(pr)?)
        }))
    }

    fn encode_len_cond(&self, _cond: bool) -> usize {
        either::for_both!(self.0.as_ref(), v => v.encode_len())
    }
}

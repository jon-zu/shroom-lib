use bytes::BufMut;

use crate::{PacketReader, PacketResult, PacketWriter, SizeHint};

use super::{DecodePacket, EncodePacket};

/// Provide a wrapper around the `Inner` with conversion methods
/// Just implementing this wrapper Trait with an `Inner` type which already
/// implements `EncodePacket` and `DecodePacket` allows you to inherit those for the implemented type
pub trait PacketWrapped: Sized {
    type Inner;
    type IntoValue<'a>
    where
        Self: 'a;

    fn packet_into_inner(&self) -> Self::IntoValue<'_>;
    fn packet_from(v: Self::Inner) -> Self;
}

/// Check `PacketWrapped` but with a failable `packet_try_from` method
pub trait PacketTryWrapped: Sized {
    type Inner;
    type IntoValue<'a>
    where
        Self: 'a;

    fn packet_into_inner(&self) -> Self::IntoValue<'_>;
    fn packet_try_from(v: Self::Inner) -> PacketResult<Self>;
}

impl<W> EncodePacket for W
where
    W: PacketTryWrapped,
    for<'a> W::IntoValue<'a>: EncodePacket,
{
    const SIZE_HINT: SizeHint = W::IntoValue::SIZE_HINT;

    fn encode_len(&self) -> usize {
        Self::SIZE_HINT
            .0
            .unwrap_or(self.packet_into_inner().encode_len())
    }

    fn encode<B: BufMut>(&self, pw: &mut PacketWriter<B>) -> PacketResult<()> {
        self.packet_into_inner().encode(pw)
    }
}

impl<'de, MW> DecodePacket<'de> for MW
where
    MW: PacketTryWrapped,
    MW::Inner: DecodePacket<'de>,
{
    fn decode(pr: &mut PacketReader<'de>) -> PacketResult<Self> {
        let inner = <MW as PacketTryWrapped>::Inner::decode(pr)?;
        MW::packet_try_from(inner)
    }
}

impl<W: PacketWrapped> PacketTryWrapped for W {
    type Inner = W::Inner;
    type IntoValue<'a> = W::IntoValue<'a> where Self: 'a;

    fn packet_into_inner(&self) -> Self::IntoValue<'_> {
        self.packet_into_inner()
    }

    fn packet_try_from(v: Self::Inner) -> PacketResult<Self> {
        Ok(<W as PacketWrapped>::packet_from(v))
    }
}

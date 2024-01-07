use euclid::{Box2D, Point2D, Vector2D};

use crate::{DecodePacket, EncodePacket};

use super::DecodePacketOwned;

impl<T: EncodePacket + Copy, U> EncodePacket for Vector2D<T, U> {
    const SIZE_HINT: crate::SizeHint = <(T, T)>::SIZE_HINT;

    fn encode<B: bytes::BufMut>(&self, pw: &mut crate::PacketWriter<B>) -> crate::PacketResult<()> {
        (self.x, self.y).encode(pw)
    }
}
impl<'de, T: DecodePacketOwned, U> DecodePacket<'de> for Vector2D<T, U> {
    fn decode(pr: &mut crate::PacketReader<'de>) -> crate::PacketResult<Self> {
        Ok(<(T, T)>::decode(pr)?.into())
    }
}

impl<T: EncodePacket + Copy, U> EncodePacket for Point2D<T, U> {
    const SIZE_HINT: crate::SizeHint = <(T, T)>::SIZE_HINT;

    fn encode<B: bytes::BufMut>(&self, pw: &mut crate::PacketWriter<B>) -> crate::PacketResult<()> {
        (self.x, self.y).encode(pw)
    }
}
impl<'de, T: DecodePacketOwned, U> DecodePacket<'de> for Point2D<T, U> {
    fn decode(pr: &mut crate::PacketReader<'de>) -> crate::PacketResult<Self> {
        Ok(<(T, T)>::decode(pr)?.into())
    }
}

impl<T: EncodePacket + Copy, U> EncodePacket for Box2D<T, U> {
    const SIZE_HINT: crate::SizeHint = <(T, T)>::SIZE_HINT;

    fn encode<B: bytes::BufMut>(&self, pw: &mut crate::PacketWriter<B>) -> crate::PacketResult<()> {
        (self.min, self.max).encode(pw)
    }
}
impl<'de, T: DecodePacketOwned + Copy, U> DecodePacket<'de> for Box2D<T, U> {
    fn decode(pr: &mut crate::PacketReader<'de>) -> crate::PacketResult<Self> {
        let (o, sz) = <(Point2D<T, U>, Point2D<T, U>)>::decode(pr)?;
        Ok(Box2D::new(o, sz))
    }
}

#[cfg(test)]
mod tests {

    use euclid::{default::Vector2D, Box2D, Point2D, UnknownUnit};

    use crate::test_util::test_enc_dec_all;

    #[test]
    fn vec_pt() {
        let v = [
            Vector2D::<u16>::new(1, 2),
            Vector2D::<u16>::new(1, 1),
            Vector2D::<u16>::new(2, 1),
        ];

        test_enc_dec_all(v);
        test_enc_dec_all(v.iter().map(|v| v.to_point()));
    }

    #[test]
    fn boxes() {
        let b: [Box2D<u32, UnknownUnit>; 3] = [
            Box2D::new(Point2D::new(1, 2), Point2D::new(3, 4)),
            Box2D::new(Point2D::new(1, 1), Point2D::new(1, 1)),
            Box2D::new(Point2D::new(2, 1), Point2D::new(1, 1)),
        ];

        test_enc_dec_all(b);
    }
}

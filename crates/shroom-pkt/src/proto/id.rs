use crate::{DecodePacket, DecodePacketOwned, EncodePacket};

pub trait Tag {
    type Value: EncodePacket + DecodePacketOwned + std::fmt::Debug + PartialEq + Eq;
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id<T: Tag>(pub T::Value);

impl<T: Tag> DecodePacket<'static> for Id<T> {
    fn decode(buf: &mut crate::PacketReader<'_>) -> crate::PacketResult<Self> {
        Ok(Self(T::Value::decode(buf)?))
    }
}

impl<T: Tag> EncodePacket for Id<T> {
    const SIZE_HINT: crate::SizeHint = T::Value::SIZE_HINT;

    fn encode_len(&self) -> usize {
        self.0.encode_len()
    }

    fn encode<B: bytes::BufMut>(&self, pw: &mut crate::PacketWriter<B>) -> crate::PacketResult<()> {
        self.0.encode(pw)
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_id() {
        pub struct ObjIdTag;
        impl Tag for ObjIdTag {
            type Value = u32;
        }
        //type ObjId = Id<ObjIdTag>;

        let id = Id::<ObjIdTag>(0);
        assert_eq!(id.0, 0);
 
    }
}
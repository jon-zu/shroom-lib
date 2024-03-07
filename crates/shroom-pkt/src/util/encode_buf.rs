use crate::{
    pkt::{EncodeMessage, Message},
    Packet,
};
use bytes::BytesMut;

#[derive(Debug)]
pub struct EncodeBuf(BytesMut);

impl Default for EncodeBuf {
    fn default() -> Self {
        Self::new()
    }
}

impl EncodeBuf {
    #[must_use]
    pub fn new() -> Self {
        Self::with_capacity(4096)
    }

    #[must_use]
    pub fn with_capacity(cap: usize) -> Self {
        Self(BytesMut::with_capacity(cap))
    }

    pub fn encode_onto(&mut self, data: impl EncodeMessage) -> Result<Message, crate::Error> {
        self.0.reserve(4096);
        data.encode_message(&mut self.0)?;
        Ok(Packet::from(self.0.split().freeze())
            .try_into()
            .expect("encoded message must have an opcode"))
    }
}

use std::ops::Deref;

use bytes::{BufMut, Bytes, BytesMut};

use crate::{
    opcode::HasOpCode, DecodePacket, EncodePacket, Error, PacketReader, PacketWriter, ShroomOpCode,
};

#[derive(Debug, Clone)]
pub struct Packet(Bytes);

impl Packet {
    /// Creates a new shared `Packet` from a static slice.
    pub const fn from_static(bytes: &'static [u8]) -> Self {
        Self(Bytes::from_static(bytes))
    }

    pub fn into_reader(&self) -> PacketReader<'_> {
        PacketReader::new(self.deref())
    }
}

impl AsRef<Bytes> for Packet {
    fn as_ref(&self) -> &Bytes {
        &self.0
    }
}

impl Deref for Packet {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl From<Bytes> for Packet {
    fn from(value: Bytes) -> Self {
        Self(value)
    }
}

impl From<BytesMut> for Packet {
    fn from(value: BytesMut) -> Self {
        Self(value.freeze())
    }
}

/// A message with an opcode
#[derive(Debug, Clone)]
pub struct Message(Packet);

impl Message {
    /// Gets the opcode as u16 value
    pub fn opcode_value(&self) -> u16 {
        u16::from_be_bytes(self.0[0..2].try_into().expect("Message opcode"))
    }

    /// Gets the typed opcode of the message
    pub fn opcode<OP: ShroomOpCode>(&self) -> Result<OP, Error> {
        ShroomOpCode::get_opcode(self.opcode_value())
    }

    /// Get the payload of the message
    pub fn payload(&self) -> &[u8] {
        &self.0[2..]
    }

    /// Creates a packet reader
    pub fn reader(&self) -> PacketReader<'_> {
        PacketReader::new(self.payload())
    }

    /// Decodes the payload
    pub fn decode<'de, T: DecodePacket<'de>>(&'de self) -> Result<T, Error> {
        T::decode(&mut self.reader())
    }
}

impl AsRef<[u8]> for Message {
    fn as_ref(&self) -> &[u8] {
        self.0.deref()
    }
}

impl Deref for Message {
    type Target = [u8];

    fn deref(&self) -> &Self::Target {
        self.0.deref()
    }
}

impl TryFrom<Packet> for Message {
    type Error = Error;

    fn try_from(value: Packet) -> Result<Self, Self::Error> {
        if value.len() < 2 {
            return Err(Error::NoOpCode);
        }

        Ok(Self(value))
    }
}

impl TryFrom<PacketWriter> for Message {
    type Error = Error;

    fn try_from(value: PacketWriter) -> Result<Self, Self::Error> {
        value.into_packet().try_into()
    }
}

/// Marks a type as encode-able as message
pub trait EncodeMessage: Sized {
    fn encode_message<B: BufMut>(self, buf: B) -> Result<(), Error>;

    fn to_message(self) -> Result<Message, Error> {
        let mut buf = BytesMut::new();
        self.encode_message(&mut buf)?;
        Ok(Message(buf.into()))
    }
}

impl<T: EncodePacket + HasOpCode> EncodeMessage for T {
    fn encode_message<B: BufMut>(self, buf: B) -> Result<(), Error> {
        let mut pw = PacketWriter::new(buf);
        pw.write_opcode(T::OPCODE)?;
        self.encode(&mut pw)?;
        Ok(())
    }
}

/// Marks a type as decode-able into a message
pub trait DecodeMessage<'de> {
    fn decode_message(msg: &'de Message) -> Result<Self, Error>
    where
        Self: Sized;
}

impl<'de, T: DecodePacket<'de> + HasOpCode> DecodeMessage<'de> for T {
    fn decode_message(msg: &'de Message) -> Result<Self, Error> {
        if msg.opcode_value() != T::OPCODE.into() {
            return Err(Error::InvalidOpCode(msg.opcode_value()));
        }
        msg.decode()
    }
}

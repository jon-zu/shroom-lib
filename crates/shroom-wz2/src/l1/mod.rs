pub mod canvas;
pub mod obj;
pub mod prop;
//pub mod ser;

pub mod sound;

pub mod str;

use binrw::{BinRead, BinWrite};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzPosValue<T> {
    /// The read value.
    pub val: T,

    /// The byte position of the start of the value.
    pub pos: u64,
}
impl WzPosValue<()> {
    pub fn new(pos: u64) -> WzPosValue<()> {
        Self { val: (), pos }
    }
}

impl<T: BinRead> BinRead for WzPosValue<T> {
    type Args<'a> = T::Args<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let pos = reader.stream_position()?;
        let val = T::read_options(reader, endian, args)?;
        Ok(Self { val, pos })
    }
}

impl<T: BinWrite> BinWrite for WzPosValue<T> {
    type Args<'a> = T::Args<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        _writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        // Writing is no-op
        Ok(())
    }
}

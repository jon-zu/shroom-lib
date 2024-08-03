use std::ops::Deref;

use binrw::{BinRead, BinWrite};

use crate::{
    ctx::{WzImgReadCtx, WzImgWriteCtx},
    util::str_table::RcStr,
};

/// String in a wz img file or stream
#[derive(Debug, Clone)]
pub struct WzImgStr(pub RcStr);

impl Deref for WzImgStr {
    type Target = RcStr;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl WzImgStr {
    pub fn new(s: RcStr) -> Self {
        Self(s)
    }
}

impl BinRead for WzImgStr {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        args.read_img_str(reader, endian).map(Self)
    }
}

impl BinWrite for WzImgStr {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        args.write_img_str(writer, endian, &self.0)
    }
}

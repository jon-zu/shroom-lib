use std::{
    cell::RefCell,
    io::{Read, Seek, Write},
    ops::Deref,
    rc::Rc,
};

use binrw::{BinRead, BinResult, BinWrite, Endian};

use crate::{
    crypto::WzCrypto,
    ty::{WzStr, WzStrRef},
    util::{
        custom_binrw_error,
        str_table::{OffsetStrTable, RcStr, StrOffsetTable},
    },
};
#[derive(Debug, Clone, Copy)]
pub struct WzContext<'a>(pub &'a WzCrypto);

impl<'a> Deref for WzContext<'a> {
    type Target = WzCrypto;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WzImgReadCtx<'a> {
    pub crypto: &'a WzCrypto,
    pub str_table: &'a RefCell<OffsetStrTable>,
}

#[derive(Debug, Clone, Copy)]
pub struct WzImgWriteCtx<'a> {
    pub crypto: &'a WzCrypto,
    pub str_table: &'a RefCell<StrOffsetTable>,
}

impl<'a> WzContext<'a> {
    pub fn new(crypto: &'a WzCrypto) -> Self {
        Self(crypto)
    }
}


impl<'a> From<&WzImgReadCtx<'a>> for WzContext<'a> {
    fn from(ctx: &WzImgReadCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> From<&WzImgWriteCtx<'a>> for WzContext<'a> {
    fn from(ctx: &WzImgWriteCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> From<WzImgReadCtx<'a>> for WzContext<'a> {
    fn from(ctx: WzImgReadCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> From<WzImgWriteCtx<'a>> for WzContext<'a> {
    fn from(ctx: WzImgWriteCtx<'a>) -> Self {
        Self(ctx.crypto)
    }
}

impl<'a> WzImgReadCtx<'a> {
    pub fn new(crypto: &'a WzCrypto, str_table: &'a RefCell<OffsetStrTable>) -> Self {
        Self { crypto, str_table }
    }

    pub fn get_str(&self, offset: u32) -> Option<RcStr> {
        self.str_table.borrow().get(offset)
    }

    pub fn read_str_offset<R: Read + Seek>(
        &self,
        mut r: R,
        endian: binrw::Endian,
    ) -> BinResult<RcStr> {
        // TODO maybe support non-sequential reads?
        // so that the reader jumps to the offset if the string does not exist
        let offset = u32::read_options(&mut r, endian, ())?;
        self.get_str(offset).ok_or_else(|| {
            custom_binrw_error(r, anyhow::format_err!("Missing string at offset {offset}"))
        })
    }

    pub fn read_str<R: Read + Seek>(&self, mut r: R) -> BinResult<RcStr> {
        let offset = r.stream_position()? as u32;
        // TODO make the reading more efficient by using a local buffer
        // and constructing the rc str from that
        let str: RcStr = Rc::from(WzStr::read_le_args(&mut r, self.into())?.as_str());
        self.str_table.borrow_mut().insert(offset, str.clone());
        Ok(str)
    }

    pub fn read_ty_str(&self, mut r: impl Read + Seek, endian: Endian) -> BinResult<RcStr> {
        let magic = u8::read_options(&mut r, endian, ())?;

        Ok(match magic {
            0x1B => self.read_str_offset(r, endian)?,
            0x73 => self.read_str(r)?,
            _ => {
                return Err(binrw::Error::NoVariantMatch {
                    pos: r.stream_position().unwrap_or(0),
                });
            }
        })
    }

    pub fn read_img_str(&self, mut r: impl Read + Seek, endian: Endian) -> BinResult<RcStr> {
        let magic = u8::read_options(&mut r, endian, ())?;

        Ok(match magic {
            1 => self.read_str_offset(r, endian)?,
            0 => self.read_str(r)?,
            _ => {
                return Err(binrw::Error::NoVariantMatch {
                    pos: r.stream_position().unwrap_or(0),
                });
            }
        })
    }
}

impl<'a> WzImgWriteCtx<'a> {
    pub fn new(crypto: &'a WzCrypto, str_table: &'a RefCell<StrOffsetTable>) -> Self {
        Self { crypto, str_table }
    }

    pub fn get_offset(&self, s: &str) -> Option<u32> {
        self.str_table.borrow().get(s)
    }

    pub fn insert_offset(&self, s: &str, offset: u32) -> bool {
        self.str_table.borrow_mut().insert(RcStr::from(s), offset)
    }

    // TODO checkout if offset is before the discriminator or after

    pub fn write_ty_str<W: Write + Seek>(
        &self,
        mut w: W,
        endian: Endian,
        s: &[u8],
    ) -> BinResult<()> {
        // TODO type str should work on u8 slices
        let s = std::str::from_utf8(s).map_err(|e| binrw::Error::Custom {
            pos: w.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        if let Some(offset) = self.get_offset(s) {
            (0x1Bu8).write_options(&mut w, endian, ())?;
            offset.write_options(&mut w, endian, ())
        } else {
            let offset = w.stream_position()? as u32;
            self.insert_offset(s, offset);
            (0x73u8).write_options(&mut w, endian, ())?;
            WzStrRef(s).write_options(&mut w, endian, self.into())
        }
    }

    pub fn write_img_str<W: Write + Seek>(
        &self,
        mut w: W,
        endian: Endian,
        s: &str,
    ) -> BinResult<()> {
        if let Some(offset) = self.get_offset(s) {
            (0x1u8).write_options(&mut w, endian, ())?;
            offset.write_options(&mut w, endian, ())
        } else {
            let offset = w.stream_position()? as u32;
            self.insert_offset(s, offset);
            (0x0u8).write_options(&mut w, endian, ())?;
            WzStrRef(s).write_options(&mut w, endian, self.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::GMS95;

    use super::*;

    #[test]
    fn crypto_string() {
        let s = ["", "a", "mmmmmmmmmmmm", "aaa", "!!!"];
        let cipher = WzCrypto::from_cfg(GMS95, 2);

        let ctx = WzContext::new(&cipher);
        for s in s {
            let mut b = s.as_bytes().to_vec();
            ctx.encode_str8(&mut b);
            ctx.decode_str8(&mut b);
            assert_eq!(s.as_bytes(), b.as_slice());
        }
    }
}

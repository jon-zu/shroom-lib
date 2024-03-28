use std::{
    cell::RefCell,
    io::{Read, Seek, Write},
    rc::Rc,
};

use binrw::{BinRead, BinResult, BinWrite, Endian};

use crate::{
    crypto::ImgCrypto,
    ty::{WzStr, WzStrRef},
    util::{
        custom_binrw_error,
        str_table::{OffsetStrTable, RcStr, StrOffsetTable},
    },
};

#[derive(Debug, Clone, Copy)]
pub struct ReadCtx<'a> {
    pub crypto: &'a ImgCrypto,
    pub str_table: &'a RefCell<OffsetStrTable>,
}

impl<'a> ReadCtx<'a> {
    pub fn new(crypto: &'a ImgCrypto, str_table: &'a RefCell<OffsetStrTable>) -> Self {
        Self { crypto, str_table }
    }

    pub fn get_str(&self, offset: u32) -> Option<RcStr> {
        self.str_table.borrow().get(offset)
    }

    pub fn read_str_offset<R: ImgRead>(
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

    pub fn read_str<R: ImgRead>(&self, mut r: R) -> BinResult<RcStr> {
        let offset = r.stream_position()? as u32;
        // TODO make the reading more efficient by using a local buffer
        // and constructing the rc str from that
        let str: RcStr = Rc::from(WzStr::read_le_args(&mut r, *self)?.as_str());
        self.str_table.borrow_mut().insert(offset, str.clone());
        Ok(str)
    }

    pub fn read_ty_str(&self, mut r: impl ImgRead, endian: Endian) -> BinResult<RcStr> {
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

    pub fn read_img_str(&self, mut r: impl ImgRead, endian: Endian) -> BinResult<RcStr> {
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

#[derive(Debug, Clone, Copy)]
pub struct WriteCtx<'a> {
    pub crypto: &'a ImgCrypto,
    pub str_table: &'a RefCell<StrOffsetTable>,
}

impl<'a> WriteCtx<'a> {
    pub fn new(crypto: &'a ImgCrypto, str_table: &'a RefCell<StrOffsetTable>) -> Self {
        Self { crypto, str_table }
    }

    pub fn get_offset(&self, s: &str) -> Option<u32> {
        self.str_table.borrow().get(s)
    }

    pub fn insert_offset(&self, s: &str, offset: u32) -> bool {
        self.str_table.borrow_mut().insert(RcStr::from(s), offset)
    }

    pub fn write_ty_str<W: ImgWrite>(
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
            WzStrRef(s).write_options(&mut w, endian, *self)
        }
    }

    pub fn write_img_str<W: ImgWrite>(
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
            WzStrRef(s).write_options(&mut w, endian, *self)
        }
    }
}

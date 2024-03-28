use binrw::{BinRead, BinReaderExt, BinResult, BinWrite};
use std::{
    collections::HashMap,
    io::{Read, Seek},
};

use arcstr::ArcStr;

use crate::{
    crypto::ImgCrypto,
    error::ImgError,
    ty::{WzStr, WzStrRef},
    writer::ImgWrite,
    Offset,
};

// Non-string offset caps
// bookName(8  for prop key)
// snail(5) for prop value

#[derive(Debug, Default)]
pub struct StrOffsetTable(HashMap<ArcStr, Offset>);

impl StrOffsetTable {
    pub fn get(&self, s: &str) -> Option<Offset> {
        self.0.get(s).copied()
    }

    pub fn insert(&mut self, s: ArcStr, offset: Offset) -> bool {
        self.0.insert(s, offset).is_none()
    }

    pub fn get_offset(&self, s: &str) -> Option<&u32> {
        self.0.get(s)
    }

    pub fn insert_offset(&mut self, s: &str, offset: u32) -> bool {
        self.0.insert(ArcStr::from(s), offset).is_none()
    }

    fn write_new_str(
        &mut self,
        mut w: impl ImgWrite,
        crypto: &ImgCrypto,
        tag: u8,
        s: &str,
    ) -> BinResult<u64> {
        let offset = w.stream_position()? + 1;
        (tag).write_le(&mut w)?;
        WzStrRef(s).write_le_args(&mut w, crypto)?;
        Ok(offset)
    }

    fn write_table_str(
        &mut self,
        mut w: impl ImgWrite,
        crypto: &ImgCrypto,
        s: &str,
        tag_table: u8,
        tag_newstr: u8,
    ) -> BinResult<()> {
        if s.len() < 5 {
            self.write_new_str(&mut w, crypto, tag_newstr, s)?;
            return Ok(());
        }

        if let Some(offset) = self.get_offset(s) {
            (tag_table).write_le(&mut w)?;
            offset.write_le(&mut w)
        } else {
            let offset = self.write_new_str(&mut w, crypto, tag_newstr, s)?;
            self.insert_offset(s, offset as u32);
            Ok(())
        }
    }

    pub fn write_ty_str(
        &mut self,
        mut w: impl ImgWrite,
        crypto: &ImgCrypto,
        s: &[u8],
    ) -> BinResult<()> {
        // TODO type str should work on u8 slices
        let s = std::str::from_utf8(s).map_err(|e| binrw::Error::Custom {
            pos: w.stream_position().unwrap_or(0),
            err: Box::new(e),
        })?;

        self.write_table_str(w, crypto, s, 0x1Bu8, 0x73u8)
    }

    pub fn write_img_str(
        &mut self,
        w: impl ImgWrite,
        crypto: &ImgCrypto,
        s: &str,
    ) -> BinResult<()> {
        self.write_table_str(w, crypto, s, 1, 0)
    }
}

#[derive(Debug, Default)]
pub struct OffsetStrTable(HashMap<Offset, ArcStr>);

impl OffsetStrTable {
    pub fn get(&self, offset: Offset) -> Option<&ArcStr> {
        self.0.get(&offset)
    }

    pub fn read_str_offset<R: Read + Seek>(&mut self, r: &mut R) -> BinResult<&ArcStr> {
        // TODO maybe support non-sequential reads?
        // so that the reader jumps to the offset if the string does not exist
        let offset: u32 = r.read_le()?;
        self.0
            .get(&offset)
            .ok_or_else(|| ImgError::UnknownStringOffset(offset).binrw_error(r))
    }

    pub fn read_str<R: Read + Seek>(
        &mut self,
        r: &mut R,
        crypto: &ImgCrypto,
    ) -> BinResult<&ArcStr> {
        let offset = r.stream_position()? as u32;
        // TODO read the string straight from the reader to avoid copyings
        let str = ArcStr::from(WzStr::read_le_args(r, crypto)?.0.as_str());
        self.0.insert(offset, str);
        Ok(self.0.get(&offset).unwrap())
    }

    fn read_table_str<R: Read + Seek>(
        &mut self,
        r: &mut R,
        crypto: &ImgCrypto,
        tag_table: u8,
        tag_newstr: u8,
    ) -> BinResult<&ArcStr> {
        let tag: u8 = r.read_le()?;

        if tag == tag_table {
            self.read_str_offset(r)
        } else if tag == tag_newstr {
            self.read_str(r, crypto)
        } else {
            Err(binrw::Error::BadMagic {
                pos: r.stream_position().unwrap(),
                found: Box::new(tag),
            })
        }
    }

    pub fn read_ty_str<R: Read + Seek>(
        &mut self,
        r: &mut R,
        crypto: &ImgCrypto,
    ) -> BinResult<&ArcStr> {
        self.read_table_str(r, crypto, 0x1B, 0x73)
    }

    pub fn read_img_str<R: Read + Seek>(
        &mut self,
        r: &mut R,
        crypto: &ImgCrypto,
    ) -> BinResult<&ArcStr> {
        self.read_table_str(r, crypto, 1, 0)
    }
}

pub struct ReadStrCtx<'a> {
    pub crypto: &'a ImgCrypto,
    pub str_table: &'a mut OffsetStrTable,
}

pub struct WriteStrCtx<'a> {
    pub crypto: &'a ImgCrypto,
    pub str_table: &'a mut StrOffsetTable,
}

#[derive(Debug, Clone)]
pub struct ImgStr(pub ArcStr);

impl BinRead for ImgStr {
    type Args<'a> = ReadStrCtx<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        Ok(Self(
            args.str_table.read_img_str(reader, args.crypto)?.clone(),
        ))
    }
}

impl BinWrite for ImgStr {
    type Args<'a> = WriteStrCtx<'a>;

    fn write_options<W: ImgWrite>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        args.str_table
            .write_img_str(writer, args.crypto, self.0.as_str())
    }
}

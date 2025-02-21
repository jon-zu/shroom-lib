use std::collections::HashSet;

use binrw::{BinRead, BinReaderExt, BinWrite, BinWriterExt, helpers::until_eof_with};
use shroom_img::{crypto::ImgCrypto, util::custom_binrw_error};

#[derive(Debug, Clone)]
pub struct ArchiveImgEntry(pub String);

impl BinRead for ArchiveImgEntry {
    type Args<'a> = &'a ImgCrypto;

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let len = reader.read_type::<u32>(endian)? as usize;
        let mut buf = vec![0u16; len + 1];
        reader.read_exact(bytemuck::cast_slice_mut(&mut buf))?;
        args.crypt(bytemuck::cast_slice_mut(&mut buf));
        Ok(Self(
            String::from_utf16(&buf[..len]).map_err(|err| custom_binrw_error(reader, err))?,
        ))
    }
}

impl BinWrite for ArchiveImgEntry {
    type Args<'a> = &'a ImgCrypto;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        let mut buf = self.0.encode_utf16().collect::<Vec<_>>();
        let len = buf.len() as u32;
        buf.push(0);
        args.crypt(bytemuck::cast_slice_mut(&mut buf));
        writer.write_type(&len, endian)?;
        writer.write_all(bytemuck::cast_slice(&buf))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveImgList(pub Vec<ArchiveImgEntry>);

impl BinRead for ArchiveImgList {
    type Args<'a> = &'a ImgCrypto;

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        Ok(Self(until_eof_with(|r, endian, args| {
            ArchiveImgEntry::read_options(r, endian, args)
        })(reader, endian, args)?))
    }
}

impl BinWrite for ArchiveImgList {
    type Args<'a> = &'a ImgCrypto;

    fn write_options<W: std::io::prelude::Write + std::io::prelude::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        self.0.write_options(writer, endian, args)
    }
}

#[derive(Debug)]
pub struct ListImgSet(HashSet<String>);

impl Default for ListImgSet {
    fn default() -> Self {
        Self::new()
    }
}

impl ListImgSet {
    pub fn new() -> Self {
        Self(HashSet::new())
    }

    pub fn from_list(list: ArchiveImgList) -> Self {
        Self(
            list.0
                .into_iter()
                .map(|entry| entry.0.to_ascii_lowercase())
                .collect(),
        )
    }

    pub fn contains(&self, s: &str) -> bool {
        self.0.contains(&s.to_ascii_lowercase())
    }

    pub fn to_list(&self) -> ArchiveImgList {
        ArchiveImgList(self.0.iter().map(|s| ArchiveImgEntry(s.clone())).collect())
    }
}

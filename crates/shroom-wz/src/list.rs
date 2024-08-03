use binrw::{helpers::until_eof_with, BinRead, BinReaderExt, BinWrite, BinWriterExt};
use shroom_img::util::custom_binrw_error;

use crate::WzContext;

#[derive(Debug, Clone)]
pub struct ArchiveImgEntry(pub String);

impl BinRead for ArchiveImgEntry {
    type Args<'a> = &'a WzContext;

    fn read_options<R: std::io::prelude::Read + std::io::prelude::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let len = reader.read_type::<u32>(endian)? as usize;
        if len % 2 != 0 || len == 0 {
            return Err(custom_binrw_error(
                reader,
                anyhow::anyhow!("List Entry name invalid string length"),
            ));
        }

        let mut buf = vec![0u16; len];
        args.img.crypt(bytemuck::cast_slice_mut(&mut buf));
        reader.read_exact(bytemuck::cast_slice_mut(&mut buf[..len - 1]))?;
        Ok(Self(
            String::from_utf16(&buf).map_err(|err| custom_binrw_error(reader, err))?,
        ))
    }
}

impl BinWrite for ArchiveImgEntry {
    type Args<'a> = &'a WzContext;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        let mut buf = self.0.encode_utf16().collect::<Vec<_>>();
        buf.push(0);
        args.img.crypt(bytemuck::cast_slice_mut(&mut buf));
        writer.write_type(&(buf.len() as u32), endian)?;
        writer.write_all(bytemuck::cast_slice(&buf))?;
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct ArchiveImgList(pub Vec<ArchiveImgEntry>);

impl BinRead for ArchiveImgList {
    type Args<'a> = &'a WzContext;

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
    type Args<'a> = &'a WzContext;

    fn write_options<W: std::io::prelude::Write + std::io::prelude::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        self.0.write_options(writer, endian, args)
    }
}

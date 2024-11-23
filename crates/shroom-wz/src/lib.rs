pub mod reader;
pub mod writer;
pub mod list;

use std::io::{self, Read, Seek};

use binrw::{binrw, BinRead, BinWrite, NullString};

use shroom_crypto::wz::offset_cipher::WzOffsetCipher;
use shroom_img::{
    crypto::ImgCrypto,
    ty::{WzInt, WzStr, WzVec}, util::custom_binrw_error,
};

#[derive(Debug)]
pub struct WzContext {
    img: ImgCrypto,
    wz: WzOffsetCipher,
    base_offset: u32,
}

/// Offset in the Wz file
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct WzOffset(pub u32);

impl From<WzOffset> for u32 {
    fn from(value: WzOffset) -> Self {
        value.0
    }
}

impl From<WzOffset> for u64 {
    fn from(value: WzOffset) -> u64 {
        u64::from(value.0)
    }
}

impl BinRead for WzOffset {
    type Args<'a> = &'a WzContext;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let pos = reader.stream_position()? as u32;
        let v = u32::read_options(reader, endian, ())?;
        let offset = args.wz.decrypt_offset(args.base_offset, v, pos);
        Ok(Self(offset))
    }
}

impl BinWrite for WzOffset {
    type Args<'a> = &'a WzContext;

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        let pos = writer.stream_position()? as u32;
        let enc_off = args.wz.encrypt_offset(args.base_offset, self.0, pos);
        enc_off.write_options(writer, endian, ())
    }
}

pub const WZ_DIR_NULL: u8 = 1;
pub const WZ_DIR_LINK: u8 = 2;
pub const WZ_DIR_DIR: u8 = 3;
pub const WZ_DIR_IMG: u8 = 4;

/// Header of a WZ file
#[binrw]
#[brw(little)]
#[brw(magic = b"PKG1")]
#[derive(Debug)]
pub struct WzHeader {
    pub file_size: u64,
    pub data_offset: u32,
    pub desc: NullString,
}

/// Directory with entries
#[binrw]
#[brw(little, import_raw(ctx: &WzContext))]
#[derive(Debug)]
pub struct WzDir(#[brw(args_raw(ctx))] pub WzVec<WzDirEntry>);

impl WzDir {
    pub fn get(&self, name: &str) -> Option<&WzDirEntry> {
        self.0.iter().find(|e| match e {
            WzDirEntry::Null(_) => false,
            WzDirEntry::Link(_) => false, // TODO: should this be handled
            WzDirEntry::Dir(dir) => dir.name.0 == name,
            WzDirEntry::Img(img) => img.name.0 == name,
        })
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut WzDirEntry> {
        self.0.iter_mut().find(|e| match e {
            WzDirEntry::Null(_) => false,
            WzDirEntry::Link(_) => false, // TODO: should this be handled
            WzDirEntry::Dir(dir) => dir.name.0 == name,
            WzDirEntry::Img(img) => img.name.0 == name,
        })
    }
}

/// Header of a
#[binrw]
#[brw(little, import_raw(ctx: &WzContext))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzImgHeader {
    #[brw(args_raw(&ctx.img))]
    pub name: WzStr,
    pub blob_size: WzInt,
    pub checksum: WzInt,
    #[brw(args_raw(ctx))]
    pub offset: WzOffset,
}

#[binrw]
#[brw(little, import_raw(ctx: &WzContext))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzDirHeader {
    #[brw(args_raw(&ctx.img))]
    pub name: WzStr,
    pub blob_size: WzInt,
    pub checksum: WzInt,
    #[brw(args_raw(ctx))]
    pub offset: WzOffset,
}

impl WzDirHeader {
    pub fn root(name: &str, root_size: usize, offset: WzOffset) -> Self {
        Self {
            name: WzStr::new(name.to_string()),
            blob_size: WzInt(root_size as i32),
            checksum: WzInt(1),
            offset,
        }
    }
}

#[binrw]
#[brw(little, import_raw(ctx: &WzContext))]
#[derive(Debug, Clone, PartialEq)]
pub struct WzLinkHeader {
    #[brw(args_raw(ctx))]
    pub link: WzLinkData,
    pub blob_size: WzInt,
    pub checksum: WzInt,
    #[brw(args_raw(ctx))]
    pub offset: WzOffset,
}

pub type WzNullHeader = [u8; 10];

#[derive(BinRead, BinWrite, Debug, Clone, PartialEq)]
#[brw(little, import_raw(ctx: &WzContext))]
pub enum WzDirEntry {
    #[brw(magic(1u8))]
    Null(WzNullHeader),
    #[brw(magic(2u8))]
    Link(#[brw(args_raw(ctx))] WzLinkHeader),
    #[brw(magic(3u8))]
    Dir(#[brw(args_raw(ctx))] WzDirHeader),
    #[brw(magic(4u8))]
    Img(#[brw(args_raw(ctx))] WzImgHeader),
}

impl WzDirEntry {
    pub fn name(&self) -> Option<&WzStr> {
        match self {
            WzDirEntry::Null(_) => None,
            WzDirEntry::Link(link) => Some(&link.link.link_img.name),
            WzDirEntry::Dir(dir) => Some(&dir.name),
            WzDirEntry::Img(img) => Some(&img.name),
        }
    }

    pub fn as_null(&self) -> Option<()> {
        match self {
            WzDirEntry::Null(_) => Some(()),
            _ => None,
        }
    }

    pub fn as_link(&self) -> Option<&WzLinkHeader> {
        match self {
            WzDirEntry::Link(link) => Some(link),
            _ => None,
        }
    }

    pub fn as_dir(&self) -> Option<&WzDirHeader> {
        match self {
            WzDirEntry::Dir(dir) => Some(dir),
            _ => None,
        }
    }

    pub fn as_img(&self) -> Option<&WzImgHeader> {
        match self {
            WzDirEntry::Img(img) => Some(img),
            _ => None,
        }
    }
}

#[derive(Debug, PartialEq, Clone)]
pub struct WzLinkData {
    pub offset: u32,
    pub link_img: WzImgHeader,
}

impl BinRead for WzLinkData {
    type Args<'a> = &'a WzContext;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let offset = u32::read_options(reader, endian, ())?;
        let old_pos = reader.stream_position()?;

        let link_offset = args.base_offset as u64 + offset as u64;
        reader.seek(io::SeekFrom::Start(link_offset))?;

        let ty = u8::read_options(reader, endian, ())?;
        if ty != WZ_DIR_IMG {
            // TODO: Support directories here?
            return Err(custom_binrw_error(
                reader,
                anyhow::format_err!("Expected link type Img, got {ty}"),
            ));
        }

        let link_img = WzImgHeader::read_options(reader, endian, args)?;
        // Seek back
        reader.seek(io::SeekFrom::Start(old_pos))?;

        Ok(Self { offset, link_img })
    }
}

impl BinWrite for WzLinkData {
    type Args<'a> = &'a WzContext;

    fn write_options<W: io::Write + io::Seek>(
        &self,
        _writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        unimplemented!()
    }
}

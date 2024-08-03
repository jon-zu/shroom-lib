use std::io;

use crate::{ctx::WzContext, util::custom_binrw_error};
use binrw::{binrw, BinRead, BinWrite, NullString};

use crate::ty::{WzInt, WzOffset, WzStr, WzVec};

pub mod archive_list;

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
#[brw(little, import_raw(ctx: WzContext<'_>))]
#[derive(Debug, Clone, derive_more::From, derive_more::Into, derive_more::IntoIterator)]
#[into_iterator(owned, ref, ref_mut)]
pub struct WzDir(#[brw(args_raw(ctx))] pub WzVec<WzDirEntry>);

impl WzDir {
    pub fn get(&self, name: &str) -> Option<&WzDirEntry> {
        self.0.iter().find(|e| match e {
            WzDirEntry::Null(_) => false,
            WzDirEntry::Link(_) => false, // TODO: should this be handled
            WzDirEntry::Dir(dir) => dir.name.as_str() == name,
            WzDirEntry::Img(img) => img.name.as_str() == name,
        })
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut WzDirEntry> {
        self.0.iter_mut().find(|e| match e {
            WzDirEntry::Null(_) => false,
            WzDirEntry::Link(_) => false, // TODO: should this be handled
            WzDirEntry::Dir(dir) => dir.name.as_str() == name,
            WzDirEntry::Img(img) => img.name.as_str() == name,
        })
    }
}

/// Header of a
#[binrw]
#[brw(little, import_raw(ctx: WzContext<'_>))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzImgHeader {
    #[brw(args_raw(ctx))]
    pub name: WzStr,
    pub blob_size: WzInt,
    pub checksum: WzInt,
    #[brw(args_raw(ctx))]
    pub offset: WzOffset,
}

#[binrw]
#[brw(little, import_raw(ctx: WzContext<'_>))]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzDirHeader {
    #[brw(args_raw(ctx))]
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
#[brw(little, import_raw(ctx: WzContext<'_>))]
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

#[derive(BinRead, BinWrite, Debug, Clone, PartialEq, derive_more::From, derive_more::TryInto)]
#[try_into(owned, ref, ref_mut)]
#[brw(little, import_raw(ctx: WzContext<'_>))]
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
    type Args<'a> = &'a WzContext<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let offset = u32::read_options(reader, endian, ())?;
        let old_pos = reader.stream_position()?;
        let abs = args.
        reader.seek(io::SeekFrom::Start(args.0.offset_link(offset)))?;

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
    type Args<'a> = WzContext<'a>;

    fn write_options<W: io::Write + io::Seek>(
        &self,
        _writer: &mut W,
        _endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        todo!()
    }
}

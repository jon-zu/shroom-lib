use std::io;

use crate::ctx::WzContext;
use binrw::{binrw, BinRead, BinWrite, NullString};

use crate::ty::{WzInt, WzOffset, WzStr, WzVec};



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
#[derive(Debug, Clone)]
pub struct WzDir {
    #[brw(args_raw(ctx))]
    pub entries: WzVec<WzDirEntry>,
}

impl WzDir {
    pub fn get(&self, name: &str) -> Option<&WzDirEntry> {
        self.entries.0.iter().find(|e| match e {
            WzDirEntry::Null(_) => false,
            WzDirEntry::Link(_) => false, // TODO: should this be handled
            WzDirEntry::Dir(dir) => dir.name.as_str() == name,
            WzDirEntry::Img(img) => img.name.as_str() == name,
        })
    }

    pub fn get_mut(&mut self, name: &str) -> Option<&mut WzDirEntry> {
        self.entries.0.iter_mut().find(|e| match e {
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

#[derive(BinRead, BinWrite, Debug, Clone, PartialEq)]
#[brw(little, import_raw(ctx: WzContext<'_>))]
pub enum WzDirEntry {
    #[brw(magic(1u8))]
    Null([u8; 10]),
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
    type Args<'a> = WzContext<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let offset = u32::read_options(reader, endian, ())?;
        let old_pos = reader.stream_position()?;
        reader.seek(io::SeekFrom::Start(args.0.offset_link(offset)))?;

        let ty = u8::read_options(reader, endian, ())?;
        if ty != 4 {
            // TODO: support dirs? and return a proper erro
            panic!("Expected link type Img, got {}", ty);
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
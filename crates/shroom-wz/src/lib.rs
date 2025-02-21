pub mod list;
pub mod reader;
pub mod writer;

pub use shroom_crypto::ShroomVersion;

use std::{
    io::{self, Cursor, Read, Seek, Write},
    ops::Deref,
    sync::Arc,
};

use binrw::{BinRead, BinResult, BinWrite, NullString, binrw};

use list::{ArchiveImgList, ListImgSet};
use shroom_crypto::{default_keys::wz::DEFAULT_WZ_OFFSET_MAGIC, wz::offset_cipher::WzOffsetCipher};
use shroom_img::{
    crypto::ImgCrypto,
    ty::{WzInt, WzStr, WzVec},
};

pub fn try_detect_versions<R: Read>(mut r: R) -> BinResult<Vec<ShroomVersion>> {
    let mut buf = [0; 128];
    r.read_exact(&mut buf)?;

    let mut r = Cursor::new(&buf);
    let hdr = WzHeader::read(&mut r)?;

    let all = ShroomVersion::wz_detect_version(hdr.version_hash);
    Ok(ShroomVersion::wz_detect_version(hdr.version_hash).collect())
}

pub fn try_detect_file_versions(
    path: impl AsRef<std::path::Path>,
) -> BinResult<Vec<ShroomVersion>> {
    let file = std::fs::File::open(path)?;
    try_detect_versions(file)
}

#[derive(Debug)]
pub struct WzContext {
    img: Arc<ImgCrypto>,
    wz: WzOffsetCipher,
    chunked_set: ListImgSet,
    ver: ShroomVersion,
}

impl WzContext {
    pub fn new(ver: impl Into<ShroomVersion>, img: Arc<ImgCrypto>) -> Self {
        let ver = ver.into();
        Self {
            img: img,
            wz: WzOffsetCipher::new(ver, DEFAULT_WZ_OFFSET_MAGIC),
            chunked_set: ListImgSet::new(),
            ver: ver,
        }
    }
    pub fn global(ver: impl Into<ShroomVersion>) -> Self {
        Self::new(ver, ImgCrypto::global().into())
    }

    pub fn kms(ver: impl Into<ShroomVersion>) -> Self {
        Self::new(ver, ImgCrypto::kms().into())
    }

    pub fn shared(self) -> Arc<Self> {
        Arc::new(self)
    }

    pub fn load_img_set(&mut self, mut reader: impl Read + Seek) -> anyhow::Result<()> {
        let list = ArchiveImgList::read_le_args(&mut reader, &self.img)?;
        self.chunked_set = ListImgSet::from_list(list);
        Ok(())
    }

    pub fn write_img_set(&self, mut writer: impl Write + Seek) -> anyhow::Result<()> {
        self.chunked_set
            .to_list()
            .write_le_args(&mut writer, &self.img)?;
        Ok(())
    }

    pub fn img_crypto(&self) -> Arc<ImgCrypto> {
        self.img.clone()
    }
}

#[derive(Debug)]
pub struct WzCryptContext {
    ctx: Arc<WzContext>,
    base_offset: u32,
}

impl WzCryptContext {
    pub fn new(ctx: Arc<WzContext>, base_offset: u32) -> Self {
        Self { ctx, base_offset }
    }
}

impl Deref for WzCryptContext {
    type Target = WzContext;
    fn deref(&self) -> &WzContext {
        &self.ctx
    }
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
    type Args<'a> = &'a WzCryptContext;

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
    type Args<'a> = &'a WzCryptContext;

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
    pub version_hash: u16,
}

/// Directory with entries
#[binrw]
#[brw(little, import_raw(ctx: &WzCryptContext))]
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
#[brw(little, import_raw(ctx: &WzCryptContext))]
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
#[brw(little, import_raw(ctx: &WzCryptContext))]
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
#[brw(little, import_raw(ctx: &WzCryptContext))]
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
#[brw(little, import_raw(ctx: &WzCryptContext))]
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
            WzDirEntry::Link(_link) => todo!(), //Some(&link.link.link_img.name),
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
    //pub link_img: WzImgHeader,
}

impl BinRead for WzLinkData {
    type Args<'a> = &'a WzCryptContext;

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let offset = u32::read_options(reader, endian, ())?;

        Ok(Self { offset })
    }
}

impl BinWrite for WzLinkData {
    type Args<'a> = &'a WzCryptContext;

    fn write_options<W: io::Write + io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        self.offset.write_options(writer, endian, ())
    }
}

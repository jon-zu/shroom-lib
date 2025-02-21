use std::{io::{Read, Seek}, ops::Deref, sync::Arc};

use binrw::{BinRead, BinWrite};
use crypto::ImgCrypto;
use error::ImgError;
use serde::{Deserialize, Serialize};
use str_table::{ImgStr, ReadStrCtx, WriteStrCtx};
use ty::{WzF32, WzInt, WzLong};

pub mod canvas;
pub mod crypto;
pub mod data;
pub mod error;
pub mod json;
pub mod reader;
pub mod sound;
pub mod str_table;
pub mod ty;
pub mod util;
pub mod value;
pub mod writer;

pub type Offset = u32;

#[derive(Debug, PartialEq, Eq, Clone)]
pub enum CanvasDataFlag {
    None,
    Chunked,
    AutoDetect
}

#[derive(Debug, Clone)]
pub struct ImgContext {
    crypto: Arc<ImgCrypto>,
    data_flag: CanvasDataFlag
}

impl ImgContext {
    pub fn new(crypto: Arc<ImgCrypto>) -> Self {
        Self {
            data_flag: CanvasDataFlag::AutoDetect,
            crypto
        }
    }

    pub fn with_flag(crypto: Arc<ImgCrypto>, flag: CanvasDataFlag) -> Self {
        Self {
            data_flag: flag,
            crypto
        }
    }

    pub fn global() -> Self  {
        Self {
            data_flag: CanvasDataFlag::AutoDetect,
            crypto: Arc::new(ImgCrypto::global())
        }
    }


}

impl From<Arc<ImgCrypto>> for ImgContext {
    fn from(crypto: Arc<ImgCrypto>) -> Self {
        Self {
            data_flag: CanvasDataFlag::None,
            crypto
        }
    }
}

impl Deref for ImgContext {
    type Target = ImgCrypto;

    fn deref(&self) -> &Self::Target {
        &self.crypto
    }
}

pub const OBJ_TYPE_PROPERTY: &[u8] = b"Property";
pub const OBJ_TYPE_CANVAS: &[u8] = b"Canvas";
pub const OBJ_TYPE_UOL: &[u8] = b"UOL";
pub const OBJ_TYPE_VEC2: &[u8] = b"Shape2D#Vector2D";
pub const OBJ_TYPE_CONVEX2D: &[u8] = b"Shape2D#Convex2D";
pub const OBJ_TYPE_SOUND_DX8: &[u8] = b"Sound_DX8";

#[derive(Debug, PartialEq, Eq, Clone, Hash)]
pub enum ObjTypeTag {
    Property,
    Vec2,
    Convex2,
    Link,
    Canvas,
    Sound,
}

impl BinRead for ObjTypeTag {
    type Args<'a> = ReadStrCtx<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let ty_str = args.str_table.read_ty_str(reader, args.crypto)?;
        Ok(match ty_str.as_bytes() {
            OBJ_TYPE_PROPERTY => Self::Property,
            OBJ_TYPE_VEC2 => Self::Vec2,
            OBJ_TYPE_CONVEX2D => Self::Convex2,
            OBJ_TYPE_UOL => Self::Link,
            OBJ_TYPE_CANVAS => Self::Canvas,
            OBJ_TYPE_SOUND_DX8 => Self::Sound,
            _ => return Err(ImgError::UnknownObjectType(ty_str.to_string()).binrw_error(reader)),
        })
    }
}

impl BinWrite for ObjTypeTag {
    type Args<'a> = WriteStrCtx<'a>;

    fn write_options<W: std::io::prelude::Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        let ty_str = match self {
            Self::Property => OBJ_TYPE_PROPERTY,
            Self::Vec2 => OBJ_TYPE_VEC2,
            Self::Convex2 => OBJ_TYPE_CONVEX2D,
            Self::Link => OBJ_TYPE_UOL,
            Self::Canvas => OBJ_TYPE_CANVAS,
            Self::Sound => OBJ_TYPE_SOUND_DX8,
        };

        args.str_table.write_ty_str(writer, args.crypto, ty_str)?;

        Ok(())
    }
}

#[derive(Debug, BinRead, BinWrite)]
pub struct ObjectHeader {
    /// Size in bytes
    pub size: u32,
}

/// A value of a property, very close to `VARIANT` windows
#[derive(Debug, BinRead, BinWrite)]
#[br(little, import_raw(ctx: ReadStrCtx<'_>))]
#[bw(little, import_raw(ctx: WriteStrCtx<'_>))]
pub enum PropertyValue {
    #[brw(magic(0u8))]
    Empty,
    #[brw(magic(11u8))]
    Bool(u8),
    #[brw(magic(2u8))]
    I16(i16),
    #[brw(magic(0x12u8))]
    U16(u16),
    #[brw(magic(3u8))]
    I32(
        #[br(map = |x: WzInt| x.0)]
        #[bw(map = |x: &i32| WzInt(*x))]
        i32,
    ),
    #[brw(magic(0x13u8))]
    U32(
        #[br(map = |x: WzInt| x.0 as u32)]
        #[bw(map = |x: &u32| WzInt(*x as i32))]
        u32,
    ),
    #[brw(magic(0x14u8))]
    I64(
        #[br(map = |x: WzLong| x.0)]
        #[bw(map = |x: &i64| WzLong(*x))]
        i64,
    ),
    #[brw(magic(4u8))]
    F32(
        #[br(map = |x: WzF32| x.0)]
        #[bw(map = |x: &f32| WzF32(*x))]
        f32,
    ),
    #[brw(magic(5u8))]
    F64(f64),
    #[brw(magic(8u8))]
    String(#[brw(args_raw(ctx))] ImgStr),
    #[brw(magic(9u8))]
    Object(ObjectHeader),
    #[brw(magic(13u8))]
    Unknown(ObjectHeader),
}

#[derive(Debug, Deserialize, Serialize, BinRead, BinWrite, Clone, PartialEq, PartialOrd)]
pub struct Vec2 {
    #[br(map = |x: WzInt| x.0)]
    #[bw(map = |x: &i32| WzInt(*x))]
    pub x: i32,
    #[br(map = |x: WzInt| x.0)]
    #[bw(map = |x: &i32| WzInt(*x))]
    pub y: i32,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq, PartialOrd)]
pub struct Convex2(pub Vec<Vec2>);

impl BinRead for Convex2 {
    type Args<'a> = ReadStrCtx<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<Self> {
        let len = WzInt::read_options(reader, endian, ())?.0 as usize;
        let mut v = Vec::with_capacity(len);

        for _ in 0..len {
            let ty = ObjTypeTag::read_options(reader, endian, ReadStrCtx {
                str_table: args.str_table,
                crypto: args.crypto,
            })?;
            if ty != ObjTypeTag::Vec2 {
                return Err(ImgError::NoVec2InConvex.binrw_error(reader));
            }

            v.push(Vec2::read_options(reader, endian, ())?);
        }

        Ok(Self(v))
    }
}

impl BinWrite for Convex2 {
    type Args<'a> = WriteStrCtx<'a>;

    fn write_options<W: std::io::prelude::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        let len = WzInt(self.0.len() as i32);
        len.write_options(writer, endian, ())?;

        for vec in &self.0 {
            ObjTypeTag::Vec2.write_options(
                writer,
                endian,
                WriteStrCtx {
                    str_table: args.str_table,
                    crypto: args.crypto,
                },
            )?;
            vec.write_options(writer, endian, ())?;
        }

        Ok(())
    }
}

#[derive(Debug, BinRead, BinWrite)]
#[br(little, import_raw(ctx: ReadStrCtx<'_>), magic = 0u8)]
#[bw(little, import_raw(ctx: WriteStrCtx<'_>), magic = 0u8)]
pub struct Link(#[brw(args_raw(ctx))] pub ImgStr);

#[derive(Debug, BinRead, BinWrite)]
#[brw(little, magic = 0u16)]
pub struct Property(
    #[br(map = |x: WzInt| x.0 as u32)]
    #[bw(map = |x: &u32| WzInt(*x as i32))]
    /// Item length
    pub u32,
);

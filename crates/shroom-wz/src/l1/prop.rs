use std::{
    io::{Read, Seek},
    ops::{Deref, DerefMut},
};

use binrw::{binrw, BinRead, BinWrite, BinWriterExt};

use crate::{
    ctx::{WzImgReadCtx, WzImgWriteCtx},
    ty::{WzF32, WzInt, WzLong, WzVec},
    util::custom_binrw_error,
};

use super::{
    obj::{WzObject, OBJ_TYPE_VEC2},
    str::WzImgStr,
};

#[derive(Debug, Clone, derive_more::Deref, derive_more::DerefMut)]
pub struct WzObjectValue(pub Box<WzObject>);

impl BinRead for WzObjectValue {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: Read + Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let len = u32::read_options(reader, endian, ())? as u64;
        let pos = reader.stream_position()?;
        let obj = Box::new(WzObject::read_options(reader, endian, args)?);
        // We don't read canvas/sound data so we need to skip It
        let after = pos + len;
        reader.seek(std::io::SeekFrom::Start(after))?;

        Ok(Self(obj))
    }
}

impl BinWrite for WzObjectValue {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        // Write a dummy length
        writer.write_type(&0u32, endian)?;

        // Write the object and record the length
        let pos = writer.stream_position()?;
        writer.write_type_args(self.deref(), endian, args)?;
        let end = writer.stream_position()?;
        let len = end - pos;

        // Write the actual length
        writer.seek(std::io::SeekFrom::Start(pos - 4))?;
        writer.write_type(&(len as u32), endian)?;
        writer.seek(std::io::SeekFrom::Start(end))?;

        Ok(())
    }
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub enum WzPropValue {
    // Null value
    #[brw(magic(0u8))]
    Null,

    // Short
    #[brw(magic(2u8))]
    Short1(i16),
    #[brw(magic(11u8))]
    Short2(i16),

    // Int
    #[brw(magic(3u8))]
    Int1(WzInt),
    #[brw(magic(19u8))]
    Int2(WzInt),

    // Long
    #[brw(magic(20u8))]
    Long(WzLong),

    // Floats
    #[brw(magic(4u8))]
    F32(WzF32),
    #[brw(magic(5u8))]
    F64(f64),

    #[brw(magic(8u8))]
    Str(#[brw(args_raw(ctx))] WzImgStr),

    #[brw(magic(9u8))]
    Obj(#[brw(args_raw(ctx))] WzObjectValue),
}

impl WzPropValue {
    pub fn as_str(&self) -> Option<&WzImgStr> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_str_mut(&mut self) -> Option<&mut WzImgStr> {
        match self {
            Self::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_obj(&self) -> Option<&WzObject> {
        match self {
            Self::Obj(o) => Some(o.deref()),
            _ => None,
        }
    }

    pub fn as_obj_mut(&mut self) -> Option<&mut WzObject> {
        match self {
            Self::Obj(o) => Some(o.deref_mut()),
            _ => None,
        }
    }

    pub fn as_short(&self) -> Option<i16> {
        match self {
            Self::Short1(s) => Some(*s),
            Self::Short2(s) => Some(*s),
            _ => None,
        }
    }

    pub fn as_short_mut(&mut self) -> Option<&mut i16> {
        match self {
            Self::Short1(s) => Some(s),
            Self::Short2(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<WzInt> {
        match self {
            Self::Int1(i) => Some(*i),
            Self::Int2(i) => Some(*i),
            _ => None,
        }
    }

    pub fn as_int_mut(&mut self) -> Option<&mut WzInt> {
        match self {
            Self::Int1(i) => Some(i),
            Self::Int2(i) => Some(i),
            _ => None,
        }
    }

    pub fn as_long(&self) -> Option<WzLong> {
        match self {
            Self::Long(l) => Some(*l),
            _ => None,
        }
    }

    pub fn as_long_mut(&mut self) -> Option<&mut WzLong> {
        match self {
            Self::Long(l) => Some(l),
            _ => None,
        }
    }

    pub fn as_f32(&self) -> Option<WzF32> {
        match self {
            Self::F32(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_f32_mut(&mut self) -> Option<&mut WzF32> {
        match self {
            Self::F32(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Self::F64(f) => Some(*f),
            _ => None,
        }
    }

    pub fn as_f64_mut(&mut self) -> Option<&mut f64> {
        match self {
            Self::F64(f) => Some(f),
            _ => None,
        }
    }

    pub fn as_null(&self) -> Option<()> {
        match self {
            Self::Null => Some(()),
            _ => None,
        }
    }
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzPropertyEntry {
    #[brw(args_raw(ctx))]
    pub name: WzImgStr,
    #[brw(args_raw(ctx))]
    pub value: WzPropValue,
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzProperty {
    pub unknown: u16,
    #[brw(args_raw(ctx))]
    pub entries: WzVec<WzPropertyEntry>,
}

impl WzProperty {
    pub fn get(&self, name: &str) -> Option<&WzPropValue> {
        self.entries
            .0
            .iter()
            .find(|e| e.name.as_ref() == name)
            .map(|e| &e.value)
    }
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzLink {
    pub unknown: u8,
    #[brw(args_raw(ctx))]
    pub link: WzImgStr,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone, Copy)]
pub struct WzVector2D {
    pub x: WzInt,
    pub y: WzInt,
}

#[derive(Debug, Clone)]
pub struct WzConvex2D(pub Vec<WzVector2D>);

impl BinRead for WzConvex2D {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: Read + Seek>(
        mut reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let len = WzInt::read_le(reader)?.0 as usize;
        let mut v = Vec::with_capacity(len);

        for _ in 0..len {
            let ty_str = args.read_ty_str(&mut reader, endian)?;
            if ty_str.as_bytes() != OBJ_TYPE_VEC2 {
                return Err(custom_binrw_error(
                    reader,
                    anyhow::format_err!("Vex2 can only consist of Vec2"),
                ));
            }
            v.push(WzVector2D::read_options(reader, endian, ())?);
        }

        Ok(Self(v))
    }
}

impl BinWrite for WzConvex2D {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + Seek>(
        &self,
        writer: &mut W,
        _endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        WzInt(self.0.len() as i32).write_le(writer)?;
        for v in self.0.iter() {
            WzObject::Vec2(*v).write_le_args(writer, args)?;
        }
        Ok(())
    }
}

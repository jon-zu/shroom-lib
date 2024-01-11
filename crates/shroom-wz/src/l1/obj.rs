use binrw::{BinRead, BinWrite};

use crate::{
    ctx::{WzImgReadCtx, WzImgWriteCtx},
    util::custom_binrw_error,
};

use super::{
    canvas::WzCanvas,
    prop::{WzConvex2D, WzLink, WzProperty, WzVector2D},
    sound::WzSound,
};

#[derive(Debug, Clone, derive_more::From, derive_more::TryInto)]
#[try_into(owned, ref, ref_mut)]
pub enum WzObject {
    Property(WzProperty),
    Canvas(WzCanvas),
    Link(WzLink),
    Vec2(WzVector2D),
    Convex2D(WzConvex2D),
    SoundDX8(WzSound),
}

impl WzObject {
    pub fn as_property(&self) -> Option<&WzProperty> {
        match self {
            Self::Property(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_property_mut(&mut self) -> Option<&mut WzProperty> {
        match self {
            Self::Property(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_canvas(&self) -> Option<&WzCanvas> {
        match self {
            Self::Canvas(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_canvas_mut(&mut self) -> Option<&mut WzCanvas> {
        match self {
            Self::Canvas(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_link(&self) -> Option<&WzLink> {
        match self {
            Self::Link(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_link_mut(&mut self) -> Option<&mut WzLink> {
        match self {
            Self::Link(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_vec2(&self) -> Option<&WzVector2D> {
        match self {
            Self::Vec2(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_vec2_mut(&mut self) -> Option<&mut WzVector2D> {
        match self {
            Self::Vec2(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_convex2d(&self) -> Option<&WzConvex2D> {
        match self {
            Self::Convex2D(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_convex2d_mut(&mut self) -> Option<&mut WzConvex2D> {
        match self {
            Self::Convex2D(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_sound_dx8(&self) -> Option<&WzSound> {
        match self {
            Self::SoundDX8(v) => Some(v),
            _ => None,
        }
    }

    pub fn as_sound_dx8_mut(&mut self) -> Option<&mut WzSound> {
        match self {
            Self::SoundDX8(v) => Some(v),
            _ => None,
        }
    }
}

pub const OBJ_TYPE_PROPERTY: &[u8] = b"Property";
pub const OBJ_TYPE_CANVAS: &[u8] = b"Canvas";
pub const OBJ_TYPE_UOL: &[u8] = b"UOL";
pub const OBJ_TYPE_VEC2: &[u8] = b"Shape2D#Vector2D";
pub const OBJ_TYPE_CONVEX2D: &[u8] = b"Shape2D#Convex2D";
pub const OBJ_TYPE_SOUND_DX8: &[u8] = b"Sound_DX8";

impl BinRead for WzObject {
    type Args<'a> = WzImgReadCtx<'a>;

    fn read_options<R: std::io::Read + std::io::Seek>(
        mut reader: &mut R,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let ty_name = args.read_ty_str(&mut reader, endian)?;

        Ok(match ty_name.as_bytes() {
            OBJ_TYPE_PROPERTY => Self::Property(WzProperty::read_options(reader, endian, args)?),
            OBJ_TYPE_CANVAS => Self::Canvas(WzCanvas::read_options(reader, endian, args)?),
            OBJ_TYPE_UOL => Self::Link(WzLink::read_options(reader, endian, args)?),
            OBJ_TYPE_VEC2 => Self::Vec2(WzVector2D::read_options(reader, endian, ())?),
            OBJ_TYPE_CONVEX2D => Self::Convex2D(WzConvex2D::read_options(reader, endian, args)?),
            OBJ_TYPE_SOUND_DX8 => Self::SoundDX8(WzSound::read_options(reader, endian, ())?),
            _ => {
                return Err(custom_binrw_error(
                    reader,
                    anyhow::format_err!("Invalid object with type: {ty_name:?}"),
                ))
            }
        })
    }
}

impl BinWrite for WzObject {
    type Args<'a> = WzImgWriteCtx<'a>;

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        mut writer: &mut W,
        endian: binrw::Endian,
        args: Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        match self {
            WzObject::Property(v) => {
                args.write_ty_str(&mut writer, endian, OBJ_TYPE_PROPERTY)?;
                v.write_options(writer, endian, args)
            }
            WzObject::Canvas(v) => {
                args.write_ty_str(&mut writer, endian, OBJ_TYPE_CANVAS)?;
                v.write_options(writer, endian, args)
            }
            WzObject::Link(v) => {
                args.write_ty_str(&mut writer, endian, OBJ_TYPE_UOL)?;
                v.write_options(writer, endian, args)
            }
            WzObject::Vec2(v) => {
                args.write_ty_str(&mut writer, endian, OBJ_TYPE_VEC2)?;
                v.write_options(writer, endian, ())
            }
            WzObject::Convex2D(v) => {
                args.write_ty_str(&mut writer, endian, OBJ_TYPE_CONVEX2D)?;
                v.write_options(writer, endian, args)
            }
            WzObject::SoundDX8(v) => {
                args.write_ty_str(&mut writer, endian, OBJ_TYPE_SOUND_DX8)?;
                v.write_options(writer, endian, ())
            }
        }
    }
}

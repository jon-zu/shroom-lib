use binrw::binrw;

use crate::ctx::{WzImgReadCtx, WzImgWriteCtx};
use crate::ty::WzInt;

use super::prop::WzProperty;
use super::WzPosValue;

#[derive(Debug, Clone, Copy)]
pub struct WzCanvasScaling(pub u8);

impl WzCanvasScaling {
    pub fn is_scaled(&self) -> bool {
        self.0 != 0
    }

    pub fn factor(&self) -> u32 {
        2u32.pow(self.0 as u32)
    }

    pub fn scale(&self, v: u32) -> u32 {
        v * self.factor()
    }
}

impl TryFrom<u8> for WzCanvasScaling {
    type Error = anyhow::Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        let n = value;
        Ok(Self(match n {
            0 | 4 => n,
            _ => anyhow::bail!("Invalid scaling: {n}"),
        }))
    }
}
impl From<WzCanvasScaling> for u8 {
    fn from(val: WzCanvasScaling) -> Self {
        val.0
    }
}

#[derive(Debug, Copy, Clone)]
pub enum WzPixelFormat {
    BGRA4444,
    BGRA8888,
    BGR565,
    DXT3,
    DXT5,
}

impl WzPixelFormat {    
    /// The pixel size in bytes
    pub fn pixel_size(&self) -> usize {
        match self {
            WzPixelFormat::BGRA4444 => 2,
            WzPixelFormat::BGRA8888 => 4,
            WzPixelFormat::BGR565 => 2,
            WzPixelFormat::DXT3 => 1,
            WzPixelFormat::DXT5 => 1,
        }
    }
}

impl TryFrom<WzInt> for WzPixelFormat {
    type Error = anyhow::Error;

    fn try_from(value: WzInt) -> Result<Self, Self::Error> {
        Ok(match value.0 as u16 {
            1 => Self::BGRA4444,
            2 => Self::BGRA8888,
            513 => Self::BGR565,
            1026 => Self::DXT3,
            2050 => Self::DXT5,
            v => anyhow::bail!("Invalid pixel format: {v}"),
        })
    }
}

impl From<WzPixelFormat> for WzInt {
    fn from(val: WzPixelFormat) -> Self {
        WzInt(match val {
            WzPixelFormat::BGRA4444 => 1,
            WzPixelFormat::BGRA8888 => 2,
            WzPixelFormat::BGR565 => 513,
            WzPixelFormat::DXT3 => 1026,
            WzPixelFormat::DXT5 => 2050,
        })
    }
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzCanvas {
    pub unknown: u8,
    pub has_property: u8,
    #[brw(if(has_property.eq(&1)), args_raw(ctx))]
    pub property: Option<WzProperty>,
    pub width: WzInt,
    pub height: WzInt,
    #[br(try_map = |x: WzInt| x.try_into())]
    #[bw(map = |x: &WzPixelFormat| WzInt(x.pixel_size() as i32))]
    pub pix_fmt: WzPixelFormat,
    #[br(try_map = |x: u8| x.try_into())]
    #[bw(map = |x: &WzCanvasScaling| u8::from(*x))]
    pub scale: WzCanvasScaling,
    pub unknown1: u32,
    pub len: WzPosValue<u32>,
}

impl WzCanvas {
    pub fn pixels(&self) -> u32 {
        self.width() * self.height()
    }

    pub fn raw_pixels(&self) -> u32 {
        self.raw_width() * self.raw_height()
    }

    pub fn height(&self) -> u32 {
        self.height.0 as u32
    }

    pub fn width(&self) -> u32 {
        self.width.0 as u32
    }

    pub fn raw_height(&self) -> u32 {
        self.height() / self.scale.factor()
    }

    pub fn raw_width(&self) -> u32 {
        self.width() / self.scale.factor()
    }

    pub fn bitmap_size(&self) -> u32 {
        self.pixels() * self.pix_fmt.pixel_size() as u32
    }

    pub fn raw_bitmap_size(&self) -> u32 {
        self.raw_pixels() * self.pix_fmt.pixel_size() as u32
    }

    pub fn data_len(&self) -> usize {
        self.len.val as usize - 1
    }

    pub fn data_offset(&self) -> u64 {
        self.len.pos + 4 + 1
    }
}

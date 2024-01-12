use binrw::binrw;
use binrw::PosValue;
use derive_more::Deref;
use derive_more::DerefMut;
use num_enum::{IntoPrimitive, TryFromPrimitive};

use crate::ctx::{WzImgReadCtx, WzImgWriteCtx};
use crate::ty::WzInt;

use super::prop::WzProperty;

/// Canvas scaling
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, TryFromPrimitive, IntoPrimitive)]
#[repr(u8)]
pub enum WzCanvasScaling {
    S0 = 0,
    S4 = 4,
}

impl WzCanvasScaling {
    /// Whether the canvas is scaled
    pub fn is_scaled(&self) -> bool {
        *self as u16 != 0
    }

    /// The scaling factor
    pub fn factor(&self) -> u32 {
        2u32.pow(*self as u32)
    }

    /// Scale the given value
    pub fn scale(&self, v: u32) -> u32 {
        v * self.factor()
    }

    /// Unscale the given value
    pub fn unscale(&self, v: u32) -> u32 {
        v / self.factor()
    }
}

/// Pixel format for the canvas
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, TryFromPrimitive, IntoPrimitive)]
#[repr(u16)]
pub enum WzPixelFormat {
    /// BGRA with 4 bits per channel
    BGRA4 = 1,
    /// BGRA with 8 bits per channel
    BGRA8 = 2,
    /// BGR with 5 bits for red and blue and 6 bits for green
    BGR565 = 0x201,
    /// DXT3 compression
    DXT3 = 0x402,
    /// DXT5 compression
    DXT5 = 0x802,
}

impl WzPixelFormat {
    /// Pixel size in bytes
    pub fn pixel_size(&self) -> usize {
        match self {
            WzPixelFormat::BGRA4 => 2,
            WzPixelFormat::BGRA8 => 4,
            WzPixelFormat::BGR565 => 2,
            WzPixelFormat::DXT3 => 1,
            WzPixelFormat::DXT5 => 1,
        }
    }
}

#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzCanvasHeader {
    pub unknown: u8,
    pub has_property: u8,
    #[brw(if(has_property.eq(&1)), args_raw(ctx))]
    pub property: Option<WzProperty>,
    pub width: WzInt,
    pub height: WzInt,
    #[br(try_map = |x: WzInt| (x.0 as u16).try_into())]
    #[bw(map = |x: &WzPixelFormat| WzInt::from(*x as u16))]
    pub pix_fmt: WzPixelFormat,
    #[br(try_map = |x: u8| x.try_into())]
    #[bw(map = |x: &WzCanvasScaling| u8::from(*x))]
    pub scale: WzCanvasScaling,
    // TODO figure out unknowns
    pub unknown1: u32,
    pub len: u32,
    pub unknown2: u8,
}

impl WzCanvasHeader {
    /// Total pixels
    pub fn pixels(&self) -> u32 {
        self.width() * self.height()
    }

    /// Pixels of the unscaled source img
    pub fn img_pixels(&self) -> u32 {
        self.img_width() * self.img_height()
    }

    /// Size of the img source in bytes
    pub fn img_data_size(&self) -> usize {
        self.img_pixels() as usize * self.pix_fmt.pixel_size()
    }

    /// Dimension of the canvas
    pub fn dim(&self) -> (u32, u32) {
        (self.width(), self.height())
    }

    /// Dimension of the img source
    pub fn img_dim(&self) -> (u32, u32) {
        (self.img_height(), self.img_width())
    }

    /// Height
    pub fn height(&self) -> u32 {
        self.height.0 as u32
    }

    /// Width
    pub fn width(&self) -> u32 {
        self.width.0 as u32
    }

    /// Height of the source image
    pub fn img_height(&self) -> u32 {
        self.scale.unscale(self.height())
    }

    /// Width of the source image
    pub fn img_width(&self) -> u32 {
        self.scale.unscale(self.width())
    }

    /// Data length as specified in the header
    pub fn data_len(&self) -> usize {
        self.len as usize - 1
    }
}


#[binrw]
#[br(little, import_raw(ctx: WzImgReadCtx<'_>))]
#[bw(little, import_raw(ctx: WzImgWriteCtx<'_>))]
#[derive(Debug, Clone)]
pub struct WzCanvas {
    #[brw(args_raw(ctx))]
    pub hdr: WzCanvasHeader,
    #[bw(ignore)]
    pub data: PosValue<()>,
}

impl Deref for WzCanvas {
    type Target = WzCanvasHeader;
    fn deref(&self) -> &Self::Target {
        &self.hdr
    }
}

impl DerefMut for WzCanvas {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.hdr
    }
}

impl WzCanvas {
    /// Data offset
    pub fn data_offset(&self) -> u64 {
        self.data.pos
    } 
}
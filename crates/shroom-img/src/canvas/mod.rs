use crate::ty::WzInt;
use binrw::binrw;
use binrw::BinRead;
use binrw::BinWrite;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use serde::Deserialize;
use serde::Serialize;

/// Canvas scaling
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    PartialOrd,
    TryFromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
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
#[derive(
    Debug,
    Copy,
    Clone,
    PartialEq,
    PartialOrd,
    TryFromPrimitive,
    IntoPrimitive,
    Serialize,
    Deserialize,
)]
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
            Self::DXT3 | Self::DXT5 => 1,
            Self::BGRA4 | Self::BGR565 => 2,
            Self::BGRA8 => 4,
        }
    }
}

#[derive(Debug, Clone, BinRead, BinWrite)]
#[brw(magic = 0u8)]
pub struct WzCanvasPropHeader {
    pub has_property: u8,
}

#[binrw]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WzCanvasHeader {
    #[br(map = |x: WzInt| x.0 as u32)]
    #[bw(map = |x: &u32| WzInt(*x as i32))]
    pub width: u32,
    #[br(map = |x: WzInt| x.0 as u32)]
    #[bw(map = |x: &u32| WzInt(*x as i32))]
    pub height: u32,
    #[br(try_map = |x: WzInt| (x.0 as u16).try_into())]
    #[bw(map = |x: &WzPixelFormat| WzInt::from(*x as u16))]
    pub pix_fmt: WzPixelFormat,
    #[br(try_map = |x: u8| x.try_into())]
    #[bw(map = |x: &WzCanvasScaling| u8::from(*x))]
    pub scale: WzCanvasScaling,
    // 4 zero unknowns
    #[serde(skip)]
    pub padding: [WzInt; 4],
}

#[binrw]
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WzCanvasLen {
    data_len: u32,
    // Zero padding
    data_pad: u8
}

impl WzCanvasLen {
    pub fn new(len: usize) -> Self {
        Self {
            data_len: len as u32 + 1,
            data_pad: 0
        }
    }

    pub fn data_len(&self) -> usize {
        self.data_len.wrapping_sub(1) as usize
    }
    
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
        self.height
    }

    /// Width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Height of the source image
    pub fn img_height(&self) -> u32 {
        self.scale.unscale(self.height())
    }

    /// Width of the source image
    pub fn img_width(&self) -> u32 {
        self.scale.unscale(self.width())
    }
}

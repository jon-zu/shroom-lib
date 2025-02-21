use std::vec;

use bit_struct::{u4, u5, u6};
use bytemuck::{Pod, Zeroable};
use image::RgbaImage;
use rgb::{RGBA8, alt::BGRA8};

use crate::{
    canvas::{WzCanvasHeader, WzCanvasScaling, WzPixelFormat},
    ty::WzInt,
};

bit_struct::bit_struct! {
    pub struct BGRA4(u16) {
        a: u4,
        r: u4,
        g: u4,
        b: u4
    }
}

// # Safety the unsafe storage is a transparent u16
unsafe impl Zeroable for BGRA4 {}
// # Safety the unsafe storage is a transparent u16
unsafe impl Pod for BGRA4 {}

impl From<BGRA4> for rgb::RGBA8 {
    fn from(mut v: BGRA4) -> Self {
        let r = v.r().get().value() * 16;
        let g = v.g().get().value() * 16;
        let b = v.b().get().value() * 16;
        let a = v.a().get().value() * 16;

        rgb::RGBA8::new(r, g, b, a)
    }
}

impl BGRA4 {
    pub fn to_bytes(&self) -> [u8; 2] {
        self.0.inner().to_le_bytes()
    }

    pub fn from_rgba8(px: rgb::RGBA8) -> Self {
        let r = px.r / 16;
        let g = px.g / 16;
        let b = px.b / 16;
        let a = px.a / 16;

        Self::new(u4::new(a).unwrap(), u4::new(r).unwrap(), u4::new(g).unwrap(), u4::new(b).unwrap())
    }
}

bit_struct::bit_struct! {
    pub struct BGR565(u16) {
        r: u5,
        g: u6,
        b: u5,
    }
}

impl BGR565 {
    pub fn to_bytes(&self) -> [u8; 2] {
        self.0.inner().to_le_bytes()
    }

    pub fn from_rgba8(px: rgb::RGBA8) -> Self {
        let r = px.r / 8;
        let g = px.g / 4;
        let b = px.b / 8;

        Self::new(u5::new(r).unwrap(), u6::new(g).unwrap(), u5::new(b).unwrap())
    }
}

// # Safety the unsafe storage is a transparent u16
unsafe impl Zeroable for BGR565 {}
// # Safety the unsafe storage is a transparent u16
unsafe impl Pod for BGR565 {}

impl From<BGR565> for rgb::RGBA8 {
    fn from(mut v: BGR565) -> Self {
        let r = v.r().get().value() * 8;
        let g = v.g().get().value() * 4;
        let b = v.b().get().value() * 8;

        rgb::RGBA8::new(r, g, b, 0xff)
    }
}

/// Represents a reference to a canvas
/// by holding a reference to the data and the header
pub struct CanvasRef<'a> {
    pub data: &'a [u8],
    pub hdr: &'a WzCanvasHeader,
}

impl<'a> CanvasRef<'a> {
    pub fn new(data: &'a [u8], hdr: &'a WzCanvasHeader) -> Self {
        Self { data, hdr }
    }

    // TODO: Allow owned conversions for dxt3 and dxt5
    // to avoid reallocations
    // also allow the caller to provide the buffer
    fn create_img<P: Into<rgb::RGBA8> + Copy>(data: &[P], (w, h): (u32, u32)) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let pix: rgb::RGBA8 = data[(x + y * w) as usize].into();
            image::Rgba(pix.into())
        })
    }

    pub fn as_bgra4(&self) -> Option<&[BGRA4]> {
        if self.hdr.pix_fmt != WzPixelFormat::BGRA4 {
            return None;
        }

        Some(bytemuck::cast_slice(self.data))
    }

    pub fn as_bgra8(&self) -> Option<&[BGRA8]> {
        if self.hdr.pix_fmt != WzPixelFormat::BGRA8 {
            return None;
        }

        Some(bytemuck::cast_slice(self.data))
    }

    pub fn as_bgr565(&self) -> Option<&[BGR565]> {
        if self.hdr.pix_fmt != WzPixelFormat::BGR565 {
            return None;
        }

        Some(bytemuck::cast_slice(self.data))
    }

    pub fn to_rgba_image(&self) -> anyhow::Result<image::RgbaImage> {
        let (w, h) = self.hdr.dim();

        Ok(match self.hdr.pix_fmt {
            WzPixelFormat::BGRA4 => {
                Self::create_img::<BGRA4>(bytemuck::cast_slice(self.data), (w, h))
            }
            WzPixelFormat::BGRA8 => {
                Self::create_img::<BGRA8>(bytemuck::cast_slice(self.data), (w, h))
            }
            WzPixelFormat::BGR565 => {
                Self::create_img::<BGR565>(bytemuck::cast_slice(self.data), (w, h))
            }
            WzPixelFormat::DXT3 => {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                texpresso::Format::Bc3.decompress(self.data, w as usize, h as usize, &mut buf);
                Self::create_img::<RGBA8>(bytemuck::cast_slice(&buf), (w, h))
            }
            WzPixelFormat::DXT5 => {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                texpresso::Format::Bc5.decompress(self.data, w as usize, h as usize, &mut buf);
                Self::create_img::<RGBA8>(bytemuck::cast_slice(&buf), (w, h))
            }
        })
    }
}

pub struct CanvasOwned {
    hdr: WzCanvasHeader,
    data: Vec<u8>,
}

impl CanvasOwned {
    pub fn new(hdr: WzCanvasHeader, data: Vec<u8>) -> Self {
        Self { hdr, data }
    }

    pub fn from_image(img: &RgbaImage, pix: WzPixelFormat) -> Self {
        let (w, h) = img.dimensions();
        let hdr = WzCanvasHeader {
            width: w,
            height: h,
            pix_fmt: pix,
            scale: WzCanvasScaling::S0,
            padding: [WzInt(0); 4],
        };

        let mut data = Vec::new();

        match pix {
            WzPixelFormat::BGRA4 => {
                data.reserve((w * h * 2) as usize);
                for (_, px) in img.pixels().enumerate() {
                    let px = BGRA4::from_rgba8(px.0.into());
                    data.extend_from_slice(&px.to_bytes());
                }
            }
            WzPixelFormat::BGR565 => {
                data.reserve((w * h * 2) as usize);
                for (_, px) in img.pixels().enumerate() {
                    let px = BGRA4::from_rgba8(px.0.into());
                    data.extend_from_slice(&px.to_bytes());
                }
            }
            WzPixelFormat::BGRA8 => {
                data.reserve((w * h * 4) as usize);
                for (_, px) in img.pixels().enumerate() {
                    let px = px.0;
                    let px = BGRA8 {
                        b: px[2],
                        g: px[1],
                        r: px[0],
                        a: px[3],
                    };
                    data.extend_from_slice(bytemuck::bytes_of(&px));
                }
            }
            _ => todo!(),
        }

        Self { hdr, data }
    }

    pub fn header(&self) -> &WzCanvasHeader {
        &self.hdr
    }

    pub fn as_ref(&self) -> CanvasRef {
        CanvasRef::new(&self.data, &self.hdr)
    }

    pub fn into_parts(self) -> (WzCanvasHeader, Vec<u8>) {
        (self.hdr, self.data)
    }
}

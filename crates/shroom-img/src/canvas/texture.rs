use std::vec;

use anyhow::Error;
use bit_struct::{u4, u5, u6};
use bytemuck::{Pod, Zeroable};
use image::{ImageBuffer, Rgba, RgbaImage};
use rgb::{alt::BGRA8, RGBA8};

use crate::canvas::{WzCanvasHeader, WzPixelFormat};

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

bit_struct::bit_struct! {
    pub struct BGR565(u16) {
        r: u5,
        g: u6,
        b: u5,
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

/// Represents a
pub struct CanvasBuffer<'a> {
    pub data: &'a [u8],
    pub hdr: &'a WzCanvasHeader,
}

pub type CanvasRgbaImage<'a> = ImageBuffer<Rgba<u8>, &'a [u8]>;

impl<'a> CanvasBuffer<'a> {
    pub fn new(data: &'a [u8], hdr: &'a WzCanvasHeader) -> Self {
        Self { data, hdr }
    }

    fn create_img<P: Into<rgb::RGBA8> + Copy>(data: &[P], (w, h): (u32, u32)) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| {
            let pix: rgb::RGBA8 = data[(x + y * w) as usize].into();
            image::Rgba(pix.into())
        })
    }

    pub fn to_raw_rgba_image(&self) -> anyhow::Result<image::RgbaImage> {
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

impl<'a> TryInto<CanvasRgbaImage<'a>> for CanvasBuffer<'a> {
    type Error = Error;

    fn try_into(self) -> Result<CanvasRgbaImage<'a>, Self::Error> {
        todo!()
    }
}

/* 
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bgra4() {
        let v = BGRA4::new(1,2,3,4);
        let rgba: RGBA8 = v.into();
        assert_eq!(rgba, [4, 3, 2, 1].into());
    }
}*/

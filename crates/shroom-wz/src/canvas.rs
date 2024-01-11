use std::{
    ops::{Deref, DerefMut},
    vec,
};

use anyhow::Error;
use bit_struct::{u4, u5, u6};
use bytemuck::{Pod, Zeroable};
use image::{ImageBuffer, Rgba, RgbaImage};
use rgb::{alt::BGRA8, RGBA8};

use crate::l1::canvas::{WzCanvas, WzPixelFormat};

bit_struct::bit_struct! {
    pub struct BGRA4(u16) {
        b: u4,
        g: u4,
        r: u4,
        a: u4
    }
}

// # Safety the unsafe storage is a transparent u16
unsafe impl Zeroable for BGRA4 {}
// # Safety the unsafe storage is a transparent u16
unsafe impl Pod for BGRA4 {}

impl From<BGRA4> for rgb::RGBA8 {
    fn from(mut v: BGRA4) -> Self {
        let r = v.r().get().value();
        let g = v.g().get().value();
        let b = v.b().get().value();
        let a = v.a().get().value();

        rgb::RGBA8::new(r, g, b, a)
    }
}

bit_struct::bit_struct! {
    pub struct BGR565(u16) {
        b: u5,
        g: u6,
        r: u5,
    }
}

// # Safety the unsafe storage is a transparent u16
unsafe impl Zeroable for BGR565 {}
// # Safety the unsafe storage is a transparent u16
unsafe impl Pod for BGR565 {}

impl From<BGR565> for rgb::RGBA8 {
    fn from(mut v: BGR565) -> Self {
        let r = v.r().get().value();
        let g = v.g().get().value();
        let b = v.b().get().value();

        rgb::RGBA8::new(r, g, b, 0xff)
    }
}

/// Represents a
pub struct CanvasBuffer<'a> {
    pub data: &'a [u8],
    pub hdr: &'a WzCanvas,
}

pub type CanvasRgbaImage<'a> = ImageBuffer<Rgba<u8>, &'a [u8]>;

impl<'a> CanvasBuffer<'a> {
    pub fn new(data: &'a [u8], hdr: &'a WzCanvas) -> Self {
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

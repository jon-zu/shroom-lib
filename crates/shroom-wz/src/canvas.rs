use std::{vec, ops::{Deref, DerefMut}};

use anyhow::Error;
use image::{Rgba, RgbaImage, ImageBuffer};

use crate::l1::canvas::{WzCanvas, WzPixelFormat, WzCanvasScaling};

const fn bit_pix<const N: u32>(v: u32, shift: u8) -> u8 {
    assert!(N > 0);
    let mask: u32 = (1 << N) - 1;
    let m = 1 << (8 - N);
    ((v >> shift) & mask) as u8 * m
}

fn bgra4_to_rgba8(v: u16) -> Rgba<u8> {
    let v = v as u32;
    let b = bit_pix::<4>(v, 0);
    let g = bit_pix::<4>(v, 4);
    let r = bit_pix::<4>(v, 8);
    let a = bit_pix::<4>(v, 12);

    [r, g, b, a].into()
}

fn bgr565_to_rgba8(v: u16) -> Rgba<u8> {
    let v = v as u32;
    let b = bit_pix::<5>(v, 0);
    let g = bit_pix::<6>(v, 5);
    let r = bit_pix::<5>(v, 11);

    [r, g, b, 0xff].into()
}

fn bgra8_to_rgba8(v: u32) -> Rgba<u8> {
    let v = v as u32;
    let b = bit_pix::<8>(v, 0);
    let g = bit_pix::<8>(v, 8);
    let r = bit_pix::<8>(v, 16);
    let a = bit_pix::<8>(v, 24);

    [r, g, b, a].into()
}

/// Represents a 
pub struct CanvasBuffer<'a> {
    data: &'a [u8],
    pix_fmt: WzPixelFormat,
    scale: WzCanvasScaling,
    size: (u32, u32)
}

pub type CanvasRgbaImage<'a> = ImageBuffer<Rgba<u8>, &'a[u8]>;

pub trait CanvasAllocator {
    type Container: Deref<Target = u8> + DerefMut;

    fn alloc(&self, size: (u32, u32), scale: WzCanvasScaling, pix_fmt: WzPixelFormat) -> Self::Container;
}

impl<'a> CanvasBuffer<'a> {
    pub fn new(data: &'a [u8], pix_fmt: WzPixelFormat, scale: WzCanvasScaling, size: (u32, u32)) -> Self {
        Self {
            data,
            pix_fmt,
            scale,
            size
        }
    }

    pub fn dim(&self) -> (u32, u32) {
        self.size
    }

    pub fn is_scaled(&self) -> bool {
        self.scale.is_scaled()
    }

    pub fn scaled_dim(&self) -> (u32, u32) {
        let (w, h) = self.size;
        (self.scale.scale(w), self.scale.scale(h))
    }
}

impl<'a> TryInto<CanvasRgbaImage<'a>> for CanvasBuffer<'a> {
    type Error = Error;

    fn try_into(self) -> Result<CanvasRgbaImage<'a>, Self::Error> {
        todo!()
    }    
}


pub struct Canvas {
    data: Vec<u8>,
    pix_fmt: WzPixelFormat,
    pub raw_w: u32,
    pub raw_h: u32,
    pub width: u32,
    pub height: u32,
    pub scale: WzCanvasScaling,
}

impl Canvas {
    pub fn from_data(data: Vec<u8>, wz_canvas: &WzCanvas) -> Self {
        Self {
            data,
            pix_fmt: wz_canvas.pix_fmt,
            width: wz_canvas.width(),
            height: wz_canvas.height(),
            scale: wz_canvas.scale,
            raw_w: wz_canvas.raw_width(),
            raw_h: wz_canvas.raw_height(),
        }
    }

    pub fn to_raw_rgba_image(&self) -> anyhow::Result<image::RgbaImage> {
        let w = self.raw_w;
        let h = self.raw_h;

        match self.pix_fmt {
            WzPixelFormat::BGRA4444 => {
                let data: &[u16] = bytemuck::cast_slice(&self.data);
                Ok(RgbaImage::from_fn(w, h, |x, y| {
                    bgra4_to_rgba8(data[(x + y * self.width) as usize])
                }))
            }
            WzPixelFormat::BGRA8888 => {
                let data: &[u32] = bytemuck::cast_slice(&self.data);
                Ok(RgbaImage::from_fn(w, h, |x, y| {
                    bgra8_to_rgba8(data[(x + y * self.width) as usize])
                }))
            }
            WzPixelFormat::BGR565 => {
                let data: &[u16] = bytemuck::cast_slice(&self.data);
                Ok(RgbaImage::from_fn(w, h, |x, y| {
                    bgr565_to_rgba8(data[(x + y * w) as usize])
                }))
            }
            WzPixelFormat::DXT3 => {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                texpresso::Format::Bc3.decompress(&self.data, w as usize, h as usize, &mut buf);
                Ok(RgbaImage::from_raw(w, h, buf)
                    .ok_or_else(|| anyhow::anyhow!("Failed to convert DXT3 to RGBA image"))?)
            }
            WzPixelFormat::DXT5 => {
                let mut buf = vec![0u8; (w * h * 4) as usize];
                texpresso::Format::Bc5.decompress(
                    &self.data,
                    self.width as usize,
                    self.height as usize,
                    &mut buf,
                );
                Ok(RgbaImage::from_raw(self.width, self.height, buf)
                    .ok_or_else(|| anyhow::anyhow!("Failed to convert DXT5 to RGBA image"))?)
            }
        }
    }

    pub fn canvas_size(&self) -> u32 {
        self.height * self.width * self.pix_fmt.pixel_size() as u32
    }
}

#[cfg(test)]
mod tests {
    use crate::canvas::bit_pix;

    #[test]
    fn bit_pix_() {
        assert_eq!(bit_pix::<8>(0x1234, 8), 0x12);
        assert_eq!(bit_pix::<4>(0x1234, 8), 0x2 * 16);
        assert_eq!(bit_pix::<3>(0x1234, 8), 2 * 32);
        assert_eq!(bit_pix::<3>(0x123F, 0), 7 * 32);
    }
}

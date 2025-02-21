use std::io::{Seek, Write};

use binrw::{BinResult, BinWrite, NullString};
use shroom_img::{ImgContext, data::DataResolver, writer::ImgWriter};

use crate::{WzCryptContext, WzHeader};

#[derive(Debug)]
pub struct WzWriter<T> {
    pub writer: T,
    pub ctx: WzCryptContext,
    desc: String,
}

const HDR_LEN: usize = 60;

impl<T: Write + Seek> WzWriter<T> {
    pub fn new(writer: T, ctx: WzCryptContext) -> Self {
        Self {
            writer,
            ctx,
            desc: String::new(),
        }
    }

    fn write_header(&mut self, hdr: WzHeader) -> BinResult<()> {
        hdr.write_le(&mut self.writer)?;
        Ok(())
    }

    pub fn write_dummy_header(&mut self) -> BinResult<()> {
        self.write_header(WzHeader {
            file_size: 0,
            data_offset: HDR_LEN as u32,
            desc: NullString::from(self.desc.as_str()),
            version_hash: self.ctx.ver.wz_encrypt()
        })
    }

    pub fn write_img<D: DataResolver>(&mut self, data_resolver: D) -> ImgWriter<&mut T, D> {
        ImgWriter::new(
            &mut self.writer,
            data_resolver,
            ImgContext::new(self.ctx.img.clone()),
        )
    }
}

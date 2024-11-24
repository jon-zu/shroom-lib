use std::io::BufRead;

use binrw::BinResult;
use serde::{Deserialize, Serialize};

use crate::{
    canvas::WzCanvasHeader,
    reader::{ImgRead, ImgReader},
    sound::WzSound,
};

#[derive(Debug, Clone, Deserialize, Serialize)]
pub enum Data {
    // Offset
    Reference(u64),
    Owned(Vec<u8>),
}

pub trait DataResolver {
    fn resolve_canvas_data(&mut self, hdr: &WzCanvasHeader, offset: u64) -> BinResult<&[u8]>;
    fn resolve_sound_data(&mut self, hdr: &WzSound, offset: u64) -> BinResult<&[u8]>;

    fn resolve_canvas<'a>(
        &'a mut self,
        data: &'a Data,
        hdr: &'a WzCanvasHeader,
    ) -> BinResult<&'a [u8]> {
        match data {
            Data::Reference(offset) => self.resolve_canvas_data(hdr, *offset),
            Data::Owned(data) => Ok(data),
        }
    }

    fn resolve_sound<'a>(&'a mut self, data: &'a Data, hdr: &'a WzSound) -> BinResult<&'a [u8]> {
        match data {
            Data::Reference(offset) => self.resolve_sound_data(hdr, *offset),
            Data::Owned(data) => Ok(data),
        }
    }
}

pub struct ReaderDataResolver<'r, R> {
    buf: Vec<u8>,
    reader: &'r mut ImgReader<R>,
}

impl<'r, R> ReaderDataResolver<'r, R> {
    pub fn new(reader: &'r mut ImgReader<R>) -> Self {
        Self {
            buf: Vec::new(),
            reader,
        }
    }
}

impl<R: ImgRead> DataResolver for ReaderDataResolver<'_, R> {
    fn resolve_canvas_data(&mut self, hdr: &WzCanvasHeader, offset: u64) -> BinResult<&[u8]> {
        self.buf.clear();
        self.reader.read_canvas_data(offset, hdr, &mut self.buf)?;
        Ok(&self.buf)
    }

    fn resolve_sound_data(&mut self, hdr: &WzSound, offset: u64) -> BinResult<&[u8]> {
        self.buf.clear();
        self.reader.read_sound_data(offset, hdr, &mut self.buf)?;
        Ok(&self.buf)
    }
}

pub struct OwnedReaderDataResolver<R> {
    buf: Vec<u8>,
    reader: ImgReader<R>,
}

impl<R> OwnedReaderDataResolver<R> {
    pub fn new(reader: ImgReader<R>) -> Self {
        Self {
            buf: Vec::new(),
            reader,
        }
    }

    pub fn reader(&self) -> &ImgReader<R> {
        &self.reader
    }

    pub fn reader_mut(&mut self) -> &mut ImgReader<R> {
        &mut self.reader
    }
}

impl<R: ImgRead + BufRead> DataResolver for OwnedReaderDataResolver<R> {
    fn resolve_canvas_data(&mut self, hdr: &WzCanvasHeader, offset: u64) -> BinResult<&[u8]> {
        self.buf.clear();
        self.reader.read_canvas_data(offset, hdr, &mut self.buf)?;
        Ok(&self.buf)
    }

    fn resolve_sound_data(&mut self, hdr: &WzSound, offset: u64) -> BinResult<&[u8]> {
        self.buf.clear();
        self.reader.read_sound_data(offset, hdr, &mut self.buf)?;
        Ok(&self.buf)
    }
}

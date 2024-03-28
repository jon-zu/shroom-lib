use arcstr::ArcStr;
use binrw::{BinResult, BinWrite, Endian};
use std::{
    fs::File,
    io::{BufWriter, Cursor, Seek, Write}, sync::Arc,
};

use crate::{
    canvas::{WzCanvasHeader, WzCanvasLen, WzCanvasPropHeader},
    crypto::ImgCrypto,
    data::{Data, DataResolver},
    error::ImgError,
    sound::WzSound,
    str_table::{ImgStr, StrOffsetTable, WriteStrCtx},
    util::WriteExt,
    Convex2, Link, ObjTypeTag, Property, PropertyValue, Vec2,
};

pub trait ImgWrite: Write + Seek {}
impl<T: Write + Seek> ImgWrite for T {}

pub struct ImgWriter<W, D> {
    w: W,
    data_resolver: D,
    crypto: Arc<ImgCrypto>,
    str_table: StrOffsetTable,
}

impl<D> ImgWriter<BufWriter<File>, D> {
    pub fn create_file(
        path: impl AsRef<std::path::Path>,
        data_resolver: D,
        crypto: Arc<ImgCrypto>,
    ) -> BinResult<Self> {
        let file = BufWriter::new(std::fs::File::create(path)?);
        Ok(Self::new(file, data_resolver, crypto))
    }
}

impl<D> ImgWriter<Cursor<Vec<u8>>, D> {
    pub fn create_buf(
        data_resolver: D,
        crypto: Arc<ImgCrypto>,
    ) -> BinResult<Self> {
        Ok(Self::new(Cursor::new(Vec::new()), data_resolver, crypto))
    }
}

impl<W: ImgWrite, D> ImgWriter<W, D> {
    pub fn new(w: W, data_resolver: D, crypto: Arc<ImgCrypto>) -> Self {
        Self {
            w,
            crypto,
            data_resolver,
            str_table: StrOffsetTable::default(),
        }
    }

    pub fn write_obj_tag(&mut self, obj_tag: ObjTypeTag) -> BinResult<()> {
        obj_tag.write_options(
            &mut self.w,
            Endian::Little,
            WriteStrCtx {
                str_table: &mut self.str_table,
                crypto: &self.crypto,
            },
        )?;
        Ok(())
    }

    pub fn write_property(&mut self, prop: Property) -> BinResult<()> {
        prop.write_le(&mut self.w)?;
        Ok(())
    }

    pub fn write_property_key(&mut self, key: &str) -> BinResult<()> {
        self.str_table
            .write_img_str(&mut self.w, &self.crypto, key)?;
        Ok(())
    }

    pub fn write_property_value(&mut self, prop: PropertyValue) -> BinResult<()> {
        prop.write_options(
            &mut self.w,
            Endian::Little,
            WriteStrCtx {
                str_table: &mut self.str_table,
                crypto: &self.crypto,
            },
        )?;
        Ok(())
    }

    pub fn write_vec2(&mut self, vec: &Vec2) -> BinResult<()> {
        vec.write_le(&mut self.w)?;
        Ok(())
    }
    pub fn write_convex2(&mut self, vex: &Convex2) -> BinResult<()> {
        vex.write_options(
            &mut self.w,
            Endian::Little,
            WriteStrCtx {
                str_table: &mut self.str_table,
                crypto: &self.crypto,
            },
        )?;
        Ok(())
    }

    pub fn write_link(&mut self, link: &ArcStr) -> BinResult<()> {
        Link(ImgStr(link.clone())).write_options(
            &mut self.w,
            Endian::Little,
            WriteStrCtx {
                str_table: &mut self.str_table,
                crypto: &self.crypto,
            },
        )
    }

    pub fn pos(&mut self) -> BinResult<u64> {
        Ok(self.w.stream_position()?)
    }

    pub fn write_pos_len(&mut self, len_pos: u64) -> BinResult<()> {
        let pos = self.w.stream_position()?;
        // Subtract the u32 4 bytes for the len aswell
        let len = (pos - len_pos - 4) as u32;

        // Write len
        self.w.seek(std::io::SeekFrom::Start(len_pos))?;
        len.write_le(&mut self.w)?;

        // Seek back to the original pos
        self.w.seek(std::io::SeekFrom::Start(pos))?;
        Ok(())
    }

    pub fn write_canvas_prop_header(&mut self, has_prop: bool) -> BinResult<()> {
        WzCanvasPropHeader {
            has_property: has_prop as u8,
        }
        .write_le(&mut self.w)?;
        Ok(())
    }

    pub fn write_canvas_header(&mut self, hdr: &WzCanvasHeader) -> BinResult<()> {
        hdr.write_le(&mut self.w)?;
        Ok(())
    }

    pub fn write_canvas_len_header(&mut self, len: u32) -> BinResult<()> {
        WzCanvasLen::new(len as usize).write_le(&mut self.w)?;
        Ok(())
    }
}


impl<W: ImgWrite, D: DataResolver> ImgWriter<W, D> {
    pub fn write_canvas(&mut self, hdr: &WzCanvasHeader, data: &Data) -> BinResult<()> {
        let Data::Reference(offset) = data else {
            return Err(ImgError::ExpectedDataOffset.binrw_error(&mut self.w));
        };

        hdr.write_le(&mut self.w)?;
        let pos = self.pos()?;
        // Dummy len
        self.write_canvas_len_header(0)?;

        let canvas_data = self.data_resolver.resolve_canvas_data(hdr, *offset)?;
        self.w.write_wz_compressed(canvas_data)?;
        self.write_pos_len(pos)?;

        Ok(())
    }

    pub fn write_sound(&mut self, sound: &WzSound, data: &Data) -> BinResult<()> {
        let Data::Reference(offset) = data else {
            return Err(ImgError::ExpectedDataOffset.binrw_error(&mut self.w));
        };
        let sound_data = self.data_resolver.resolve_sound_data(sound, *offset)?;

        sound.write_le(&mut self.w)?;
        self.w.write_all(sound_data)?;
        Ok(())
    }
}
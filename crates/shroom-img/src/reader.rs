// TODO: support binrw's BufReader for better performance
// However read_canvas would somehow call the seek_invalidate function
// instead of normal seeking for that special reader

use arcstr::ArcStr;
use binrw::{BinRead, BinResult};

use std::{
    fs::File,
    io::{BufRead, BufReader, Cursor, Read, Seek},
    path::Path,
};

use crate::{
    canvas::{WzCanvasHeader, WzCanvasLen, WzCanvasPropHeader}, crypto::ImgCrypto, data::{Data, OwnedReaderDataResolver, ReaderDataResolver}, error::ImgError, sound::WzSound, ty::WzInt, util::{chunked::ChunkedReader, BufReadExt}, Convex2, ImgContext, Link, PropertyValue, Vec2
};
use crate::{
    str_table::{OffsetStrTable, ReadStrCtx},
    ObjTypeTag, Property,
};

pub trait ImgRead: BufRead + Read + Seek {}
impl<T: BufRead + Read + Seek> ImgRead for T {}

pub struct ImgReader<R> {
    r: R,
    ctx: ImgContext,
    str_table: OffsetStrTable,
}

impl ImgReader<BufReader<File>> {
    pub fn open(path: impl AsRef<Path>, ctx: ImgContext) -> BinResult<Self> {
        let file = File::open(path)?;
        Ok(Self::new(BufReader::new(file), ctx))
    }
}

impl<'a> ImgReader<Cursor<&'a [u8]>> {
    pub fn from_bytes(bytes: &'a [u8], ctx: ImgContext) -> Self {
        Self::new(Cursor::new(bytes), ctx)
    }
}

impl<R: ImgRead> ImgReader<R> {
    pub fn new(reader: R, cfg: ImgContext) -> Self {
        Self {
            r: reader,
            ctx: cfg,
            str_table: OffsetStrTable::default(),
        }
    }

    pub fn as_resolver(&mut self) -> ReaderDataResolver<R> {
        ReaderDataResolver::new(self)
    }

    pub fn into_owned_resolver(self) -> OwnedReaderDataResolver<R> {
        OwnedReaderDataResolver::new(self)
    }

    pub fn into_inner(self) -> R {
        self.r
    }

    fn read_img_str(&mut self) -> BinResult<&ArcStr> {
        self.str_table.read_img_str(&mut self.r, &self.ctx)
    }

    pub fn read_vec2(&mut self) -> BinResult<Vec2> {
        Vec2::read_le(&mut self.r)
    }

    pub fn read_convex2(&mut self) -> BinResult<Convex2> {
        Convex2::read_le_args(
            &mut self.r,
            ReadStrCtx {
                crypto: &self.ctx,
                str_table: &mut self.str_table,
            },
        )
    }

    pub fn read_link(&mut self) -> BinResult<Link> {
        Link::read_le_args(
            &mut self.r,
            ReadStrCtx {
                crypto: &self.ctx,
                str_table: &mut self.str_table,
            },
        )
    }

    pub fn read_obj_type_tag(&mut self) -> BinResult<ObjTypeTag> {
        ObjTypeTag::read_le_args(
            &mut self.r,
            ReadStrCtx {
                crypto: &self.ctx,
                str_table: &mut self.str_table,
            },
        )
    }

    pub fn read_property_key(&mut self) -> BinResult<&ArcStr> {
        self.read_img_str()
    }

    pub fn read_property_value(&mut self) -> BinResult<PropertyValue> {
        PropertyValue::read_le_args(
            &mut self.r,
            ReadStrCtx {
                crypto: &self.ctx,
                str_table: &mut self.str_table,
            },
        )
    }

    pub fn read_int(&mut self) -> BinResult<i32> {
        Ok(WzInt::read_le(&mut self.r)?.0)
    }

    pub fn skip(&mut self, n: u64) -> BinResult<()> {
        self.r.seek(std::io::SeekFrom::Current(n as i64))?;
        Ok(())
    }

    pub fn read_canvas_prop_header(&mut self) -> BinResult<WzCanvasPropHeader> {
        WzCanvasPropHeader::read_le(&mut self.r)
    }

    pub fn read_canvas_header(&mut self) -> BinResult<WzCanvasHeader> {
        WzCanvasHeader::read_le(&mut self.r)
    }

    pub fn read_canvas_len(&mut self) -> BinResult<(Data, WzCanvasLen)> {
        let data = Data::Reference(self.r.stream_position()?);
        let len = WzCanvasLen::read_le(&mut self.r)?;
        Ok((data, len))
    }

    pub fn read_sound_header(&mut self) -> BinResult<WzSound> {
        WzSound::read_le(&mut self.r)
    }

    pub fn read_property(&mut self) -> BinResult<Property> {
        Property::read_le(&mut self.r)
    }

    pub fn read_sound_data(
        &mut self,
        offset: u64,
        hdr: &WzSound,
        w: impl std::io::Write,
    ) -> BinResult<()> {
        self.r.seek(std::io::SeekFrom::Start(offset))?;
        self.copy_n(hdr.data_size() as u64, w)?;
        Ok(())
    }

    pub fn read_canvas_data(
        &mut self,
        offset: u64,
        hdr: &WzCanvasHeader,
        w: impl std::io::Write,
    ) -> BinResult<()> {
        self.r.seek(std::io::SeekFrom::Start(offset))?;
        let (_, len) = self.read_canvas_len()?;
        let mut limited = (&mut self.r).take(len.data_len() as u64);

        let eu_crypto = ImgCrypto::europe();
        let mut chunked = ChunkedReader::new(&mut limited, &eu_crypto);

        chunked
            .decompress_flate_size_to(w, hdr.txt_data_size() as u64)
            .map_err(|err| ImgError::DecompressionFailed(offset, err).binrw_error(&mut self.r))?;

        Ok(())
    }

    pub fn pos(&mut self) -> BinResult<u64> {
        Ok(self.r.stream_position()?)
    }

    fn copy_n(&mut self, ln: u64, mut w: impl std::io::Write) -> std::io::Result<()> {
        let mut limited = (&mut self.r).take(ln);
        std::io::copy(&mut limited, &mut w)?;
        Ok(())
    }
}

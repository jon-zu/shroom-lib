use std::{
    fs::File,
    io::{self, BufRead, BufReader, Cursor, Read, Seek},
    path::Path,
    sync::Arc,
};

use binrw::BinRead;
use shroom_img::{CanvasDataFlag, ImgContext, reader::ImgReader};

use crate::{WzContext, WzCryptContext, WzDir, WzDirHeader, WzHeader, WzImgHeader, WzOffset};

pub struct SubReader<R> {
    reader: R,
    offset: u64,
    #[allow(dead_code)]
    len: u64, //TODO cap by length
}

impl<R: Seek> SubReader<R> {
    pub fn create(mut reader: R, offset: u64, len: u64) -> io::Result<Self> {
        reader.seek(std::io::SeekFrom::Start(offset))?;
        Ok(Self {
            reader,
            offset,
            len,
        })
    }

    fn adj_relative(&self, offset: i64) -> i64 {
        //TODO
        offset
    }
}

impl<R: Read> Read for SubReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
        self.reader.read(buf)
    }
}

impl<R: BufRead> BufRead for SubReader<R> {
    fn fill_buf(&mut self) -> io::Result<&[u8]> {
        self.reader.fill_buf()
    }

    fn consume(&mut self, amt: usize) {
        self.reader.consume(amt)
    }
}

impl<R: io::Seek> io::Seek for SubReader<R> {
    fn seek(&mut self, pos: io::SeekFrom) -> io::Result<u64> {
        let pos = match pos {
            io::SeekFrom::Start(s) => self.reader.seek(io::SeekFrom::Start(s + self.offset))?,
            io::SeekFrom::End(e) => self.reader.seek(io::SeekFrom::End(self.adj_relative(e)))?,
            io::SeekFrom::Current(c) => self
                .reader
                .seek(io::SeekFrom::Current(self.adj_relative(c)))?,
        };
        Ok(pos - self.offset)
    }
}

#[derive(Debug)]
pub struct WzReader<T> {
    pub reader: T,
    pub ctx: WzCryptContext,
    pub hdr: WzHeader,
}

impl WzReader<BufReader<File>> {
    pub fn open(p: impl AsRef<Path>, ctx: Arc<WzContext>) -> anyhow::Result<Self> {
        let file = File::open(p).unwrap();
        let reader = BufReader::new(file);
        Self::new(reader, ctx)
    }
}

impl<T: Read + Seek> WzReader<T> {
    pub fn new(mut reader: T, ctx: Arc<WzContext>) -> anyhow::Result<Self> {
        let hdr = WzHeader::read_le(&mut reader)?;
        let ctx = WzCryptContext::new(ctx, hdr.data_offset);
        Ok(Self { reader, ctx, hdr })
    }

    /// Reads the root directory node
    pub fn read_root_dir(&mut self) -> anyhow::Result<WzDir> {
        // Skip encrypted version at the start
        self.read_dir_node_at(WzOffset(self.hdr.data_offset + 2))
    }

    /// Read a dir node at the given offset
    pub fn read_dir_node(&mut self, hdr: &WzDirHeader) -> anyhow::Result<WzDir> {
        self.read_dir_node_at(hdr.offset)
    }

    /// Read a dir node at the given offset
    pub fn read_dir_node_at(&mut self, offset: WzOffset) -> anyhow::Result<WzDir> {
        //self.set_pos(u64::from(offset.0))?;
        self.reader
            .seek(std::io::SeekFrom::Start(u64::from(offset.0)))?;
        Ok(WzDir::read_le_args(&mut self.reader, &self.ctx)?)
    }

    pub fn img_reader(&mut self, img: &WzImgHeader) -> anyhow::Result<ImgReader<SubReader<&mut T>>>
    where
        T: BufRead,
    {
        let off = img.offset.0 as u64;
        let len = img.blob_size.0 as u64;

        let sub = SubReader::create(&mut self.reader, off, len)?;

        Ok(ImgReader::new(
            sub,
            ImgContext::with_flag(self.ctx.img.clone(), CanvasDataFlag::AutoDetect),
        ))
    }
}

impl WzReader<Cursor<&[u8]>> {
    pub fn borrowed_img_reader(
        &self,
        img: &WzImgHeader,
    ) -> anyhow::Result<ImgReader<SubReader<Cursor<&[u8]>>>> {
        let off = img.offset.0 as u64;
        let len = img.blob_size.0 as u64;
        let sub = SubReader::create(self.reader.clone(), off, len)?;

        Ok(ImgReader::new(
            sub,
            ImgContext::with_flag(self.ctx.img.clone(), CanvasDataFlag::AutoDetect),
        ))
    }
}

#[cfg(test)]
mod tests {
    use shroom_img::{canvas::CanvasRef, crypto::ImgCrypto, data::DataResolver, value::Object};

    use crate::list::ArchiveImgList;

    use super::*;

    #[test]
    fn kms() {
        let file = "/home/jonas/Downloads/Item.wz";
        let ctx = WzContext::kms(71).shared();
        let mut wz = WzReader::open(file, ctx.clone()).unwrap();

        let root = wz.read_root_dir().unwrap();
        let etc = root.get("Etc").unwrap();
        let etc = wz.read_dir_node(etc.as_dir().unwrap()).unwrap();

        let img = etc.get("0414.img").unwrap();
        let mut img: ImgReader<SubReader<&mut BufReader<File>>> =
            wz.img_reader(img.as_img().unwrap()).unwrap();
        let root = Object::from_reader(&mut img).unwrap();
        let root = root.as_property().unwrap();

        // 04140201
        //4140300

        let item = root
            .get("04140300")
            .unwrap()
            .as_object()
            .unwrap()
            .as_property()
            .unwrap();
        let info = item
            .get("info")
            .unwrap()
            .as_object()
            .unwrap()
            .as_property()
            .unwrap();
        let icon = info
            .get("icon")
            .unwrap()
            .as_object()
            .unwrap()
            .as_canvas()
            .unwrap();
        dbg!(&icon);

        let mut resolver = img.as_resolver();
        let canvas_data = resolver.resolve_canvas(&icon.data, &icon.hdr).unwrap();

        let canvas = CanvasRef::new(canvas_data, &icon.hdr);
        let img = canvas.to_rgba_image().unwrap();
        img.save("icon.png").unwrap();

        // hdr 30 49, 1Eh 31h
    }

    #[test]
    fn gms() {
        let ctx = WzContext::global(95).shared();
        let mut wz =
            WzReader::open("/home/jonas/shared_vm/maplestory/Npc.wz", ctx.clone()).unwrap();
        dbg!(&wz.hdr);

        let root = wz.read_root_dir().unwrap();
        dbg!(&root);
        assert!(false);
    }

    #[test]
    fn img() {
        let file =
            "/home/jonas/Downloads/bms/5366a09f4e67570decdbef93468edf19/DataSvr/Item/Etc/0403.img";
        let bytes = std::fs::read(file).unwrap();

        let img_crypto = ImgCrypto::none();
        let mut img_reader = ImgReader::new(
            std::io::Cursor::new(bytes),
            ImgContext::new(Arc::new(img_crypto)),
        );

        let root = Object::from_reader(&mut img_reader).unwrap();
        for sub in root.as_property().unwrap().0.iter() {
            if sub.0.contains("4032017") {
                println!("{:?}", sub.0);
            }
        }
    }

    #[test]
    fn list() {
        let file = "/home/jonas/shared_vm/maplestory/List.wz";
        let bytes = std::fs::read(file).unwrap();

        let mut r = std::io::Cursor::new(&bytes);
        let img_crypto = ImgCrypto::global();

        let list = ArchiveImgList::read_le_args(&mut r, &img_crypto).unwrap();
        dbg!(&list);
    }
}

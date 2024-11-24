use std::{
    io::{Cursor, Read, Seek},
    sync::Arc,
};

use binrw::BinRead;
use shroom_crypto::wz::offset_cipher::WzOffsetCipher;
use shroom_img::{crypto::ImgCrypto, reader::ImgReader, ImgContext};

use crate::{WzContext, WzDir, WzDirHeader, WzHeader, WzImgHeader, WzOffset};

#[derive(Debug)]
pub struct WzReader<T> {
    pub reader: T,
    pub ctx: Arc<WzContext>,
    pub hdr: WzHeader,
}

impl<T: Read + Seek> WzReader<T> {
    pub fn new(mut reader: T, wz_cipher: WzOffsetCipher, img: ImgCrypto) -> anyhow::Result<Self> {
        let hdr = WzHeader::read_le(&mut reader)?;
        let ctx = Arc::new(WzContext {
            img,
            wz: wz_cipher,
            base_offset: hdr.data_offset,
        });
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

    pub fn img_reader(&mut self, img: &WzImgHeader) -> anyhow::Result<ImgReader<Cursor<Vec<u8>>>> {
        let off = img.offset.0 as u64;
        let len = img.blob_size.0 as u64;
        self.reader.seek(std::io::SeekFrom::Start(off))?;
        let mut d = vec![0; len as usize];
        self.reader.read_exact(&mut d)?;

        Ok(ImgReader::new(Cursor::new(d), ImgContext {
            data_flag: shroom_img::CanvasDataFlag::AutoDetect,
            crypto: Arc::new(self.ctx.img.clone()),
        }))
    }
}

#[cfg(test)]
mod tests {
    use shroom_crypto::{default_keys::wz::DEFAULT_WZ_OFFSET_MAGIC, ShroomVersion};
    use shroom_img::{canvas::CanvasRef, data::DataResolver, value::Object};

    use super::*;

    #[test]
    fn kms() {
        let file = "/home/jonas/Downloads/Item.wz";
        let bytes = std::fs::read(file).unwrap();

        let img_crypto = ImgCrypto::kms();
        let ver = 71;

        let offset_cipher =
            WzOffsetCipher::new(ShroomVersion::new(ver as u16), DEFAULT_WZ_OFFSET_MAGIC);
        let mut reader = std::io::Cursor::new(&bytes);
        let mut wz = WzReader::new(&mut reader, offset_cipher, img_crypto.clone()).unwrap();

        let root = wz.read_root_dir().unwrap();
        let etc = root.get("Etc").unwrap();
        let etc = wz.read_dir_node(etc.as_dir().unwrap()).unwrap();


        let img = etc.get("0414.img").unwrap();
        let mut img  = wz.img_reader(img.as_img().unwrap()).unwrap();
        let root = Object::from_reader(&mut img).unwrap();
        let root = root.as_property().unwrap();

        // 04140201
        //4140300

        let item = root.get("04140300").unwrap().as_object().unwrap().as_property().unwrap();
        let info = item.get("info").unwrap().as_object().unwrap().as_property().unwrap();
        let icon = info.get("icon").unwrap().as_object().unwrap().as_canvas().unwrap();
        dbg!(&icon);

        let mut resolver = img.as_resolver();
        let canvas_data = resolver.resolve_canvas(&icon.data, &icon.hdr).unwrap();

        let canvas = CanvasRef::new(canvas_data, &icon.hdr);
        let img = canvas.to_rgba_image().unwrap();
        img.save("icon.png").unwrap();

        // hdr 30 49, 1Eh 31h

    }

    #[test] 
    fn kms_zlib_hdr() {
        let hdr = [0xD3, 0xF9];
        let crypto = ImgCrypto::europe();
        let mut hdr2 = hdr.clone();
        crypto.crypt(hdr2.as_mut_slice());
        println!("{:X?}", hdr2);
    }

    #[test]
    fn gms() {
        let file = "/home/jonas/shared_vm/maplestory//Item.wz";
        let bytes = std::fs::read(file).unwrap();

        let offset_cipher = WzOffsetCipher::new(ShroomVersion::new(95), DEFAULT_WZ_OFFSET_MAGIC);
        let img_crypto = ImgCrypto::global();

        let mut reader = std::io::Cursor::new(bytes);
        let mut wz = WzReader::new(&mut reader, offset_cipher, img_crypto).unwrap();
        dbg!(&wz.hdr);

        let root = wz.read_root_dir().unwrap();
        dbg!(&root);
    }

    #[test]
    fn img() {
        let file = "/home/jonas/Downloads/bms/5366a09f4e67570decdbef93468edf19/DataSvr/Item/Etc/0403.img";
        let bytes = std::fs::read(file).unwrap();

        let img_crypto = ImgCrypto::none();
        let mut img_reader = ImgReader::new(std::io::Cursor::new(bytes), ImgContext {
            data_flag: shroom_img::CanvasDataFlag::AutoDetect,
            crypto: Arc::new(img_crypto),
        });

        let root = Object::from_reader(&mut img_reader).unwrap();
        for sub in root.as_property().unwrap().0.iter() {
            if sub.0.contains("4032017") {
                println!("{:?}", sub.0);
            }
            
        }
    }
}

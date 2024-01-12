use std::{
    cell::RefCell,
    collections::VecDeque,
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom, Write},
    path::Path,
    sync::Arc,
};

use anyhow::Context;
use binrw::BinRead;

use crate::{
    crypto::WzCrypto,
    ctx::{WzContext, WzImgReadCtx},
    l0::{WzDir, WzDirEntry, WzDirHeader, WzHeader, WzImgHeader},
    l1::{canvas::WzCanvas, obj::WzObject, sound::WzSound},
    ty::{WzOffset, WzStr},
    util::{
        chunked::ChunkedReader, path::PathTraverser, str_table::OffsetStrTable, BufReadExt,
        PeekExt, SubReader,
    },
    WzConfig,
};
pub trait WzIO: BufRead + Seek {}
impl<T> WzIO for T where T: BufRead + Seek {}

pub struct WzImgReader<R> {
    r: R,
    crypto: Arc<WzCrypto>,
    str_table: RefCell<OffsetStrTable>,
    chunked_canvas: Option<bool>,
}

impl<R> WzImgReader<R>
where
    R: WzIO,
{
    /// Create a new image reader from an existing WzReader
    /// and the given crypto context
    pub fn new(r: R, crypto: Arc<WzCrypto>, chunked_canvas_encoding: bool) -> Self {
        Self {
            r,
            crypto,
            str_table: Default::default(),
            chunked_canvas: Some(chunked_canvas_encoding),
        }
    }

    /// Heuristic check to guess If chunked encoding is used
    fn heuristic_chunk_check(&mut self) -> io::Result<bool> {
        // This makes use of a small trick, that the default deflater
        // does not set with_preset but the chunked deflater always starts
        // with an 2u16, which means the 13th bit is not set for the
        // standard deflation
        let hdr = self.r.peek_u16()?;
        // 5th bit => 3rd bit from the end -> 16-13
        let is_zlib = (hdr & 0xFF) == 0x78;
        let with_preset = hdr & (1 << 13) != 0;

        Ok(!is_zlib || with_preset)
    }

    /// Copy n bytes at the given offset to the writer
    fn copy_n(&mut self, off: u64, ln: u64, mut w: impl Write) -> io::Result<()> {
        self.r.seek(SeekFrom::Start(off))?;
        let mut limited = (&mut self.r).take(ln);
        std::io::copy(&mut limited, &mut w)?;
        Ok(())
    }

    /// Calculates the checksum of the img
    pub fn checksum(&mut self) -> anyhow::Result<i32> {
        Ok(self.r.wz_checksum_eof()?)
    }

    /// Read the root object for that image
    pub fn read_root_obj(&mut self) -> anyhow::Result<WzObject> {
        self.r.rewind()?;
        WzObject::read_le_args(
            &mut self.r,
            WzImgReadCtx::new(&self.crypto, &self.str_table),
        )
        .context("Root Object")
    }

    /// Read data at the given offset
    pub fn read_data_at(&mut self, offset: u64, ln: u64, w: impl Write) -> io::Result<()> {
        self.copy_n(offset, ln, w)?;
        Ok(())
    }

    /// Read the canvas data at the given offset by the header
    pub fn read_canvas_data(&mut self, canvas: &WzCanvas, w: impl Write) -> anyhow::Result<()> {
        let len = canvas.data_len() as u64;
        let off = canvas.data_offset();
        let canvas_size = canvas.img_data_size() as u64;
        self.r.seek(SeekFrom::Start(off))?;

        // Check for chunked, either use the value or
        // the heuristic
        let chunked = match self.chunked_canvas {
            Some(chunked) => chunked,
            None => self.heuristic_chunk_check()?,
        };

        // Create a new sub reader
        let mut r = (&mut self.r).take(len);
        if !chunked {
            r.decompress_flate_size_to(w, canvas_size)?;
        } else {
            ChunkedReader::new(r, &self.crypto).decompress_flate_size_to(w, canvas_size)?;
        }

        Ok(())
    }

    /// Read the sound data with the given header
    pub fn read_sound_data(&mut self, sound: &WzSound, buf: impl Write) -> io::Result<()> {
        let ln = sound.data_size() as u64;
        self.copy_n(sound.offset.pos, ln, buf)
    }

    /// Read the sound data with the given header
    /// but include the sound header for example the wave
    /// header for wav sounds
    pub fn read_sound_data_with_header(
        &mut self,
        sound: &WzSound,
        buf: impl Write,
    ) -> io::Result<()> {
        let ln = sound.data_size();
        let hdr_size = sound.header.header_size();

        let ln = (ln + hdr_size) as u64;
        self.copy_n(sound.offset.pos - hdr_size as u64, ln, buf)
    }
}

impl WzImgReader<BufReader<File>> {
    /// Open a file at the given path
    pub fn open_file(
        path: impl AsRef<Path>,
        crypto: Arc<WzCrypto>,
        chunked_canvas: bool,
    ) -> anyhow::Result<Self> {
        let file = BufReader::new(File::open(path)?);
        Ok(Self::new(file, crypto, chunked_canvas))
    }
}

#[derive(Debug, Clone)]
pub struct WzArchiveReader<R> {
    inner: R,
    data_offset: u64,
    crypto: Arc<WzCrypto>,
}

pub type SubWzReader<'a, R> = WzArchiveReader<SubReader<'a, R>>;
pub type WzReaderFile = WzArchiveReader<BufReader<File>>;

impl WzReaderFile {
    /// Open a file at the given path
    pub fn open_file(path: impl AsRef<Path>, cfg: WzConfig) -> anyhow::Result<Self> {
        Self::open(BufReader::new(File::open(path)?), cfg)
    }
}

impl<R> WzArchiveReader<R>
where
    R: WzIO,
{
    pub fn open(mut rdr: R, cfg: WzConfig) -> anyhow::Result<Self> {
        let hdr = WzHeader::read_le(&mut rdr).context("WzHeader")?;
        rdr.seek(SeekFrom::Start(hdr.data_offset as u64))?;

        let encrypted_version = u16::read_le(&mut rdr)?;
        let ver = cfg.version;
        if ver.wz_encrypt() != encrypted_version {
            anyhow::bail!(
                "Expected encrypted version: {}({ver:?}), got {encrypted_version}",
                ver.wz_encrypt()
            );
        }

        Ok(Self::new(rdr, cfg, hdr.data_offset as u64))
    }

    pub fn open_img(rdr: R, cfg: WzConfig) -> Self {
        Self::new(rdr, cfg, 0)
    }

    fn new(rdr: R, cfg: WzConfig, data_offset: u64) -> Self {
        Self {
            inner: rdr,
            crypto: WzCrypto::from_cfg(cfg, data_offset as u32).into(),
            data_offset,
        }
    }

    fn sub_reader(&mut self, offset: u64, size: u64) -> SubReader<'_, R> {
        SubReader::new(&mut self.inner, offset, size)
    }

    /// Get the root offset
    pub fn root_offset(&self) -> WzOffset {
        WzOffset(self.data_offset as u32 + 2)
    }

    /// Get a fake dummy root header
    pub fn root_dummy_enty(&self) -> WzDirEntry {
        WzDirEntry::Dir(WzDirHeader {
            name: WzStr::default(),
            blob_size: i32::MAX.into(),
            checksum: i32::MAX.into(),
            offset: self.root_offset(),
        })
    }

    /// Reads the root directory node
    pub fn read_root_dir(&mut self) -> anyhow::Result<WzDir> {
        // Skip encrypted version at the start
        self.read_dir_node_at(self.root_offset())
    }

    /// Read a dir node at the given offset
    pub fn read_dir_node(&mut self, hdr: &WzDirHeader) -> anyhow::Result<WzDir> {
        self.read_dir_node_at(hdr.offset)
    }

    /// Read a dir node at the given offset
    pub fn read_dir_node_at(&mut self, offset: WzOffset) -> anyhow::Result<WzDir> {
        self.set_pos(offset.0 as u64)?;
        Ok(WzDir::read_le_args(
            &mut self.inner,
            WzContext::new(&self.crypto),
        )?)
    }

    /// Create an image reader at the given offset
    pub fn img_reader(
        &mut self,
        hdr: &WzImgHeader,
        chunked_canvas: bool,
    ) -> io::Result<WzImgReader<SubReader<'_, R>>> {
        let off = hdr.offset.into();
        self.set_pos(off)?;
        let crypto = self.crypto.clone();

        Ok(WzImgReader::new(
            self.sub_reader(off, hdr.blob_size.0 as u64),
            crypto,
            chunked_canvas,
        ))
    }

    /// Calculate the checksum of the given range
    pub fn checksum(&mut self, offset: u64, ln: u64) -> anyhow::Result<i32> {
        let old = self.inner.stream_position()?;
        self.set_pos(offset)?;
        let checksum = self.inner.wz_checksum(ln)?;
        self.set_pos(old)?;
        Ok(checksum)
    }

    pub fn traverse_images(&mut self) -> WzImgTraverser<'_, R> {
        let mut q = VecDeque::new();
        q.push_back((
            Arc::new("".to_string()),
            WzDirEntry::Dir(WzDirHeader::root("root", 1, self.root_offset())),
        ));
        WzImgTraverser { r: self, q }
    }

    /// Read the path
    pub fn read_path(&mut self, path: &str) -> anyhow::Result<WzDirEntry> {
        let mut trav = PathTraverser::new(path);
        trav.try_fold(self.read_root_dir()?, |dir, p| {
            let next = dir
                .get(p)
                .ok_or_else(|| anyhow::anyhow!("Path not found: {p} for {path}"))?
                .as_dir()
                .context("Path should be a dir")?;
            self.read_dir_node(next)
        })?
        .get(trav.remaining())
        .context("Last part not existing")
        .cloned()
    }

    /// Set the stream position
    fn set_pos(&mut self, p: u64) -> io::Result<()> {
        self.inner.seek(SeekFrom::Start(p))?;
        Ok(())
    }
}

pub struct WzImgTraverser<'r, R> {
    r: &'r mut WzArchiveReader<R>,
    q: VecDeque<(Arc<String>, WzDirEntry)>,
}

impl<'r, R: WzIO> WzImgTraverser<'r, R> {
    fn handle_dir(
        &mut self,
        root_name: &str,
        dir: &WzDirHeader,
    ) -> anyhow::Result<(Arc<String>, WzDir)> {
        let node = self.r.read_dir_node(dir)?;
        let node_name = Arc::new(format!("{}/{}", root_name, dir.name.as_str()));

        self.q
            .extend(node.0.iter().map(|x| (node_name.clone(), x.clone())));

        Ok((node_name.clone(), node))
    }
}

impl<'r, R> Iterator for WzImgTraverser<'r, R>
where
    R: WzIO,
{
    type Item = anyhow::Result<(String, WzImgHeader)>;

    fn next(&mut self) -> Option<Self::Item> {
        while let Some((root_name, node)) = self.q.pop_front() {
            match node {
                WzDirEntry::Dir(dir) => {
                    if let Err(err) = self.handle_dir(root_name.as_str(), &dir) {
                        return Some(Err(err));
                    }
                }
                WzDirEntry::Img(img) => {
                    let name = format!("{}/{}", root_name, img.name.as_str());
                    return Some(Ok((name, img)));
                }
                WzDirEntry::Link(link) => {
                    let img = link.link.link_img;
                    let name = format!("{}/{}", root_name, img.name.as_str());
                    return Some(Ok((name, img)));
                }
                _ => {
                    continue;
                }
            }
        }

        None
    }
}

#[cfg(feature = "mmap")]
pub mod mmap {
    use std::{fs::File, io::Cursor, path::Path, sync::Arc};

    use memmap2::Mmap;

    use crate::{WzArchiveReader, WzConfig};

    #[derive(Debug, Clone)]
    pub struct SharedMmapFile(Arc<Mmap>);
    impl AsRef<[u8]> for SharedMmapFile {
        fn as_ref(&self) -> &[u8] {
            self.0.as_ref()
        }
    }

    pub type WzReaderMmap = WzArchiveReader<Cursor<Mmap>>;
    pub type WzReaderSharedMmap = WzArchiveReader<Cursor<SharedMmapFile>>;

    impl WzReaderMmap {
        pub fn open_file_mmap(path: impl AsRef<Path>, cfg: WzConfig) -> anyhow::Result<Self> {
            let file = File::open(path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            Self::new_mmap(mmap, cfg)
        }

        fn new_mmap(mmap: Mmap, cfg: WzConfig) -> anyhow::Result<Self> {
            Self::open(Cursor::new(mmap), cfg)
        }
    }

    impl WzReaderSharedMmap {
        pub fn open_file_mmap_shared(
            path: impl AsRef<Path>,
            cfg: WzConfig,
        ) -> anyhow::Result<Self> {
            let file = File::open(path)?;
            let mmap = unsafe { Mmap::map(&file)? };
            Self::new_mmap_shared(SharedMmapFile(mmap.into()), cfg)
        }

        fn new_mmap_shared(mmap: SharedMmapFile, cfg: WzConfig) -> anyhow::Result<Self> {
            Self::open(Cursor::new(mmap), cfg)
        }
    }
}

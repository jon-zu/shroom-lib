use binrw::{binrw, BinRead, BinReaderExt, BinWrite};
use serde::{Deserialize, Serialize};
use std::ops::Deref;
use uuid::uuid;

use crate::util::custom_binrw_error;

#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Guid(
    #[br(map = uuid::Uuid::from_bytes_le)]
    #[bw(map = uuid::Uuid::to_bytes_le)]
    pub uuid::Uuid,
);

impl PartialEq<uuid::Uuid> for Guid {
    fn eq(&self, other: &uuid::Uuid) -> bool {
        self.0 == *other
    }
}

impl From<uuid::Uuid> for Guid {
    fn from(uuid: uuid::Uuid) -> Self {
        Self(uuid)
    }
}

impl Deref for Guid {
    type Target = uuid::Uuid;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

pub const MEDIA_TYPE_STREAM: uuid::Uuid = uuid!("E436EB83-524F-11CE-9F53-0020AF0BA770");

pub const MEDIASUBTYPE_MPEG1_AUDIO: uuid::Uuid = uuid!("e436eb87-524f-11ce-9f53-0020af0ba770");
pub const MEDIASUBTYPE_WAVE: uuid::Uuid = uuid!("E436EB8B-524F-11CE-9F53-0020AF0BA770");

pub const WMFORMAT_WAVE_FORMAT_EX: uuid::Uuid = uuid!("05589f81-c356-11ce-bf01-00aa0055595a");
pub const NIL_GUID: uuid::Uuid = uuid!("00000000-0000-0000-0000-000000000000");

pub const WAVE_FORMAT_PCM: u16 = 0x0001;
pub const WAVE_FORMAT_MP3: u16 = 0x0055;

pub const WAVE_HEADER_SIZE: usize = 18;

// See WAVEFORMATEX
// https://learn.microsoft.com/en-us/windows/win32/api/mmeapi/ns-mmeapi-waveformatex
#[binrw]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct WaveHeaderEx {
    pub format: u16,
    pub channels: u16,
    pub samples_per_sec: u32,
    pub avg_bytes_per_sec: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub extra_size: u16,
}

impl WaveHeaderEx {
    pub fn size(&self) -> usize {
        WAVE_HEADER_SIZE + self.extra_size as usize
    }
}

// see MPEGLAYER3WAVEFORMAT
// https://learn.microsoft.com/en-us/windows/win32/api/mmreg/ns-mmreg-mpeglayer3waveformat
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Mpeg3WaveHeader {
    pub wav: WaveHeaderEx,
    pub id: u16,
    pub flags: u32,
    pub block_size: u16,
    pub frames_per_block: u16,
    pub codec_delay: u16,
}

//PCMWAVEFORMAT
#[binrw]
#[brw(little)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PcmWaveHeader {
    pub wav: WaveHeaderEx,
    /*#[bw(pad_size_to = 4)]
    pub bit_per_sample: u16,*/
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum WaveHeader {
    Pcm(PcmWaveHeader),
    Mpeg3(Mpeg3WaveHeader),
}

impl From<PcmWaveHeader> for WaveHeader {
    fn from(hdr: PcmWaveHeader) -> Self {
        Self::Pcm(hdr)
    }
}

impl From<Mpeg3WaveHeader> for WaveHeader {
    fn from(hdr: Mpeg3WaveHeader) -> Self {
        Self::Mpeg3(hdr)
    }
}

impl Deref for WaveHeader {
    type Target = WaveHeaderEx;

    fn deref(&self) -> &Self::Target {
        match self {
            WaveHeader::Pcm(h) => &h.wav,
            WaveHeader::Mpeg3(h) => &h.wav,
        }
    }
}

impl BinRead for WaveHeader {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        (): Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let fmt: u16 = reader.read_le()?;
        reader.rewind()?;

        Ok(match fmt {
            WAVE_FORMAT_PCM => Self::Pcm(PcmWaveHeader::read_options(reader, endian, ())?),
            WAVE_FORMAT_MP3 => Self::Mpeg3(Mpeg3WaveHeader::read_options(reader, endian, ())?),
            _ => {
                return Err(custom_binrw_error(
                    reader,
                    anyhow::anyhow!("Unknown wave format: {}", fmt),
                ))
            }
        })
    }
}

impl BinWrite for WaveHeader {
    type Args<'a> = ();

    fn write_options<W: std::io::Write + std::io::Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        (): Self::Args<'_>,
    ) -> binrw::BinResult<()> {
        match self {
            WaveHeader::Pcm(h) => h.write_options(writer, endian, ())?,
            WaveHeader::Mpeg3(h) => h.write_options(writer, endian, ())?,
        }
        Ok(())
    }
}

impl WaveHeader {
    pub fn header_size(&self) -> usize {
        self.deref().size()
    }
}

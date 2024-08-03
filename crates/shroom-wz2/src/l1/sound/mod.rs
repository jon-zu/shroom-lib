use std::{
    io::{Cursor, Seek},
    time::Duration,
};

use binrw::{binrw, BinRead, BinReaderExt, BinWrite, BinWriterExt};

use crate::{l1::WzPosValue, ty::WzInt, util::custom_binrw_error};

use self::dshow::{
    Guid, WaveHeader, MEDIASUBTYPE_MPEG1_AUDIO, MEDIASUBTYPE_WAVE, MEDIA_TYPE_STREAM, NIL_GUID,
    WMFORMAT_WAVE_FORMAT_EX,
};

pub mod dshow;

pub const MEDIA_HEADER_SIZE: usize = 3 * 16 + 2; // TODO: maybe use +1 for the hdr_ty

#[binrw]
#[derive(Debug, Clone)]
#[repr(u8)]
pub enum SoundHeaderType {
    #[brw(magic = 1u8)]
    Mpeg1 = 1,
    #[brw(magic = 2u8)]
    Wave = 2,
}

#[binrw]
#[brw(little)]
#[derive(Debug, Clone)]
pub struct WzMediaHeader {
    pub hdr_ty: SoundHeaderType,
    pub major_type: Guid,
    pub sub_type: Guid,
    #[br(map = |b: u8| b != 0)]
    #[bw(map = |&b| u8::from(b))]
    pub u1: bool, // TODO: 0 for wave header, 1 for mpeg1 raw hdrs, could be has header
    #[br(map = |b: u8| b != 0)]
    #[bw(map = |&b| u8::from(b))]
    pub u2: bool, // TODO: always 1
    pub format_type: Guid,
}

/// Sound header
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WzSoundHeader {
    /// Mpeg1 sound data with no header
    Mpeg1,
    /// Wave sound data with a Wave header
    Wave(WaveHeader),
}

impl From<WaveHeader> for WzSoundHeader {
    fn from(hdr: WaveHeader) -> Self {
        Self::Wave(hdr)
    }
}

impl WzSoundHeader {
    /// The size of the header in memory
    pub fn header_size(&self) -> usize {
        MEDIA_HEADER_SIZE
            + match self {
                WzSoundHeader::Mpeg1 => 0,
                WzSoundHeader::Wave(h) => h.header_size() + 1, // WAVE + header size
            }
    }

    /// Get the matching
    pub fn media_header(&self) -> WzMediaHeader {
        match self {
            WzSoundHeader::Mpeg1 => WzMediaHeader {
                hdr_ty: SoundHeaderType::Mpeg1,
                major_type: MEDIA_TYPE_STREAM.into(),
                sub_type: MEDIASUBTYPE_MPEG1_AUDIO.into(),
                u1: true,
                u2: true,
                format_type: NIL_GUID.into(),
            },
            WzSoundHeader::Wave(_) => WzMediaHeader {
                hdr_ty: SoundHeaderType::Wave,
                major_type: MEDIA_TYPE_STREAM.into(),
                sub_type: MEDIASUBTYPE_WAVE.into(),
                u1: false,
                u2: true,
                format_type: WMFORMAT_WAVE_FORMAT_EX.into(),
            },
        }
    }
}

impl BinRead for WzSoundHeader {
    type Args<'a> = ();

    fn read_options<R: std::io::Read + std::io::Seek>(
        reader: &mut R,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::BinResult<Self> {
        let media_hdr: WzMediaHeader = reader.read_type(endian)?;

        // Right now only stream is supported
        if media_hdr.major_type != MEDIA_TYPE_STREAM {
            return Err(custom_binrw_error(
                reader,
                anyhow::format_err!("Invalid sound major type: {:?}", media_hdr.major_type),
            ));
        }

        // Read the whole header
        // TODO: maybe there should be further checking for the hdr ty,
        // and actual guids
        Ok(match media_hdr.sub_type.0 {
            MEDIASUBTYPE_MPEG1_AUDIO => Self::Mpeg1,
            MEDIASUBTYPE_WAVE => {
                let hdr_len = reader.read_type::<u8>(endian)? as usize;

                // Read the whole header
                let mut hdr = [0u8; u8::MAX as usize];
                let hdr = &mut hdr[..hdr_len];
                reader.read_exact(hdr)?;

                // Read the wave header
                Self::Wave(Cursor::new(&hdr).read_type(endian)?)
            }
            _ => {
                return Err(custom_binrw_error(
                    reader,
                    anyhow::format_err!("Invalid sound sub type: {:?}", media_hdr.sub_type),
                ))
            }
        })
    }
}

impl BinWrite for WzSoundHeader {
    type Args<'a> = ();

    fn write_options<W: std::io::prelude::Write + Seek>(
        &self,
        writer: &mut W,
        endian: binrw::Endian,
        _args: Self::Args<'_>,
    ) -> binrw::prelude::BinResult<()> {
        writer.write_type(&self.media_header(), endian)?;
        match self {
            WzSoundHeader::Mpeg1 => {}
            WzSoundHeader::Wave(hdr) => {
                writer.write_type(&(hdr.header_size() as u8), endian)?;
                writer.write_type(&hdr, endian)?;
            }
        };
        Ok(())
    }
}

/// Sound entry
#[binrw]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WzSound {
    pub unknown: u8, // TODO: unknown
    pub size: WzInt,
    pub len_ms: WzInt,
    pub header: WzSoundHeader,
    #[bw(ignore)]
    pub offset: WzPosValue<()>,
}

impl WzSound {
    /// Data offset
    pub fn data_offset(&self) -> u64 {
        self.offset.pos
    }

    /// Gets the size in bytes of the sound data
    pub fn data_size(&self) -> usize {
        self.size.0 as usize
    }

    /// Gets the playtime as Duration
    pub fn play_time(&self) -> Duration {
        Duration::from_millis(self.len_ms.0 as u64)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        l1::sound::dshow::{
            Mpeg3WaveHeader, PcmWaveHeader, WaveHeaderEx, WAVE_FORMAT_MP3, WAVE_FORMAT_PCM,
            WAVE_HEADER_SIZE,
        },
        util::test_util::test_bin_write_read_default,
    };

    use super::*;

    #[test]
    fn sound_rw() {
        // 3: unknown, size, len_ms
        // +1: hdr type
        const HDR_SIZE: usize = MEDIA_HEADER_SIZE + 3 + 1;

        test_bin_write_read_default(
            WzSound {
                unknown: 0,
                size: WzInt(0),
                len_ms: WzInt(0),
                header: WzSoundHeader::Mpeg1,
                offset: WzPosValue::new(HDR_SIZE as u64),
            },
            binrw::Endian::Little,
        );

        // +1: hdr size
        const WAVE_HDR_SIZE: usize = HDR_SIZE + 1 + WAVE_HEADER_SIZE;
        test_bin_write_read_default(
            WzSound {
                unknown: 0,
                size: WzInt(0),
                len_ms: WzInt(0),
                header: WzSoundHeader::Wave(
                    PcmWaveHeader {
                        wav: WaveHeaderEx {
                            format: WAVE_FORMAT_PCM,
                            channels: 1,
                            samples_per_sec: 44100,
                            avg_bytes_per_sec: 512,
                            block_align: 0,
                            bits_per_sample: 0,
                            extra_size: 0,
                        },
                    }
                    .into(),
                ),
                offset: WzPosValue::new(WAVE_HDR_SIZE as u64),
            },
            binrw::Endian::Little,
        );

        // +12: mp3 hdr
        const MP3_HDR_SIZE: usize = WAVE_HDR_SIZE + 12;
        test_bin_write_read_default(
            WzSound {
                unknown: 0,
                size: WzInt(0),
                len_ms: WzInt(0),
                header: WzSoundHeader::Wave(
                    Mpeg3WaveHeader {
                        wav: WaveHeaderEx {
                            format: WAVE_FORMAT_MP3,
                            channels: 1,
                            samples_per_sec: 44100,
                            avg_bytes_per_sec: 512,
                            block_align: 0,
                            bits_per_sample: 0,
                            extra_size: 12,
                        },
                        id: 4,
                        flags: 5,
                        block_size: 6,
                        frames_per_block: 7,
                        codec_delay: 8,
                    }
                    .into(),
                ),
                offset: WzPosValue::new(MP3_HDR_SIZE as u64),
            },
            binrw::Endian::Little,
        );
    }
}

use crc::{Crc, Digest};

/// CRC32 used for integrity checks(as in memory edits)
pub struct SCrc32 {
    table: [u32; 256],
}

pub const POLY_INT: u32 = 0xDD10EE12 - 0x191;

impl Default for SCrc32 {
    fn default() -> Self {
        Self::new(POLY_INT)
    }
}

const fn scrc32_table(poly: u32) -> [u32; 256] {
    let mut table = [0; 256];
    let mut i = 0;
    while i < 256 {
        let mut crc = i as u32;
        let mut j = 0;
        while j < 8 {
            if crc & 1 != 0 {
                crc = (crc >> 1) ^ poly;
            } else {
                crc >>= 1;
            }
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
}

impl SCrc32 {
    pub const fn new(poly: u32) -> Self {
        Self {
            table: scrc32_table(poly),
        }
    }

    fn update_inner<T: AsRef<[u8]>>(&self, mut crc: u32, data: T) -> u32 {
        for &byte in data.as_ref() {
            crc = self.table[((crc as u8) ^ byte) as usize] ^ (crc << 8);
        }
        crc
    }

    fn update_slice16(&self, mut crc: u32, data: &[u8]) -> u32 {
        //TODO use slice into [u8; 16]
        let mut chunks = data.chunks_exact(16);
        while let Some(chunk) = chunks.next() {
            crc = self.update_inner(crc, chunk);
        }
        let rem = chunks.remainder();
        if !rem.is_empty() {
            crc = self.update_inner(crc, rem);
        }
        crc
    }

    pub fn update(&self, crc: u32, data: &[u8]) -> u32 {
        self.update_slice16(crc, data)
    }

    pub fn table(&self) -> &[u32; 256] {
        &self.table
    }
}

pub const CRC_32_SHROOM: crc::Algorithm<u32> = crc::Algorithm {
    width: 32,
    poly: 0x04c11db7,
    init: 0x00000000,
    refin: false,
    refout: false,
    xorout: 0,
    check: 0x765e7680,
    residue: 0xc704dd7b,
};

pub static SCRC32_INT: SCrc32 = SCrc32::new(POLY_INT);
pub static CRC32_GAME: Crc<u32> = Crc::<u32>::new(&CRC_32_SHROOM);

pub struct GameDigest<'a> {
    inner: Digest<'a, u32>,
}

impl Default for GameDigest<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl<'a> GameDigest<'a> {
    pub fn new() -> Self {
        Self {
            inner: CRC32_GAME.digest(),
        }
    }

    pub fn with(init: u32) -> Self {
        Self {
            inner: CRC32_GAME.digest_with_initial(init),
        }
    }

    pub fn update(mut self, data: &[u8]) -> Self {
        self.inner.update(data);
        self
    }

    pub fn update_str(mut self, data: &str) -> Self {
        self.inner.update(data.as_bytes());
        self
    }

    pub fn update_i32(mut self, data: i32) -> Self {
        self.inner.update(&data.to_le_bytes());
        self
    }

    pub fn update_i64(mut self, data: i64) -> Self {
        self.inner.update(&data.to_le_bytes());
        self
    }

    pub fn update_u32(mut self, data: u32) -> Self {
        self.inner.update(&data.to_le_bytes());
        self
    }

    pub fn update_u64(mut self, data: u64) -> Self {
        self.inner.update(&data.to_le_bytes());
        self
    }

    pub fn update_u8(mut self, data: u8) -> Self {
        self.inner.update(&[data]);
        self
    }

    pub fn update_f32(mut self, data: f32) -> Self {
        self.inner.update(&data.to_le_bytes());
        self
    }

    pub fn update_f64(mut self, data: f64) -> Self {
        self.inner.update(&data.to_le_bytes());
        self
    }

    pub fn finalize(self) -> u32 {
        self.inner.finalize()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc() {
        assert_eq!(
            GameDigest::with(148854160).update_str("sp").finalize(),
            367474251
        );
        assert_eq!(GameDigest::new().update_u32(95).finalize(), 0xC36FDB97);
        assert_eq!(GameDigest::new().update_u32(270).finalize(), 954028113);
    }
}

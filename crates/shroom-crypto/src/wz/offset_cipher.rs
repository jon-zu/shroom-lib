use std::num::Wrapping;

use crate::ShroomVersion;

#[derive(Clone)]
pub struct WzOffsetCipher {
    offset_magic: u32,
    version_hash: u32,
}

impl WzOffsetCipher {
    pub fn new(version: ShroomVersion, offset_magic: u32) -> Self {
        Self {
            offset_magic,
            version_hash: version.wz_hash(),
        }
    }

    fn offset_key_at(&self, pos: u32, data_offset: u32) -> u32 {
        let mut off = Wrapping(!(pos - data_offset));
        off *= self.version_hash;
        off -= self.offset_magic;

        let off = off.0;
        off.rotate_left(off & 0x1F)
    }

    pub fn decrypt_offset(&self, data_off: u32, enc_off: u32, pos: u32) -> u32 {
        let k = self.offset_key_at(pos, data_off);
        (k ^ enc_off).wrapping_add(data_off * 2)
    }

    pub fn encrypt_offset(&self, data_off: u32, off: u32, pos: u32) -> u32 {
        let off = off.wrapping_sub(data_off * 2);
        off ^ self.offset_key_at(pos, data_off)
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        default_keys::wz::DEFAULT_WZ_OFFSET_MAGIC, wz::offset_cipher::WzOffsetCipher,
        ShroomVersion,
    };

    #[test]
    fn wz_offset() {
        const OFF: u32 = 60;
        let crypto = WzOffsetCipher::new(ShroomVersion::new(95), DEFAULT_WZ_OFFSET_MAGIC);
        let c = crypto.encrypt_offset(OFF, 4681, 89);
        assert_eq!(c, 3555811726);
        assert_eq!(crypto.decrypt_offset(OFF, c, 89), 4681);
    }
}

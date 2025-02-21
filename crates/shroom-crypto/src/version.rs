/// Represents a version
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShroomVersion(u16);

impl From<u16> for ShroomVersion {
    fn from(v: u16) -> Self {
        Self::new(v)
    }
}

impl From<ShroomVersion> for u16 {
    fn from(v: ShroomVersion) -> Self {
        v.0
    }
}

impl ShroomVersion {
    /// Creates a new version
    pub const fn new(v: u16) -> Self {
        Self(v)
    }

    /// Gets the raw version
    pub const fn raw(&self) -> u16 {
        self.0
    }

    /// Inverts the version bitwise
    #[must_use]
    pub const fn invert(&self) -> Self {
        Self(!self.0)
    }

    /// Calculates the wz version hash
    pub const fn wz_hash(&self) -> u32 {
        let mut n = self.0;

        // Reverse the number
        let mut reversed = 0;
        while n > 0 {
            reversed = reversed * 10 + (n % 10);
            n /= 10;
        }

        let mut hash: u32 = 0;
        while reversed > 0 {
            let digit = ((reversed % 10) as u8 + b'0') as u32;
            hash = (hash << 5) + digit + 1;
            reversed /= 10;
        }

        hash
    }

    /// Calculates the encrypted wz version
    pub const fn wz_encrypt(&self) -> u16 {
        // Xor each byte of the version hash
        let data = self.wz_hash().to_be_bytes();
        let mut result = 0xFF;
        result ^= data[0] as u16;
        result ^= data[1] as u16;
        result ^= data[2] as u16;
        result ^= data[3] as u16;
        result
    }


    pub fn wz_detect_version(encrypted: u16) -> impl Iterator<Item = ShroomVersion> {
        (1..=400)
            .map(Self::new)
            .filter(move |v| v.wz_encrypt() == encrypted)
    }
}

#[cfg(test)]
mod tests {
    use crate::ShroomVersion;

    #[test]
    fn version_invert() {
        assert_eq!(ShroomVersion::new(95).invert().raw() as i16, -96);
        assert_eq!(ShroomVersion::new(83).invert().raw() as i16, -84);
    }

    #[test]
    fn version_wz() {
        let v95 = ShroomVersion::new(95);
        assert_eq!(v95.wz_hash(), 1910);
        assert_eq!(v95.wz_encrypt(), 142);

        let v71 = ShroomVersion::new(71);
        assert_eq!(v71.wz_hash(), 1842);
        assert_eq!(v71.wz_encrypt(), 202);
    }
}

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
    pub fn raw(&self) -> u16 {
        self.0
    }

    /// Inverts the version bitwise
    #[must_use]
    pub fn invert(&self) -> Self {
        Self(!self.0)
    }

    /// Calculates the wz version hash
    pub fn wz_hash(&self) -> u32 {
        let mut buffer = itoa::Buffer::new();
        buffer
            .format(self.0)
            .as_bytes()
            .iter()
            .fold(0, |mut acc, &c| {
                acc <<= 5;
                acc + u32::from(c) + 1
            })
    }

    /// Calculates the encrypted wz version
    pub fn wz_encrypt(&self) -> u16 {
        // Xor each byte of the version hash
        self.wz_hash()
            .to_be_bytes()
            .iter()
            .fold(0xFF, |acc, &b| acc ^ u16::from(b))
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
    }
}

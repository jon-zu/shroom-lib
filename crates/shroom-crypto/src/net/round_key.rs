use cipher::{generic_array::GenericArray, typenum::U16};
use rand::{CryptoRng, Rng, RngCore};

use crate::ig_cipher::IgContext;

pub type ExpandedRoundKey = GenericArray<u8, U16>;
pub type RoundKeyBytes = [u8; 4];

/// Represents a key for the current AES-OFB crypt round
/// Due to a bug only first 4 bytes are used and expanded to 16 bytes
#[derive(Debug, Copy, Clone, PartialEq, PartialOrd, Eq)]
pub struct RoundKey(RoundKeyBytes);

impl From<RoundKeyBytes> for RoundKey {
    fn from(value: RoundKeyBytes) -> Self {
        Self(value)
    }
}

impl From<RoundKey> for RoundKeyBytes {
    fn from(value: RoundKey) -> Self {
        value.0
    }
}

impl From<RoundKey> for u32 {
    fn from(value: RoundKey) -> Self {
        u32::from_le_bytes(value.0)
    }
}

impl From<u32> for RoundKey {
    fn from(value: u32) -> Self {
        Self(value.to_le_bytes())
    }
}

impl rand::Fill for RoundKey {
    fn try_fill<R: rand::Rng + ?Sized>(&mut self, rng: &mut R) -> Result<(), rand::Error> {
        self.0 = rng.gen();
        Ok(())
    }
}

impl From<ExpandedRoundKey> for RoundKey {
    fn from(value: ExpandedRoundKey) -> Self {
        Self([value[0], value[1], value[2], value[3]])
    }
}

impl RoundKey {
    pub const fn new(key: RoundKeyBytes) -> Self {
        Self(key)
    }


    /// Returns a Roundkey just containing zeros
    pub const fn zero() -> Self {
        Self::new([0; 4])
    }

    /// Generate a random round key
    pub fn get_random<R>(mut rng: R) -> Self
    where
        R: CryptoRng + RngCore,
    {
        let mut zero = Self::zero();
        rng.fill(&mut zero);
        zero
    }

    /// Update the round key
    pub fn update(self, ig: &IgContext) -> RoundKey {
        ig.hash(&self.0).into()
    }

    /// Expands the round key to an IV
    pub fn expand(&self) -> ExpandedRoundKey {
        array_init::array_init(|i| self.0[i % 4]).into()
    }
}

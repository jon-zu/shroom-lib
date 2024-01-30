use std::{
    ffi::{CStr, CString},
    io::{self, Write},
};

const STRING_KEY_SIZE: usize = 16;
pub const DEFAULT_STRING_KEY: [u8; STRING_KEY_SIZE] = [
    0xd6, 0xde, 0x75, 0x86, 0x46, 0x64, 0xa3, 0x71, 0xe8, 0xe6, 0x7b, 0xd3, 0x33, 0x30, 0xe7, 0x2e,
];

fn rotate_left(slice: &mut [u8; STRING_KEY_SIZE], shift: usize) {
    let len = slice.len();
    if len == 0 {
        return;
    }

    fn combine(a: u8, b: u8, shift: usize) -> u8 {
        (a << shift) | (b >> (8 - shift))
    }

    let bit_shifts = shift % 8;
    let byte_shifts = shift / 8;
    slice.rotate_left(byte_shifts);

    if bit_shifts != 0 {
        let first = slice[0];
        for i in 0..len - 1 {
            slice[i] = combine(slice[i], slice[i + 1], bit_shifts);
        }

        let last = len - 1;
        slice[last] = combine(slice[last], first, bit_shifts);
    }
}

pub struct StringCipher([u8; STRING_KEY_SIZE]);

impl Default for StringCipher {
    fn default() -> Self {
        Self(DEFAULT_STRING_KEY)
    }
}

impl StringCipher {
    pub fn new(key: [u8; STRING_KEY_SIZE]) -> Self {
        Self(key)
    }

    pub fn get_key(&self, seed: u8) -> [u8; STRING_KEY_SIZE] {
        let mut key = self.0;
        rotate_left(&mut key, seed as usize);
        key
    }

    fn xor_key(&self, data: &mut [u8], seed: u8) {
        let key = self.get_key(seed);
        for (i, b) in data.iter_mut().enumerate() {
            let k = key[i % key.len()];
            // Xoring with the key itself would produce a 0
            *b = if *b != k { *b ^ k } else { k };
        }
    }

    pub fn decrypt(&self, data: &mut [u8], seed: u8) {
        self.xor_key(data, seed)
    }

    pub fn decrypt_str<'a>(&self, data: &'a mut [u8]) -> Option<&'a CStr> {
        let (seed, data) = data.split_first_mut()?;
        let (_, data_enc) = data.split_last_mut()?;
        self.decrypt(data_enc, *seed);
        CStr::from_bytes_until_nul(data).ok()
    }

    pub fn encrypt(&self, data: &mut [u8], seed: u8) {
        self.xor_key(data, seed)
    }

    pub fn encrypt_str(&self, s: CString, seed: u8, mut w: impl Write) -> io::Result<()> {
        w.write_all(&[seed])?;
        let mut v = s.into_bytes();
        self.encrypt(&mut v, seed);
        w.write_all(&v)?;
        w.write_all(&[0])?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key() {
        assert_eq!(
            StringCipher::default().get_key(0x37),
            [
                0xB8, 0xF4, 0x73, 0x3D, 0xE9, 0x99, 0x98, 0x73, 0x97, 0x6B, 0x6F, 0x3A, 0xC3, 0x23,
                0x32, 0x51
            ]
        );
    }

    #[test]
    fn decrypt_str() {
        let mut enc = [
            0x37, 0xd0, 0x80, 0x07, 0x4d, 0xd3, 0xb6, 0xb7, 0x03, 0xf6, 0x18, 0x1c, 0x4a, 0xac,
            0x51, 0x46, 0x7f, 0xd6, 0x91, 0x0b, 0x52, 0x87, 0xb7, 0xf6, 0x16, 0xe3, 0x44, 0x50,
            0x6a, 0x82, 0x71, 0x66, 0x6c, 0x97, 0xa6, 0x16, 0x5a, 0x80, 0xea, 0xec, 0x01, 0xf6,
            0x1f, 0x06, 0x55, 0xad, 0x0c, 0x73, 0x36, 0xdd, 0xb7, 0x1b, 0x58, 0x8a, 0xf2, 0x00,
        ];

        assert!(StringCipher::default().decrypt_str(&mut enc).is_some());
    }
}

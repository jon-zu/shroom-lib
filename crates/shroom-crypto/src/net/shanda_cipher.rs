use std::num::Wrapping;

use cipher::inout::InOutBuf;

#[derive(Default, Debug)]
pub struct ShandaCipher;

const SHANDA_ROUNDS: usize = 3;

impl ShandaCipher {
    fn round_even_encrypt(mut b: u8, state: u8, len: u8) -> (u8, u8) {
        b = b.rotate_left(3);
        b = b.wrapping_add(len);
        let next_state = b;
        b ^= state;
        b = b.rotate_right(len as u32);
        b = !b;
        b = b.wrapping_add(0x48);
        (b, next_state ^ state)
    }

    fn round_even_decrypt(mut b: u8, state: u8, len: u8) -> (u8, u8) {
        b = b.wrapping_sub(0x48);
        b = !b;
        b = b.rotate_left(len as u32);
        let next_state = b;
        b ^= state;
        b = b.wrapping_sub(len);
        b = b.rotate_right(3);
        (b, next_state)
    }

    fn round_odd_encrypt(mut b: u8, state: u8, len: u8) -> (u8, u8) {
        b = b.rotate_left(4);
        b = b.wrapping_add(len);
        let next_state = b;
        b ^= state;
        b ^= 0x13;
        b = b.rotate_right(3);

        (b, next_state ^ state)
    }

    fn round_odd_decrypt(mut b: u8, state: u8, len: u8) -> (u8, u8) {
        b = b.rotate_left(3);
        b ^= 0x13;
        let next_state = b;
        b ^= state;
        b = b.wrapping_sub(len);
        b = b.rotate_right(4);
        (b, next_state)
    }

    /// Even round iterates through the data and applies the round mutations
    fn do_even_round<F>(data: &mut [u8], apply: F)
    where
        F: Fn(u8, u8, u8) -> (u8, u8),
    {
        let n = data.len();
        let mut state = 0;
        let mut ln = Wrapping(n as u8);

        for d in data.iter_mut() {
            let (b, next_state) = apply(*d, state, ln.0);
            *d = b;
            state = next_state;
            ln -= 1;
        }
    }

    /// Odd round iterates through the REVERSED data and applies the round mutations
    fn do_odd_round<F>(data: &mut [u8], apply: F)
    where
        F: Fn(u8, u8, u8) -> (u8, u8),
    {
        let n = data.len();
        let mut state = 0;
        let mut ln = Wrapping(n as u8);

        // REV
        for d in data.iter_mut().rev() {
            let (b, next_state) = apply(*d, state, ln.0);
            *d = b;
            state = next_state;
            ln -= 1;
        }
    }

    /// Encrypts the given block of data
    pub fn encrypt_inout(data: InOutBuf<u8>) {
        let data = data.into_out();
        for _ in 0..SHANDA_ROUNDS {
            Self::do_even_round(data, Self::round_even_encrypt);
            Self::do_odd_round(data, Self::round_odd_encrypt);
        }
    }

    /// Encrypts the given block of data
    pub fn encrypt(data: &mut [u8]) {
        Self::encrypt_inout(data.into());
    }

    /// Decrypts the given block of data
    pub fn decrypt_inout(data: InOutBuf<u8>) {
        let data = data.into_out();
        for _ in 0..SHANDA_ROUNDS {
            Self::do_odd_round(data, Self::round_odd_decrypt);
            Self::do_even_round(data, Self::round_even_decrypt);
        }
    }

    /// Decrypts the given block of data
    pub fn decrypt(data: &mut [u8]) {
        Self::decrypt_inout(data.into());
    }
}

#[cfg(test)]
mod tests {
    use crate::ShandaCipher;


    #[test]
    fn en_dec_shanda() {
        let data = b"abcdef";

        let mut data_enc = *data;
        ShandaCipher::encrypt(data_enc.as_mut_slice());
        ShandaCipher::decrypt(data_enc.as_mut_slice());
        assert_eq!(*data, data_enc);
    }

    // Simple bruteforce with all possible values for `b` and `state`
    // The len parameter can shift at most by 7 places and is then wrapped in EVEN rounds
    // In odd round It's simply wrapped_add/sub so not really worth It to bruteforce here further
    // Not supposed to run on CI, testing it locally is enough since this test is essentially a bruteforce
    #[ignore]
    #[test]
    fn odd_even_enc_dec() {
        for b in 0u8..=u8::MAX {
            for state in 0u8..=u8::MAX {
                for ln in 0u8..=8 {
                    let (enc, _) = ShandaCipher::round_odd_encrypt(b, state, ln);
                    let (dec, _) = ShandaCipher::round_odd_decrypt(enc, state, ln);
                    assert_eq!(b, dec);

                    let (enc, _) = ShandaCipher::round_even_encrypt(b, state, ln);
                    let (dec, _) = ShandaCipher::round_even_decrypt(enc, state, ln);
                    assert_eq!(b, dec);
                }
            }
        }
    }
}

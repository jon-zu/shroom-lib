use std::{fmt, hash::Hasher};

use cipher::{
    typenum::U1, AlgorithmName, AsyncStreamCipher, BlockBackend, BlockCipher, BlockDecryptMut,
    BlockEncryptMut, BlockSizeUser, ParBlocksSizeUser,
};

use super::{default_keys, ShuffleKey};


/// Context for the ig crypto functions, used to create the hasher and cipher
#[derive(Debug, Clone)]
pub struct IgContext {
    shuffle_key: ShuffleKey,
    seed: u32,
}

/// Default
pub const DEFAULT_IG_CONTEXT: IgContext = IgContext {
    shuffle_key: *default_keys::DEFAULT_IG_SHUFFLE,
    seed: default_keys::DEFAULT_IG_SEED,
};

impl IgContext {
    /// Creates a new IgContext
    pub const fn new(shuffle_key: ShuffleKey, seed: u32) -> Self {
        Self { shuffle_key, seed }
    }

    /// Creates a new hasher with this context
    pub fn hasher(&self) -> IgHasher<'_> {
        IgHasher {
            state: self.seed,
            ctx: self,
        }
    }

    /// Creates a new cipher with this context
    pub fn cipher(&self) -> IgCipher {
        IgCipher {
            state: self.seed,
            ctx: self.clone(),
        }
    }

    /// Hash the data slice
    pub fn hash(&self, data: &[u8]) -> u32 {
        let mut hasher = self.hasher();
        hasher.update(data);
        hasher.finalize()
    }

    /// Get the shuffled value for the value `a`
    fn shuffle(&self, a: u8) -> u8 {
        self.shuffle_key[a as usize]
    }

    /// Updates the given key `k` with the given data
    fn update_key(&self, k: u32, data: u8) -> u32 {
        let mut k = k.to_le_bytes();
        k[0] = k[0].wrapping_add(self.shuffle(k[1]).wrapping_sub(data));
        k[1] = k[1].wrapping_sub(k[2] ^ self.shuffle(data));
        k[2] ^= self.shuffle(k[3]).wrapping_add(data);
        k[3] = k[3].wrapping_sub(k[0].wrapping_sub(self.shuffle(data)));

        u32::from_le_bytes(k).rotate_left(3)
    }

    /// Encrypt the given data with the given key
    fn enc(&self, data: u8, key: u32) -> u8 {
        let key = key.to_le_bytes();
        let v = data.rotate_right(4);
        // v(even bits) = (a << 1) & 0xAA(even bits)
        let even = (v & 0xAA) >> 1;
        // v(odd bits) = (a >> 1) & 0x55(odd bits)
        let odd = (v & 0x55) << 1;

        let a = even | odd;
        a ^ self.shuffle(key[0])
    }

    fn dec(&self, data: u8, key: u32) -> u8 {
        let key = key.to_le_bytes();
        let a = self.shuffle(key[0]) ^ data;
        let b = a << 1;

        let mut v = a;
        v >>= 1;
        v ^= b;
        v &= 0x55;
        v ^= b;
        v.rotate_left(4)
    }
}

pub struct IgHasher<'ctx> {
    state: u32,
    ctx: &'ctx IgContext,
}

impl<'ctx> IgHasher<'ctx> {
    pub fn update(&mut self, data: &[u8]) {
        self.state = data
            .iter()
            .fold(self.state, |key, b| self.ctx.update_key(key, *b))
    }

    pub fn finalize(&self) -> u32 {
        self.state
    }
}

impl<'ctx> Hasher for IgHasher<'ctx> {
    fn write(&mut self, bytes: &[u8]) {
        self.update(bytes)
    }

    fn finish(&self) -> u64 {
        self.finalize() as u64
    }
}

pub struct IgCipher {
    ctx: IgContext,
    state: u32,
}

impl BlockSizeUser for IgCipher {
    type BlockSize = U1;
}

impl BlockCipher for IgCipher {}

impl fmt::Debug for IgCipher {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IgCipher")
    }
}

impl AlgorithmName for IgCipher {
    fn write_alg_name(f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("IgCipher")
    }
}

impl IgCipher {
    pub(crate) fn encrypt_inner(&mut self, data: u8) -> u8 {
        let plain = data;
        let cipher = self.ctx.enc(plain, self.state);
        self.state = self.ctx.update_key(self.state, plain);
        cipher
    }

    pub(crate) fn decrypt_inner(&mut self, data: u8) -> u8 {
        let cipher = data;
        let plain = self.ctx.dec(cipher, self.state);
        self.state = self.ctx.update_key(self.state, plain);
        plain
    }
}

impl BlockEncryptMut for IgCipher {
    fn encrypt_with_backend_mut(
        &mut self,
        f: impl cipher::BlockClosure<BlockSize = Self::BlockSize>,
    ) {
        struct BlockEnc<'a>(&'a mut IgCipher);
        impl BlockSizeUser for BlockEnc<'_> {
            type BlockSize = U1;
        }
        impl ParBlocksSizeUser for BlockEnc<'_> {
            type ParBlocksSize = U1;
        }
        impl BlockBackend for BlockEnc<'_> {
            fn proc_block(&mut self, mut block: cipher::inout::InOut<'_, '_, cipher::Block<Self>>) {
                let mut data: u8 = block.clone_in()[0];
                data = self.0.encrypt_inner(data);
                block.get_out().copy_from_slice(&data.to_be_bytes());
            }
        }

        f.call(&mut BlockEnc(self))
    }
}

impl BlockDecryptMut for IgCipher {
    fn decrypt_with_backend_mut(
        &mut self,
        f: impl cipher::BlockClosure<BlockSize = Self::BlockSize>,
    ) {
        struct BlockDec<'a>(&'a mut IgCipher);
        impl BlockSizeUser for BlockDec<'_> {
            type BlockSize = U1;
        }
        impl ParBlocksSizeUser for BlockDec<'_> {
            type ParBlocksSize = U1;
        }
        impl BlockBackend for BlockDec<'_> {
            fn proc_block(&mut self, mut block: cipher::inout::InOut<'_, '_, cipher::Block<Self>>) {
                let mut data: u8 = block.clone_in()[0];
                data = self.0.decrypt_inner(data);
                block.get_out().copy_from_slice(&data.to_be_bytes());
            }
        }

        f.call(&mut BlockDec(self))
    }
}

impl AsyncStreamCipher for IgCipher {}

#[cfg(test)]
mod tests {
    use cipher::AsyncStreamCipher;

    use super::DEFAULT_IG_CONTEXT;

    #[test]
    fn ig_dec_enc() {
        let data: &[&[u8]] = &[&[1u8, 2], &[], &[1]];

        for data in data {
            let ctx = &DEFAULT_IG_CONTEXT;
            let mut buf = data.to_vec();
            ctx.cipher().encrypt(&mut buf);
            ctx.cipher().decrypt(&mut buf);
            assert_eq!(buf, *data);
        }
    }
}

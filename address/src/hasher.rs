use {
    crate::{Pubkey, PUBKEY_BYTES},
    core::{
        cell::Cell,
        hash::{BuildHasher, Hash, Hasher},
        mem,
    },
    rand::{thread_rng, Rng},
};

/// Custom impl of Hash for Pubkey
/// allows us to skip hashing the length of the pubkey
/// which is always the same anyway
impl Hash for Pubkey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        state.write(self.as_array());
    }
}

/// A faster, but less collision resistant hasher for pubkeys.
///
/// Specialized hasher that uses a random 8 bytes subslice of the
/// pubkey as the hash value. Should not be used when collisions
/// might be used to mount DOS attacks.
///
/// Using this results in about 4x faster lookups in a typical hashmap.
#[derive(Default)]
pub struct PubkeyHasher {
    offset: usize,
    state: u64,
}

impl Hasher for PubkeyHasher {
    #[inline]
    fn finish(&self) -> u64 {
        self.state
    }
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        debug_assert_eq!(
            bytes.len(),
            PUBKEY_BYTES,
            "This hasher is intended to be used with pubkeys and nothing else"
        );
        // This slice/unwrap can never panic since offset is < PUBKEY_BYTES - mem::size_of::<u64>()
        let chunk: &[u8; mem::size_of::<u64>()] = bytes
            [self.offset..self.offset + mem::size_of::<u64>()]
            .try_into()
            .unwrap();
        self.state = u64::from_ne_bytes(*chunk);
    }
}

/// A builder for faster, but less collision resistant hasher for pubkeys.
///
/// Initializes `PubkeyHasher` instances that use an 8-byte
/// slice of the pubkey as the hash value. Should not be used when
/// collisions might be used to mount DOS attacks.
///
/// Using this results in about 4x faster lookups in a typical hashmap.
#[derive(Clone)]
pub struct PubkeyHasherBuilder {
    offset: usize,
}

impl Default for PubkeyHasherBuilder {
    /// Default construct the PubkeyHasherBuilder.
    ///
    /// The position of the slice is determined initially
    /// through random draw and then by incrementing a thread-local
    /// This way each hashmap can be expected to use a slightly different
    /// slice. This is essentially the same mechanism as what is used by
    /// `RandomState`
    fn default() -> Self {
        std::thread_local!(static OFFSET: Cell<usize>  = {
            let mut rng = thread_rng();
            Cell::new(rng.gen_range(0..PUBKEY_BYTES - mem::size_of::<u64>()))
        });

        let offset = OFFSET.with(|offset| {
            let mut next_offset = offset.get() + 1;
            if next_offset > PUBKEY_BYTES - mem::size_of::<u64>() {
                next_offset = 0;
            }
            offset.set(next_offset);
            next_offset
        });
        PubkeyHasherBuilder { offset }
    }
}

impl BuildHasher for PubkeyHasherBuilder {
    type Hasher = PubkeyHasher;
    #[inline]
    fn build_hasher(&self) -> Self::Hasher {
        PubkeyHasher {
            offset: self.offset,
            state: 0,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::PubkeyHasherBuilder,
        crate::Pubkey,
        core::hash::{BuildHasher, Hasher},
    };
    #[test]
    fn test_pubkey_hasher_builder() {
        let key = Pubkey::new_unique();
        let builder = PubkeyHasherBuilder::default();
        let mut hasher1 = builder.build_hasher();
        let mut hasher2 = builder.build_hasher();
        hasher1.write(key.as_array());
        hasher2.write(key.as_array());
        assert_eq!(
            hasher1.finish(),
            hasher2.finish(),
            "Hashers made with same builder should be identical"
        );
        // Make sure that when we make new builders we get different slices
        // chosen for hashing
        let builder2 = PubkeyHasherBuilder::default();
        for _ in 0..64 {
            let mut hasher3 = builder2.build_hasher();
            hasher3.write(key.as_array());
            std::dbg!(hasher1.finish());
            std::dbg!(hasher3.finish());
            if hasher1.finish() != hasher3.finish() {
                return;
            }
        }
        panic!("Hashers built with different builder should be different due to random offset");
    }

    #[test]
    fn test_pubkey_hasher() {
        let key1 = Pubkey::new_unique();
        let key2 = Pubkey::new_unique();
        let builder = PubkeyHasherBuilder::default();
        let mut hasher1 = builder.build_hasher();
        let mut hasher2 = builder.build_hasher();
        hasher1.write(key1.as_array());
        hasher2.write(key2.as_array());
        assert_ne!(hasher1.finish(), hasher2.finish());
    }
}

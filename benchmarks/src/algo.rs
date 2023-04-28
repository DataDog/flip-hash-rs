use std::{fmt, ops::RangeToInclusive};

use flip_hash::{
    flip_hash_64_with_seed, flip_hash_xxh3_128_with_seed, flip_hash_xxh3_64_with_seed,
};

use crate::jump_hash;

pub(crate) trait Algorithm: fmt::Display {
    fn hash(&self, key: &[u8], seed: u64, range: RangeToInclusive<u64>) -> u64;
}

#[derive(Clone, Debug)]
pub(crate) struct FlipHash64;
impl fmt::Display for FlipHash64 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flip Hash (64 bits)")
    }
}
impl Algorithm for FlipHash64 {
    #[inline]
    fn hash(&self, key: &[u8], seed: u64, range: RangeToInclusive<u64>) -> u64 {
        debug_assert!(key.len() >= 8);
        flip_hash_64_with_seed(
            u64::from_ne_bytes(key[..8].try_into().unwrap()),
            seed,
            range,
        )
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FlipHashXXH364;
impl fmt::Display for FlipHashXXH364 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flip Hash (XXH3, 64 bits)")
    }
}
impl Algorithm for FlipHashXXH364 {
    #[inline]
    fn hash(&self, key: &[u8], seed: u64, range: RangeToInclusive<u64>) -> u64 {
        flip_hash_xxh3_64_with_seed(key, seed, range)
    }
}

#[derive(Clone, Debug)]
pub(crate) struct FlipHashXXH3128;
impl fmt::Display for FlipHashXXH3128 {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Flip Hash (XXH3, 128 bits)")
    }
}
impl Algorithm for FlipHashXXH3128 {
    #[inline]
    fn hash(&self, key: &[u8], seed: u64, range: RangeToInclusive<u64>) -> u64 {
        flip_hash_xxh3_128_with_seed(key, seed, ..=range.end.into())
            .try_into()
            .unwrap()
    }
}

#[derive(Clone, Debug)]
pub(crate) struct JumpHash;
impl fmt::Display for JumpHash {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Jump Hash")
    }
}
impl Algorithm for JumpHash {
    #[inline]
    fn hash(&self, key: &[u8], seed: u64, range: RangeToInclusive<u64>) -> u64 {
        debug_assert!(key.len() >= 8);
        jump_hash(
            u64::from_ne_bytes(key[..8].try_into().unwrap()) ^ seed,
            ..=u32::try_from(range.end).unwrap(),
        )
        .into()
    }
}

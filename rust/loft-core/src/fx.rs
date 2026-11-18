//! A fast non-cryptographic hasher (the Fx hash of rustc) for the core's maps: the inputs are the compiler's own,
//! so SipHash's flooding resistance buys nothing and cost 40% of `share`.

use std::hash::{BuildHasherDefault, Hasher};

#[derive(Default, Clone, Copy)]
pub struct Fx(u64);

const K: u64 = 0xf135_7aea_2e62_a9c5;

impl Hasher for Fx {
    #[inline]
    fn write(&mut self, bytes: &[u8]) {
        let mut chunks = bytes.chunks_exact(8);
        for c in &mut chunks { self.add(u64::from_le_bytes(c.try_into().unwrap())); }
        let rest = chunks.remainder();
        if !rest.is_empty() {
            let mut b = [0u8; 8];
            b[..rest.len()].copy_from_slice(rest);
            self.add(u64::from_le_bytes(b) ^ (rest.len() as u64) << 56);
        }
    }
    #[inline]
    fn write_u8(&mut self, i: u8) { self.add(i as u64); }
    #[inline]
    fn write_u32(&mut self, i: u32) { self.add(i as u64); }
    #[inline]
    fn write_u64(&mut self, i: u64) { self.add(i); }
    #[inline]
    fn write_usize(&mut self, i: usize) { self.add(i as u64); }
    #[inline]
    fn finish(&self) -> u64 { self.0.rotate_left(26) }
}

impl Fx {
    #[inline]
    fn add(&mut self, w: u64) { self.0 = (self.0.rotate_left(5) ^ w).wrapping_mul(K); }
}

pub type FxMap<K, V> = std::collections::HashMap<K, V, BuildHasherDefault<Fx>>;
pub type FxSet<K> = std::collections::HashSet<K, BuildHasherDefault<Fx>>;

//! Philox-4x32-7. Seed occupies key.x, stream index occupies counter.x.
//! Spatial hashes floor coordinates to signed integer cells and retain the two's-complement bits.
pub fn philox(mut counter: [u32; 4], mut key: [u32; 2]) -> [u32; 4] {
    for _ in 0..7 {
        let a = u64::from(counter[0]) * 0xd2511f53;
        let b = u64::from(counter[2]) * 0xcd9e8d57;
        counter = [
            (b >> 32) as u32 ^ counter[1] ^ key[0],
            b as u32,
            (a >> 32) as u32 ^ counter[3] ^ key[1],
            a as u32,
        ];
        key[0] = key[0].wrapping_add(0x9e3779b9);
        key[1] = key[1].wrapping_add(0xbb67ae85);
    }
    counter
}
fn uniform(word: u32) -> f32 {
    (word >> 8) as f32 * (1.0 / 16777216.0)
}
fn cell(x: f32) -> u32 {
    x.floor() as i32 as u32
}
pub fn rng(seed: u32, index: u32) -> f32 {
    uniform(philox([index, 0, 0, 0], [seed, 0])[0])
}
pub fn hash1(x: u32) -> f32 {
    rng(0, x)
}
pub fn hash2(p: [f32; 2]) -> f32 {
    uniform(philox([cell(p[0]), cell(p[1]), 0, 0], [0; 2])[0])
}
pub fn hash3(p: [f32; 3]) -> f32 {
    uniform(philox([cell(p[0]), cell(p[1]), cell(p[2]), 0], [0; 2])[0])
}
pub fn rng_key(px: [f32; 2], sample: u32, bounce: u32) -> [u32; 4] {
    philox([cell(px[0]), cell(px[1]), sample, bounce], [0; 2])
}

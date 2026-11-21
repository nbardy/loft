@group(2) @binding(7) var<storage, read_write> Source: array<u32>;
@compute @workgroup_size(1)
fn seed() {
    Source[0] = 7u;
    Source[1] = 11u;
}

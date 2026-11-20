@group(2) @binding(5) var<storage, read_write> answer: array<u32>;

@compute @workgroup_size(1)
fn stamp() {
    answer[0] = 40u + arrayLength(&answer);
}

@group(1) @binding(5) var<storage, read> Source: array<u32>;
@group(2) @binding(9) var<storage, read_write> Output: array<u32>;

@compute @workgroup_size(1)
fn copy_rows() {
    Output[0] = 3u * Source[0];
    Output[1] = 3u * Source[1];
}

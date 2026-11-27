// --- generated entry points ---
{{VIS}}
{{DIAGNOSTICS}}
@group(2) @binding(0) var vis_tex: texture_2d<u32>;
@group(2) @binding(1) var hdr: texture_storage_2d<rgba16float, write>;
@group(2) @binding(2) var<storage, read> tiles: array<u32>;
@group(2) @binding(3) var shadow_tex: texture_depth_2d;
@group(2) @binding(4) var shadow_smp: sampler_comparison;

{{SHADOW}}

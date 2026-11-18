# Loft in Keel

Keel draws Loft art through one crate, `loft-gpu`, on its own wgpu device. Keel never links the compiler: a bundle
(`scene.loft`, `Format::V1`) is built by `loft` and loaded as bytes. Plan: `docs/proposals/2026-11-19_lean_rust_loft.md`
§5.

## The contract

Inputs go in and pixels come out. Nothing comes back. Hit shapes stay in Keel's sim, and a Loft attack is built from the
sim's timing, never the other way round. Loft reads no game state except the inputs and the time Keel gives it.

## Pins

`wgpu` and `naga` are pinned exactly (`=30.0.1`) in both workspaces. `tools/check_pins.sh [KEEL_LOCK]` compares the two
`Cargo.lock` files for `wgpu`, `wgpu-core`, `wgpu-hal`, `wgpu-types` and `naga`, and fails on any difference, or on a
crate that resolves twice. A bump lands in both repos in one
change. Keel runs its mirror script from its side.

## One subject over Keel's frame (step 1)

```rust
let canvas = Canvas { color: surface_format, depth: TextureFormat::Depth32Float, samples: 1 };
let mut loft = Loft::load(&device, &queue, &bytes, Precision::Mixed, &[canvas], None).await?;
loft.set_time(t);
loft.set_input("strike.at", &[x, z])?;
loft.encode_into(&device, &queue, &mut encoder, &View { canvas, color: &color, depth: &depth, size: [w, h],
                                                        owner: Owner::Caller, layers: Layers::Subject })?;
```

With `Owner::Caller`, every view attachment is loaded and nothing is cleared. With `Layers::Subject`, only the subject is
drawn: no sky and no ground. Pipelines are built at load for each `Canvas` given. A canvas that was not given is refused
as `Usage`. Loft depth-tests with `Less` against a depth cleared to 1.0, so a reverse-Z frame is not supported yet.

## Many bosses in one pass (step 8)

```rust
let mut bosses: Vec<Instance> = (0..5).map(|_| loft.instance(&device, &queue)).collect::<Result<_, _>>()?;
bosses[k].set_input(&loft, "at", &[x, 0.0, z])?;          // per instance: time, params, inputs
bosses[k].time = t;
loft.prepare(&queue, &mut encoder, &bosses);              // Frame kernels for all of them, one compute pass
let mut pass = encoder.begin_render_pass(&keel_pass);     // Keel's own pass
loft.draw_subject(&mut pass, canvas, &bosses)?;           // subject draws only, each pipeline set once
```

Each instance owns its frame uniform and a copy of every resource a `Frame` kernel writes. Load-time state, `Every`
histories and pipelines are shared. `instance()` refuses, as `Usage` with the reason, a bundle whose state a copy could
not own: a live `Keep::Latest` stepped kernel, an offscreen or shadow pass, or a view-sized resource written each frame.
Draw such a bundle once per instance with `encode_into`.

MEASURED (M4, Metal, 1280×800, load average 8–11, synthetic sphere-traced boss): five instances in one pass take
12.5–12.8 ms of GPU time (median of 60 frames), against 14.1–14.5 ms for five loads drawn in five passes.

## Hot reload (step 6)

`loft_gpu::validate(&bundle, precision, &device.limits())` runs every check and Naga on every module before any GPU
object exists. If it refuses, keep the old `Loft`.

## The browser (step 2)

`loft-gpu` loads asynchronously, with no `pollster`, and builds for `wasm32-unknown-unknown` with wgpu's `webgpu`
feature. Native backends are `metal, vulkan, gles`, and `dx12` is behind a feature.

## Safety

Keel calls no `unsafe` function. `PipelineCache::open` is safe because it holds an exclusive lock on its directory
until the cache drops, so a second open is refused as `Environment`. Inside `loft-gpu`, modules with
`Origin::Library` compile without Naga's runtime bounds and loop checks. Those modules are emitted with every index
clamped and every loop bounded. Author WGSL (`Origin::Author`) keeps every check.

## Per-boss GPU budget (step 5)

Use `set_timestamps(QuerySet)` with `2 + 2·passes` entries: the kernel pass writes 0 and 1, and render pass k writes
`2 + 2k` and `3 + 2k`. Resolve the set only after the frame has completed, because on Metal an earlier resolve reads
stale ends. On a tiled GPU, passes overlap, so take a frame's time as the span from its first begin to its last end, not
as the sum of the passes.

# Loading Loft pipelines

`Artefact::warm` validates an artefact and creates its layouts, modules, and every compute and render pipeline.
The returned value keeps the pipeline handles and the data needed to start the level. Its `load` allocates and
initializes the fixed draw resources. The existing one-call `Loft::load` uses these same two steps.

On Vulkan, request `wgpu::Features::PIPELINE_CACHE` and open one cache on that device. It can accept several
artefacts before saving an atomic snapshot. The filename uses wgpu's adapter key and the driver strings.
wgpu checks the underlying binary driver compatibility key. An existing rejected file reports its path.

```rust
let artefact = loft_loader::read_dir(creature_dir)?;

// During the loading screen. cache_dir is game-owned and contains only its previous cache snapshots.
let (cache, _) = unsafe { loft_loader::PipelineCache::open(&device, cache_dir)? };
let warmed = cache.warm(artefact)?;
cache.save()?;

// Entering the level. The queue belongs to the device that warmed the pipelines.
let mut creature = warmed.load(&queue)?;
```

On Metal, the call during the loading screen is `artefact.warm(&device)?`. Apple manages shader and pipeline reuse
across launches. wgpu 30 does not expose `MTLBinaryArchive` in its Metal pipeline creation path.
[Apple's Metal binary archive walkthrough](https://developer.apple.com/videos/play/wwdc2020/10615/).

The existing `loft serve` content cache retains Dawn objects on one device. The installed bun-webgpu 0.1.7 has
the sType for a native cache descriptor and a raw descriptor-chain pointer. It lacks the callbacks and cache
userdata needed for persistence. The native bridge and its smallest workable storage contract are specified in
[Persistent Dawn cache](../docs/reliable_render.md#persistent-dawn-cache).

## Work counted

Shell, Serpent and Reefback Titan use 9, 41 and 108 pipelines, respectively. Every load that constructs pipelines
still asks wgpu to process those descriptors and their 7, 30 and 78 WGSL modules. A cache can reduce the driver
work per request. Keeping the actual pipeline handles moves all those requests out of level activation. The
three manifests declare 65,856, 110,868 and 178,456 bytes of fixed GPU buffers, respectively, derived by adding
the buffer descriptor sizes. They upload 0, 10,320 and 10,952 bytes of static blobs. View-sized textures and
driver pipeline allocations are additional.

## Timing method

Apple M4, macOS 15.5 (24F74), wgpu 30.0.1, release. Vulkan used MoltenVK 1.4.2 with Vulkan Loader 1.4.363.0.
Built `examples/shell.ts`, `examples/serpent.ts` and the base of `examples/bosses/reefback_titan.ts`.
Stills and frame timings use 384×240, time 1 s, views `three`, `three` and `game`. The benchmark ran on a
shared host with a one-minute load average between about 37 and 76. The example sources were at `26fa91a`.

Cold runs used worktree-local copies that appended a unique per-backend suffix to entry-point declarations
and their manifest references. This changes shader identity without changing the shader body. Metal's
shared cache contents were uncontrolled. Vulkan cold runs additionally began with zero bytes in the Loft
cache. `--loads 3` runs on one device and draws the final load. A separate process reopens the saved
file and logs the exact byte count. Another process loads the same artefact without the explicit cache to
check how much of the observed benefit survives the OS and driver caches already being warm.

```sh
cargo build --release --locked --manifest-path rust/loft-loader/Cargo.toml
# For the MoltenVK run, also build with --features wgpu/vulkan-portability.
WGPU_BACKEND=vulkan rust/loft-loader/target/release/loft-view <artefact> \
  --view <camera> --size 384x240 --time 1 --loads 3 --pipeline-cache <cache-dir> --out out/first.png
WGPU_BACKEND=vulkan rust/loft-loader/target/release/loft-view <artefact> \
  --view <camera> --size 384x240 --time 1 --pipeline-cache <cache-dir> --out out/restart.png
cmp out/first.png out/restart.png
```

## Metal

The times below are wall milliseconds from manifest validation through the last fixed bind group. File reads,
device creation and the first frame are timed outside this interval. The fresh process column includes whatever
reuse Metal provided across launches.

| Artefact | First shader identities | Same process, loads 2 and 3 | Fresh process | Peak viewer RSS, MiB | Median GPU ms/frame |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shell | 14,797.9 | 23.1, 102.5 | 141.1 | 36.6 | 0.586 |
| Serpent | 15,816.1 | 123.9, 450.9 | 243.4 | 102.9 | 1.371 |
| Reefback Titan | 43,506.9 | 2,081.2, 2,579.9 | 1,327.7 | 268.0 | 16.529 |

## Vulkan on MoltenVK

Cache open includes the file read and `create_pipeline_cache`. MoltenVK serializes converted MSL and invokes
the Metal library compiler while importing it. A Vulkan cache reopen here can therefore include a substantial
amount of compilation before the first Loft pipeline is requested. A native Vulkan driver determines its own
cache contents and import cost.
[MoltenVK cache import](https://github.com/KhronosGroup/MoltenVK/blob/v1.4.2/MoltenVK/MoltenVK/GPUObjects/MVKPipeline.mm#L2736),
[Metal library creation](https://github.com/KhronosGroup/MoltenVK/blob/v1.4.2/MoltenVK/MoltenVK/GPUObjects/MVKShaderModule.mm#L213).

| Artefact | Cold load, ms | Same process, loads 2 and 3, ms | Disk restart, open + read + load, ms | Restart with no explicit cache, read + load, ms | Cache bytes |
| --- | ---: | ---: | ---: | ---: | ---: |
| Shell | 13,601.3 | 19.2, 38.3 | 23,650.7 + 4.3 + 1,887.0 = 25,542.0 | 11,253.2 | 120,286 |
| Serpent | 73,441.6 | 99.4, 204.0 | 2,205.5 + 6.7 + 2,909.9 = 5,122.2 | 128.9 | 1,575,798 |
| Reefback Titan | 75,426.1 | 9,910.2, 9,658.7 | 146.2 + 23.5 + 9,414.1 = 9,583.9 | 13,843.9 | 5,053,955 |

Shell and Serpent's warm repeats followed the disk-seeded load. Titan's followed the cold load. The first
two cold runs used an earlier viewer build with the same creation and storage path. The library API was then
adjusted so cached warming always uses the cache's own device. All disk restarts and the Titan cold run used
the final API. The largest wall times include system compiler and scheduling delays on the shared host.

The three disk restarts read the exact saved byte counts. PNGs match byte for byte across the first, disk-seeded
and cache-disabled draws on Vulkan, and between the same-process and restarted draws on Metal. The Vulkan
disk-seeded processes peaked at 32.2, 108.9 and 206.5 MiB RSS. Their 30-frame GPU medians were 0.809, 1.614 and
2.428 ms at 384×240. Cache writes took 10.7–173.2 ms including a file sync. Activating an already warmed
artefact took 0.078–1.279 ms across the Rust runs. That last reading is CPU setup of fixed resources and
excludes the first frame's view-sized textures and GPU execution.

Process startup remained on the order of seconds with these MoltenVK cache files. The warmed control sometimes
loaded faster than the disk restart. Native Vulkan timings on the game's target driver are still needed.

## Bun and Dawn on Metal

The stock `loft serve` received the built directories directly, three times per process, then once in a new
process. Its `ms.load` excludes reading the files and drawing. The first request starts with an empty Loft
object cache. The OS cache state was uncontrolled. The restart column measures a new server with the ordinary
Metal cache in effect. bun-webgpu has no explicit warm-from-disk mode for Dawn blobs.

| Artefact | First request, ms | Same server, requests 2 and 3, ms | Fresh server, ms | Object hits each repeat |
| --- | ---: | ---: | ---: | ---: |
| Shell | 10,518.7 | 167.7, 208.9 | 837.0 | 28 |
| Serpent | 5,497.2 | 394.9, 328.1 | 6,446.4 | 83 |
| Reefback Titan | 150,704.9 | 27.7, 59.7 | 11,082.4 | 199 |

The frame SHA-256 matched across all four requests for each artefact. Persisting Dawn's blobs and measuring
reuse between training processes requires the native binding described above.

## Verification

- `cargo build --release --locked` passed with the default features and with `wgpu/vulkan-portability`.
  The Vulkan viewer refused a regular file passed as the cache directory and the Metal viewer refused
  `--pipeline-cache` with an actionable message.
- `rtk proxy pnpm typecheck` passed. The generated TypeScript API matches `docs/api.md` byte for byte.
- `loft build` produced all three artefacts. `loft check` passed for Serpent and all seven Reefback Titan
  variants. Shell declares no checks. `loft render --artefact` drew Shell at 384×240 and loaded zero
  compiler modules.
- The targeted `files` and `serve` suites passed all nine tests.
- The existing `soft.render.test.ts` example pipeline check reached its 180 s deadline on the full-suite run
  and on an isolated retry. Both runs were on the shared host. The full-suite process later exited 143
  (SIGTERM) before Bun printed totals. The worktree logs are in `out/pipecache/suite.log` and
  `out/pipecache/soft-render-retry.log`.

## Reviewer rerun (Shell, MoltenVK, same host, load average about 35)

Viewer rebuilt from the final source with `wgpu/vulkan-portability`; Shell rebuilt by `loft build`; 384×240, `three`.

| Run | Cache open, ms | Pipelines, ms |
| --- | ---: | ---: |
| Empty cache directory (cold) | 1.8 | 15,506 |
| Fresh process, no `--pipeline-cache` | — | 7,130 |
| Fresh process, saved 118,481-byte cache | 2.2 | 55.5 |
| Fresh process, saved cache, rebuilt viewer | 646 | 116.8 |

On this rerun the disk cache cut Shell's restart from about 7 s to under 0.8 s including the open; the original
25.5 s restart above looks like host contention. All PNGs were byte-identical across these runs.
On Metal the same Shell loaded in 395 ms (first load in a process) and 6.6–10.7 ms on repeats.

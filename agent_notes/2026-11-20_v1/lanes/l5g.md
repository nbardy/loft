# Phase 5 L7 — bundle and replay

## Prediction before compiling

A canonical directory and fixed-width little-endian metadata make each load/repack
identical, including signed zero and packed words. The warm CPU path does one
file read, two linear BLAKE3 passes and one copy of the shader and row sections.
I predict under 2 ms on this checkout's scene files, against the 10 ms cap.
Naga validation is measured separately from decoding. For 481 fixed crab ticks,
record and replay should differ by 0 bits, including a same-tick parameter update.
The standalone probe should have zero compiler dependencies in its Cargo tree.

## Implementation

- `loft build SCENE -o OUT.loftb` writes an atomic LOFTB001 file. The
  manifest uses fixed-width, little-endian bincode field order. Table bytes and
  portable WGSL are in separate aligned sections. Per-section and aggregate
  BLAKE3 checks cover the payload. The writer emits MANIFEST, ROWS, and MODULES.
- The CLI lowers the existing Program's row, snapshot and rendering declarations
  into the frozen Bundle types. It derives pass bindings from Naga entry-point
  usage. It moves read-only storage, writable storage and images to groups 1, 2,
  and 3. The legacy per-kind uniform becomes read-only storage; group 0 stays
  fixed. Frame-dependent resources are external. The classify dispatch targets
  the existing 1280×800 schedule size.
- `loft_runner::bundle::decode` and the `loftb` runner example open a file
  without the compiler. `Runner::load_bundle_bytes` delegates to the existing
  device load after decoding. Explicit Naga validation stays off the timed
  read-and-decode path.
- `loft_runner::bundle::WebGpu::attach` takes the host's browser GPUDevice as
  a JS handle and uses its queue. The optional `web-sys` loader prepares the WGSL
  modules and buffers for a host encoder. Three.js can supply its existing device.
- `LOFTLOG1` and JSON use a strictly ordered fixed-step trace. The runner API
  records Level, Event, PosePacket and Param updates. `End` carries the last tick
  even when there are no prior updates. The CLI binds the log to the BLAKE3 of the
  built bundle. It replays scalar updates on the existing expression twin,
  hashes final row bytes, and reports the first missed Truth verdict on
  `check --replay`. Recording preserves failing traces. With `--out`, render
  uses the final twin snapshot as input to the existing native renderer.

## CPU proof

| Check | Result |
| --- | --- |
| Locked native check, `tools/dev.sh check loft-cli` | Passed |
| `tools/dev.sh check loft-runner --target wasm32-unknown-unknown` | Passed, also with `--features web-sys` |
| `tools/dev.sh test loft-runner -E 'test(bundle::container_rejects_corruption)'` | Passed, including exact signed-zero bytes, checksum and truncation |
| `tools/dev.sh test loft-cli -E 'test(build::trace::tests::scramble_replays_every_state_bit)'` | Passed. 481 steps, 32 final state bytes, 0 differing bits in binary and JSON. Removing the later same-tick update changes state and fails identity verification. |
| `loft check` on the checked-in scramble log | Passed, state BLAKE3 `aee66a2dfb6c1707cb3b017d55b4dd0cd91894c6edd8c92f0c79dba276267d72` |
| `loft render --record` and `loft check --replay` | Identical CLI state identities. The 20 mm pin-drift recording replays with exit 2. |
| Compiler-free standalone example | The normal Cargo tree and the binary symbol table have no `loft-passes` or `loft-parse`. |
| Scene sweep | 11/11 builds and repacks were byte-identical. Naga validated all 60 shaders in those files. |

| Build | Bytes | Warm file read + decode |
| --- | ---: | ---: |
| blobs-1 | 41,216 | 0.048 ms |
| blobs-20 | 43,712 | 0.053 ms |
| capsules-1 | 40,801 | 0.049 ms |
| capsules-20 | 54,625 | 0.061 ms |
| crab | 155,669 | 0.154 ms |
| hermit-1 | 173,358 | 0.166 ms |
| hermit-20 | 222,766 | 0.205 ms |
| heron | 97,382 | 0.096 ms |
| pin-drift | 26,800 | 0.035 ms |
| reeds | 83,548 | 0.084 ms |
| scramble | 93,778 | 0.092 ms |

The sweep covers the six gate configurations, three syntax scenes, the pin
mutant, and the scramble. `engines/realtime.loft` is a recipe. The baseline
source build of that file alone refused `Drawables` (E011). The 295-scene
regenerated corpus is on other lanes and was outside this checkout.
Times use the standalone sample's second read and decode in one process.
Naga parsing, pipeline creation, and drawing are outside that measurement.
No GPU device, pipeline or render was invoked.

## Files and integration

- `crates/loft-runner/src/{bundle.rs,bundle/wire.rs,bundle/web.rs,replay.rs}`
  and `crates/loft-runner/examples/loftb.rs`
- `crates/loft-cli/src/{main.rs,build.rs,build/artifact.rs,build/trace.rs}`
- `crates/loft-cli/tests/l5g/{scramble.loft,scramble.loftlog}`.
  The scene uses L23c's scramble and required motion helpers with local names,
  and a live speed parameter. The log changes that parameter twice on tick 80.

Run the CPU gate on a freshly built branch with:

```sh
CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --bin loft --profile test
CARGO_BUILD_JOBS=2 tools/dev.sh build loft-runner --example loftb --profile test
target/debug/loft build engines/hermit.loft -o out/hermit.loftb
target/debug/examples/loftb out/hermit.loftb out/hermit.repacked.loftb
cmp out/hermit.loftb out/hermit.repacked.loftb
target/debug/loft check crates/loft-cli/tests/l5g/scramble.loft --replay crates/loft-cli/tests/l5g/scramble.loftlog
```

The GPU integrator can use `loft render … --replay … --out repro.png` with the
same fixture to exercise picture submission. If merged compiler output changes
the fixture's bundle identity, regenerate using the focused test with
`LOFT_L5G_FIXTURE_OUT` pointing to an output file. Then compare the expected
state bytes and re-score Truth before replacing the fixture.

The device-side bundle load and browser integration need a GPU smoke test.
The base Runner has a group 3 execution refusal. L5b's host renderer also
expects one color and one depth target, while the compiler's legacy render
schedule has intermediates. Browser attachment prepares resources for the
host. Browser command encoding is pending. The CPU CLI backend has no
event or pose packet port, and cross-device tolerance verification still
needs a numeric reference beyond the final digest. The file decoder accepts
the generated portable three-section format. Optional asset, span and Naga IR
sections still need readers. This lane exceeds the LOC prediction, and the
runner's pre-existing total already exceeded its project ceiling. No frozen
types, golden vectors, dependencies or crate entries changed.

# s1_escapes

The lane starts at origin/main `5369870dfae8c5776961f8d044c613284346dc96`. The retained IPF renderer
at `b0fdb44` is an ancestor. The scored I1 candidate is outside main's ancestry.

## Prediction before implementation

- StudyPaper exchanges 64 segment evaluations per material call for one bilinear level-zero
  sample. With a 512² bake over two world units, the interpolation error of the 1-Lipschitz
  distance is bounded by 0.002763 world units inside the texture plus R16 quantization.
  Compare decoded RGBA and 300-frame p95 against a procedural control on the same adapter.
  Predict mean RGB error below 1/255 and a smaller steady-state p95.
- Eight invocations adding their 1-based ID to one i32 cell give 36 exactly. Adding one half
  of those values to one float cell gives 18.0 with exactly representable test operands.
  The integer accumulator is Det and the float accumulator is NonDet.
- A whole-pass read of [7, 11] with a multiply-by-three store gives [21, 33]. It reflects
  one group-1 read binding and one group-2 writable binding. Its truth is Unknown absent a twin.
- A sharp field with an opted-out step of 0.5 takes half the returned distance per iteration.
  Normals use four field taps. Its rendered normals and beauty contain foreground pixels,
  and the kind carries Unproved(AuthorOptOut).
- Existing scenes without these operations keep the same emitted WGSL and all 18 scored RGBA
  digests. The base grammar corpus currently has 281 cases per recognizer. Six mutants still fire.

## Source and runtime contracts

Standalone compute schedules declare typed storage with
`table(Name, row: RowType, capacity: n)`. The row type defaults to the table name.
The runner initializes the storage to zero. Schedule a typed entry with
`compute(entry: kernel_name, extent: n, dst: Name)`. A one-dimensional extent
counts workgroups. `workgroup` sets the local size. Both accept XYZ vectors.
`global_id(0)` reads the X invocation ID. Kernel arguments retain the source
type and unit checks. The integer deposit uses a native atomic operation.
Float deposits use compare-exchange on the stored binary32 bits and tag the
written table NonDet.

A whole compute module uses `dispatch("module.wgsl", "entry", extent)` inside
a pass. The linker resolves the module relative to the scene, validates the
entry with Naga, checks its resource names and buffer strides, and assigns
binding slots from each resource's access. The compiler reports Unknown(NoCpuTwin)
for authored compute. The wire format does not carry these proof records.

Sampled kernel textures use group 3 and accept externally supplied baked views
through the runner. The test supplies a texture produced by the native load bake.
2D and 3D filterable kernel sampling uses texel interpolation and explicit mip
selection. Material code emits the filterable texture and sampler declarations.
The existing native renderer and renderer bundle exporter need matching material
and path bindings. The lane's file ownership excludes those paths, so Spectral
Caustic Study still uses its procedural material and there is no paired speed or
image result for StudyInk.

Opted-out traces derive their culling proxy from natural intervals and use the
authored scale for marching. Surfaces use four diagonal taps to recover their
two tangents. The compiler report carries Unproved(AuthorOptOut). The CLI on
this base lacks the `truth` subcommand and its implementation is outside lane
ownership.

## Measurements

- The requested all-targets check and workspace doctests passed. Earley and
  llguidance agreed with all 281 existing cases and both new scenes. All four
  compiler integration tests and six gate mutants passed.
- On Apple M4 Metal, `loft build` produced a 12,369-byte bundle. Runner readback
  gave i32 **36**, f32 **18.0**, baked mip sample **0.375**, and raw WGSL output
  **[21, 33]**, all exact. The full suite initially hit the runner's requirement
  to declare the external texture shape before attaching it. The focused rerun
  passed after the test harness declared that shape.
- The 96×64 opt-out test rendered beauty and 660 colored normal pixels on Apple
  M4 Metal. The compiler reported Unproved(AuthorOptOut). The CLI also rendered
  both 320×200 AOVs through pixel readback. Its WebP export failed because the
  installed FFmpeg has no libwebp encoder.
- The untouched Study scene compiled. A compile-only evidence copy changed the
  paper expression to `sample(StudyInk, 0.5 * (p.xz + [1.0, 1.0]))`.
  The spectral Present shader validated with one group-3 StudyInk declaration,
  a `textureSampleLevel` call and no procedural ink call. The copy has no GPU
  visual or timing score.
- All **18/18** rendered RGBA hashes matched retained main. Frozen reference
  files had an empty diff. Pictures was 0/18 against the older frozen pins,
  Twin 36/36, Truth 34/34, Load 23/33 and Speed 6/12. All six frame-time
  measurements were contended. The 1280×800 Apple M4 Metal run used 300 timed
  frames per scenario and the shipped 1/20-instance overrides.

| Scene | P95 (ms) | Triangle control p50 (ms) |
| --- | ---: | ---: |
| capsules-1 | 6.816 | 0.873 |
| capsules-20 | 6.552 | 1.036 |
| blobs-1 | 6.979 | 0.905 |
| blobs-20 | 8.509 | 0.785 |
| hermit-1 | 10.906 | 2.186 |
| hermit-20 | 16.767 | 0.791 |

The workspace passed 158/160 tests in the full run. The successful focused
readback rerun brings the observed total to 159/160. The remaining miss was
the existing Tier 0 promotion gate at 0.595 ms p95 against a 0.500 ms cap,
with zero pixels over 1/255. Retained main previously measured 0.637 ms on
that test.

Evidence is in `out/meta/s1_escapes/`. The full test results are in
`native.log`, the readback rerun is in `compute-gpu.log`, the proof and AOV
readback are in `opt-out-gpu.log`, and `rgba.diff` contains the digest comparison.

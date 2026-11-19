# Render implementation, Step 3

Branch `feat/design-render` starts at 5d94531, which contains the draws merge 4e8ee7b. Scope is the vertical slice in §8 of the consolidated architecture. The motion lane owns BODY, STATE, POSE and its tick. Render consumes ordinary packed rows. Both lanes need to reconcile the recorder version, native handshake and bundle reader if both extend them.

## Predictions before implementation

| # | Instrument | Prediction and acceptance |
|---|---|---|
| 1 | Hash every module at N=10,100,1000 | Identical texts, at most 8 pipelines and 2,000 lines |
| 2 | Diff row sections and module hashes | Washer edit changes one row only. Adding a bead changes three modules across the two engines |
| 3 | Native profile after device creation | 30 ms warm, 280 ms cold, ceilings 60/350 ms and spread ≤10% |
| 4–5 | GPU record and shadow against integer pixel-centre oracle | 9,664 and 12,080 ring samples, same counts shifted 128 pixels left for shadow. Solid shadow 12,892. Zero bit errors |
| 6–7 | Pick, secondary and bead probes | Washer ID 9/9 then 9/7, secondary distances 2/2 then 2/1. Bead miss, 2.2, 0.35 inward, 0.4 outward |
| 8 | Record vs independent centre ray, two cameras | Interior IDs identical and relative depth error ≤1e-4, tolerance 0.01% pixels |
| 9–11 | Linear radiance probes | Closed box 1.50/1.75/1.999985 E within 1%. Rough white metal 0.50 within 2%. One bounce mean bias ≤1% |
| 12–13 | Pass timestamps and whole-frame p50/p95 | Shade 3.0/5.2 ms, frame 4.5/6.7 ms at N=10/30. Training 21.7 ms, p95 ≤50 ms |
| 14 | Isolated still, clip and static viewer | Same still bytes. Viewer vs 64-sample still ≤1% |
| 15 | 9 bead frames from R0's commit in flat colour and log depth | 9/9 under rule (a) |
| 16–17 | Added core identifiers and o200k_base over runnable consumer cases | Zero domain identifiers. Cases 263, 231, 171+69, 377, 107 tokens within 15% |

The initial R0g score was 5/9. The four oblique 512² frames had 53 over-threshold pixels each, all on reference edges. A ray made by subtracting the eye from an unprojected near point loses precision at the near plane. R0 derives its ray from the interpolated proxy position. Matching that arithmetic on the cover should put the oblique count below 26.

The corrected cover passes all nine RGBA comparisons with the existing parity-diff rule, including log distance in alpha. The high camera had 3 pixels over one code, the oblique camera 16, and the detail view 6. All clusters were singletons on reference edges. The washer masks had zero wrong bits, with 9,664/12,080/12,892 blocked as predicted. Picks and secondary queries gave the predicted IDs and distances. The same bead WGSL ran in a CPU interpreter and on Metal, with maximum distance error below 2e-7. Both 512² record/trace views had zero mismatched interior pixels. The oblique view had 197 mixed-coverage edge pixels.

The first equal-bounce light score missed. Mean per-pixel signed relative errors were −25.6% to −25.7%. At distant floor pixels the record shader sometimes returned exactly zero while the path result was about 0.2. Unprojecting a 32-bit device depth at distance placed the origin behind the floor by more than the secondary ray's minimum distance. Since analytic record shading already reads the one selected shape for its normal, its intersection also gives a surface-consistent origin. Keep stored depth for the coverage and depth oracle, use the local analytic hit for the lighting query.

### Calibration before scoring the timed slice

At 256² a dependent chain of 256 scalar FMAs per fragment cost 0.561 ms above baseline, or 16.72 ps per operation. A 128-load coherent loop measured 51.53 ps per load after subtracting an add. The original constants were 2 and 30 ps. The 8² encoder measured 0.625 µs per additional draw in one render pass. That last number applies only within a pass. Using the proposal's ten operations plus one load per bound gives 218.8 ps, and sixty operations plus three loads per hit gives 1.158 ns. Before timing the slice, scaling its intercept by the hit ratio and slope by the bound ratio predicts shade = 11.0 + 0.481N ms, or 15.8/25.4 ms at N=10/30, and whole frames 17.3/26.9 ms. The corresponding 16-sample 512² projection is 104 ms. The original 50 ms training ceiling still applies. The microbenchmark uses dependent arithmetic and can overpredict parallel shading.

The analytic-origin correction brought the three channel-integrated biases to −0.23%, −0.31% and −0.34%. The stricter mean of the per-pixel relative differences still missed at −2.07%, −1.99% and −1.83% at 48². The regular reference jitters its camera ray while the record shades at the centre, so a pixel containing four equal IDs can still include a highlight or shadow boundary. For an equal-observation light check, run the reference on the same primary rays and report both results. Keep the independent jittered score above as a picture measurement.

At motion head 21bd409 both lanes use wire 5. Motion uses eleven spans and adds kernel/tick declarations, with native stamp `abi 5 wire 5`; render uses a twelfth span for ordered programs and stamp `abi 6 wire 5`. The merged decoder can classify by span count. Retain motion's tick fields in Bundle V1 and its tick prefix in FrameKey, with the ordered schedule in V2 alongside them. Init/advance/present must run before a render pass reads POSE. The first slice's analytic rows are static; rendering a live post-IK POSE requires an integration test.

### Light and timing results

The closed furnace gave 1.50, 1.75 and 1.999956–1.999998 E. The white furnace gave 0.499843 at centre and 0.500136 at rim. At 256 samples with identical centre rays, the mean per-interior-pixel biases were −0.840%, −0.841%, −0.834% over 2,229 pixels. A four-sample isolated still matched the clip byte for byte. The first history probe, measuring the red channel in linear light, differed from a 64-sample still by 0.917%.

At 10/100/1,000 things the same six pipelines and six module texts loaded. The timed bundle had 614 shader lines, excluding the host camera module. Changing the washer's radius flipped exactly pack 1 word 0. Introducing the bead changed the cover and shade in realtime and the path in reference. For this edit both sides have an existing copper box, so the bead introduces only its shape.

Cold device loads were 353.208/339.694/340.789 ms. The first exceeded 350 ms. Warm medians were 6.402/6.408/6.435 ms. The cross-N spreads were 3.98% cold and 0.52% warm. At 1280 × 800 the shade medians were 2.280/4.427 ms, giving 1.207 ms + 0.1073 ms per thing. The calibrated slope and intercept missed the ±30% gate. Whole-frame medians were 2.381/4.531 ms. Training at 512² with 16 samples, N=30, took 17.644 ms median and 18.020 ms p95 over 16 frames. These are the first scored timings. GPU work at different pixels overlaps far better than the dependent arithmetic microbenchmark predicts.

### The author’s side

```ts
import { scene, thing, washer, matte, print } from "loft/render";

const red = matte([0.8, 0.3, 0.1]);
const ring = thing(washer(0.5, 1), red, [0, 0, 1]);
export default scene({
  things: [ring],
  shadows: [thing(washer(0, 1), red, ring.at)],
  cameras: { front: { kind: "orthographic", eye: [0, 0, 3], target: [0, 0, 0], height: 8 } },
  engine: print(),
});
```

The outer and inner radii are instance data. The camera and secondary rays see the hole. A shadow ray sees the disc. The default print look calls the same sun, material, albedo and visibility readings as an authored look can call. A material may define `surface(m,x)` to vary its fields with the chart before any BRDF reading. [Runnable striped material](../../examples/slice/looks.ts) · [Whole reference engine](../../src/engines/reference.ts).

Check 14 over all three linear channels gave 0.917%, 0.965%, 0.915% mean absolute relative difference, or 0.932% overall. The final static still remained byte-identical. No per-channel average was taken after display mapping.

Check 16 found zero new domain identifiers in the scanned core. The five lexical matches were existing resource-layout `Shape/shape`, validation mode `Naga::Emit`, Naga's `emit_to_string` and wgpu's mandatory `occlusion_query_set` field. The vocabulary and exceptions are printed by the audit. Check 17 measured the actual source excerpts using o200k_base. Cases 2/3/4a/4b/5/6 cost 200/185/153/28/666/163 tokens against 263/231/171/69/377/107. Only 4a falls within the specified two-sided 15% band. The reference count includes its bounce loop and executable schedule. Instance fields and sharing the same shadow membership table make cases 2, 3 and 4b shorter.

### Normal entry points and replay

`loft build examples/slice/looks.ts --out out/looks` opts in through the scene. `loft render examples/slice/slice.ts --views front --out out/slice.webp` renders the program directly. `loft show examples/slice/looks.ts --thing shade` prints the linked material, shade and world functions, including the world membership bit the visibility query uses. Built directories work with `render`, `frames` and `clip`. A two-frame built-directory clip reported AV1 and two frames. Ordered pictures use WebP quality 95. [64-sample slice, 480 × 300](render_impl/slice.webp).

The reader accepts eleven spans on wire 4 and eleven or twelve on wire 5. Motion's additional tick declarations still require the motion decoder when combined. Native frame keys with previous-image reads carry the digest of the prior observations, including their times and controls; resizing or switching cameras begins a fresh prefix. The actual shader sample counter advances only when the program requests previous images. Wire 4 keeps its original identity shape. Native shader inspection currently reports zero authored site annotations. A legacy bead built on wire 4 and drew three draws in two passes at 32².

### Nominal graph and CPU observations

Wire 5 declares nominal kinds and open functions as tags 12 and 13. Tag 11 is reserved for motion's RowStep. The shared graph decodes and types U32, Kind, Make, Field and Open. The WGSL emitter specialises a literal selector to a direct call and emits one switch for a dynamic selector. The scalar CPU tape keeps each u32 in two exact 16-bit pieces. Focused tests distinguished 0xfffffffe from 0xffffffff on the tape, validated emitted WGSL with Naga, and refused an out-of-range field and a non-u32 selector at decode. Native linking now constructs the WGSL row readers. Numeric and vector template holes become row fields; changing a captured number from 2 to 6 gave identical code and tag and changed one word.

`const q = await twin({things:[ring,card]}); q.trace([0,0,3],[0,0,-1])` returned ID 9 at distance 3. Moving the ray to y=0.75 returned ID 7 at distance 2. The twin interprets the actual linked WGSL and packed rows on the CPU. The GPU and CPU bead checks now exercise this public entry point. The raw shader bodies use Naga's WGSL IR and its validation; the legacy TS scalar expression writer still owns its numeric subset. The final native stamp is `abi 7 wire 5` because the CPU observer calls the same native source linker as the bundle writer. The identifier audit also encountered `Decl.Medium` on an existing enumeration line extended with Kind and Open.

### Phase 2 instrument, before running

Use two covers, each with its own indexed grid and a GPU-written indirect command. A frame kernel reads the chosen level per copy, compacts the original row ids and writes the command count. The 64 × 16 fixture has four 16-pixel-wide slots. At time 0 its levels are [coarse, hidden, fine, coarse], at time 1 they are [hidden, fine, coarse, hidden]. Predict 512/256 coarse/fine pixels at time 0, and 256/256 at time 1, with zero wrong pixels against the ordinal slot oracle. Both frames must come from one load. This exercises actual index and indirect buffer use and the empty draw case independently of the corpus migration.

The indexed indirect test matched all 1,024 pixels at each of the three times. Counts were 512/256, 256/256 and 0/0. The last frame hid every copy and both GPU-written commands had zero instances. The GPU executes two indexed indirect draws and two frame kernels, independent of the number of copies. The compactor preserves source-row order with a 64-lane workgroup prefix scan; it processes successive blocks on the same workgroup. Throughput on a full corpus grid remains to be measured. [Grid and compaction laws](../../src/render/covers/grid.ts) · [Readback oracle](../../tools/slice_indirect.ts).

### Motion seam, before integration fixture

Render one washer twice in the same bundle. A graph-filled POSE row translates its centre from x=0 to x=1 with time. With an orthographic span of 8 and width 64, predict an 8-pixel centroid change and equal visible area. The primary record and independent trace must agree on every pixel at both times. This tests the ordinary table boundary in this checkout. A combined tick-driven crab remains the merge reviewer’s test.

The shared POSE fixture passed. Both independent renders found 156 pixels at both times, with centroids 32 and 40 and zero mask errors. An ordered scene names the existing graph tables it reads in `tables`, e.g. `tables:[pose], origin:"fn origin(i:u32)->vec3f {return t_POSE(i).xyz;}"`. The native linker preserves existing kernels, resources and packs, relocates ordered group-1 bindings using Naga's parsed resource identities and appends the ordered work. Each shared table retains the recorder's existing accessor and packed offset. Covers, primary and secondary queries, and sampled sphere emitters use the same origin law. A CPU query on a live table takes the corresponding frame snapshot. [Executable shared-table fixture](../../tools/slice_pose.ts).

Motion head 90f9b23 now preserves wire-4 job bytes and uses tag 11 only for RowStep. Integrate its scheduling, POSE and tick-prefix work with render's 11/12-span decoder, tags 12/13, ABI 7 and previous-frame key. Keep its `loft show` laws alongside ordered pass-module inspection. The live graph-table fixture is ready to rerun with the actual post-IK POSE in the integration worktree.

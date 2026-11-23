# perf-edges: `edges: none | coverage | taps` (23 Nov 2026)

Branch `perf-edges`, base `perf-frame` (`683bcb3`). Replaces `shade_tiles(..., samples: 4)` with
`edges: none | coverage | taps`. `taps` is the old `samples: 4` path (4 BVH rays from the root per edge pixel,
unchanged WGSL), `none` the old Center path, `coverage` is new.

## Theory (written before the code)

### Where coverage comes from

Legacy took coverage from the closest approach of a pixel's own ray to the surface (min SDF along the march,
in pixels): full on a hit, `clamp(1.3 - gapPx, 0, 1)` on a miss, resolved by the hardware 4x MSAA. The native engine
has one ray per pixel in the raster Vis64 pass, and a miss there is discarded, so the closest approach of a missing
ray is not recorded anywhere. Recording it would need a second, non-depth-tested attachment. Instead the shade pass
recomputes it, only at pixels whose 3x3 neighbourhood holds a different visibility ID, from data already present:

- A 4-neighbour's Vis64 record names the other row and its hit distance `t0` along the neighbour's ray.
- The row's `trace` field (every non-surface kind has one; it is also what the World path calls `world_field`)
  evaluated along *this* pixel's ray near `t0` gives the closest approach `m = min_t f(o + d t)`.
  Three samples at `t0 - w, t0, t0 + w` (w = one pixel's world size at `t0`) and one parabola-vertex step
  (a Newton step on the closest approach), keeping the smallest of the four. For a sphere of radius 50 px seen 0.5 px
  inside its rim the hit lies 7 px in front of the rim depth and the field at `t0` alone reads 0; the Newton step lands
  within 0.15 px of the rim depth and reads the exact -0.5 px. Every value is a field sample on the ray, so for an
  exact SDF it never under-reports the gap.
- `s = -m / w` is the signed distance in pixels from the pixel centre to that row's silhouette, positive inside.
  This holds on both sides of the edge: inside a silhouette, min f along the chord is minus the depth of the deepest
  chord point, which near a rim is the screen distance to the rim. So the inside pixel gets a real coverage too,
  not only the outside one (legacy only ramped the outside, which dilated silhouettes by ~0.8 px).

Coverage is the 1-pixel box filter of that half-plane, `clamp(0.5 + s, 0, 1)`, the SDF-text antialiasing rule:

- neighbour row nearer than this pixel's own surface (or this pixel is sky): it occludes; blend weight toward the
  neighbour's colour = `clamp(0.5 + s, 0, 0.5)` with the neighbour row's field;
- neighbour farther (or sky): this pixel's own silhouette; weight toward the neighbour = `clamp(0.5 - s, 0, 0.5)`
  with the own row's field.

The pixel keeps the largest weight and the direction it came from. Searches for an occluder stop at the pixel's own
hit distance (the ray is blocked behind it).

Per geometry kind:
- SDF trace kinds and analytic kinds: the kind's `trace` query (analytic kinds must also declare one). Raw field
  values are used as distances; the stdlib solids are exact SDFs.
- Surface grids and meshes (raster-only, `light:` path only): no field, so no coverage. Those kind modules keep the
  centre path. Hardware raster coverage could supply it later; that needs a multisampled Vis target.
- World (`shade:`) path: `world_field(kind, at, p)` exists for every kind, so every pair of rows can be resolved.
- Bsdf (`light:`) path: each kind's module only has its own field. Own silhouettes always work; an occluder of a
  different kind is skipped (one-sided ramp, half-height steps on those edges). Same-kind rows (most scenes) are full.

### Blend without the hardware resolve

Ownership is the existing halo rule (classify_halo + `loft_vis_neighborhood`): exactly one module writes each pixel,
including sky pixels next to geometry. Shade writes its own colour (centre shading, the same code as `none`) and
encodes in HDR alpha: `0` unshaded sky (classify zeroed it), `1` plain, `+(2 + k + w)` blend toward 4-neighbour
`k` by `w <= 0.5`, and `-(2 + k + w)` the same for a sky pixel. `rgba16float` keeps `w` to 1/256. Present reads the
partner texel only at blended pixels and uses the sky when that partner's alpha is `<= 0`. The perf-frame present test
`alpha == 0` becomes `alpha <= 0` (sky) for coverage; `taps` keeps its present unchanged. No extra BVH traversal.

### Predicted cost (realtime_matched, Apple M4, 1280x800)

From perf-frame's paired probes: the frame ratio of centre-only shading to taps is 0.68 (B-1) / 0.47 (B-20) and the Shade
ratio 0.52 / 0.31; perf-frame's window-2 frames were 0.910 ms (B-1) and 5.287 ms (B-20). So Shade(taps) ~ 0.61 / 4.06 ms,
Shade(none) ~ 0.32 / 1.26 ms, frame(none) ~ 0.62 / 2.49 ms.

An edge pixel under coverage pays 4 Vis loads, 2 ray directions and ~1.5 candidates x 4 capsule field evaluations (each
a row load and ~40 flops): about a third of one BVH ray, against ~2.5 ray-equivalents for the centre shade (sun ray,
bounce ray, BSDF). Halo sky pixels pay the same without the shade. SIMD groups containing an edge pay it for the whole
group: ~25% of covered groups at B-1, ~50% at B-20.

| | Shade B-1 | frame B-1 | Shade B-20 | frame B-20 |
| --- | ---: | ---: | ---: | ---: |
| none | 0.32 | 0.62 | 1.26 | 2.49 |
| coverage (predicted) | 0.34 (none x1.05) | 0.64 | 1.36 (none x1.08) | 2.59 |
| taps | 0.61 | 0.91 | 4.06 | 5.29 |

Predicted edge cost = coverage - none: +0.02 ms Shade at B-1 and +0.10 ms at B-20, i.e. Shade(coverage)/Shade(taps) = 0.55
and 0.33, frame ratio 0.70 and 0.49. Present: +1 texel read at the ~2-4% blended pixels, under 0.01 ms.
Hermit-20 (`light:` path, Center today, no taps possible): Shade +3..6%, frame +2..4%.

Pictures: `none` and `taps` byte-identical to the base. `coverage` changes only pixels with a differing neighbour, so it
cannot pass the deterministic picture rule (< 1 in 1,000 pixels) on any gate scene with visible silhouettes; the default
therefore cannot become `coverage` without a re-pin (owner decision). Predicted changed share: 1-3% of pixels on
capsules/blobs/hermit.

## What was built

- `shade_tiles(..., edges: none | coverage | taps)`; `samples:` is gone (no alias). `taps` on a `light:` schedule is an
  error (as `samples: 4` was); `coverage` works on both `light:` and `shade:`; `coverage` or `taps` with a paper
  present is refused. Omitted means `none`.
- `crates/loft-emit/src/emit/shade_coverage.wgsl` (closest approach + per-pixel weight/direction),
  `shade_coverage_owner.wgsl` (halo ownership and the alpha store, shared by both paths), `shade_tiles::bsdf_coverage`
  (rewrites a ray kind's `light:` module), the coverage arm of the World module, the coverage resolve in `emit_present`.
  `tiled_shade.wgsl` takes `{{ALPHA}}` (`1.0` for none/taps, so their WGSL text is unchanged).
- Migrated `stdlib/engines/realtime_matched.loft` and the `shade_schedule.rs` test to `edges: taps`; the test also lowers
  coverage on both paths through naga and the bundle.
- Per-kind override: not built. It needs a new construct (a per-kind map argument or a kind attribute); the per-kind
  modules would accept it, but the language has nowhere to say it.

## Verification

- `tools/dev.sh test loft-passes`: 77/77 (includes the new coverage lowering). `tools/check.sh static`: pass.
- `loft-verify gate --only` the seven gate scenes (gpu_lock, Apple M4): Pictures 19/19, Twin 36/36, Truth 34/34.
  Speed contended (triangle p95 4.8-12 ms), Load fails on cold/warm and hermit 14 vs 13 pipelines exactly as on base.
- Stills, 1280x800, t = 0, 4, 9, counts 1 and 20 (`out/perf-edges/` in the primary checkout, lossless PNG + WebP q95):
  base perf-frame binary vs this branch, `none` (scenes that omit `edges`) and `taps` (matched B): **0 pixels differ**
  in all 24 + 12 comparisons.

## Pictures: none -> coverage (pixels with any channel change > 1 code / all)

| scene | t=0 | t=4 | t=9 | max code |
| --- | ---: | ---: | ---: | ---: |
| capsules-1 | 0.154% | 0.154% | 0.160% | 150 |
| capsules-20 | 1.19% | 1.31% | 1.36% | 165 |
| blobs-1 | 0.093% | 0.079% | 0.082% | 87 |
| blobs-20 | 0.72% | 0.74% | 0.73% | 151 |
| hermit-1 | 0.34% | 0.38% | 0.34% | 205 |
| hermit-20 | 2.40% | 2.49% | 2.41% | 240 |
| matched B-1 | 0.156% | 0.154% | 0.161% | 164 |
| matched B-20 | 1.18% | 1.31% | 1.35% | 173 |

Predicted 1-3% at 20 groups: hit. Only silhouette pixels change. Visible result (crops in `out/perf-edges/crops`): the
stairs on capsule, blob and crab silhouettes become ramps, and the dark one-pixel fringe that centre shading leaves
at grazing silhouettes (capsule against floor, blob rims) is gone; thin crab legs and the shell grid against the
ground are smoother. Against `taps` on matched B the per-pixel error on edge pixels falls 57-70% at B-1 t=0/4, but
only 12-38% elsewhere: matched B is 1-sample bounce noise, and `taps` averages up to four shades per edge pixel,
so that metric is dominated by noise `coverage` does not average. Sub-pixel features the centre ray misses (a leg
thinner than a pixel) are not recovered; `taps` recovers them.

## Timing (Apple M4, 1280x800, 300 frames after 30 warm-up, 8 shuffled paired rounds, `tools/quiet_gpu.sh`,
## load 6-9; the window was flagged "build overlap" by quiet_gpu, so treat absolute ms as orientation)

Medians of the 8 rounds (ms). Absolute times are ~1.5-2x perf-frame's window 2 on the same code (`none` and `taps`
are byte-identical to base), i.e. the machine was slower, not the code.

| | Shade | Classify | Present | frame p50 | frame p95 |
| --- | ---: | ---: | ---: | ---: | ---: |
| B-1 none | 0.985 | 0.140 | 0.276 | 1.637 | 1.845 |
| B-1 coverage | 1.131 | 0.459 | 0.282 | 2.117 | 2.328 |
| B-1 taps | 2.586 | 0.457 | 0.269 | 3.599 | 4.042 |
| B-20 none | 3.352 | 0.086 | 0.193 | 4.102 | 6.841 |
| B-20 coverage | 3.684 | 0.272 | 0.206 | 4.670 | 7.074 |
| B-20 taps | 6.378 | 0.147 | 0.203 | 7.103 | 7.905 |
| Hermit-20 none | 2.684 | 0.071 | 0.229 | 4.788 | 6.577 |
| Hermit-20 coverage | 3.012 | 0.185 | 0.195 | 5.099 | 5.902 |

Predicted vs measured:

| ratio | predicted | measured |
| --- | ---: | ---: |
| Shade coverage / none, B-1 | 1.05 | 1.15 |
| Shade coverage / none, B-20 | 1.08 | 1.10 |
| Shade coverage / taps, B-1 | 0.55 | 0.44 |
| Shade coverage / taps, B-20 | 0.33 | 0.58 |
| frame coverage / taps, B-1 | 0.70 | 0.59 |
| frame coverage / taps, B-20 | 0.49 | 0.66 |
| Hermit-20 Shade coverage / none | 1.03-1.06 | 1.12 |
| Hermit-20 frame coverage / none | 1.02-1.04 | 1.065 |

- The edge cost itself (Shade coverage - none): +0.15 ms at B-1, +0.33 ms at B-20, +0.33 ms on Hermit-20; predicted
  +0.02 / +0.10. A miss by 2-7x at B-1, close at B-20. The SIMD-group cost model was too optimistic: the edge loop
  (up to four candidates x four field taps, each a row load) runs in the same group as the centre shading.
- The `taps` share of Shade in this window was smaller than perf-frame measured (none/taps Shade 0.38 at B-1,
  0.53 at B-20, against 0.52 / 0.31), so the B-20 coverage/taps prediction, built on perf-frame's 0.31, missed.
- Not in the prediction: the halo classify (clears HDR alpha at every pixel; shared with `taps`) costs +0.32 ms
  at B-1 and +0.19 ms at B-20 in this window, more than the coverage Shade work at B-1. perf-frame measured +0.01 ms
  for it; this window disagrees. Worth a paired re-measure in a quiet window; if it holds, a coverage present that
  tests `vis` for sky (as `none` does) would need only blended pixels cleared.
- Against legacy (perf-frame's window: legacy B-1 median 0.83 ms against native taps 0.91 ms): coverage removes
  41% of the B-1 frame and 34% of the B-20 frame relative to taps here, so B-1 lands at roughly 0.54 ms on
  perf-frame's scale, under legacy. Not measured side by side with legacy in this lane.

## Default

Left as `none`. Coverage is a visible improvement on every scene that omits `edges`, but it moves 0.08-2.5% of
pixels by up to 240 codes, so every gate scene fails the deterministic picture rule against main (no change above
one code is allowed; even blobs-1, whose 0.08-0.09% share is under the 1-in-1,000 count, changes pixels by up to 87
codes), and all 19 frozen hashes would change. Paper presents (woodblock,
coral_shrine) refuse coverage, so a coverage default would also need a named fallback for them. Flipping the default
is a re-pin: owner decision. The code site is the `None =>` arm in `crates/loft-passes/src/shade_schedule.rs`.

## Files touched outside the stated ownership

`crates/loft-emit/src/emit.rs` (`emit_present` takes the visibility), `crates/loft-passes/src/lib.rs` (dropped the
unused `stage0` import), `crates/loft-passes/src/pipeline/{lower_passes,schedule}.rs`, `stdlib/core/tiled_shade.wgsl`.

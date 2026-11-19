# Phase 5 · L3 corpus-a

Batches 1–5 claim 75 pinned example paths. The R0 entry records predict 30 drawable
sources and 45 load refusals for modules with no default scene export. For the bead
and two open lanterns, expect one bead row, fifteen paper rows per lantern, and one
ground row wherever the original uses a plane. Expect the bead's axial bore ray to
miss, its equatorial test ray to enter after 2 m, and each lantern section at its
midpoint to have the source radius to within 1e-5 m. Measure build, Naga validation,
CPU query truth and the decoding grammar before queueing.

The first build round also tests both coated sweeps (three geometrically smooth
patches plus the ground), the two point-light ports, and the two texture-compute
ports. The four light/compute probes are expected to expose the missing point
lamp and custom-resource lowering in the prebuilt snapshot.

## CPU results and GPU queue

Freeze: `b7471e85862039f30787717e40f1f869a0ca544b`. The original R0
camera names map to the `view` override. Run each row with the R0 frame
times and sizes for that camera.

| Source | Scene | `--set view=0` | `--set view=1` | Built modules | Drawn rows |
| --- | --- | --- | --- | ---: | ---: |
| `examples/bead.ts` | `corpus/l5c/bead.loft` | whole | through | 5 | 2 |
| `examples/first/lantern.ts` | `corpus/l5c/first__lantern.loft` | three | low | 5 | 16 |
| `examples/first/lantern_nothing.ts` | `corpus/l5c/first__lantern_nothing.loft` | three | low | 4 | 15 |

- All six invocations built with the prebuilt `~/git/loft/target/release/loft`.
  The build calls `naga::valid::Validator` for each emitted module.
- The three focused tests in `corpus/l5c/proof.rs` passed for both views
  using `Compiled::cpu_query` and the existing main checkout's
  `target/lab/deps` rlibs. The tests sampled all 66 drawn rows across the six
  builds. They checked bounds, distance slopes, surface normals against
  finite differences, finite material samples, and the geometry predictions
  above. The bead probes also checked the inner-wall distance of
  `0.35 / 0.6` m. These are sampled CPU results. The paired GPU comparison
  remains with the integrator.
- All nine authored files, including the quarantined source drafts, passed
  both Earley and llguidance; the same command checked the author card.
  `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-passes` and the existing
  `bun test --timeout 120000 corpus/manifest.test.ts` passed.

Use the workspace root as the working directory for the CPU tests. To relink
the test without building Loft, use the pinned Rust toolchain and the existing
matching `libloft_passes-*.rlib` and `libloft_ir-*.rlib` in `target/lab/deps`:

```sh
rustup run 1.99.1 rustc --edition=2024 --test -L dependency="$deps" \
  --extern loft_passes="$passes_rlib" --extern loft_ir="$ir_rlib" \
  corpus/l5c/proof.rs -o out/meta/p5/l5c/cpu_proof
out/meta/p5/l5c/cpu_proof
```

The bead intersects a sphere minus an infinite cylinder, including entry
through the bore's wall. Each paper row spans one smooth segment of the
legacy rib profile, with the same radius and paper colors. Cameras are
rebased onto the supported orbit's initial direction and held effectively
static with a long period. These ports use the default v1 realtime
lighting and GGX. The plane is represented by a 256 m half-width ground
proxy. The original outline and lighting, the lantern tessellation, and the
ground at the horizon still require GPU comparison.

## Quarantine

The six source drafts and one-line labels live in `corpus/quarantine/`.
`out/meta/p5/quarantine.md` repeats the diagnostics for the integrator.
Here `R0g` labels a source migration blocked by the frozen renderer.
No R0g images were compared in this lane. For the two `E307` rows the table
records the follow-on error after removing the kink from the geometry.

| R0 id | Label | First attempt | Second attempt |
| --- | --- | --- | --- |
| `examples__compute_frame` | R0g | E405, unsupported `target` | E405, checked frame kernel reaches pending lowering |
| `examples__compute_volume` | R0g | E404, unsupported `bake_tex` | Exit 101, the direct ring equation reached the CPU evaluator's unimplemented `atan2` |
| `examples__light__area_lamp` | R0g | E011, missing `PointLamp` kind | E405, custom point-light pass is outside the renderer |
| `examples__light__lantern_night` | R0g | E307, `max` in differentiated paper chart | E405 after moving band endpoints to rows and trying a custom point-light pass |
| `examples__materials__beetle` | E307 | E307, `select` in differentiated tip chart | E011 after smoothing the chart, stage-0 `atan2` in camera yaw |
| `examples__materials__lacquer` | E307 | Same differentiated tip branch | Same stage-0 camera yaw |

For the coated sweeps the next isolatable authoring change is to precompute
the two constant camera yaws. The glow and area-light ports need light
sampling, and the compute ports need resource and pass lowering to preserve
their original behavior. Reproducing the point-light probe took a local
row declaration and a custom pass. The checked-in draft preserves the
spec's `PointLamp` call.

## Coverage and handoff

| Batch | Assigned | R0 modules without a default scene | Queued | Quarantined | Unported drawable |
| ---: | ---: | ---: | ---: | ---: | ---: |
| 1 | 15 | 10 | 1 | 0 | 4 |
| 2 | 15 | 15 | 0 | 0 | 0 |
| 3 | 15 | 15 | 0 | 0 | 0 |
| 4 | 15 | 5 | 0 | 2 | 8 |
| 5 | 15 | 0 | 2 | 4 | 9 |
| Total | 75 | 45 | 3 | 6 | 21 |

The 45 R0 refusals all report a missing default scene export. These
identities still belong to their assigned batches in the frozen manifest.
The 21 remaining drawable scenes have no accepted port or two-attempt
quarantine in this lane:

| Still to port | Legacy behavior the port must preserve |
| --- | --- |
| `bosses/crab_lineup`, `bosses/maelstrom_wyrm`, `bosses/reefback_titan`, `bosses/thundermaw`, `bosses/tithe_fleet`, `bosses/wreckers_lamp` | Posed anatomy and rigs, authored actions and cameras, sea interactions and bespoke compositing or lights |
| `brig`, `coral`, `coral_list`, `fleet`, `grab`, `islandback`, `maelstrom` | Animated sea and waterline behavior with the original hull, branch, grab, island or vortex geometry |
| `compute_steps`, `creep`, `flock` | Evolving compute state with the original sample and view schedule |
| `dust_veil` | Grain billboards attenuated by the moving dust volume |
| `first/lantern_swings` | Moving sweep and rope, with conservative bounds over the full swing |
| `light/heron_lamp`, `light/lure`, `light/road` | Original animated geometry, moving or patterned point light, and visibility |

Only `corpus/l5c/`, the six paired files named in the quarantine table,
and this note are tracked lane changes. Scratch diagnostics and the
quarantine report are under the lane workspace's ignored `out/meta/p5/`.
The frozen manifest, compiler, ABI, vectors and picture references stay
at the freeze revision.

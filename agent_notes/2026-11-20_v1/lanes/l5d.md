# Phase 5 · L5d · Corpus batches 6–10

The frozen manifest assigns 75 identities to this lane: 50 examples and 25
`crab-actions-v1` seeds. Expect one disposition per identity. Predict that the
prebuilt CPU build will validate the closed-form examples whose geometry and
materials already fit the realtime renderer. The pinned source set has at least
12 legacy refusals and the prebuilt renderer has one perspective orbit camera.
For a fixed perspective view, a yaw about the target maps the eye to the
orbit's +z start without changing the picture; a period of 10⁹ s moves the
camera less than 2×10⁻⁸ radians over the four R0 times. Orthographic projection,
frame-stage transported strand tables and specialized legacy depth passes
require support beyond the one-camera recipe. R0g and speed are for the GPU
integrator.

## Results

The candidate and quarantine inventories cover every batch identity exactly once.
The original R0 source revision is `d3674dbdb918b604a5e82f9736db84780a6894a8`.
The seed input was read at creative `b7f79a82` and matched the pinned SHA-256 of `26d2a9b1e3919e183b3737a53ad3390d79a3988416b7be58014cbaaa7b10fc99`.

| Batch | CPU-built candidates | E307 | R0g |
| --- | ---: | ---: | ---: |
| 6 | 11 | 0 | 4 |
| 7 | 1 | 0 | 14 |
| 8 | 2 | 0 | 13 |
| 9 | 0 | 10 | 5 |
| 10 | 0 | 15 | 0 |
| Total | 14 | 25 | 36 |

With 61 quarantines in this lane, the maximum corpus coverage is 234 of 295 even if every other entry passes. The 95% integration gate needs 281.

## CPU proof

- Used the prebuilt main `target/release/loft`, SHA-256 `6fd1e7d533140b20911bf2cd2a837c16b4f3c4cda0c846be9db1b8c8357179b2`. All 14 candidates returned a nonempty build, passed `loft ir --sexp`, and passed `loft build` of the serialized IR through Naga’s validator again.
- `loft check` reported the rows and derived bounds on all 14 candidates. Their IR had zero authored check dispatches before running that command, so it stayed on the CPU. The build exercises the shared f32 and interval evaluators. The CLI has no pointwise `Compiled::cpu_query` command.
- The warm CPU validation sweep took 5.9–18.5 ms per scene. The first Pearls run took 278 ms while deriving 292 individual pearl bounds. Neither measure uses the training clip timing instrument.
- Shatter’s source polygon clip has 36 nonempty cells, totals 3.9 m² and needs at most seven edges per cell. All original clipped vertices satisfied the exported halfplanes to within 1e-12 m before decimal serialization. Each face of the resulting prism keeps the original 30 mm overall thickness.
- `bun test corpus/manifest.test.ts` passed 27 assertions. This lane changes no Rust crate and uses no Cargo builds, GPU commands, golden updates or tolerance changes.
- All 25 seed reproductions first received E001 for the frame-stage scan and then E307 from the load-stage alternative. Removing only the surface and testing scan staging isolated E403 on 23 of those reproductions. The other two reduced scans bake because their animated radius is evaluated outside the scan. Their full reduced surface still hits E307.

## Candidates and integration limits

| Batch | Source | Rows | Picture and check limits |
| --- | --- | ---: | --- |
| 6 | `examples/materials/moving_sun.ts` | 1 | Perspective preview of R0’s orthographic view. The sky and ceramic response follow the original color cycle. |
| 6 | `examples/materials/occlusion.ts` | 1 | Preserves the two channels and relief field. The preview uses perspective and the realtime occlusion path. |
| 6 | `examples/materials/poker.ts` | 2 | Keeps the temperature law, Planck emission and whole view. Tips and toon response differ. |
| 6 | `examples/materials/refocus.ts` | 12 | Keeps the close view and sphere arrangement. Depth of field, grain and hammered bump remain open. |
| 6 | `examples/materials/spheres.ts` | 12 | Eleven authored balls with their roughness and metal lanes. The hammered normal and close camera remain open. |
| 6 | `examples/materials/thin_film.ts` | 2 | Keeps the 380 nm film phase and the whole view. Uses a custom lobe and the realtime light path. |
| 6 | `examples/mirror.ts` | 3 | The torus and sphere retain original transforms. The mirror lobe traces the ring and floor with an opaque preview. |
| 6 | `examples/mixed_precision.ts` | 1 | The original color ramp runs analytically. The mixed-precision mode and bloom are open. |
| 6 | `examples/pearls.ts` | 293 | The original signed-cell PCG and motion are ported. The sea is opaque. Secondary views are open. |
| 6 | `examples/pool.ts` | 10 | Keeps the seven swaying posts, arch, orb and ripple phase. The offscreen mirror camera is open. |
| 6 | `examples/ranged.ts` | 5 | Keeps the sweep, ballistic path and speed range. The interval-quantified checks are open. |
| 7 | `examples/reeds_shadow.ts` | 8 | Keeps the six reed sway laws, stone, and 2,048-pixel shadow map. Tip shape and the original caster check differ. |
| 8 | `examples/shatter.ts` | 37 | One analytic convex-prism kind with 36 rows. Keeps the 0.4 s break, staggered release, fall and spin. Secondary view is open. |
| 8 | `examples/shell.ts` | 2 | Keeps the logarithmic spiral, tile pattern and swell. Tile color hash, sea composition and secondary views differ. |

The active files are under `corpus/l5d/`. Each active name is the R0 example path with `/` replaced by `__` and `.ts` by `.loft`. They are CPU candidates for the selected legacy view. R0g, GPU frame time and the training contract await the integrator.

Quarantines live under `corpus/quarantine/` with a matching one-line `.err`. Legacy entries without a viable scene preserve the original source identity and refusal. Seed files contain a reduced, source-derived strand rather than the entire seed. The per-attempt quarantine log is in `out/meta/p5/quarantine.md`. The tracked table below preserves the dispositions when the scratch tree is unavailable.

## Quarantined identities

| Batch | Source | Code | Reason |
| --- | --- | --- | --- |
| 6 | `examples/nest.ts` | `R0g` | The folded field still fails typechecking in the color conversion. Water composition also remains open. |
| 6 | `examples/open.ts` | `R0g` | R0 refused the JavaScript-filled blob and cluster tables before producing frames. |
| 6 | `examples/recipes/film.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 6 | `examples/recipes/hull.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 7 | `examples/recipes/ship.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 7 | `examples/recipes/thunderhead.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 7 | `examples/rope.ts` | `R0g` | Requires the 40-row nearest-neighbor recurrence at 240 Hz and its pre-roll and seam check. |
| 7 | `examples/scenes/island_tide.ts` | `R0g` | The island field reads a per-sample pixel footprint. The attempted port has no equivalent input and the water/subject layers are incomplete. |
| 7 | `examples/scenes/lib/fleet.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 7 | `examples/scenes/lib/wyrm.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 7 | `examples/scenes/maelstrom.ts` | `R0g` | Requires the shared deforming sea, its whirlpool chart, the ship placement and the transported strand rig. |
| 7 | `examples/scenes/night_hunt.ts` | `R0g` | Requires layered water, relief scales, depth-dependent wakes and the moving hunter. |
| 7 | `examples/scenes/stormfront.ts` | `R0g` | R0 refused the JavaScript-filled blob and cluster tables before producing frames. |
| 7 | `examples/scenes/the_strike.ts` | `R0g` | Requires contact timing, the deforming sea, telegraphs, wakes and time-sliced smears. |
| 7 | `examples/second/buoys.ts` | `R0g` | Requires the 16-count duet, the two bell solvers and the moving sea placements. |
| 7 | `examples/second/crane.ts` | `R0g` | Requires moving strand frames, its planted targets and the contact/clearance checks. |
| 7 | `examples/second/heron.ts` | `R0g` | Requires the foot stance schedule, the heron’s driven frame and the leg/neck sweeps. |
| 7 | `examples/second/heron_leap.ts` | `R0g` | Requires the solved flight and rail clearance, plus the stance and strand rig. |
| 8 | `examples/second/heron_leap_refused.ts` | `R0g` | R0 preserves a deliberate solve refusal at t=0, before frames exist. |
| 8 | `examples/second/lantern.ts` | `R0g` | Requires the driven pendulum, the moving pole/lantern and the moth path. |
| 8 | `examples/second/lanternfish.ts` | `R0g` | Requires the design’s moving strand hierarchy and its emissive bulbs in water. |
| 8 | `examples/second/lib/heron.ts` | `R0g` | The pinned entry is a library or recipe with no standalone scene and no R0 frames. |
| 8 | `examples/second/lighthouse.ts` | `R0g` | Requires the moving boat, its sea placement and the projected search-band telegraph. |
| 8 | `examples/second/owl.ts` | `R0g` | Requires articulated strand frames and the eye/eyelid occlusion checks. |
| 8 | `examples/second/pair.ts` | `R0g` | Requires the two moving bells and tendrils with water composition. |
| 8 | `examples/serpent.ts` | `R0g` | Requires the time-varying transported spine and attached head, including the orthographic view. |
| 8 | `examples/serpent_clip.ts` | `R0g` | R0 failed while loading the clip wrapper, before frames exist. |
| 8 | `examples/serpent_relief.ts` | `R0g` | Requires depth-aware relief with the moving serpent rig and sea. |
| 8 | `examples/serpent_scales.ts` | `R0g` | Requires custom screen-space scale filtering on the moving serpent rig. |
| 8 | `examples/shell_relief.ts` | `R0g` | Requires true shell-local relief traversal, with the displaced depth and two-sided inner shell. |
| 8 | `examples/skin.ts` | `R0g` | Requires the 128×128 Gray–Scott recurrence after 9,000 bake steps, sampled in a moving strand. |
| 9 | `examples/spray_burst.ts` | `R0g` | The boot field reaches Naga but fails its vector builtin validation. The grain and dust passes and second view are incomplete. |
| 9 | `examples/storm.ts` | `R0g` | R0 refused the JavaScript-filled blob and cluster tables before producing frames. |
| 9 | `examples/stormwing.ts` | `R0g` | Requires the articulated wing membranes, the tail sweep and the orthographic view. |
| 9 | `examples/strike.ts` | `R0g` | Requires the bite/lash constraints, moving ship geometry and the interaction with water. |
| 9 | `examples/volume_dust.ts` | `R0g` | Requires the joined density, transmittance and shadow marches and the design variants. |
| 9 | `crab-actions-v1/v1/train-0/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-0/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-1/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-1/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-1/2` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-1/3` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-10/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-10/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-10/2` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 9 | `crab-actions-v1/v1/train-11/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-11/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-11/2` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-12/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-12/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-12/2` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-12/3` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-13/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-13/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-13/2` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-14/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-14/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-14/2` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-14/3` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-15/0` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |
| 10 | `crab-actions-v1/v1/train-15/1` | `E307` | The runtime scan spelling is refused. The baked-scan attempt then hits a C0 clamp in the strand surface derivative. |

# Whole-pass controls

Run from the Loft checkout root with an available timestamp-capable adapter.

```sh
tools/check.sh parity --validate
tools/check.sh parity --out out/meta/s1_controls
tools/check.sh parity --only hermit_shade --mutant --out out/meta/s1_controls
```

The mutant command is expected to exit 7 with a measured slowdown. Its Loft
arm repeats every Shade dispatch before Present. Shrine's mutant repeats the
entire primary Vis pass, including its clears. `--reverse` starts with
B/A/A/B.

| Fixture | Scene and replay | Candidate pass |
| --- | --- | --- |
| `hermit_shade` | Hermit, count 20, 1280×800, 300 camera frames at 60 Hz | Five 8×8 classified tile dispatches. Read Vis64, shadow PCF, row fields, scan and GGX LUTs. Rebuild the five surfaces and shade into RGBA16Float. |
| `shrine_vis` | Coral Shrine, distance 6.4, solid 0, 1280×800, 100 frames at t=0 | Three indirect raster draws. Evaluate the floor slab, the capped cone, and the lantern's 192-step march with the row's Lipschitz bound. Write Vis64 and depth. |

A and B share one device and queue and are resident throughout timing. Each
block begins with the native triangle control and 30 warmups. The timestamp
ring reports the selected pass's begin-to-end span and the earliest-to-latest
frame span. A triangle median over 0.35 ms invalidates the run. The bootstrap
makes 4,096 circular resamples of 10 adjacent replay indices, shared across
both repetitions of both arms. The gate holds when output parity holds and the
measurement is uncontended with an upper 95% bound of Loft/hand pass p95 at
most 1.05. An uncontended lower bound above 1.05 exits 7. A crossing interval
or contended run with matching output triggers one reversed-order measurement.
A persistent crossing is inconclusive and exits 6.

Parity uses the compiler's indexed cull on both arms to fix the order at
equal-depth ties. Timed frames use the original compacting cull. Shrine's
hand arm aliases shared caster kinds to keep their generated shadow shaders
and the same sun frustum. The aliases add two bounds records to its cull
and are charged in the whole-frame timing. Hermit parity samples the first,
middle, and last orbit frame. Shrine's timed camera is constant.

The diagnostic Present writes exact ID, payload, and HDR half words through
an RGBA8 linear attachment. The depth-only replay writes the fragment depth
to the auxiliary word, or the interpolated raster depth for the shell.
Examples, SHA-256 and exact difference counts are reported per attachment.
Float ULP distance is measured in its stored format. ID, visibility payload,
D32, and RGBA8 are exact. HDR is exact for Shrine. Hermit HDR allows one half
ULP. The FMA check scans the emitted WGSL and a Naga MSL translation for
forbidden directives. The Metal run also
executes a buffer-fed contraction witness through wgpu's default Naga route.

The fixture table in `crates/loft-cli/src/controls.rs` pins the paths, replay,
entry names, and row strides. The hand sources implement the authored
geometry, materials, light, and binding ABI directly.

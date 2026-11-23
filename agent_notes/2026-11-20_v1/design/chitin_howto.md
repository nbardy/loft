# Retexture a div1 crab with `stdlib::materials::chitin` (HOWTO for Haiku lanes)

The material is `stdlib/materials/chitin.loft`, extracted from the lead-matched swimmer
(match1 `final.loft`). You choose parameters. Do not write new material code. The worked
example is `out/meta/chitin/ghost_chitin.loft` (the ghost crab `pebble-walker`), and the
template is the diff between it and `ghost_original.loft`.

## Edits to a div1 seed (all 16 share the same `Plate`, `Limb` and `crab_body` text)
1. **Imports.** Change `use stdlib::materials::bsdf::{ggx, coat, emit_lobe}` to
   `use stdlib::materials::bsdf::{ggx}` and add
   `use stdlib::materials::chitin::{carapace, limb, wet, granules}`.
   *Woodblock seeds* (spider, decorator, boxer, fiddler): replace the first four `use` lines
   (woodblock, emitters, paper_grain, sky) with `use stdlib::integrators::direct::{sun_sky}` and
   `use stdlib::sky::environment::{sky_color}`. In `schedule`, set `light: sun_sky` and delete
   `outline_and_paper(ColorHdr)`.
2. **Plate material.** Replace the whole `material(p, n) { ... }` block of `kind Plate` with
   `  material(p, n) = wet(pigment, 0.5 * Gloss.x, 0.714 * Gloss.y, granules((p - joint(at, motion)).xyz, n.xyz, Grain.x, 0.18 * Grain.y))`
3. **Shell kind.** Directly after `kind Plate`, paste a copy of `kind Plate` with its first
   line changed to `kind Shell @World (at: p3@World, axes: rgb, motion: v3) {`, the same
   `trace` and `visible`, and this material:
   `  material(p, n) = carapace((p - joint(at, motion)).xyz, axes, n.xyz, Ground, Groove, Mark, Pattern, Gloss, Grain)`
4. **Limb material.** Replace `material(p, n) = ggx(pigment, 0.46, 0.04)` in `kind Limb` with
   ```
     material(p, n) {
       let start = joint(a, motion)
       limb((p - start).xyz, (joint(b, motion) - start).xyz, n.xyz, pigment, Mark, Banding, f32(tip / root), Tip, Gloss, Grain)
     }
   ```
5. **Carapace emit.** In `crab_body`, change
   `emit Plate(at: at, axes: axes, pigment: paint, finish: finish, motion: motion)` to
   `emit Shell(at: at, axes: axes, motion: motion)`.
6. **Preset.** After `const Finish = ...`, paste the ten `const` lines of your species from the
   table below.
7. **Colours at the emit sites.** In the top-level `build { }`, replace the seed's paint literal
   (the 6th argument of `crab_body` and the 9th of each `chela`) with `Leg`, and the chela accent
   literal (its last argument) with `Claw`. Use replace-all on the exact literal.
8. **Extra plates.** An `emit Plate` in `build { }` that uses the paint literal and sits on the
   body (a brow, extra carapace lobe or abdomen) becomes `emit Shell(at: ..., axes: ..., motion: Motion)`.
   Any other Plate (sponge, kelp, debris, **hermit shell**) keeps its own pigment; for a hermit,
   set its pigment to the preset's Home colour (see below).
9. **Schedule.** Add `Shell` first in `cull_indirect`, `draw_indirect` and `find_shadow_map`.

## Check and render
From `~/git/loft` (the material is on main):
```
L=target/release/loft; F=<your>.loft
$L lint --strict $F && tools/gpu_lock.sh $L check $F && uv run --script tools/check_loft_grammar.py $F
tools/gpu_lock.sh $L clip $F --out out/meta/chitin/<id> --size 512x512 --frames 1 --fps 1 --start 0 --aov beauty --max-build-ms 2000 --max-frame-ms 50
```
This is the seed-camera render of the match1 loop. To use the loop itself:
copy the file to `~/git/_wt/creative-match1/training/loft/match1/rounds/r<NN>/<id>.loft` with
the NN the lead assigns you, then run, in that worktree,
`uv run --no-project --with pillow python training/loft/match1/loop.py <NN> <id>`. It runs lint,
check and both grammars, renders the seed camera (`seed-*/frame_000.png`) and a top-down studio
view, and writes `out/meta/gallery/match1/round-<NN>.webp`. Ignore its swimmer-photo column and
pale/median numbers, which are for the swimmer. Do not write in `match1/negatives`.
Never wrap a render in `agent-build`; renders take only the GPU lock.

## Parameters
| const | meaning |
|---|---|
| `Ground`, `Groove` | carapace ground colour; the colour it takes in the H-groove and sutures |
| `Mark` | colour of the carapace marks and of limb mottle and bands |
| `Pattern` | `[kind, cover, scale]`: kind 0 none, 1 spots, 2 net (spots and vermiculated lines), 3 transverse bands or ridges; cover 0–1; scale in carapace half-widths (0.04–0.12) |
| `Gloss` | `[roughness, coat]`: between-granule roughness and the weight of the wet film. Glossy: 0.3–0.5 and 0.5–0.8. Matte: 0.6–0.85 and 0–0.15 |
| `Grain` | `[pitch m, strength]`: granule size in metres; strength 1 is the swimmer's sparkle, 0 is smooth |
| `Leg`, `Claw` | limb colour (per-segment shading comes from the seed), claw-finger colour |
| `Tip` | the colour of dactyl, claw-finger and spine tips; every joint also darkens |
| `Banding` | `[mottle cover, band strength]` on the limbs, in the Mark colour |

To layer a body-specific albedo on a limb (the swimmer's setae fringe), call `limb_albedo` and
`limb_tilt` and pass them to `wet`, as `swimmer_library.loft` does. A material cannot
`mix_bsdf` a helper's lobes (E405).

Colours are linear RGB albedo. Pale shell is about 0.5, mid 0.15–0.3 and dark 0.02–0.06.

## Presets (the 16 div1 species)
Order: Ground, Groove, Mark, Pattern, Gloss, Grain, Leg, Claw, Tip, Banding.

| species (seed) | values |
|---|---|
| spider (copper-engraving) | [0.30,0.09,0.03] [0.12,0.035,0.015] [0.55,0.45,0.35] [1,0.25,0.06] [0.6,0.25] [0.03,1.0] [0.30,0.10,0.035] [0.30,0.10,0.035] [0.08,0.03,0.015] [0.15,0.35] |
| fiddler (spring-fiddler) | [0.05,0.05,0.045] [0.02,0.02,0.02] [0.35,0.38,0.40] [2,0.35,0.06] [0.5,0.4] [0.012,0.5] [0.08,0.06,0.05] [0.65,0.30,0.08] [0.70,0.62,0.50] [0.0,0.2] |
| boxer (paper-cutout) | [0.45,0.30,0.22] [0.20,0.10,0.08] [0.12,0.05,0.06] [2,0.6,0.08] [0.35,0.6] [0.01,0.3] [0.42,0.30,0.25] [0.42,0.30,0.25] [0.15,0.06,0.08] [0.0,0.7] |
| decorator (coral-regent) | [0.18,0.11,0.06] [0.07,0.045,0.03] [0.10,0.16,0.07] [1,0.35,0.12] [0.8,0.05] [0.025,1.0] [0.20,0.12,0.06] [0.20,0.12,0.06] [0.06,0.04,0.03] [0.3,0.2] |
| coconut (black-glass) | [0.10,0.05,0.06] [0.35,0.12,0.04] [0.45,0.40,0.42] [3,0.15,0.09] [0.45,0.45] [0.02,0.6] [0.10,0.12,0.25] [0.12,0.10,0.20] [0.03,0.02,0.03] [0.2,0.0] |
| porcelain (timid-porcelain) | [0.65,0.60,0.55] [0.45,0.40,0.38] [0.40,0.04,0.04] [1,0.55,0.10] [0.25,0.8] [0.01,0.1] [0.62,0.55,0.50] [0.65,0.60,0.55] [0.55,0.10,0.08] [0.4,0.0] |
| sponge (bubble-scatter) | [0.20,0.12,0.08] [0.09,0.05,0.035] [0.30,0.20,0.14] [0,0,0.1] [0.85,0.0] [0.008,1.0] [0.20,0.12,0.08] [0.55,0.35,0.30] [0.60,0.40,0.35] [0.0,0.0] |
| moon hermit (amber-tidepool) | [0.40,0.12,0.05] [0.18,0.05,0.02] [0.62,0.55,0.45] [1,0.3,0.08] [0.4,0.5] [0.015,0.7] [0.42,0.13,0.05] [0.45,0.15,0.06] [0.06,0.03,0.02] [0.3,0.4]; Home [0.55,0.45,0.33] |
| pea (exhausted-glow) | [0.55,0.42,0.33] [0.42,0.30,0.24] [0.40,0.22,0.15] [0,0,0.1] [0.3,0.6] [0.01,0.0] [0.55,0.44,0.36] [0.55,0.44,0.36] [0.45,0.30,0.22] [0.0,0.0] |
| king (ferocious-signal) | [0.30,0.06,0.04] [0.12,0.025,0.02] [0.50,0.18,0.08] [1,0.25,0.09] [0.5,0.35] [0.04,1.0] [0.30,0.07,0.04] [0.30,0.07,0.04] [0.08,0.03,0.02] [0.2,0.3] |
| swimmer (blue-reef) | [0.0072,0.0080,0.0042] [0.0072,0.0080,0.0042] [0.27,0.27,0.16] [2,0.45,0.07] [0.68,0.7] [0.02,1.0] [0.026,0.024,0.060] [0.030,0.020,0.045] [0.010,0.007,0.016] [0.10,0.0] |
| tower hermit (sand-scatter) | [0.25,0.16,0.08] [0.10,0.06,0.03] [0.55,0.45,0.30] [3,0.25,0.12] [0.45,0.4] [0.012,0.6] [0.06,0.10,0.22] [0.08,0.12,0.25] [0.50,0.25,0.06] [0.0,0.5]; Home [0.50,0.40,0.28] |
| whelk hermit (shimmer-patina) | [0.42,0.14,0.06] [0.20,0.06,0.03] [0.60,0.45,0.28] [0,0,0.1] [0.5,0.35] [0.02,1.0] [0.40,0.15,0.06] [0.45,0.18,0.08] [0.15,0.06,0.03] [0.15,0.35]; Home [0.45,0.42,0.36] |
| ghost (pebble-walker) | [0.34,0.28,0.15] [0.12,0.095,0.055] [0.17,0.135,0.075] [1,0.30,0.05] [0.62,0.12] [0.015,0.8] [0.40,0.33,0.19] [0.58,0.53,0.42] [0.09,0.07,0.045] [0.0,0.25] |
| box (windup-snap) | [0.50,0.38,0.24] [0.30,0.20,0.12] [0.30,0.08,0.05] [1,0.5,0.06] [0.35,0.6] [0.03,0.9] [0.50,0.38,0.26] [0.52,0.40,0.26] [0.25,0.08,0.05] [0.4,0.0] |
| arrow (jaunty-sketch) | [0.55,0.40,0.20] [0.30,0.18,0.08] [0.10,0.04,0.03] [2,0.2,0.05] [0.35,0.6] [0.01,0.2] [0.55,0.18,0.06] [0.40,0.12,0.25] [0.35,0.10,0.35] [0.0,0.45] |

The swimmer values are the lead-matched ones. They are very dark because the match was made
under the studio light. The other presets come from field knowledge of each animal; the ghost is
also checked against refs/crab/03, 04, 11 and 12. Adjust them by eye against a photo.
Known gaps: the arrow crab's real lines run along the body, but kind 3 bands run across it,
so the preset uses a sparse net. Decorator algae and the sponge crab's sponge are separate
Plates in their own pigment, not chitin.

**Hermit crabs:** the snail shell is not chitin. It is a separate Plate (moon, tower and whelk
seeds) and keeps the plain `wet` Plate material in its own pigment: set that emit's pigment to
the `Home` colour. It also inherits the crab's `Gloss` and `Grain`, which is acceptable for now.
A shell family (growth lines, spiral bands, chalky or porcelain gloss) is a separate library
material still to be extracted. Do not paint the shell through `carapace`.

# The `pbr` style (2026-11-18)

Built from `docs/proposals/2026-11-18_next.md` section 4 and its 7b contract, branch `loft/pbr` (from main 9d3e38f).

## Predictions (written before building)

Conventions the numbers rest on. A light's `amount` 1 is the sun at noon: an irradiance under which a white Lambert
surface facing it has radiance 1 (so every BRDF term carries a factor pi). Paint `base` is sRGB as written and is
linearised by x^2.2; `emits` is linear. Measured on the JS twin unless "GPU" is said, with tone set to the identity
(`tone: (c) => c`) so a number is radiance.

1. **White furnace, environment.** A white (base 1), rough, non-metal sphere under a uniform sky of radiance 0.5 with
   no sun (`amount` 0): every pixel is 0.5. The environment's diffuse is weighted by 1 - DFG and its specular by DFG,
   so the sum is 0.5 for any roughness; the spherical-harmonic projection of a constant is its L00 term alone (the
   other eight are quadrature noise, O(1/N) with N = 1024 Fibonacci directions). Twin: 0.5 to 1e-3 at rough 0.2, 0.5
   and 1. GPU: mean of the sphere's pixels 127.5/255 +- 1, no pixel off by more than 2 levels. No energy gain: nothing
   above 0.5 + 1e-3 on the twin.
2. **The lobe itself, without the split sum.** The specular lobe's directional albedo E(mu) = integral of
   pi D Vis n.l over the hemisphere / pi with F = 1 (numerical, 2D quadrature on the twin's parts): never above 1 (+1e-3
   quadrature). E(1) in [0.97, 1.0] at rough 0.25; in [0.70, 0.88] at rough 1; E(0.1) in [0.30, 0.60] at rough 1
   (single scattering loses energy at high roughness; no multiple-scattering term is added). The analytic DFG (Karis)
   A + B within 0.06 of the numerical E at every sample.
3. **Roughness sweep.** Light and eye together on z, the normal swept off z by theta. The highlight's full width at
   half maximum in theta, from GGX alone, is 2 atan(0.644 alpha), alpha = rough^2: 0.74, 6.6, 18.2, 35, 55 degrees at
   rough 0.1, 0.3, 0.5, 0.7, 0.9. Measured on the whole specular term (D Vis F n.l), within 15% of that at rough <= 0.5
   and narrower by up to 30% at 0.9 (Vis and n.l fall off the peak too). Width strictly increasing and peak strictly
   decreasing with roughness.
4. **Metal tints specular by base.** Copper, base (0.95, 0.64, 0.54) sRGB, linear (0.893, 0.375, 0.258). At the
   highlight's peak (v.h = 1, F = f0 exactly) the metal's specular has G/R = 0.420 and B/R = 0.289 and its diffuse is 0;
   the same base at metal 0 has a white highlight (ratios 1, 1) of strength 0.04 of the metal's red. Toward grazing
   (v.h -> 0) the metal's highlight goes white (ratios -> 1).
5. **Twin against GPU.** Every part and the assembled body term over a few hundred random (normal, view, light, base,
   rough, metal) samples: worst relative disagreement under 1e-4 (f32 against f64), under 1e-3 for the tone map's pow.
6. **Cost.** A few hundred operations a body fragment (two lights: about 2 x 45 for the lights, 60 for the projected
   sky, 30 for one sky tap, 25 for the tone) against toon's 3 antialiased bands, rim and glint. At 512 x 512 the bodies
   are about a third of the pixels: pbr - toon in [-0.2, +0.5] ms a frame on the crab and on the lamp scene, most
   likely unresolved (the GPU clock ticks in 0.065 ms and the machine is shared).

Environment: projected once per scene, not integrated per pixel. Irradiance from nine SH coefficients is about 30
multiply-adds a pixel and is the cosine convolution to L2 (Ramamoorthi and Hanrahan 2001: under 3% for smooth
lighting); integrating a sky per pixel to the same accuracy takes 16 to 32 sky evaluations (each 20 to 30 ops). The
projection is 1024 evaluations of the sky on the CPU at compile, cached per (sky, sun). The cost of that choice: a sky
that moves with time or with a moved sun lights bodies as it was at the stated sun (the clouds' CPU tables have the
same limit), though the sharp part of a reflection (one tap of the sky per pixel, below) follows it.

## As built

- `src/style/pbr_parts.ts`: the parts `ggx`, `smithGgx`, `schlick`, `lambert`, `skyLight` (environment), `daylight`
  (sky), `aces` (tone), and the type `PbrParts`. `src/style/pbr.ts`: `pbr(parts?)`, registered as `--style pbr`.
- What an author writes: `pbr()`, `pbr({ fresnel: thinFilm(380) })`, `{ ...pbr(), ink }`; paint words `rough`,
  `metal`, `emits`, `normal` (and `gloss` read as the default of `rough`: 1 - gloss, so toon-era glossy scenes stay
  glossy; 0.5 when neither is given). `gradient` (src/grad.ts) is now exported so a paint can write a bump:
  `normal: normalize(n - (g - n (n.g)))`, g the gradient of a height in the world (`examples/materials/spheres.ts`).
- The environment's specular weight: the theory said Karis's analytic split sum would do; it does not for these parts
  (see 2). Replaced by a fit to a numerical integration of exactly `ggx` and `smithGgx` (9 + 4 coefficients). A Fresnel
  law of any shape enters it through one cosine, c with (1 - c)^5 = B / (A + B): exact for Schlick, and what lets a
  swapped `fresnel` (a thin film) colour the sky's reflection as well as the lights'.
- New beside the inherited pieces: the sky projected once per scene as an exact closed form (no cube map); the
  reflection as the projection blurred band by band plus the residual sky tapped once and faded as band 3
  (S(r) = sum a_l L_l Y_l(r) + a_3 (sky(r) - L2 sky(r)), a_l = exp(-l(l+1) alpha^2)): sky(r) exactly at rough 0, the
  blurred projection at rough 1; the Fresnel-shape trick above.
- Where it differs from the proposal's sketch of `PbrParts`: `diffuse` takes `(albedo, n_l, n_v, v_h, rough)`, not the
  shade context (the albedo is the linearised, metal-weighted paint; a part reading `ctx.base` would light the sRGB
  value and the metal's colour); `environment` takes `({ normal, view, sun }, rough, sky)`, so the sea and the ground
  call it too and a bumped normal reaches it; and a seventh slot, `sky`, is the one closed-form sky both the background
  and the environment read (the furnace test swaps it for a uniform 0.5).
- Twin against GPU: `tests/style.pbr.gpu.test.ts` (256 random samples of every part, the environment, the whole body
  term and ink, worst disagreement under 1e-3 with |gpu - cpu| / max(1, |cpu|); the white furnace on both, under 1e-4).
- Lamp shadows come through the light fold unchanged (the heron's lamp shadow below).
- Not added: anything for the sky's sun disc (the sun reaches bodies as a light; a disc in the environment would count
  it twice).

## Measured (`bun tools/pbr_sanity.ts`)

| | Predicted | Measured |
|---|---|---|
| 1. Furnace, rough 0.2 / 0.5 / 1 | 0.5 to 1e-3, no gain; GPU 127.5 +- 1 of 255 | twin 0.50000 in [0.49997, 0.50003] at all three; GPU the same; 8-bit mean 127.51, every pixel 127 or 128. **Held** |
| 2. Lobe albedo E(1) rough 0.25 | [0.97, 1.0] | 0.996. **Held** |
| E(1) rough 1 | [0.70, 0.88] | **0.307. Missed**: the prediction was from memory. Exact by hand at alpha = 1 (D = 1/pi, Vis = 0.5 / (n.l + n.v)): 2 int_0^{pi/4} cos 2t tan t dt = 1 - ln 2 = 0.307. A second quadrature in l-space agrees to 4 digits |
| E(0.1) rough 1 | [0.30, 0.60] | 0.760. **Missed** (grazing keeps more than face on at alpha = 1) |
| Never above 1 | yes | yes (largest 0.996; the grid's rough 0.05 corner reads 1.02, quadrature on a lobe 0.1 degree wide) |
| Karis A + B within 0.06 | yes | **Missed**: off by up to 0.47 (0.86 against 1.00 at rough 0.25; 0.45 against 0.31 at rough 1). Karis's fit is for another visibility and depends on roughness alone. Replaced by a fit to these parts: within 0.06 of f0 A + B at f0 = 1, 0.04 at f0 = 0.04, mean 0.01 (E(1) at rough 0.25 / 0.5 / 0.75 / 1: 1.000, 0.917, 0.650, 0.314 against 0.996, 0.916, 0.627, 0.307) |
| 3. FWHM rough 0.1 / 0.3 / 0.5 | 0.74 / 6.6 / 18.2 deg within 15% | 0.74 / 6.68 / 19.36 (ratios 1.003, 1.007, 1.058). **Held** |
| rough 0.7 / 0.9 | 35 / 55 deg, narrower by up to 30% | 44.6 (ratio 1.27, **wider**, not narrower) / **no half maximum**: in this geometry the term tends to alpha^3 of its peak at grazing (D falls to alpha^4, Vis n.l rises by 1/alpha), 0.53 at rough 0.9. The direction of the correction was wrong: Vis n.l rises off the peak |
| Monotone | width up, peak down | width 0.74, 6.68, 19.4, 44.6, (no half) deg; peak 100, 1.24, 0.160, 0.042, 0.015. **Held** |
| 4. Copper G/R, B/R at v.h = 1 | 0.420, 0.289 | 0.419, 0.289. **Held** |
| Dielectric highlight | white, 0.0448 of the metal's red | (1.235, 1.235, 1.235), 0.045. **Held** |
| Toward grazing | ratios toward 1 | at v.h = 0.17: G/R 0.66, B/R 0.58. **Held** |
| 5. Twin against GPU | under 1e-4 (1e-3 through pow) | under 1e-3 for the whole set (the test's bound); the furnace under 1e-4 |

## Cost (`loft profile`, 512 x 512, least of 30 frames, six interleaved rounds pbr then toon)

Load average 6.2 to 8.1 throughout (shared machine). The GPU switches clock state between runs: the crab's first round
(pbr 1.11, toon 2.10) and toon's sixth (0.92) are in another state and are left out of its median.

| Scene (view) | toon ms | pbr ms | pbr - toon |
|---|---|---|---|
| crab (`game`, toon by `--style`) | 2.36 to 2.56 | 3.01 to 3.08 | **+0.52** (median of rounds 2 to 5; two more paired runs: 2.49 against 3.01). Missed the [-0.2, +0.5] band by a hair. Per draw: the full-screen sky +0.18 (0.09 to 0.28: `daylight`'s pow and the tone map on every pixel), the sea +0.05 to +0.12, the atoll +0.1; the titan's body no more than toon's |
| heron under its lamp (`close`, its own toon) | 0.79 to 0.85 | 0.92 | +0.07 to +0.13 |
| spheres (`whole`) | 1.05 to 1.11 | 1.18 to 1.25 | +0.07 to +0.20 |
| lantern at night (`three`) | 0.39 | 0.52 | +0.13 |

## Pictures (WebP q95, 512 x 512 a view)

`crab_{pbr,toon}.webp` (the crab, `game`); `heron_lamp_{pbr,toon}.webp` (a lamp with its own shadow); `lantern_{pbr,toon}.webp`
(emits and a lamp under a moon); `spheres_{pbr,toon}.webp` (roughness 0.1 to 0.9, copper metal in front, dielectric
behind, hammered copper with a bump at the right); `lacquer_{pbr,toon}.webp` (toon's `gloss` read as roughness);
`thin_film_pbr.webp` (one part swapped) beside `beetle_toon.webp` (the same shell by `scatter` in toon).

## Undone, and limits found

- No occlusion: a crevice sees open sky.
- The specular weight is fitted to the default lobe; a replaced `distribution` or `visibility` keeps it.
- Specular aliasing: a sharp highlight on a small curved body sparkles (no roughness from the pixel's footprint).
- `--style toon` on a scene whose own style is toon with another outline fails (`param "toon.outline" declared twice`):
  found here, not caused here, not fixed.
- `bun tools/style_imports.ts` reports `calmSea` (papercraft) on main already; pbr's files pass.

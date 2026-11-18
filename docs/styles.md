# Styles: parts, styles, looks

A scene says what things are; a **style** says how they look. Swapping one never touches the scene
(`withStyle(scene, sketch())`, `--style sketch`). Styles come in three layers, each built only from the one below:

- **parts**: functions a style is composed from, each importable on its own from the public entry point;
- **styles**: named ones (`toon`, `woodblock`, `biolume`, `papercraft`, `sketch`, `neon`, `pbr`), each a short file that
  composes parts and makes its own few decisions;
- **looks**: named presets of a style, chosen by one word (`--light engraving`).

The rule that keeps this honest: a built-in style uses nothing a user cannot import. `bun tools/style_imports.ts`
proves it (no file in `src/style/` imports another style's file, and every name a style imports is exported by
`src/index.ts`).

## What a style is

One value of type `Style` (`src/style.ts`), written in the same expression language as the shapes. The compiler shares
surface shading functions across shaders and emits volume shading with each field.

| Member | What it returns | Called for |
|---|---|---|
| `shade(ctx)` | a body's colour | every opaque fragment (`ShadeCtx`: paint, normal, sun, view, world point, depth below the sea, the surface's coordinates, anchor and identity; `lights(combine, term)`, the fold over every light; `channel(name, otherwise)`, a word the thing's paint says; `scatter(toward, otherwise)`, the thing's own answer to one light; `sky(dir)`, the style's sky in any direction) |
| `volume(ctx)` | source colour per unit extinction | nonempty `draw.volume` ray samples (`VolumeCtx`: world, time, view, extinction, albedo, emission and the light fold) |
| `ink(ctx)` | the outline hull's colour | the hull, `outline` world units wide, never under 1.5 px |
| `mark(ctx)` | a number written to the picture's alpha | read back by the style's post passes; 1 for none |
| `water(ctx)` | the sea's colour and alpha | the sea's fragments (`WaterCtx`: normal, crest, the swell's phase, how near a body is) |
| `shadow(ctx)` | colour and alpha over the water | a cast shadow on the sea |
| `sky(ctx)` | the background | a view direction |
| `post` | full-screen WGSL passes, in order | after the scene |
| `block`, `lights` | the shadow printed after the bodies; other lightings by name | optional |

A style that draws no water still returns something from `water`; a scene without a sea never calls it.

**Lights.** `ShadeCtx`, `GroundCtx` and `WaterCtx` fold every light at their point (`ctx.lights("sum" | "largest",
(lit, which) => term)`); `SkyCtx` holds the sun alone. A style never sees a list. With no lights the fold is the sun's
term itself, so a style written this way is unchanged on a scene without them (`docs/lighting.md`, "Lights").

`VolumeCtx.lights` presents arrivals after weighting by the scattering phase. `lit.unshadowed` carries that colour,
`lit.visibility` combines the volume's transmittance with any lamp depth-map visibility, and `lit.amount` is their
product. Omit `volume` for single scattering (`albedo * sum(amount) + emission`). Toon bands the visibility tint
and woodblock bands the luminance in its inks. A custom look can use the same terms and leave extinction to Loft.

**Words a paint says.** A paint may return `{ base, gloss: 0.8, ... }` (`Painted`, `src/map/types.ts`). A style reads
a word with `ctx.channel(name, otherwise)`; the words it reads are recorded on its one evaluation (`styleReads`), and a
thing that gives one gets its own copy of the style's functions in its own module (`src/draw/painted.ts`), so a scene
whose paints say nothing more than a colour keeps every shader's text. The three identities that make a default free
(`mul` by a constant 0, `add` of a constant 0, `mix` at a constant 0 or 1) hold while a style's body functions are built
(`withIdentities`): applied library-wide they moved geometry's rounding. A word no part of the style reads prints
`note: <thing> gives "<word>", which no part of <style> reads`.

| Style | `gloss` (f32, default 0) | `metal` (f32, default 0) | `emits` (vec3, linear) | `scatter` |
|---|---|---|---|---|
| toon | `glossed` over its colour; glint 0.16 + 0.5 gloss | tints the mirror by the paint | added after the bands, e / (1 + lum e) | in place of the paint in each light's term |
| woodblock | a flat cream shape, 6 degrees about the half vector, 0.9 x gloss | not read | not read | not read |
| biolume | not read | not read | **is** the glow (default: the guess `glowOf`) | each local light |
| neon | not read | not read | **is** the tube's light in `shade` and `ink` (default `tubeOf`) | each local light |
| pbr | `rough` defaults to 1 - gloss | 0 to 1: specular takes the paint's colour, diffuse goes | added before the tone map | in place of the diffuse term for each light |
| papercraft, sketch | not read | not read | not read | not read |

`pbr` alone reads two more words: `rough` (f32, 0 mirror to 1 chalk; default 1 - `gloss`, 0.5 when neither is given)
and `normal` (vec3, the shading normal for a bump; default the surface's own). A bump is the gradient of a height in the
world less its part along the normal, written in the paint with `gradient`:

```ts
paint: (u, v, at) => {
  const g = gradient(mul(noise3(mul(at.world, 12)), 0.01), at.world);   // dents 0.08 across, 1 cm deep
  return { base: COPPER, metal: 1, rough: 0.3, normal: normalize(sub(at.normal, sub(g, mul(at.normal, dot(at.normal, g))))) };
}
```

`glossed(colour, ctx, { gloss, metal?, seen? })` (`src/style/gloss.ts`) is the part: the sky mirrored in a body, four
taps on a cone of half-angle 0.02 + 0.5 max(0.02, (1 - gloss)^2), Fresnel-weighted (Schlick); `seen` replaces the sky.

## The parts

| Part | One line |
|---|---|
| `light.recipe`, `LIGHT`, `lit` | ten shading treatments, each on or off (core shadow, hatching, rim, highlight, bokashi, cast shadows, waterline, overlap); each gives a tone and the darkest wins (`docs/lighting.md`) |
| `litBodies(recipe, block, ink, after?)` | a recipe applied to any colour: returns a style's `shade`, `mark` and `block` |
| `skyOf(sky, ctx)` | a horizon ink with washes wiped over it by view height (abutting washes are a gradient's stops, overlapping ones a print's bokashi), a sun disc, cloud bands, an ink below the horizon |
| `swellLines(ctx, { perWave, width })` | lines along the swell's crests, heavier at crests, absent in troughs, turned into their tone where they crowd |
| `contactFoam(pattern, ctx, lines)`, `marksFoam(print, ctx)` | foam where a body meets the water (`foam.ridge/rings/apron/stipple/breakers/curls/roll`) and where the sea's deformations mark it; both return coverage per ink |
| `printLayers(under, layers, { fill, line, edge })` | prints that coverage in a style's inks |
| `rollingFoam`, `curledHeads`, `crown` | white water from any mass: hooks, lobes, droplets drawn as curves in the world |
| `warmed`, `mottled`, `weighted`, `paint.*` | repaint before lighting (hotter reds, blotches that ride the body); a line inside the hull whose weight follows the light (`swelling`) or wanders (`wandering`) |
| `paperGrain`, `bloom`, `reflection` | post passes with their numbers as options |
| `edgeOf`, `pixelsInside`, `rgb`, `POST_COMMON` | an edge antialiased by a pixel; pixels in from the silhouette; WGSL helpers for post passes |
| `noise2`, `noise3`, `hash2`, `hash22`, `cellular2`, `luminance`, `fwidth`, `pixelWorld`, `wgslFn` | what patterns are made of |

Each part's file opens with its law, its parameters, what it cannot do, and where to go past it.

## `pbr`: a style from swappable parts

`pbr()` (`src/style/pbr.ts`) lights bodies as physical surfaces: for the sun and every light (lamp shadows included,
as the light fold delivers them) a GGX specular lobe and a Lambert body, the sky as an environment, then a filmic tone
map. `--style pbr` draws any scene this way without editing it. It is built from nine slots: eight parts, each a plain function over
expressions (`src/style/pbr_parts.ts`), so each runs on the GPU and in the CPU twin and ports to another engine as
the maths it is, and a clear colour:

| Slot (`PbrParts`) | Signature | Default | What it is |
|---|---|---|---|
| `distribution` | `(n_h, rough) => F` | `ggx` | Trowbridge-Reitz microfacet normals, alpha = rough^2 |
| `visibility` | `(n_l, n_v, rough) => F` | `smithGgx` | Smith shadowing, height-correlated, over 4 n.l n.v |
| `fresnel` | `(v_h, f0) => P3` | `schlick` | the share reflected, per colour; f0 is 0.04 or the metal's colour |
| `diffuse` | `(albedo, n_l, n_v, v_h, rough) => P3` | `lambert` | the body's light per unit arrival, cosine included |
| `environment` | `({ normal, view, sun }, rough, sky) => { diffuse, specular }` | `skyLight` | the sky in a body: what a white diffuse surface reflects, what a mirror of `rough` sees |
| `sky` | `(dir, sun) => P3` | `daylight` | linear radiance of the sky: the background and the environment |
| `tone` | `(c) => P3` | `aces` | linear radiance to the picture (Narkowicz's ACES fit, then x^(1/2.2)) |
| `lobeAlbedo` | `(n_v, rough) => { scale, bias }` | integrated from the `distribution` and `visibility` given | the environment's specular weight: under Schlick a uniform sky reflects f0 scale + bias |
| `clear` | `[r, g, b]`, display values | the sky on the horizon, toned | the colour where nothing is drawn |

Conventions: cosines arrive clamped to [0, 1]; `rough` is the paint's (a part squares it); a light's amount 1 is the
sun at noon, under which a white Lambert surface has radiance 1, so a term is BRDF x pi x n.l; colours are linear
until `tone`. The sky is radiance per unit of the sun's amount: the background, the haze and the environment are
`sky` times the sun's amount, so a moonlit sky is dark and a sun of 0 leaves any sky black (tested in
`tests/style.pbr.gpu.test.ts`). A sky that glows with no sun (a studio) puts the sun at amount 1 below the horizon.

What is built once, on the CPU twin, when the style is made: the sky's projection (at the sun's stated direction), the
default clear colour, and the default `lobeAlbedo` (about 30 ms per new `distribution`/`visibility` pair; a lobe of 0
reflects no sky, a swapped visibility reflects the sky by its own albedo, within 0.02). So those parts must evaluate
on the CPU and must not read time or a param; one that does is refused by name (`pbr sky: given a sky that reads time`,
exit 4). A sky only the GPU can evaluate is fine beside its own `environment` and a given `clear`.

**Swap one part**; the others stay:

```ts
style: pbr({ fresnel: thinFilm(380) })                       // a film's colours in every reflection (examples/materials/thin_film.ts)
style: pbr({ sky: (dir, sun) => mix(DUSK, GOLD, pow(saturate(dot(dir, sun)), 4)), tone: (c) => c })
style: { ...pbr(), ink: () => vec3(0.05, 0.06, 0.08) }     // any member of the Style record, as with every style
style: { ...pbr(), water: myWater, ground: myFloor }         // the sea and the floor: their colours, roughness and fog are not parts
```

The sea's colours and roughness (0.12), the floor's albedo and roughness (0.9) and the fog are constants inside
`water`, `ground` and `shadow`; a different sea or floor replaces that member of the Style record, as above.

**Write your own `shade` with the parts.** The light fold hands one light at a time; the parts say what it does:

```ts
const lit = pbr();
const myShade = (ctx: ShadeCtx) => {
  const base = pow(ctx.base, 2.2), rough = ctx.channel<"f32">("rough", 0.4), f0 = vec3(0.04, 0.04, 0.04);
  const n_v = max(dot(ctx.normal, ctx.view), 1e-4);
  const direct = ctx.lights("sum", (l) => {
    const h = normalize(add(l.toward, ctx.view));
    const n_l = saturate(dot(ctx.normal, l.toward)), v_h = saturate(dot(ctx.view, h));
    const F = schlick(v_h, f0);
    const spec = mul(F, mul(ggx(saturate(dot(ctx.normal, h)), rough), smithGgx(n_l, n_v, rough), n_l, Math.PI));
    return mul(add(mul(sub(1, F), lambert(base, n_l, n_v, v_h, rough)), spec), l.amount); // l.unshadowed x l.visibility
  });
  const sky = skyLight(ctx, rough, daylight);
  return aces(add(direct, mul(base, sky.diffuse)));
};
style: { ...lit, shade: myShade }
```

What it cannot do, and what it costs: `src/style/pbr.ts`'s top comment and `skyLight`'s (no occlusion, no
inter-reflection; the sky is projected on spherical harmonics once per scene, so a moved sun lights bodies from the
sky as it stood, except through the one direct tap a reflection takes; a sky that reads time or a param is refused).
The default pair's environment weight is a fixed fit of its lobe albedo (within 0.06, kept so its pictures do not
change); any other pair is integrated. A Fresnel law other than Schlick is read at one representative cosine. The
sanity numbers (white furnace, roughness sweep, metal tint) and the cost against toon:
`agent_notes/2026-11-18_pbr_style/NOTE.md`, rerun with `bun tools/pbr_sanity.ts`.

## Writing one: `sketch`, all of it

Graphite on paper: hatching for tone, a wobbling line, no flat colour. This is `src/style/sketch.ts` (inside the
library it imports from the part files; outside, the same names come from the entry point).

```ts
import { abs, add, contactFoam, edgeOf, foam, fract, fwidth, light, litBodies, luminance, marksAs, marksFoam, mix, mul,
  paint, paperGrain, param, printLayers, rgb, smoothstep, sub, swellLines, vec4, weighted,
  type FoamInk, type Rgb, type Style } from "loft";

const PAPER: Rgb = [0.95, 0.93, 0.88];
const GRAPHITE: Rgb = [0.17, 0.17, 0.2];
/** White water is left as paper and outlined; ridge foam's softer edge is graphite too. */
const FOAM: Record<FoamInk, Rgb> = { fill: PAPER, line: GRAPHITE, edge: GRAPHITE };

export function sketch(): Style {
  const recipe = light.recipe({ hatch: light.hatch({ spacing: 0.12, least: 4, cap: 0.85, ragged: 0.3 }) });
  const line = paint.wandering({ least: 0.004, most: 0.05 });
  const bodies = litBodies(
    recipe,
    { ink: GRAPHITE, amount: 0.9, value: 0.95 },
    (ctx) => mix(rgb(GRAPHITE), rgb(PAPER), edgeOf(luminance(ctx.base), 0.12, 1e-5)),
    (c, ctx) => weighted(line, c, GRAPHITE, ctx),
  );
  return {
    name: "sketch",
    ...bodies,
    outline: param("sketch.outline", 0.012, { min: 0, max: 0.1 }),
    ink: () => rgb(GRAPHITE),
    clear: PAPER,
    post: [paperGrain({ fibre: 0.05, tooth: 0.09 })],
    sky: () => rgb(PAPER),
    water: (ctx) => {
      const lines = swellLines(ctx, { perWave: 3, width: [0.005, 0.015] });
      let c = mix(rgb(PAPER), rgb(GRAPHITE), mul(lines.cover, 0.4));
      c = printLayers(c, contactFoam(foam.roll(), ctx, lines), FOAM);
      c = printLayers(c, marksFoam(marksAs.heads(), ctx), FOAM);
      return vec4(mix(c, rgb(PAPER), smoothstep(40, 160, ctx.distance)), 1);
    },
    shadow: (ctx) => {
      const p = mul(add(ctx.world.x, ctx.world.z), 1 / 0.15);
      const fw = fwidth(p);
      const stroke = sub(1, smoothstep(sub(0.16, fw), add(0.16, fw), abs(sub(fract(p), 0.5))));
      return vec4(rgb(GRAPHITE), mul(mul(edgeOf(ctx.amount, 0.3, 0.01), stroke), 0.7));
    },
  };
}
```

Line by line:

1. **The tone.** A recipe with one treatment, `hatch`: lines round the form that swell from nothing above the
   terminator to 85% of their spacing in deep shadow, 0.12 world units apart but never closer than 4 px on screen,
   their ends ragged. Every other treatment is off. `litBodies` applies it: given the colour a body has in full light,
   it prints the recipe's tone as one block of graphite (`amount` 0.9 of the ink over the paper, at 0.95 value).
2. **The colour in full light** is paper, except a paint darker than 0.12 luminance (a pupil, a black stripe), which a
   pencil fills solid. That is the only flat tone.
3. **The line.** `after` runs on the lit colour before the block: `weighted` with `paint.wandering` draws a graphite
   line inside the hull from 0.004 to 0.05 world units wide, by noise over the body's anchor (so it rides the body) and
   heavier where the form turns from the light. The hull itself is thin (`outline` 0.012) and graphite (`ink`).
4. **Paper** is a post pass, `paperGrain`, with less fibre and more tooth than the woodblock's (graphite catches on
   the tooth). The sky is the bare sheet.
5. **The water** is paper with the swell's lines drawn at 40% graphite, three a wavelength. The foam is the white
   water painter (`foam.roll()`) and the deformations' marks as curled heads; both come back as coverage and print in
   the sketch's inks: the fill is paper, the line graphite, so foam is drawn as outlined shapes. Far off the water is
   left bare.
6. **A cast shadow** on the water is diagonal strokes masked by the shadow's coverage: the one thing written here
   that no part does.

Register it in `STYLES` (`src/style/index.ts`) to get `--style sketch` and a place in `loft gallery`; outside the
library, hand the value to `scene({ style: sketch() })` or `withStyle`.

## A look

A look is a preset of one style chosen by name. A style offers looks through `lights`, a record of constructors:
`withLight(scene, name)` and `--light name` pick one without editing the scene. Toon offers every recipe of `LIGHT`
(`--light hatched` draws toon's bands with hatching in its shadows); woodblock offers `LOOKS`, each a function from the
scene's own spec to a new one (`engraving` is `hatched` in the reference palette). To make one: a function of your
style's spec and a line saying what it is for, added to `lights`.

```ts
const pressed = (s: WoodblockSpec): WoodblockSpec => ({ ...s, light: LIGHT.hatched, foam: foam.stipple() });
woodblock(pressed({ print: print.bands() }));
```

## Getting variety

Retuning one style's numbers moves a picture inside that style's basin: a woodblock with other inks is still a
woodblock. Variety comes from changing what composes it. Swap a part (the same `contactFoam` call with `rings` in
neon is a cyan ripple, with `roll` in sketch an outlined drawing of foam: the pattern is coverage, the style chooses
what it means). Drop one (sketch has no colour rule at all; neon has no lighting recipe, its light is the paint).
Use a part for something it was not made for (the swell's lines are a cut-paper sheet's edge in papercraft's way of
thinking, a pencil stroke in sketch's, a scanline in neon's; `pixelsInside` is a rim, a heavier line or a tube's
glow). And when a picture needs something no part does, write the function: a part is only a function of the shade,
water or sky context, and a new one written well (its law, what it cannot do, its relatives) is the next style's
starting point. Each part's comment ends with relatives worth trying: start there.

## The plan, scored (2026-11-12)

The plan was committed before building (`git show 99d5161:docs/styles.md`). Against it:

| | Predicted | Measured |
|---|---|---|
| woodblock.ts | 360 lines (from 857) | 333 |
| toon.ts | 115 | 119 |
| biolume.ts | 95 (from 167) | 103 |
| papercraft.ts | 205 (from 218) | 218 (the sky became a part, and a 3-line constant names it) |
| sketch.ts / neon.ts | 80 / 85 | 58 / 54 |
| parts: sky, sea, foam, paint, post, light | 90, 45, 300, 100, 110, 495 | 98, 49, 315, 116, 121, 506 |
| pictures of the four old styles | byte-identical | byte-identical: 17 `rgba` hashes (the crab's three `game` frames as printed, in `engraving` and in `hatched`; serpent, storm, night hunt; the serpent in biolume, papercraft and woodblock; maelstrom; the lineup's sunset sky) |
| sketch, crab `game` 1280x800 | woodblock within 0.2 ms | **+1.0 ms** (6.4 against 5.4, two rounds in one clock state, heavily loaded machine): missed, unattributed (the per-pass frames fell in different clock states). Suspects: the hatch and the wandering line's noise on every body fragment, which the crab's woodblock does not run |
| neon, the same | woodblock +0.6 ms | **-0.4 ms** (5.0 against 5.4): missed the other way. Its bodies and sea are cheaper than the crab's woodblock (no rolling white water, no print rule); bloom and reflection cost less than that saved |

Weaker of the two: sketch, as predicted, but not for the reason predicted. It reads as a pencil drawing, not an
engraving; what it lacks is local value. Its tone comes only from the light, so a serpent striped in two mid-value
paints is one white shape, and a form lit from the camera's side is nearly bare. The fix is a part: a hatch whose
spacing or terminator follows the paint's value (listed under "Going past it" in `light.ts`).
Soft things (the storm's cloud, glows) are drawn in their own colours in every style: no style reaches them.

# Light: a function of a world point

Written 2026-11-13. A design proposal: nothing here is built. It obeys the nine rules of `docs/open_design.md` and
argues with one sentence of it ("things that glow provide one"; section 3).

The short form. A light is a function: for a point in the world, the unit direction toward the light, how much arrives
(a colour), and how far away the light is. A scene carries a short list of them in `lights`; the sun stays where it
is and gains an optional `amount`, so a night is a dim or absent sun. Every fragment that already has a world point
(bodies, the plane, the sea) receives the list evaluated there, the sun first, and each style decides how to print
it. One helper, `lamp`, makes the commonest light; one check, `litBy`, measures it on the CPU. A glowing thing does
not light anything by itself: the author writes one `const` for where the glow is and hands it to both.

## 1. What an author writes

Three scenes, written before the design and then read cold. The questions a stranger would ask are listed after each,
with what the design does about them.

### 1.1 A lantern lights its floor at night

```ts
// A paper lantern on a floor on a moonlit night: the flame inside lights the floor round it; the paper glows.
import { add, lamp, mul, plane, scene, strand, toon, vec3 } from "../../src/index.ts";
import { cameras, paper, radius } from "./lantern.ts";

const FLAME = vec3(0, 0.9, 0); // inside the paper, half way up

const lantern = strand({
  name: "lantern",
  spine: (m) => vec3(0, add(0.04, mul(m, 1.8)), 0),
  girth: radius,
  tips: "open",
  paint: (m) => ({ base: paper(m), emits: mul(paper(m), 0.9) }), // lit from inside: the materials proposal's channel
});

export default scene({
  ground: plane,
  things: [lantern],
  style: toon(),
  sun: { toward: [-0.55, 0.75, 0.35], amount: [0.06, 0.07, 0.14] }, // the moon: a dim blue key
  lights: [lamp(FLAME, { colour: [1.6, 1.0, 0.5], reach: 0.9 })],
  cameras,
});
```

| A stranger asks | Answer, and where it is said |
|---|---|
| Does the paper block the flame? | No. Lights cast no shadows unless their function computes them; a paper lantern is the case where that is right. `lamp`'s comment, first limit. |
| Why is the paper not lit by its own flame? | Its outside faces away from a light inside it (N·toward < 0 everywhere). How a surface glows is its paint (`emits`), not a light. `lamp`'s comment. |
| What is `reach`? Units of `colour`? | `reach`: the distance at which half of `colour` arrives; nothing arrives past four reaches. `colour` 1 is the sun at noon, so 1.6 is brighter than daylight at the flame. `lamp`'s law. |
| Why does the sun take `toward` here and a bare vector elsewhere? | A bare vector is still accepted and means `amount` 1. Defect kept on purpose: the old form must stay byte-identical. |

### 1.2 A creature lit by its own lure, no moon

```ts
// A long low walker at night whose stalk carries a lure: the lure lights its own face and the floor in front.
import { add, flare, lamp, litBy, mul, plane, scene, sin, strand, time, toon, vec3, type F } from "../src/index.ts";

/** The stalk, m 0 at the head to 1 at the lure: up and forward, bobbing. */
const stalk = (m: F) => vec3(add(0.3, mul(m, 1.4)), add(1.1, mul(sin(mul(m, 2.6)), 0.5)), mul(mul(m, m), mul(sin(mul(time, 1.7)), 0.3)));
const LURE = stalk(1); // written once: the flare shows it, the lamp lights from it

const body = strand({
  name: "body",
  spine: (m) => vec3(mul(m, -3), add(0.55, mul(sin(add(mul(m, 5), time)), 0.06)), 0),
  girth: (m) => add(0.08, mul(sin(mul(m, Math.PI)), 0.45)),
  tips: ["round", "point"],
  paint: [0.35, 0.3, 0.28],
});
const lure = lamp(LURE, { colour: [0.5, 1.8, 1.3], reach: 1.2 });

export default scene({
  ground: plane,
  things: [
    body,
    strand({ name: "stalk", spine: stalk, girth: 0.03, tips: ["open", "point"], paint: [0.3, 0.27, 0.25] }),
    flare("glint", LURE, { size: 0.06, intensity: 3, rays: 0, halo: 5, colour: [0.3, 1, 0.75], core: [1, 1, 1], veil: { kind: "clear" } }),
  ],
  style: toon(),
  sun: { toward: [0.3, 0.8, -0.5], amount: [0, 0, 0] }, // no moon: only the lure lights anything
  lights: [lure],
  cameras: {
    whole: { kind: "perspective", eye: [5, 2.5, 6], target: [-0.5, 0.8, 0], fovY: 0.6 },
    face: { kind: "perspective", eye: [2.5, 1.2, 1.8], target: [0.3, 0.8, 0], fovY: 0.5 },
  },
});

// The head (m = 0, about 1.9 from the lure) receives at least 0.15 of the lure's light (luminance): predicted 0.31 (section 7).
export const checks = [litBy(lure, body, { least: 0.15, from: 0, to: 0.1 })];
```

| A stranger asks | Answer |
|---|---|
| Why must a moonless night still give a sun direction? | The sun is also the key: the direction a print's terminator, the sky's disc and the cast-shadow block are drawn from. `amount: [0, 0, 0]` keeps those defined and adds no light. Open question 1. |
| Why does the lure need a `flare` *and* a `lamp`? | One is what you see (light toward the eye), the other what it does (light on others). Different laws, different costs; one `const` joins them. Section 3, "Emission". |
| Is the lure's light hidden behind the body? | No: no shadows. The far side of the body faces away and is dark; the floor behind it is lit. Said in `lamp`. |

### 1.3 A road of lamps behind carved screens, as one function

```ts
// A road of lamps, one every 4 units, each behind a screen cut in six petals: one function lights the whole road.
import { abs, add, atan2, cos, floor, lamp, mul, plane, scene, smoothstep, sub, vec3, woodblock, type Light } from "../src/index.ts";

const SPACING = 4;
const road: Light = (p) => {
  const x = mul(floor(add(mul(p.x, 1 / SPACING), 0.5)), SPACING);            // the nearest lamp's x
  const near = lamp(vec3(x, 3, 0), { colour: [1.4, 1.1, 0.7], reach: 1.5 })(p);
  const out = mul(near.toward, -1);                                             // from the lamp toward p
  const petals = smoothstep(0.3, 0.5, abs(cos(mul(atan2(out.z, out.x), 3))));  // six openings round the lamp
  const seam = smoothstep(SPACING / 2, SPACING / 2 - 0.4, abs(sub(p.x, x)));   // fade before the next lamp takes over
  return { ...near, amount: mul(mul(near.amount, petals), seam) };
};

export default scene({ ground: plane, things: [/* posts, a figure */], style: woodblock(), sun: { toward: [0.2, 0.6, 0.7], amount: [0.1, 0.1, 0.2] }, lights: [road], cameras: { /* ... */ } });
```

This is the case a menu would never hold: an infinite row of patterned lights for the price of one term, by domain
repetition (the same `floor` trick a field uses to repeat a shape). It also shows the limit of "one direction per
point": only the nearest lamp lights a point, so the author fades each pool before the midpoint (`seam`); without it
a body standing half way would see its light jump from one side to the other.

| A stranger asks | Answer |
|---|---|
| Can I call `lamp(...)` and then use the result as a function? | Yes: a `Light` is a function, and `lamp` returns one. |
| Why `seam`? | Above. Said in `Light`'s comment as the cost of one direction per point, with the relative (sum two neighbours, section 6). |
| What does woodblock do with petals of light? | Prints them as hard pools (section 4.3, woodblock): a lit ink where the light's measure wins, the block elsewhere. |

What the cold read changed in the design: `colour` on `lamp` takes expressions (a flicker is `mul(colour, flicker)`,
so `Flash`'s separate `power` goes); the sun's new form is `{ toward, amount }`, not a second scene field
(`moon`, `sunlight`), so one place says where the key is and how bright it is; `litBy` takes a stretch of a body
(`from`, `to` in m) because "the head is lit" is the question, not "the body".

## 2. What exists (verified in the code)

- **One sun.** `scene({ sun })` takes a vector or `movable(dir)` (`src/scene.ts:24`, `src/sun.ts`), refuses a sun
  below `y > 0.05 |sun|` (`scene.ts:64`), and classifies it once into `Sun = fixed | input` (`sun.ts:12`). It has a
  direction and no strength.
- **Styles see it as `ShadeCtx.sun`** (`src/style.ts:11`), also in `WaterCtx`, `SkyCtx`, `GroundCtx`. `styleWgsl`
  (`src/compile/scene_wgsl.ts:14`) emits `shade`, `ink`, `mark`, `ground_colour`, `sky` **once** into a shared chunk
  as WGSL functions of `c_world`, `c_normal`, `c_view` and the rest; the sun is a literal or an input inside them.
  Every lit drawable calls that one `shade` (`src/draw/surface.ts:322-431`, `relief.ts:164`, `trace.ts:240`, field,
  impostors, copies). So a light written as an expression of `ctx.world` reaches every drawable through one function,
  with no plumbing per drawable. This is the fact the design rests on.
- **Measured from a build of the crab** (`examples/bosses/reefback_titan.ts`, `loft build`): that shared chunk is
  33.9 KB and is included in 74 of the 95 shader modules; all modules together compile 6.14 MB of WGSL.
- **The lighting recipe** (`src/style/light.ts`). Every treatment is keyed by `Key = sun | offLens | camera` and
  measured by `turn(t, ctx)` = N·K minus a threshold, positive in the light. Core, hatch, rim and highlight each read
  one key; `lit` combines their tones, darkest wins; `litBodies` turns a recipe into a style's `shade`, `mark` and
  `block`. Its comment already lists the limit this proposal removes: "light that is not one direction for the whole
  picture (a lamp, a point light falling off)".
- **The styles.** `toon` bands N·sun (`toon.ts:63`) and its plane is `banded(ctx.sun.y)`; `woodblock` splits its print
  rule at N·sun = 0 with no recipe (`litSide`, `woodblock.ts:163`) or uses `litBodies`; `sketch` is `litBodies` with a
  hatch; `papercraft` builds a studio key from the sun plus a fill (`papercraft.ts:173`); `biolume` treats the sun as a
  faint moon and makes light from the paint (`glowOf`); `neon` ignores the sun on bodies (`tubeOf`).
- **Cast shadows** come from the sun only: ground patches lifted along the sun (`src/draw/shadows.ts`, 0.30 ms on the
  crab, 526 capsule instances), and the recipe's `cast` block, one box per capsule (`src/draw/light.ts`, the `cel`
  look +1.2 ms).
- **Point lights exist in one corner.** `flash({ at, power, colour, reach })` (`src/soft/medium.ts:170`) lights a
  medium's blobs on the CPU with `power·colour·reach²/(reach²+d²)·(1−x²)²`, `x = d/(4 reach)`, and nothing else
  ("bodies, the ground and the water are not lit by it"). `boltFlash` and `sheetFlash` (`src/soft/lightning.ts`)
  return one.
- **Glows light nothing.** `glow`, `glows`, `flare` (`src/soft/glow.ts`) add light toward the eye only; their comment
  says so.
- **Grounds.** `plane` is a full-screen pass calling the style's `ground_colour(p, view, distance, shadow)`
  (`src/draw/ground.ts`); `nothing` draws nothing. So `examples/first/lantern.ts` has a floor, and nothing can light it
  but the sun.

## 3. Options

| Family | What a user writes | What it cannot express | What it costs the library |
|---|---|---|---|
| **Special types** | `lights: [point({at, colour, range}), spot({…}), area({…})]` | Anything off the menu: a patterned lamp, a curve of light, the road of 1.3, a light that is brighter where a table says. Each needs a class, and every style must know every class. | A sum type per kind, a handler per kind in six styles, a page in the context per kind. |
| **Parameters only** | more suns: `suns: [{dir, colour}]` | Position and falloff: a lamp is not a direction. | Small, and useless for the lantern. |
| **Functions only** | `lights: [(p) => ({ toward, amount })]` | Nothing a point or directional light does; it cannot be culled (the compiler cannot look inside); one direction per point. | One type, one field on the scene, one field on each context. |
| **Shaders only** | WGSL that styles call: `wgslFn` returning a direction and colour | No CPU twin: no `litBy`, no medium lit on the CPU. | Nothing; possible today. |
| **Function plus optional facts** (recommended) | A function, exactly as above; a helper's result also *carries* facts the compiler may use (a bound, a source point), which plain authors never write | The same as functions only, until a pass reads the facts | Functions only, plus two optional properties read by nobody in the first stage |
| **Light as a thing's member** (open_design's "things that glow provide one") | `glow(...)` returns a drawable that also lights the scene | Lights the author did not ask for, each a term in every fragment; a 40-stroke bolt would be 40 lights. Which point of a curve lights? | A field on `Drawable` and a collection rule |
| **Light as emission sampled** (area lights from `emits`) | nothing: a surface with `emits` lights others | Real-time cost: many samples per fragment per emitter | A sampler per surface kind; noise; section "Headroom" |

**Why functions.** The open design's argument holds and the code makes it cheaper than it looked: `shade` is one
function of `c_world` shared by every drawable, so a light written as an expression of that point reaches every
lit thing with no change to any drawable. A point, a spot, a tube (closest point on a segment), a window, a gobo, a
flicker, a row by repetition, a light read from a baked table (the state proposal's grid) are all the same type. Special
types would buy one thing: the compiler could cull them. That is bought more cheaply by an optional bound carried
beside the function (Headroom, row 2).

**Why not "things that glow provide one".** I tested it against scene 1.2 and the lightning in `examples/storm.ts`.
It makes a cost the author did not write (each light is a term in every fragment, about 0.02 to 0.05 ms on the crab:
section 7), it has no good answer for *which* light a curve provides (its middle? n samples? an analytic line light?),
and a glow is shaded toward the eye with a different law from what it sheds. The one-`const` join (1.2) costs the author
one line and says exactly what lights what. Changed, with reasons; not adopted.

**Why the sun stays special.** The owner's first question was whether lights need to be special. Local lights do
not. The sun is special in two ways that are true of the picture, not of the API: it is the **key** (one direction
for the whole frame, which the print's terminator measured round a section, the sky's disc, the cloud's light tables
and the cast-shadow block are all drawn from), and it is at **infinity** (so its shadow is a projection, cheap). A
local light has neither property. So the sun keeps its own field and its shadow machinery, gains an `amount`, and is
also handed to styles as the first entry of the list, so a style that wants to treat all lights alike can.

## 4. The recommendation

### 4.1 Types

```ts
// src/light.ts (new; it also takes `movable` and the sun's classification from src/sun.ts, which it replaces)

/**
 * What arrives at a point from one light. `toward`: unit direction from the point to the light. `amount`: the light
 * arriving, as a colour; 1 is the sun at noon. `far`: how far along `toward` the light is (a shadow ray's length);
 * absent means at infinity, as the sun is.
 */
export type Arrival = { readonly toward: Expr<"vec3">; readonly amount: In<"vec3">; readonly far?: In<"f32"> };

/**
 * A light: for a world point, what arrives there. Any expression of the point, of time, inputs and tables.
 *
 * The law is yours. Loft evaluates it at every lit fragment (bodies, a plane, the sea) and on the CPU for checks.
 * Cannot: arrive from more than one direction at a point (an area light, a sky: see `lamp`'s relatives); cast a
 * shadow unless it computes one; be culled by the compiler unless it carries `bound`. At most 8 lights with the sun.
 * Optional facts a helper carries so later passes can use them; an author writing a function leaves them out:
 *  - `bound`: no light arrives outside it (lets the compiler skip the light's term outside; unused in stage 1).
 *  - `source`: where the light is, for a shadow-map pass: a point, or a direction for a light at infinity.
 */
export type Light = ((p: Expr<"vec3">) => Arrival) & {
  readonly bound?: TraceBound;
  readonly source?: { readonly kind: "point"; readonly at: Expr<"vec3"> } | { readonly kind: "direction"; readonly toward: In<"vec3"> };
};

/**
 * A point light: `colour` × reach² / (reach² + d²) × (1 − x²)², x = d / (4 reach), nothing past 4 reaches. Half of
 * `colour` arrives at `reach`. `at` and `colour` take expressions (it moves; it flickers: mul(colour, flicker)).
 * The law is `flash`'s, which it replaces. Carries `bound` (a sphere of 4 reaches, when `at` is a constant) and
 * `source` (the point).
 *
 * Cannot: cast shadows (the floor under a body standing in its pool is lit; light passes through every thing);
 * light the outside of a thing it is inside (the paper of a lantern: give the paper `emits`); have size (its highlights
 * are points; its shadows, when written, are hard unless the ray is given a width).
 * Relatives: a spot is this times smoothstep over dot(−toward, axis); a gobo is this times a pattern of −toward (1.3);
 * a tube is this from the closest point on a segment, its amount × the segment's length seen (an analytic line light);
 * a pool from a table is `fetch` of a baked grid at p; a soft shadow is this times the coverage of
 * `castRay({ o: p, d: toward, t0: 0.02, t1: far, width: (t) => mul(t, size) })` (geometry proposal).
 */
export function lamp(at: In<"vec3">, o: { readonly colour: In<"vec3">; readonly reach: number }): Light;
```

```ts
// src/scene.ts
export type SceneSpec = {
  // ...as now, and:
  /** A vector or movable(dir): the sun at full strength, as before. { toward, amount }: a dimmer or coloured sun, a moon, or none (amount 0). */
  readonly sun: V3 | MovableSun | { readonly toward: V3 | MovableSun; readonly amount: In<"vec3"> };
  /** Lights besides the sun, each a function of a world point (`Light`, `lamp`). At most 7. */
  readonly lights?: readonly Light[];
};
// Scene gains `lights: readonly Light[]` and `sunAt` gains `amount: { kind: "full" } | { kind: "given"; amount: In<"vec3"> }`,
// classified once here.

// src/style.ts: ShadeCtx, GroundCtx and WaterCtx each gain
  /**
   * Every light at this point, the sun first: the sun's arrival is { toward: sun, amount: its amount }, the scene's
   * `lights` follow in order. `sun` is `lights[0].toward`, kept so every style written before lights is unchanged.
   */
  readonly lights: readonly Arrival[];
// SkyCtx gains the same, holding the sun only (a local light has no meaning at infinity). LayerCtx gains nothing.
```

The sun's amount, when the scene states none, is the JavaScript literal `[1, 1, 1]` (an `In<"vec3">`). Built-in
styles multiply by it through one private helper in `src/style/shared.ts` that returns its argument when the amount
is that literal, so the shader text of every existing scene is unchanged byte for byte. (The expression layer does
not fold `x * 1`; I checked: `src/expr.ts` has no constant folding.) A user's own style that multiplies by it
unconditionally changes the text and not the pixels.

### 4.2 Where it plugs in

| File | Change | Lines |
|---|---|---|
| `src/light.ts` (new; `src/sun.ts` folds in) | `Arrival`, `Light`, `lamp`, the sun's amount, `arrivalsAt(p, sun, lights)` | 70 |
| `src/scene.ts` | the sun's third form, `lights`, the cap (a `SceneError` naming the fix: "combine sources in one function, as a row by repetition"), `restyled` keeps lights | 25 |
| `src/compile/scene_wgsl.ts` | build `lights` for each context from `c_world` (shade, ink, mark, ground, water) and from the sun alone for the sky. No new argument to any WGSL function: the lights are expressions of variables those functions already take. | 15 |
| `src/compile/context.ts` | `CompileContext.lights` (for the medium) | 3 |
| `src/style.ts` | the `lights` member on four contexts; `shadeVariables` takes the lights | 15 |
| `src/style/light.ts` | `turn`, `rimmed`, `highlit` take the union over lights (4.3) | 40 |
| six styles | section 4.3 | 75 |
| `src/soft/medium.ts`, `lightning.ts` | `flashes` go; a medium is lit by the scene's `lights` on the CPU (the same law: `lamp` is `flash`) | −20 / +25 |
| `src/geom/sea.ts`, `src/spacetime/smear.ts` | pass `lights` where they build a context by hand | 6 |
| `src/check.ts` | `litBy` | 35 |
| docs: `context.md`, `lighting.md`, `styles.md`, each style's comment | | (docs) |

About **290 lines of library code**. A drawable needs no change: every one calls the shared `shade`.

**What stays byte-identical.** A scene with no `lights` and a sun as a vector or `movable`: every shader's text (the
list holds the sun alone, with the literal amount; the union over lights of one light is that light's measure, built
by the same expression), so every picture. The 17 `rgba` hashes of `docs/styles.md` are the test.

**What changes for existing scenes.** Nothing. `flash` and `Flash` go; their four users (`storm.ts`, `stormwing.ts`,
`stormfront.ts`, `tithe_fleet.ts`) change `flash(...)` to `lamp(...)` and `flashes:` to the scene's `lights` in the
same commit. Then the bolt also lights the bodies, the floor and the sea, which it could not before: those four
scenes' pictures change on purpose, once, and that is listed in the commit.

### 4.3 How each style prints two lights

The rule for the recipe, and so for `litBodies`. Today every treatment asks one measure, `turn` = N·K − threshold,
positive in the light. With several lights it asks the **union**: the largest of the lights' measures, where a
local light's measure is also cut by how much of it arrives:

    turn = max( turn_sun , max_i min( N·L_i − at , lum(amount_i) − pool ) )

`max` of positive-inside measures is the union of the lit regions and `min` their intersection: the same algebra as
`union` and `intersect` on distance fields, applied to "lit". `edgeOf` antialiases the result by its screen
derivative, as it does today. So a lamp prints as a **hard pool**, its edge where `lum(amount) = pool` (default 0.1:
two reaches from a lamp of colour 1), which is how a printer would cut it, not as a falloff. The sun's measure is cut
the same way only when its amount is stated, so a moonless night (`amount` 0) leaves the sun's measure out entirely.
`keyOf`'s `offLens` and `camera` keys apply to the sun only: a lamp swung off the lens would lose where it is.

**`litBodies` generalises; no second form.** Core, hatch and the section terminator read `turn`; the rim and the
highlight take the largest over lights of their own shape, each cut by the same pool. One new recipe number, `pool`,
on `light.recipe`. Where a local light wins, the lit ink is tinted by its hue (the amount divided by its largest
channel): one `mix` per light.

- **toon.** Today: `base × mix(shadowTint, warm, banded(N·sun))` plus rim and gloss. With lights: that term times the
  sun's amount, plus for each local light `base × hue_i × banded(saturate(N·L_i) × lum_i)`, where `banded` is toon's own
  step function, so a lamp's falloff prints as two or three flat rings of light; a gloss per light from its half
  vector. The plane: `banded(L_i.y × lum_i)` per light, so the lantern stands in rings of warm floor. A night toon is
  the same bands under a sun of amount 0.06: dark and blue, with the sky scaled by the sun's luminance (toon's sky is
  daylight today; this is the one look change a night scene in toon gets).
- **woodblock.** Without a recipe the print splits its rule at N·sun = 0 (`litSide`); with lights it splits at the
  union measure above: the lit inks where the sun or a lamp's pool wins, the rule's shadow inks elsewhere. With a recipe
  it is `litBodies`, already general. Its sea gets each light's pool on the water as the cream line ink, cut by the
  same pool, and its plane the same. A night print reads as a print does: inks in the block, pools cut out of it.
- **biolume.** Already a night style whose sun is a faint moon. Lights add `base × amount_i × saturate(N·L_i)` to the
  dark skin before the glow (this is the one style where light falls off softly: the picture is about light), and on
  the water a road of light under each lamp from its half vector, which `reflection` smears and `bloom` finds. The
  paint's guessed glow stays (or becomes `emits` under the materials proposal).
- **papercraft.** The studio key is built from the sun (`keyDir`); lights add a wrapped term per light on the
  **facet** normal, `saturate((n·L_i + 0.1)/1.1) × amount_i`, so a lamp lights a folded model one facet at a time, as a
  candle lights paper. The fill is unchanged.
- **sketch.** It is `litBodies` with a hatch, so it gets lamps for nothing: a lamp's pool is paper left among the
  hatching; the lines thin toward the lamp because they read `turn`. Its plane: the pool left as paper, the rest
  lightly hatched only under a dim sun.
- **neon.** Its light is the paint, and stays so. Lights add `base × amount_i × saturate(N·L_i) × 0.35` to the
  near-black body (the tube still dominates), and on its dark floor and black-glass water a soft pool in the light's
  colour, which bloom carries. A neon scene is where a coloured lamp reads best.

### 4.4 Checks

Everything here is an expression, so on the CPU (`compileJs`): **is this point lit, how much, from where, how far**.
A light function evaluated at a skeleton's joints at a time; the lit measure at a point with a normal; the share of a
stretch of a body inside a lamp's pool.

The one check worth having:

```ts
/**
 * The light reaching a stretch of a body (m from `from` to `to`, default the whole) at time `at` (default 0) is at
 * least `least` in luminance, on average over its joints. Fails naming the body, the light's value and where it fell
 * short. Measures what arrives, not what faces it: a joint has no normal.
 */
export function litBy(light: Light, feature: Measured, o: { readonly least: number; readonly from?: number; readonly to?: number; readonly at?: CheckTime }): Check;
```

It catches the night scene's commonest failure, the subject in the dark, which a model cannot see without rendering
and looking: scene 1.2 with `reach: 0.7` instead of 1.2 puts 0.05 on the head and fails.

## 5. The line in `loft context`

In the opening paragraph, replace "One sun lights bodies (no other lights on them);" (8 words) with:

> A light is a function of a world point returning `{ toward, amount, far? }`; `lights: [lamp(at, { colour, reach })]`
> lights bodies, floor and water, at most 7, no shadows unless your function computes them; `sun: { toward, amount }`
> dims the sun (a moon, or 0).

Thirty-eight words, so +30. API map: `sun: movable(dir, name?)` becomes `light: lamp(at, {colour, reach}); movable(dir,
name?)` (+3); `scene({ground, things, style, sun, cameras?, outline?, period?})` gains `lights?` (+1); Checks gain
`litBy(light, feature, {least, from?, to?, at?})` (+6); Shapes lose `flash({at, power, colour, reach})` (−5). Craft
gains one sentence under "Range in style":

> **Light a night by what is in it:** a dim sun or none, and a `lamp` at the same expression as the glow that shows it;
> check the subject is lit with `litBy`.

Twenty-eight words. Net: about +63 words on 2,251.

## 6. Limits and relatives (as the comments state them)

**`Light`.** *Law:* a function from a world point to one arrival; Loft evaluates it per lit fragment and on the CPU.
*Parameters:* any expression of the point (time, inputs, params, tables). *Cannot:* light from more than one direction
at a point (an area light, a sky dome, two lamps through one function: a function that picks the nearest shows a seam
where the nearest changes, 1.3); be skipped where it is dark (each light is a term in every lit fragment's shader,
about 0.02 to 0.05 ms on the largest example, section 7); cast a shadow unless it computes one; light a medium
except at its blobs' centres; at most 7 besides the sun. *Relatives:* the sum of two neighbours' arrivals by a weighted
average of directions (no seam, and wrong in a small way where they disagree); a light from a baked grid (`fetch`); an
occluder written inside (`castRay` or the author's `march` as a shadow ray: the geometry proposal).

**`lamp`.** As its comment in 4.1.

**`ShadeCtx.lights`.** *Law:* every light at this point, the sun first. *Cannot:* tell a style which light is
whose (a style printing "the lure's light" differently from "the moon's" needs the lights' order, which is the
scene's). *Relatives:* a style that prints only the brightest light (the key per point: `max` by luminance) for a
graphic look; a style that reads `far` for an atmosphere (light scattered along the way).

**What is inherited and what is mine.** Inherited: the point light's inverse-square with a smooth window (every
real-time engine; Karis 2013's window), gobos and cookies (stage lighting), domain repetition (distance-field
practice), the toon ramp per light. Mine: a light as a function of the point with the sun as the first evaluated entry;
the union of lit measures by `max`/`min` (the distance-field algebra applied to "lit", which makes a lamp a printed
pool in every recipe treatment at once); dropping "things provide lights" for one shared `const`; `far` so a light
function is also a shadow ray's specification.

## Headroom

What the design leaves room for, against state-of-the-art practice. The types in 4.1 are shaped so nothing in this
table needs an author to rewrite a light.

| Practice | Status | Why, and what the addition would be |
|---|---|---|
| **Area lights and soft shadows** (light from many directions) | *Needs an addition* | One arrival per point is a point or directional light. An emitter with extent is a surface whose paint gives `emits` (materials proposal); Loft's surfaces are parametric, so a sample is a (u, v) and its density is 1 / \|∂P/∂u × ∂P/∂v\| over the area. The addition is one optional property on `Light`, `sample?: (p, xi: vec2) => Arrival & { pdf }`, which a pass that integrates calls with random numbers; a plain function stays the one-sample case (pdf a delta), so what a simple author writes does not change. A *soft shadow* from a lamp of size s works as designed today: `castRay` with `width: t => t·s` gives the penumbra from the closest approach (geometry proposal 4.2). |
| **Many lights** (hundreds, culled) | *Needs this addition, already in the type* | The function is opaque to the compiler, so every light costs every fragment. `bound` beside the function lets the compiler wrap the term in a test against the bound (a branch, cheap when coherent), and later bin lights into screen tiles (clustered shading: a compute pass over bounds into a table the shade function reads). `lamp` sets it; an author's function may. Recommendation: carry it from stage 1 and read it in stage 4, after a scene with more than 7 lights exists; past the cap, the honest form for 200 motes is a light read from a baked density grid (state proposal). |
| **Shadow maps** (a depth render from the light) | *Needs a pass; nothing from authors* | A shadow map needs the light's projection: a point (a cube map or a dual paraboloid) or a direction (an orthographic map, cascaded). `source` declares exactly that and `lamp` and the sun fill it. A later pass renders depth from `source` and multiplies the arrival's amount by the map's test before the style sees it; styles, scenes and the light functions do not change. A light with no `source` (a road, a pattern) gets no map, and says so. |
| **A physically based mode** (a path tracer reading the same scene) | *Partly as designed* | It can use: every light as written (an arrival at any point, on the GPU or CPU), `far` for the shadow ray's length, `castRay`/`march`/`draw.traced` hits for visibility, paint channels (`gloss`, `metal`, `emits`) as a material, parametric surfaces as sample spaces. It needs: `sample` and `pdf` (row 1); a BRDF that is not a style (a style is a look; a path tracer needs a reflectance, which the materials channels give but no style defines); `amount` in physical units (today 1 is "the sun at noon", a convention a tracer would keep as its unit); triangles intersectable by rays (geometry proposal says no BVH: a tracer would trace stand-ins and fields only). |
| **Wavelength** (dispersion, thin films, diffraction) | *Not blocked* | `amount` is `In<"vec3">` because styles mix colours; a spectral mode would add `spectrum?: (lambda) => F` on `Arrival` and sample it, with `amount` its RGB projection. Diffraction and thin-film colour as an appearance work today in a style or a wrap (they depend on `toward`, `view` and the normal, all present: the materials proposal's beetle is one); diffraction as propagation (a wave) is outside a ray model and ruled out by it, not by these types. |
| **Global illumination** (bounced light) | *Ruled out in real time here* | A light function cannot see other surfaces' radiance. A relative: a baked irradiance grid over the period (state proposal) read by a light function, which is one light. |

## 7. Predictions a build will be scored against

Measured first, on this machine (Apple M4, load average 15 to 46 from other work), with a throwaway module
`out/light/cost.ts` (not committed): the crab with N point lights added to its style's `shade` (and `block.body`),
each `colour × saturate(N·L) × reach²/(reach²+d²) × window`, positions moving with time so nothing folds; profiled
with `loft profile --view game` at 1280x800 in alternating rounds (`out/light/pairs.sh`).

| Measured | N = 0 | N = 1 | N = 4 | N = 32 |
|---|---|---|---|---|
| `shade` function in WGSL, bytes | 1,309 | 2,100 | 4,137 | |
| all modules' compiled WGSL, bytes | 6,135,231 | 6,194,098 | 6,345,931 | |
| bodies' draws, ms, slow clock state (3 pairs, sea at 2.1 ms in each) | 2.09, 2.10, 2.23 | | | 3.62, 3.65, 3.69 |
| warm render wall time (cached pipelines), s | 5.6 | | | (N = 8: 5.3; N = 16: 6.2) |

So: **+1.05 KB of WGSL per light** in the shared chunk, compiled in 74 modules: +59 KB, **+0.96% of the compiled
WGSL per light**. The body draws: **+1.55 ms for 32 lights in the slow clock state, 0.048 ms per light**; the fast
state runs the same frame about 2.4 times faster (the sea reads 0.81 to 0.88 ms there against 2.1), so **about 0.02 ms
per light** on the 2.1 ms frame. Cold load time (cache bust by a constant) read 10 to 47 s with no order in N on this
loaded machine: unresolved; warm load is unchanged.

Predictions for the build (the bare Lambert term above is the floor; a style prints each light with bands, edges and
a gloss, about twice the work):

| Quantity | Prediction | How measured |
|---|---|---|
| Frame time, crab `game` 1280x800, no lights | unchanged; byte-identical shaders | the 17 `rgba` hashes and the artefact digest |
| per light, crab in toon, bodies only | **+0.04 ms** in the fast state (±0.02) | `loft profile`, alternating N=0 / N=8 rounds, median difference of 5 pairs, slow-state numbers divided by the sea's ratio |
| per light, a style that also lights the sea | **+0.07 ms** (the sea's 1M pixels pay too) | the same, sea's own row |
| 7 lights (the cap) in toon on the crab | **+0.3 to +0.5 ms** on 2.1 ms | the same |
| shader size | **+1.0 to +2.5 KB per light** in the shared chunk (Lambert 1.05 KB; toon's bands and gloss per light more) | `loft build`, size of the style chunk |
| cold load time | **+1% to +2.5% per light** (compiled WGSL grows by that much) | `loft render` wall time with a cache-busting constant, on an idle machine, 3 runs each |
| library code | **about 290 lines**, ±60 | `git diff --stat` on `src/` |
| exported names | **+3, −2: net +1.** Added `Light`, `lamp`, `litBy`; removed `flash` and `Flash`. (`Arrival` is not exported: it is `ReturnType<Light>`; authors write the object literally.) | `tools/` export count |
| `litBy` on scene 1.2 | head (m = 0) receives luminance 0.31 at t = 0 (0.207 of the lure's colour, whose luminance is 1.49); passes at 0.15 | `loft check` |

**The cap: 7 lights besides the sun.** At the predicted +0.04 to +0.07 ms each, 7 lights cost about a quarter of
the crab's 2.1 ms frame, which is the most I would let a constant in every lit fragment cost before culling exists. A
scene that needs more is either one function (a row, a grid, a table) or waits for `bound` to be read (Headroom, row 2).

## 8. How it meets the other three proposals

**Materials** (paint channels, a wrap, `glossed`).
- *I need* `emits` exactly as it is proposed: a colour a style adds after lighting, untouched by lights, the block or
  the night. The lantern's paper (1.1) is that and nothing more. Nothing links `emits` to a light; I agree with the
  materials proposal that a surface colour has no position or falloff, and I take the same join: one `const`.
- *I give* `ShadeCtx.lights`, which a wrap sees with no change (it receives the same `ShadeCtx`). `glossed` should
  answer every light, not only the sun: its glint is a sum over `lights` of the half-vector cone, each times its
  amount (a wet stone under two lamps shows two highlights). A small change to `glossed`'s 45 lines; I would make it
  in stage 2.
- *The headroom row 1 needs from them*: `emits` on a parametric surface is an area light's radiance; when a sampling
  pass exists it reads the same channel.

**Geometry and rays** (`draw.mesh`, `draw.traced`, `march`, `castRay`, `Ray`, `Traced`).
- *I need* nothing for the first stages: `draw.mesh` and `draw.traced` call the shared `shade`, so they are lit by
  every light at once.
- *I use* `castRay` and `march` as **shadow rays**: an occluded lamp is
  `const a = lamp(at, o)(p); const c = castRay({ o: p, d: a.toward, t0: 0.02, t1: a.far, width: (t) => mul(t, 0.1) });`
  then `amount × saturate(gap / width(t))`. That is why `Arrival` has `far`: it is the ray's `t1`. Their measured cost,
  0.3 to 0.6 ms per light over body pixels with culling, sets the shadowed-light budget: two shadowed lamps at most
  on the crab's frame.
- *I give* a reason for `castRay` to exist on the GPU from the fragment stage (where lights are evaluated).

**State** (a step function baked into a table over the period).
- *I need* lights to read tables, which they do: a light is any expression, `fetch` included. A pool read from a
  baked grid, a flicker baked from a recurrence.
- *I give* the answer to their 200 motes: past the cap, one light whose amount is summed from their baked density.

## 9. Staged build plan

| Stage | Work | Days | Done when |
|---|---|---|---|
| 1. A lantern lights its floor | `Light`, `lamp`, `scene({ lights })`, the sun's amount, `ShadeCtx/GroundCtx.lights`, toon only (bodies and plane) | 1.5 | `examples/first/lantern.ts` at night with a lamp renders a lit floor in rings; the 17 hashes are identical |
| 2. Every style | the union measure in `light.ts`; woodblock, sketch, papercraft, biolume, neon; the sea's `WaterCtx.lights`; `glossed` per light | 2 | scene 1.3 in woodblock prints petal pools; each style's comment lists how it prints lights; hashes identical |
| 3. One light everywhere | `flash` → `lamp`, the medium lit by `lights`; the four storm scenes moved; `litBy`; context lines | 1.5 | the storm's bolt lights the sea and the cloud from one value; `loft check` on scene 1.2 passes and the reach-0.7 version fails |
| 4. Scored | the predictions of section 7 measured on an idle machine | 0.5 | the table filled in, misses attributed |
| later | `bound` read for culling; a `source`-driven shadow pass; `sample` | | a scene that needs it |

**Stop rule.** Abandon "a light is a function evaluated in the shared `shade`" if, in stage 1, either (a) a scene with
no lights does not stay byte-identical without per-style special cases beyond the one helper, or (b) one toon light
costs more than **0.15 ms** on the crab's bodies in the fast state (three times the prediction), which would mean the
shared function defeats the compiler (for instance, every module inlining the light into vertex stages). Then the
fallback is lights as a table of points the styles loop over, which gives up patterns and the CPU twin for points only.

## 10. Open questions for the owner

1. **A moonless night still states a sun direction.** Keep it (the key for prints and the sky), or allow `sun: none`
   and give prints a key from the brightest light per point? *Recommend: keep it; revisit when a scene suffers.*
2. **Should the sea answer lights in every style?** It is the largest per-pixel cost (+0.03 ms a light on the crab).
   *Recommend: yes in biolume, neon and woodblock (a lamp's road on water is the picture); toon and papercraft follow
   only if a scene asks.*
3. **The cap: 7 besides the sun.** *Recommend 7, refused by name with the fix; raise it when `bound` is read.*
4. **Remove `flash` now** (the storms' pictures change once, the bolt then lights bodies and sea), or keep it beside
   `lamp`? *Recommend: remove now; two laws for one light is the duplication the rules forbid.*
5. **Shadowed lamps by default in `lamp`?** *Recommend: no. A shadow is a line the author adds with `castRay` (section
   8), so its cost is visible where it is written.*

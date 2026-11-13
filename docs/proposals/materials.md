# How a surface answers light

A design proposal, 2026-11-13. Nothing here is built. It follows the rules in `docs/open_design.md` and tests the
direction sketched there ("channels and a wrap"). It keeps that direction and changes five things about it:

1. The wrap goes inside the paint (a key named `shade`) and does not become a new option on every drawable.
2. A channel's type is the type of the default the style reads it with.
3. A channel no paint gives costs nothing. This comes from three exact identities in the expression constructors,
   not from a flag.
4. The shade context gains `sky(dir)`, so a body reflects the sky of whatever style is drawing it.
5. Names that nothing reads are reported, not refused.

Translucency stays with `draw.glazed`.

## 1. What an author writes

Three scenes, written before the design was fixed and then re-read cold. The questions a stranger asked on that
reading are listed after each one, with where the design now answers them.

### A lacquered bar, wet across its middle

```ts
// A red lacquered bar on a floor, wet across its middle where a cloth has just passed. "Wet" is not a word any style
// knows: it is darker and glossier, said in the two terms every style that cares about gloss understands.
import { abs, mix, mul, plane, scene, smoothstep, strand, sub, toon, vec3, type F } from "../src/index.ts";

const LACQUER = vec3(0.55, 0.06, 0.05);
const bar = strand({
  name: "bar",
  spine: (m: F) => vec3(mul(sub(m, 0.5), 3), 0.6, 0),
  girth: 0.6,
  section: { aspect: 1, squareness: 8 }, // a rounded square: a box
  tips: { round: 0.08 },
  paint: (m: F) => {
    const wet = sub(1, smoothstep(0.08, 0.1, abs(sub(m, 0.5)))); // 1 on the middle fifth of the length
    return { base: mix(LACQUER, mul(LACQUER, 0.55), wet), gloss: mix(0.75, 0.97, wet) };
  },
});

export default scene({
  ground: plane,
  things: [bar],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [4, 3, 5], target: [0, 0.6, 0], fovY: 0.6 },
    stripe: { kind: "perspective", eye: [1.2, 1.6, 2], target: [0, 0.6, 0], fovY: 0.5 },
  },
});
```

Drawn in toon, the bar mirrors toon's sky, blurred where it is lacquer and sharp on the stripe, and its sun glint is
stronger. With `--style woodblock` the bar is printed in woodblock's inks, and a flat cream glint is left where it
is glossy. With `--style papercraft` it is card, and `loft` notes on stderr that `bar` gives `gloss`, which no part of
papercraft reads.

A stranger asked:

- What range does `gloss` take? The `loft context` line now says it: 0 is matte and 1 is a mirror.
- Does wet need its own channel? No. The comment above says why.

### A beetle whose shell turns colour with the view

```ts
// A beetle with a thin-film shell: magenta face on, green at a slant, blue at the rim. The style still lights it
// (bands, shadow, reflection); the film decides only the hue.
import { add, cos, div, dot, luminance, mul, plane, scene, sin, sqrt, strand, sub, toon, vec3, type F, type P3 } from "../src/index.ts";

/**
 * Two-beam interference in a film of index 1.56 and thickness `nm`, at 650, 530 and 450 nm: 0.5 + 0.5 cos(4 pi n d
 * cos(theta_t) / lambda), theta_t from Snell's law. The constant phase lost on reflection is dropped.
 */
function film(cosI: F, nm: F): P3 {
  const n = 1.56;
  const cosT = sqrt(sub(1, div(sub(1, mul(cosI, cosI)), n * n)));
  const k = (l: number) => (4 * Math.PI * n) / l;
  return add(mul(cos(mul(vec3(k(650), k(530), k(450)), mul(nm, cosT))), 0.5), 0.5);
}

const beetle = strand({
  name: "beetle",
  spine: (m: F) => vec3(mul(sub(m, 0.5), 2.2), 0.35, 0),
  girth: (m: F) => add(0.12, mul(sin(mul(m, Math.PI)), 0.55)),
  section: { aspect: 0.55 },
  paint: (m: F) => ({
    base: [0.08, 0.1, 0.06],
    gloss: 0.85,
    // The style's value (its light, shadow, reflections) times the film's hue; x2 because the film averages one half.
    shade: (ctx, styled) => mul(film(dot(ctx.normal, ctx.view), add(400, mul(m, 60))), mul(luminance(styled), 2)),
  }),
});

export default scene({
  ground: plane, things: [beetle], style: toon(), sun: [-0.4, 0.8, 0.45],
  cameras: {
    whole: { kind: "perspective", eye: [3.5, 2.5, 4], target: [0, 0.35, 0], fovY: 0.6 },
    shell: { kind: "perspective", eye: [0.8, 1.6, 1.4], target: [0, 0.4, 0], fovY: 0.5 },
  },
});
```

A stranger asked:

- Is `styled` the colour before or after the style sinks it under water? It is after. The doc comment on `shade`
  says `styled` is the style's whole colour for this fragment, as it would be drawn.
- Does the outline turn iridescent too? No. The same comment says the wrap never sees the outline.
- Can `m` be used inside `shade`? Yes: it is a closure, which TypeScript makes plain.

The film values were checked numerically (`out/materials/math.ts`). At 400 nm, cos θ = 1 gives (0.94, 0.19, 0.57),
which is magenta. 0.7 gives (0.37, 0.92, 0.01), green. 0.1 gives (0.00, 0.69, 0.83), blue.

### A poker in a forge: a word nobody else knows

```ts
// A poker whose tip glows by Planck's law from its own temperature, which pulses with the bellows every 3 s. The
// temperature is a channel this file invents (`kelvin`), and the style below is the only thing that reads it.
import { add, div, exp, mul, plane, scene, sin, strand, sub, time, toon, vec3, TAU, type F, type P3, type ShadeCtx, type Style } from "../src/index.ts";

/** Planck's law at 650, 530 and 450 nm: a body's light at `kelvin`, relative to green at 1300 K, compressed below 1. */
function planck(kelvin: F): P3 {
  const at = (nm: number): F => {
    const l = nm * 1e-9, g = 530e-9, c2 = 0.014388; // metres, metres, metre kelvins
    return div((g / l) ** 5 * (Math.exp(c2 / (g * 1300)) - 1), sub(exp(div(c2 / l, kelvin)), 1));
  };
  const b = vec3(at(650), at(530), at(450));
  return div(b, add(b, 1));
}

const poker = strand({
  name: "poker",
  spine: (m: F) => vec3(mul(m, 2.5), add(0.3, mul(m, 0.4)), 0),
  girth: 0.04,
  paint: (m: F) => ({
    base: [0.18, 0.18, 0.2],
    // 1250 K at the tip, falling off over the last eighth; the bellows add up to 120 K every 3 s.
    kelvin: add(300, mul(add(950, mul(sin(mul(time, TAU / 3)), 120)), exp(mul(sub(m, 1), 8)))),
  }),
});

const lit = toon();
/** Toon, plus a body's own light from its temperature: `kelvin`, 300 K (no visible light) where a paint gives none. */
const forge: Style = { ...lit, name: "forge", shade: (ctx: ShadeCtx) => add(lit.shade(ctx), planck(ctx.channel("kelvin", 300))) };

export default scene({
  ground: plane, things: [poker], style: forge, sun: [-0.4, 0.8, 0.45], period: 3,
  cameras: {
    whole: { kind: "perspective", eye: [3, 2, 4], target: [1.2, 0.6, 0], fovY: 0.6 },
    tip: { kind: "perspective", eye: [3, 1.2, 1], target: [2.4, 0.7, 0], fovY: 0.4 },
  },
});
```

With `--style woodblock`, the poker is grey iron and `loft` notes that `poker` gives `kelvin`, which no part of
woodblock reads. That note is the honest answer. The poker would glow in every built-in style that reads `emits` if
the paint also gave `emits: planck(T)`. A word you invent works where you wrote a reader for it.

The numbers check out (`out/materials/math.ts`). Relative to green at 1300 K, a 300 K body gives 2e-25 in red, which
is no light. 1100 K gives (0.048, 0.0014, 0.00004), a deep red. 1300 K is bright orange. At 300 K the GPU's f32 `exp`
overflows to infinity, and the division then gives exactly 0.

A stranger asked:

- Can a style I write read a channel? Yes. `ctx.channel` belongs to every shade context, in `shade`, `ink` and `mark`.
- What if two things give `kelvin` with different types? The scene is refused, naming both things (§4).

## 2. What exists

Each item below was checked in the code at `a3c7612`.

- **Paint is a colour function.** `PaintSpec = Rgb | Paint` (`src/geom/strand.ts:16`).
  `Paint = (u, v, at: PaintAt) => P3` (`src/map/types.ts:117`), where `PaintAt` is `{ world, normal, copy }`. Other
  drawables have their own paint signatures, each returning `P3`: `draw.field` takes `(hit: FieldHit)`
  (`src/draw/field.ts:73`), `draw.relief` takes `(at: ReliefPoint)` (`relief.ts:37`), `draw.copies` takes
  `(hit: CopiesHit)` (`copies.ts:47`), and impostors take `(hit: ImpostorHit)` (`impostors.ts:41`).
- **One style answers light for every thing.** `Style.shade(ctx: ShadeCtx)` (`src/style.ts:144`). `ShadeCtx` holds
  `base, normal, sun, view, world, depth, uv, anchor, id` (`style.ts:11`). `styleWgsl` emits `shade`, `ink` and `mark`
  **once**, as shared WGSL functions over the free variables `c_base … c_id` (`src/compile/scene_wgsl.ts:19-56`). A
  style with a printed shadow (`Style.block`) gives the bodies `block.body(ctx)` instead, and `src/draw/light.ts`
  prints the block afterwards from the tone carried in the picture's alpha (`mark`).
- **The shared functions are called from seven fragment stages**, all with the same argument list:
  - `src/draw/surface.ts:322, 329, 420-421, 431`: the border, none, hull and screen outlines.
  - `src/draw/trace.ts:239-252`: `trace_shade` and `trace_ink`, used by `field.ts:520, 631`, `impostors.ts:297` and
    `copies.ts:210`.
  - `src/draw/relief.ts:164-165`.

  A lit surface computes its paint in its own module as `paint_<name>(u, v, p_world, p_normal, p_copy) -> vec3`
  (`surface.ts:305`). The smear is the one place that inlines `style.shade` per drawable
  (`src/spacetime/smear.ts:61`), which shows that a per-thing copy of the style's shading already works.
- **The shared WGSL goes into every module.** Each module's chunks are prelude, variant, `sceneWgsl`, then its own
  code (`src/host/lower.ts:332-340`). On the crab (`examples/bosses/reefback_titan.ts`, 73 things, 74 modules),
  `sceneWgsl` is 31,025 bytes and the modules' own code is 1,452,812 bytes. The compiler is therefore given
  74 × 31 KB + 1.45 MB = 3.75 MB of WGSL. The shared `shade` alone is 1,309 bytes in the crab's woodblock. Across the
  six styles it runs from 1,622 (neon) to 6,984 (sketch). (`out/materials/sizes.ts`.)
- **Gloss and glow exist today, but only as style-wide constants or guesses.**
  - Toon prints one glint of strength 0.16 at N·H > 0.9785 on everything (`src/style/toon.ts:80-82`).
  - Woodblock's recipes can print a cream `highlight` shoulder, the same on everything (`src/style/light.ts:74, 418`).
  - Biolume decides what glows by guessing from the paint's brightness and saturation (`glowOf`, `biolume.ts:27`).
    Neon turns every paint into a tube light (`tubeOf`, `neon.ts:21`).

  So a thing cannot say "I am glossy here" or "this glows and that does not".
- **`draw.glazed` sets its own colour and alpha.** It is blended, writes no depth, and its phase is `water`,
  `on-water` or `transparent` (`surface.ts:54, 446`). Its `Fragment` carries `style` and `sun`, so a glazed colour
  can already call the style itself, as the smear does.
- **Reflection.** The `reflection` post pass is in `src/style/post.ts`. For a pixel of open water (`sty_water`), it
  gathers the colour of bodies above it in the picture, over 24 steps of 4 px up a wavering column. It is right only
  for a camera that looks across the water. Bodies mirror nothing. `Style.sky(ctx: SkyCtx)` is a closed-form function
  of direction (`src/style/sky.ts`), and nothing else ever evaluates it at any other direction.
- **Constant folding without identities.** Constant subgraphs are folded when code is emitted
  (`fold`, `src/expr.ts:1208`; lowering, `src/compile/rates.ts:7`). `mul(x, 0)`, `add(x, 0)` and `mix(a, b, 0)` with
  a non-constant `x` or `b` are kept and computed.
- **On the CPU, only biolume's body shading evaluates in full.** `compileJs` refuses the other five: toon, woodblock,
  sketch and neon reach `loft_fwidth`, and papercraft reaches `pc_facet`, all GPU-only (`out/materials/cpu.ts`).

## 3. Options

| Family | What a user writes | What it cannot express | Cost to the library |
|---|---|---|---|
| Special types | `material: metal({ roughness: 0.2 })` | Anything not on the menu: a thin film, temper colours, a retroreflector. A new idea is a new class. | Every class times every style (six today). Context grows by a line per class. Breaks rule 4. |
| Parameters only | `paint` returns `{ colour, gloss, metal, emission }`, a fixed record | Any word outside the record. A style part cannot be taught one. | Small, but each new word is a library change and a release. |
| Functions only | `shade: (ctx) => colour` on the thing, replacing the style | Nothing on the GPU. But `--style` and `--light` no longer reach the thing, and no style can translate it. | Small. Breaks rule 6 for the scene's look. |
| Shaders only | Raw WGSL (`wgslFn`) in the paint or a shade | CPU checks, restyling, composition with the style | Nothing: it exists today, as the bottom level. |
| A style per thing | `withStyleOn(bar, chrome())` | A look that follows `--style`. Ink, outline and the alpha mark disagree with the scene style's post passes. | Two styles in one picture. Every post pass must cope with both. |
| Style slots (the TSL shape) | The style names slots (`specular`, `emissive`) and a thing overrides one | Any slot the style author did not name. Each style has different slots, so an override is tied to one style. | Every style must publish slots. That is a menu, owned by each style. |
| **Channels and a wrap, refined** | Paint returns `{ base, gloss: 0.8, kelvin: …, shade: (ctx, styled) => … }` | Nothing a function can write. A thing that ignores `styled` gives up restyling, by its own visible choice. | One type, two context members, one part, about 230 lines. |

Why the last row wins:

- **Channels are the parameters-only option with the menu removed.** The vocabulary belongs to the author, and each
  style honours the words it knows.
- **The wrap is the functions-only option made to compose.** It receives the style's answer, so `--style` still
  reaches the thing. A wrap that ignores its input is reported (§4), so losing restyling is never silent.
- **Shaders-only stays as the bottom level.** It is unchanged.
- **Slots are rejected** because the style author would choose the menu.
- **A style per thing is rejected** because it splits the one thing a style owns: the post passes and the alpha
  mark agree with exactly one style.

Inherited and new. Inherited: Schlick's Fresnel term, the idea of a material channel, and sampling an environment in
the reflected direction. New here:

- The vocabulary is open, and each style translates words it knows into its own medium. Woodblock's gloss is a flat
  cream shape, not a highlight.
- Defaults cost nothing by algebra.
- A reflection is the closed-form sky evaluated exactly, with no environment map to bake and no resolution.
- The wrap closes over body coordinates.
- An unread word is reported as data.

## 4. The recommendation

### Types

```ts
// src/map/types.ts
/**
 * Surface coordinates -> what a thing says about its surface: a colour, or a colour with channels and a wrap.
 * Lit by the scene's style in the fragment stage.
 */
export type Paint = (u: F, v: F, at: PaintAt) => P3 | Painted;

/**
 * A colour with more to say.
 * - `base`: the colour, as a bare paint returns it.
 * - Every other key except `shade` is a channel: a number or vector under a name you choose. A style reads it with
 *   `ctx.channel(name, otherwise)` if it knows the name. The built-in styles read `gloss` (0 matte to 1 mirror), `metal`
 *   (0 or 1) and `emits` (a colour), each listed in its style's comment. A name no part of the scene's style reads is
 *   reported (`loft` prints a note; the artefact lists it), not refused, so restyling never breaks a scene.
 * - `shade`: this thing's own answer, given the style's. `styled` is the style's whole colour for this fragment as it
 *   would be drawn (lit, reflected, sunk under water); the outline never reaches it. Return a colour. A wrap that does
 *   not read `styled` no longer follows `--style`, and `loft` notes that too.
 */
export type Painted = {
  readonly base: In<"vec3">;
  readonly shade?: (ctx: ShadeCtx, styled: P3) => P3;
  readonly [channel: string]: In<NumTy> | ((ctx: ShadeCtx, styled: P3) => P3) | undefined;
};
```

Every paint signature returns `P3 | Painted`: `Paint`, `FieldOptions.paint`, `ReliefOptions.paint`,
`CopiesOptions.paint`, the impostors' paint, the paint given to `strand` and `spiralTube`, and the paint of
`draw.mesh` from the geometry proposal. The type annotations in `PaintSpec` and `PaintAt` are unchanged.

```ts
// src/style.ts, ShadeCtx gains two members
  /**
   * The channel `name` as this thing's paint gave it, else `otherwise`. The channel's type is `otherwise`'s: a paint
   * giving another type is refused, naming the thing. Read it where the style decides what a word means; a part takes
   * the value as an argument instead of reading by name.
   */
  channel<T extends NumTy>(name: string, otherwise: In<T>): Expr<T>;
  /** The style's own sky along a direction (its `sky` with this sun): what a body reflects. */
  sky(dir: Expr<"vec3">): Expr<"vec3">;
```

`shade`, `ink` and `mark` all receive the same `ShadeCtx`, so all three can read channels. Neon's `ink` reads
`emits`. The light proposal adds `lights` to the same type, and a wrap sees it without further change.

### How a channel travels

The paint is classified once, at the boundary. A new file, `src/draw/painted.ts` (about 60 lines), holds:

```ts
type Answer = { readonly kind: "styled" } | { readonly kind: "wrapped"; readonly wrap: (ctx: ShadeCtx, styled: P3) => P3 };
type Classified = { readonly base: P3; readonly channels: ReadonlyMap<string, Expr<NumTy>>; readonly answer: Answer };
/** A paint's result, classified once: an Expr is a bare colour; an object is `base`, channels and `shade`. Refuses a non-numeric channel. */
export function classify(thing: string, x: P3 | Painted): Classified;
/**
 * The functions a lit fragment calls for this thing: the shared `shade`/`ink`/`mark` when it gives no channel the
 * style reads and no wrap (byte-identical to today); otherwise its own copies, emitted into its own module.
 */
export function lookFns(ctx: CompileContext, thing: string, c: Classified, paintArgs: FnSpec["args"]): { readonly shader: Shader; readonly shade: string; readonly ink: string; readonly mark: string };
```

1. **Reading the style once.** `styleWgsl` already calls `style.shade(shadeCtx)` once. That context gets a recording
   `channel`, which returns `lift(otherwise)` and records `(name, type)`. The result is the set of names the style
   reads, and the shared `shade` is exactly today's text plus whatever the defaults fold to (step 4). A style that
   reads one name with two types is refused: `style "forge" reads "kelvin" as f32 and as vec3`.
2. **Per thing.** Each drawable's fragment stage calls `classify` on its paint's result. If the thing gives a channel
   in the read set, or has a wrap, `lookFns` emits `look_<thing>`, `ink_<thing>` and `mark_<thing>` into that thing's
   module. Their arguments are the shared shade arguments plus the drawable's paint arguments. Each is
   `style.shade(ctx)` evaluated with a `channel` that returns the paint's expression, followed by `wrap(ctx, ·)` in
   the case of `shade`. A channel given with a different type than the style reads is refused (exit 4):
   `bar: paint gives "gloss" as vec3; toon reads it as f32. Fix: give "gloss" as one number (toon's comment says what
   it means).`
3. **The seven call sites** change from the literal `shade(…)` to `${look.shade}(…)`, and likewise for `ink` and
   `mark`. `trace_shade` and `trace_ink` take the function names. `smear.ts` and the bake (glTF takes `base`) call
   `classify` too. A flat look (`draw.curve`) has no style to wrap, so a wrap there is refused:
   `draw.curve "x": its paint is the final colour; there is nothing to wrap. Fix: return the colour, or draw a lit
   surface.` `readsBody` also asks whether a wrap reads `uv`, `anchor` or `id`.
4. **Exact identities in the constructors** (`src/expr.ts`, about 12 lines). They are what makes a missing channel
   free:
   - `mix(a, b, t)` returns `a` when `t` is the constant 0, and `b` when it is the constant 1.
   - `mul` by the constant 0 returns the zero of the result's type.
   - `add` of the constant 0 returns the other operand, splatted to the result type.

   Each rule is exact for finite operands: a·1 + b·0 = a, x + 0 = x and x·0 = 0. They differ from the GPU only by
   giving a value where a NaN or infinite operand would have given NaN. The rewritten graph feeds both backends, so
   the CPU and GPU agree. A default of 0 therefore removes a style's whole term, sky taps included, before anything
   is emitted. Today's shared shades contain one node each that these rules rewrite: an `add` of 0 in toon and in
   woodblock, and a `mix` with t = 0 in sketch (`out/materials/ident.ts`).
5. **Reporting.** The artefact gains `unread: Record<string, readonly string[]>` (thing to channel names no part of
   the style reads) and `ignoresStyle: readonly string[]` (things whose wrap never reads `styled`, found by checking
   whether that variable is among the wrap's leaves). `render`, `frames`, `clip` and `check` print one stderr line per
   entry, before any final `loft:` line: `note: bar gives "gloss", which no part of papercraft reads`. Exit codes are
   unchanged.

   **A note, not a refusal.** A refusal would make restyling fail on any scene that says more than one style
   understands, and the whole point of channels is that a scene may. Silence would hide a misspelt `glos`. The note
   and the artefact entry make the drop visible to a person and measurable by a test.

### What the wrap may see, and what it cannot break

- **It sees** the full `ShadeCtx` (channels, `sky`, the light proposal's `lights`), the style's colour, and through
  its closure the paint's own `u`, `v` and `at`.
- **It cannot reach the outline.** The hull's `INK` branch returns `ink_<thing>`, the style's ink with the thing's
  channels. The wrap is applied only on the shade branch.
- **It cannot reach the alpha mark.** The mark is the style's, so post passes that read it (woodblock's line weight,
  the printed block) keep working.
- **It cannot block restyling.** `--style` and `--light` rebuild every `look_<thing>` from the new style, and the wrap
  receives the new style's colour.
- **The shadow block is printed over the wrap.** Under a style with a printed block (`Style.block`), the per-thing
  copy is `wrap(ctx, block.body(ctx))`, and the block is then printed over it from the style's mark. A glowing wrap
  on the shadow side therefore gets the block. This is a stated limit (§6).
- **Post passes see the wrapped colour like any other**, so bloom finds a bright wrap and `reflection` smears it
  down the water.

### Reflection, at three depths

1. **The sky and sun in a body: a part, `glossed`.** It is written out below.
   - It is exact because the sky is a closed-form function. Evaluating it at the reflected direction needs no cube
     map and has no resolution limit.
   - The sun is reflected wherever the style's sky draws one (a `skyOf` disc). Where it does not (toon's sky has no
     disc), the style's own glint stays the sun's highlight, scaled by `gloss`.
   - The blur is four taps on a cone around the reflected ray, with half-angle 0.02 + 0.5 (1 - gloss)² radians.
     Evaluating a closed-form sky at a few directions is the closed-form answer to an environment map's mip chain.
2. **The ground or water mirroring bodies.** This exists as the `reflection` post pass, for water only. It should be
   made general for a plane ground, but not as part of this proposal. The general law is a march of the reflected
   view ray in screen space against `scene_position` (inherited: screen-space reflection), at the 24 steps it uses
   today. That is about today's cost, and it would be right for any camera instead of only one looking across the
   water. It still sees only what is on screen. The exact alternative is a second, mirrored draw of the bodies, about
   +1.5 ms on the crab (the bodies' share of its draws), and belongs to whoever builds reflective grounds. Bodies
   mirroring each other through a post pass would need a gloss render target (+4 MB at 1280x800) and would still
   only see what is on screen, so I reject it.
3. **One object mirrored in another.** This belongs to the geometry-and-rays proposal. It needs a tracer that returns
   the colour seen along a ray. Every body already carries capsules (the crab has 574 joints), but brute force
   against 574 capsules is about 14,000 operations per glossy fragment, so it needs per-tile lists or a hierarchy.
   The join is one option: `glossed` takes `seen: (dir) => colour`, which defaults to `ctx.sky`. When the tracer
   lands, a style passes `seen: (d) => trace(ctx.world, d)` and nothing else changes.

```ts
// src/style/gloss.ts (about 45 lines with its comment)
//
// A body reflecting the scene's sky: four taps of the sky round the reflected view ray, mixed over the colour by a
// Fresnel weight.
//
// The law: r = 2 (n.v) n - v. seen = the mean of sky(normalize(r +/- a t1)) and sky(normalize(r +/- a t2)), t1 and t2
// unit and perpendicular to r, a = 0.02 + 0.5 (1 - gloss)^2 radians. F = f0 + (1 - f0)(1 - n.v)^5 with
// f0 = mix(0.04, 1, metal) (Schlick). The result is mix(colour, seen * mix(1, base, metal), mix(gloss, 1, metal) F):
// a dielectric reflects white and weakly face on, a metal reflects in its own colour and fully. With gloss and metal
// the constant 0, the weight is the constant 0 and the colour comes back unchanged at no cost.
//
// Parameters: gloss and metal are expressions, 0 to 1 (a style passes ctx.channel("gloss", 0)); `seen` is what is
// reflected along a direction, the style's sky by default.
//
// Cannot: reflect the ground as itself (a ray below the horizon sees the sky's own colour below the horizon), other
// bodies, or the body itself (a cavity reflects open sky); a blur wider than four taps can hold (near gloss 0 the four
// taps show as faint ghosts of a sharp sky feature); antialias a sharp feature reflected by a curved body near gloss 1
// (a sun disc sparkles: keep gloss under 0.95, or give the sky no sharp feature). It runs on the CPU: no derivative.
//
// Going past it: pass `seen` (a tracer, a second sky, a painted studio of softboxes as a function of direction); take
// more taps for a wider blur; colour the Fresnel term per wavelength (a thin film, as the beetle does in a wrap); an
// anisotropic cone stretched along the body's own m (brushed metal: the cone's t1 along dP/dm). Relatives: a
// reflection quantised to two inks (a print of chrome); a reflection of the sky's gradient only, without its features.
export function glossed(colour: V, ctx: ShadeCtx, o: { readonly gloss: In<"f32">; readonly metal?: In<"f32">; readonly seen?: (dir: V) => V }): V {
  const gloss = lift<"f32">(o.gloss), metal = lift<"f32">(o.metal ?? 0), seen = o.seen ?? ctx.sky;
  const r = sub(mul(ctx.normal, mul(dot(ctx.normal, ctx.view), 2)), ctx.view);
  const side = select(lt(abs(r.y), 0.9), vec3(0, 1, 0), vec3(1, 0, 0));
  const t1 = normalize(cross(r, side)), t2 = cross(r, t1);
  const rough = sub(1, gloss);
  const a = add(0.02, mul(mul(rough, rough), 0.5));
  const tap = (t: V, s: number) => seen(normalize(add(r, mul(t, mul(a, s)))));
  const sky = mul(add(add(tap(t1, 1), tap(t1, -1)), add(tap(t2, 1), tap(t2, -1))), 0.25);
  const f0 = mix(0.04, 1, metal);
  const fresnel = add(f0, mul(sub(1, f0), pow(sub(1, saturate(dot(ctx.normal, ctx.view))), 5)));
  return mix(colour, mul(sky, mix(vec3(1, 1, 1), ctx.base, metal)), mul(mix(gloss, 1, metal), fresnel));
}
```

### The six built-in styles on day one

They read three words between them: `gloss`, `metal` and `emits`.

| Style | `gloss` (f32, default 0) | `metal` (f32, default 0) | `emits` (vec3) |
|---|---|---|---|
| toon | `glossed` over its colour; glint strength 0.16 + 0.5 gloss | passed to `glossed` | added after the bands, before the water sinks it; default (0, 0, 0) |
| woodblock | a flat cream shape (cone of 6° about the half vector) at 0.9 × gloss of the cream ink: paper left for a light | not read | not read |
| biolume | not read | not read | **is** the glow; default `glowOf(base)`, today's guess, so the same node |
| neon | not read | not read | **is** the tube's light in `shade` and `ink`; default `tubeOf(base)`, the same node |
| papercraft | not read: card has no gloss | not read | not read |
| sketch | not read: graphite on paper | not read | not read |

Each style's top comment gains one line naming what it reads and what it does with each word.

For biolume and neon the channel turns today's guess into a statement. With no `emits`, the style guesses as it does
now. With `emits` given, the paint decides, and `emits: [0, 0, 0]` means nothing here glows.

Toon's three changes are written so that the defaults fold back to today's nodes:

- `add(0.16, mul(gloss, 0.5))` with gloss the constant 0 becomes the interned constant 0.16 by the `mul` and `add`
  identities.
- `glossed(…)` with gloss and metal 0 returns its colour.
- `add(c, emits)` with (0, 0, 0) returns `c`.

### What changes, and what stays the same

- **Scenes:** no source changes. A paint that returns a colour is classified as a bare colour.
- **Styles written outside the library:** unchanged. They read no channel, and `ShadeCtx` only gains members.
- **Shaders:** every scene's WGSL in biolume, neon and papercraft is byte-identical. Toon, woodblock and sketch lose
  one identity node each in their shared shade (step 4 above), and the channel terms then fold away.
- **Pictures:** the 17 `rgba` hashes in `docs/styles.md` are predicted identical (§7).

### Translucency and `draw.glazed`

**A channel does not replace `draw.glazed`.** Translucency is a different draw, not a different response to light.
It changes pipeline state (blending, no depth write) and the phase in which the thing is drawn. If an `alpha` channel
switched the drawable's pipeline, the compiler would be dispatching on a reserved word inside the author's vocabulary.
That is the menu again, and a sum type over looks (rule 4).

What a see-through lit thing needs exists already. A glazed colour receives `Fragment.style` and can call
`style.shade` with the paint's base, as `smear.ts` does. The one improvement worth making is to classify a glazed
colour's `style.shade` call too, so a glazed thing's channels reach the style. That falls out of `classify` and adds
no name.

### Cost

Measured with throwaway prototypes under `out/materials/` (not committed). Numbers are from the crab in its `game`
view at 1280x800, with alternating rounds in one session on a machine at load average 4 to 7.

| Case | Shader bytes | Frame time |
|---|---|---|
| A channel a paint gives that no style reads | 0: its expression never reaches a root | 0 |
| A channel a style reads that no paint gives (default 0, or today's guess) | 0: the term folds away (step 4) | 0 |
| A channel given and read, per thing | That thing's module gets its own `shade`, `ink` and `mark`: toon 2.5 KB, toon with `glossed` 7.0 KB, sketch 7.0 KB. The shared `sceneWgsl` is unchanged. | The style's term on that thing's fragments only |
| `gloss` on **every** body of the crab, toon with a 4-tap `glossed` (worst case) | +4.5 KB per copy (2.5 → 7.0 KB `shade`) | **+0.13 ms**: draws 1.51 → 1.64 ms in the fast clock state; +0.40 ms in the slow state (4.06 → 4.46), the same ratio of about 9% |
| `emits` in toon or neon, per fragment | about 40 bytes | one add: below the timer's 0.07 ms tick |
| woodblock's gloss shape | about 300 bytes | one cone test: below the tick |

The copies go into each thing's own module and not into the shared code because the shared code is compiled 74 times.
Putting 4.5 KB there would cost 333 KB of compiled WGSL on the crab. One glossy thing's own copy costs 7 KB, which is
0.19% of the 3.75 MB the compiler sees.

## 5. The line in `loft context`

**In the opening paragraph**, after "paint is a function of them.", add (57 words):

> Paint may also return `{ base, gloss: 0.8, shade: (ctx, styled) => colour }`: `base` is the colour; any other name is
> a channel a style reads if it knows it (built-ins: `gloss` 0 matte to 1 mirror, `metal` 0 or 1, `emits` a colour;
> unread names are noted); `shade` changes the style's answer for that thing.

**In the same paragraph**, "bodies mirror nothing" becomes "bodies mirror the sky by their `gloss`, not each other"
(+6 words).

**In the craft, under "Range in style"**, add (23 words):

> Say what a thing is with a channel before you wrap the style; a wrap that ignores `styled` drops out of every
> restyle.

**In the API map**, one generated entry from the doc comment (about 18 words):

> `glossed(colour, ctx, { gloss, metal?, seen? })`: the sky mirrored in a body, blurred as gloss falls, Fresnel-weighted.

That is about 105 words in all, under 5% of the 2,200.

## 6. Limits and relatives

These are written as the four parts a component's comment gives. They describe the mechanism; `glossed` has its own
comment, above.

**The law.**
- A paint says what a thing is (`base` and named channels) and, optionally, how its answer differs (`shade`).
- The style decides what each word means in its medium. Words it does not know are dropped and reported.
- A missing word is the style's default, and costs nothing when that default zeroes a term.

**Parameters.**
- Channel names are any string except `base` and `shade`.
- Values are numbers or vectors (f32 up to vec4). The style's default fixes the type.
- The wrap is a function of the shade context and the style's colour.

**Cannot.**
- **Per channel:** a channel cannot be a function, a texture or a list. It cannot change pipeline state, so a
  channel cannot make a thing translucent, unlit or uncast.
- **Per style:** a channel means only what each style makes of it. Two styles may disagree, by design.
- **The wrap's reach:** a wrap cannot change the outline, the alpha mark, the printed block or a cast shadow. Under a
  style with a printed shadow block, a glowing wrap on the shadow side is darkened by the block, and a thing's glow is
  darkened by a shadow another thing casts on it.
- **The ground and the sea** have no paint, so they have no channels. Their gloss is their style's.
- **Reflections:** no body reflects another body or itself (depth 3 belongs to the geometry-and-rays proposal).
- **The CPU:** checks reach a style's shade only where that style uses no GPU-only function. Today that is biolume
  alone. Channels, wraps and `glossed` are pure expressions and always evaluate on the CPU.

**Going past it.**
- Write a style part that reads your own word. The forge above does.
- Wrap the style with `shade`.
- Draw a translucent thing with `draw.glazed` and call `style.shade` yourself.
- Write raw WGSL with `wgslFn`.

**Relatives worth trying:**
- A `rough` word that is the reciprocal of `gloss` but drives the woodblock highlight's size, not its amount.
- An anisotropic `glossed` along dP/dm, for brushed metal.
- `seen` as a studio of softboxes: a function of direction that no sky is.
- A channel read by a post pass, carried in the alpha mark where a style has no other use for it.
- A `--style` that prints only the channels: a debug view of what every thing says.

## 7. Predictions a build will be scored against

| | Predicted |
|---|---|
| Pictures of today's scenes, after stage 1 and after stage 4 | The 17 `rgba` hashes in `docs/styles.md` are identical. If one moves, it will be because a dropped `add 0` let the GPU compiler fuse a multiply-add, changing a few pixels by one level. The fallback is to apply the identities only inside the emitted style functions (3 lines). |
| WGSL of today's scenes | Byte-identical in biolume, neon and papercraft. One line shorter in the shared shade of toon, woodblock and sketch. |
| Frame time, crab `game` 1280x800, no channels | Unchanged (within the 0.07 ms timer tick, alternating rounds). |
| Frame time, crab with every body glossy in toon | +0.13 ms ±0.05 in the fast state (prototype measured; the built part should match). |
| The bar, the beetle and the poker, at 1280x800 | Each under 0.5 ms a frame. Each `look_<thing>` copy is under 8 KB. |
| Load time | +0.2% per thing that gives a read channel, on the crab. 0 for a scene that gives none. |
| Lines of library code | About 230. The breakdown: types 15, `ShadeCtx` and recording 25, identities 12, `painted.ts` 60, seven call sites and the smear 45, artefact and CLI note 20, `glossed` 45, styles 20. |
| Exported names added | **2**: `glossed` and the type `Painted`. `ShadeCtx` gains 2 members (`channel`, `sky`). |
| `loft context` | +105 words (§5). |

## 8. How it meets the other three proposals

- **Light** (a light is a function of a world point; a scene carries a short list; things that glow provide one).
  - I need `ShadeCtx.lights`: the scene's lights, already evaluated at the fragment, each with a colour that is an
    expression. A style's glint and `glossed` can then answer each light, and a wrap sees them for free.
  - I give `emits` as how a surface looks, kept separate from how it lights others. A glowing thing provides a light
    through their mechanism, and the author writes one `const` for both. No link is drawn between `emits` and a
    light: a surface colour has no position or falloff.
- **Geometry and rays** (meshes made by code, author-written tracers, secondary rays against distance fields).
  - I need `draw.mesh`'s paint to return `P3 | Painted` through `classify`, and later a tracer, `(origin, dir) =>
    colour`.
  - I give the `seen` option of `glossed`, so object-in-object reflection is a style passing a tracer. The `gloss`
    channel also tells a tracer which fragments are worth a secondary ray.
- **State** (a step function baked into a table over the period).
  - Nothing is needed either way. Channels are expressions, so a channel can read a baked table: a stripe that dries
    over the period is `gloss: fetch(dryness, …)`.

## 9. Build plan

| Stage | Work | Days | Done when |
|---|---|---|---|
| 1. The bar draws | `Painted`, `classify`, `ShadeCtx.channel` and `.sky`, the identities, `lookFns` at the four `surface.ts` call sites, `glossed`, toon reading `gloss` | 2 | The bar renders in toon with a sharp sky on its stripe. The 17 hashes are identical. Crab frame time is unchanged. |
| 2. Every lit drawable | Field, relief, copies, impostors, `trace_*`, the smear, the bake; the type refusals and the flat-look refusal | 1 | A field and a relief give `gloss`. Each refusal has a test that names its bug. |
| 3. The wrap | The `shade` key, `ignoresStyle`, `readsBody` through wraps | 1 | The beetle renders in toon and woodblock and keeps its hue in both. |
| 4. Built-ins and the report | `metal` and `emits` in toon; woodblock's gloss shape; `emits` in biolume and neon; the artefact's `unread`; stderr notes; style comments; context lines and snippets | 1 | The poker gives notes in woodblock. The context snippets run. The hashes still hold. Frame times are scored against §7. |
| 5. Optional: ground mirror | `reflection` as a screen-space march, for a plane ground too | 2 | Waits for the owner's answer to question 5. |

Stages 1 to 4 take 5 days.

**Stop rule.**
- **Abandon the wrap and keep channels only** if, in a trial of 10 scenes written cold by a model given only
  `loft context`, more than half the wraps ignore `styled`. That would make the wrap the functions-only option in
  disguise. The `ignoresStyle` report makes this a count.
- **Abandon the design** if a second, unrelated author cannot write the three scenes above from the context lines
  without asking what a channel means more than once (rule 9).
- **Also abandon it** if the per-thing copies cost more than 5% of load time on the crab with 10 glossy things
  (predicted 1.9%).

## 10. Open questions for the owner

1. **Should an unread channel be a note or a refusal?**
   - **Recommendation:** a note, plus the artefact field. A refusal breaks restyling, which is the point of having
     channels at all.
2. **Three built-in words (`gloss`, `metal`, `emits`) or two?** Cutting `metal` would leave metals to a wrap.
   - **Recommendation:** three. "I am metal" was your own example, and only toon pays for it.
3. **May a wrap ignore `styled`?**
   - **Recommendation:** yes, with the note. Forbidding it would need a check that is easy to defeat (`mul(styled, 0)`),
     and the shaders-only level allows it anyway.
4. **May the three identities (`mix` at 0 or 1, `mul` by 0, `add` 0) go in the expression constructors for the whole
   library, not only for styles?**
   - **Recommendation:** yes, gated on the 17 hashes, with the style-only fallback in §7. They are exact, and they
     make "a default costs nothing" true of every part anyone writes.
5. **Make the ground's `reflection` general (a screen-space march on a plane ground) now, or after the rays proposal?**
   - **Recommendation:** after. Nothing in this design waits on it, and the exact version (a mirrored draw) may be
     what a reflective floor wants instead.

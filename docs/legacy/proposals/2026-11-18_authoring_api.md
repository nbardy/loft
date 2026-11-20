# Loft authoring API: the open concerns, clean solutions and a work plan (18 Nov 2026)

This is a design document; nothing in it has been built. It answers `agent_notes/2026-11-18_cook/API_BRIEF.md`. It was
written after reading the plan (`2026-11-18_next.md`), `src/expr.ts`, `src/compile/context.ts`, `src/lib/wgsl.ts`,
`src/check.ts`, `src/ranged.ts`, `src/geom/solve.ts`, the guide, and the diffs of the nine running lanes (spray, volume,
compute, dof, f16, pbrsky, small, pipecache, rustplan). The code below is the code a user would write. The library code
it implies comes after each example.

## 0. Decisions in one table

| # | Concern | Decision | New public names | Deleted or folded |
|---|---|---|---|---|
| 1 | Maths as nested calls | `wgsl` tagged template. A subset of WGSL is parsed into the existing graph, and functions outside the subset become named raw functions. The call form stays as the meta-language for building graphs in TypeScript. | `wgsl` | `wgslFn` |
| 4 | Primitives take shader code | No per-primitive API. A parsed WGSL function *is* a function of expressions, so every slot already accepts one. Struct returns fill record slots. Slots never pass `time`. | none | `t` parameter of `draw.volume` / `specks` slots |
| 3 | Uneven escape hatches | One ladder, one text form: graph function, then raw function, then raw drawable, then raw compute. Holes (`${}`) are the only way Loft values enter raw code, and the compiler generates accessors for them. | `draw.shader` | compute `reads`, `values`, and `entry` when there is a single entry; the hand-built `Drawable` boilerplate for scene authors |
| 2 | Hidden generated code | Every generated statement carries the site it came from. `loft show` prints a thing's modules. Tint errors in author text become `SceneError`s quoting the author's line. | `loft show` (CLI verb) | the misclassification of author WGSL errors as `EnvironmentError` (a bug today) |
| 5 | Three condition vocabularies | One vocabulary: conditions are boolean expressions, quantified by `always`/`eventually` over ranges. `solve` takes the same formulas. Domain checks become *measures* (expressions). The checks that cannot become expressions stay `sampled` and are labelled as such. | `clearance` (a measure) | `atLeast`, `When`, the `{at}`/`{from,to}` options of `equal`, `staysClear`/`stopsAt` as checks |
| 6 | Model tooling | Confirm `loft check --at … --explain`. It names the author's text lines and prints the command that renders the witness frame. | none | `measure`/`why`/`try` stay cut |
| 7 | Determinism and precision | One `Identity` record with `build`, `frame` and `device` parts and one hash. It appears in serve answers, the manifest, render output and verdicts. | none (a field) | the separate `tier`/`precision` fields of serve answers |
| 8 | Other traps | Two import paths, `design` boilerplate, the `draw.points` name clash, raw inputs read by slot number, and WGSL semantics that differ from Loft's (`round`, `%`). See §8. | none | `draw.points` (old instancing) folds into `draw.posed` |

## 1. Measure before deciding: the token claim does not hold

The brief says nested calls cost 2–3× the tokens of shader text. I tested this before designing anything. I took five
pieces of real Loft code and wrote each as WGSL text with the same meaning, then counted tokens with `o200k_base`
(tiktoken). That is a proxy, not Claude's tokenizer. Counting was a CPU-only probe. The text versions include the `.0`
on literals and the `struct` declarations WGSL needs.

| Code | Call form tokens | WGSL text tokens | Ratio |
|---|---:|---:|---:|
| The brief's own line (`sub(length(sub(vec3(abs(p.x)…`) | 34 | 34 | 1.00 |
| `spray_burst.ts` `flight` + grain slot | 468 | 481 | 0.97 |
| `spray_burst.ts` boot field (laces, cleats, `local`) | 479 | 515 | 0.93 |
| `volume_dust.ts` `folded` (a 4-step fold) | 224 | 213 | 1.05 |
| `src/draw/march.ts` sphere-trace loop (9-tuple `iterate` of `select`s) | 663 | 401 | 1.65 |

**Straight-line maths costs the same in both forms.** `mul(a, b)` and `a * b` tokenize to about the same count. The only
large gain is in **stateful loops**. There, call form forces every mutable variable into an `iterate` tuple and every
`if` into a `select` on every component. That costs 1.65× here, and the code is very hard to read.

The decision to author in text therefore rests on other grounds, each a capability the call form lacks:

1. **Statements.** Loops with mutable state, `break`, early `return` and `if` are written as a shader writer writes
   them. The march loop above is the extreme case, and every fold, iterated fractal, marcher and integrator is the same
   shape.
2. **Swizzles and precedence.** `x.yzx + 0.38 * x.zxy` has no call-form spelling short of `swizzle(x, "yzx")`, and
   `Expr` has getters only for `x y z w xy xz zw xyz`. A large share of nesting mistakes come from operator precedence
   written by hand.
3. **The model's prior.** Models have read much more GLSL/WGSL (Shadertoy, three.js shaders) than Loft. A trick such
   as domain repetition, a polynomial smooth-min or a fold transfers verbatim.
4. **One language from graph to raw.** Moving a function across the line between what checks can read and what they
   cannot means deleting one construct (for example, a texture read). Today it means a rewrite from `mul(...)` into a
   `wgslFn` string.
5. **Sites for free.** The parser knows the file and line of every node it builds. The call form would need a stack
   capture per construction (494k constructions for one crab), so it has no sites today. §2 and §6 depend on sites.

**Prediction, to be scored by the blind test (§10):** final scene tokens 0.92× overall (0.75× on loop-heavy briefs, 1.0×
on straight-line ones). Compile errors per sample 0.55×. Looks no worse. If those numbers do not appear, §10 says what
is kept anyway.

## 2. Concern 1: shader text as the way to write maths

### The problem, in real code

`src/draw/march.ts` is the library's own sphere trace:

```ts
const init = [t0, t0, f(0), f(0), t0, f(1e30), f(1e30), select(gt(t0, t1), f(2), f(0)), f(0)] as const;
const [t, prevT, prevDist, prevExact, nearT, , nearGap, done, hitDist] = iterate(o.steps, init, (s) => {
  const [t, prevT, prevDist, prevExact, nearT, nearPx, nearGap, done, hitDist] = s;
  const running = lt(done, 0.5);
  ...
  return [
    select(advance, tn, t), select(advance, t, prevT), select(advance, dist, prevDist), select(advance, f(1), prevExact),
    select(nearer, th, nearT), select(nearer, hPx, nearPx), select(nearer, h, nearGap),
    select(running, ended, done), select(and(running, hit), dist, hitDist),
  ] as const;
}, (s) => ne(s[7], 0));
```

A model asked for a marcher of its own writes this, or gives up and writes a `wgslFn`, which the checks cannot read.

### The user's code

A creature with motion: an eel that lunges over a rock to bite a mark at 1.2 s. Its body, paint and the motion's
constraints are written in text.

```ts
import { always, equal, plane, pbr, scene, solve, spline, strand, time, vec2, wgsl } from "loft";

const eel = wgsl`
fn spine(m: f32, head: vec2f) -> vec3f {          // m: 0 head .. 1 tail
  let wave = sin(6.0 * m - 5.0 * time) * 0.25 * m;
  return vec3(head.x - 3.0 * m, head.y * (1.0 - m) + 0.2 * m, wave);
}
fn girth(m: f32) -> f32 {
  return 0.24 * sqrt(4.0 * m * (1.0 - m)) * (1.0 - 0.5 * m) + 0.02;
}
struct Skin { base: vec3f, rough: f32 }
fn skin(m: f32, v: f32) -> Skin {
  let back = smoothstep(-0.3, -0.12, sin(v * 6.2832));
  let spot = step(0.8, noise3(vec3(m * 18.0, v * 4.0, 0.0)));
  let top = mix(vec3(0.1, 0.25, 0.2), vec3(0.9, 0.7, 0.2), spot);
  return Skin(mix(vec3(0.85, 0.8, 0.6), top, back), mix(0.6, 0.25, back));
}
fn clears(x: f32, y: f32) -> bool {               // the head stays off the rock
  return distance(vec2(x, y), vec2(1.3, 0.5)) >= 0.5;
}`;

const lunge = solve({
  over: [0, 2], vary: { x: spline(12), y: spline(12, 0.9) },
  hold: ({ x, y }) => [
    always([time, 0, 0.5], equal(vec2(x, y), [0, 0.4])),     // coiled
    always([time, 1.2, 1.2], equal(vec2(x, y), [2.6, 0.5])), // bites at 1.2 s
    eel.clears(x, y),                                        // the whole span
  ],
  prefer: { jerk: 1 },
});
const head = vec2(lunge.curves.x, lunge.curves.y);

export default scene({
  ground: plane, style: pbr(), sun: [-0.4, 0.8, 0.45],
  things: [strand({ name: "eel", spine: (m) => eel.spine(m, head), girth: eel.girth, paint: eel.skin })],
  cameras: { side: { kind: "perspective", eye: [1, 1.5, 8], target: [1, 0.5, 0], fovY: 0.6 } },
});
export const checks = lunge.holds;
```

The rules, read from that code:

- **Form.** One tagged template, `wgsl`. It returns a module: an object with one callable per `fn`, plus its structs. A
  module function takes expressions (or plain numbers and arrays) and returns an expression, or a record of
  expressions for a struct. **It is an ordinary function of expressions.** Its caller cannot tell it from a TypeScript
  one. `wgsl` also works as a plain call on a string, so text kept in a file loads as
  `wgsl(source)` with `import source from "./eel.wgsl" with { type: "text" }`. No second form is needed.
- **Lowering happens per call, not per module.** The text is parsed once, when `wgsl` runs. Each call lowers the
  function body with the actual argument expressions, the way a TypeScript function body runs. Because nodes are
  interned, the graph built from text is *the same graph* as the call-form version: same node ids, same WGSL output,
  same JS twin, same interval enclosures, same `gradient`. Nothing downstream changes. This is the property the lane's
  pass/fail check tests (§9 L1).
- **Names in scope.** These are, in order: the module's own `fn`s and `struct`s; holes; `time`; and the library's
  expression functions under their exported names (`noise3`, `hash22`, `sdRoundBox`, `smin`, `fold3`, …). The last
  group is the prelude, and `loft context` lists it. `gradient(h, p)` is valid in text (lowered through `src/grad.ts`),
  so a bump normal is a single line. It is not WGSL, so it is valid only in graph functions.
- **Arguments.** A slot may pass more arguments than a function declares. The function takes a prefix, as a
  JavaScript function does. So `fn skin(m: f32, v: f32)` fills a paint slot that passes `(u, v, at)`. Struct-typed
  parameters bind records. The prelude declares the structs that slots pass (`PaintAt`, `Ray`, `ShadeFields`), so
  `fn dent(u: f32, v: f32, at: PaintAt)` works.
- **Holes (`${}`) bring in Loft values the text cannot name.** These are params, host inputs, tables, compute outputs,
  solved curves, and TypeScript functions of expressions. A hole holding a number or an array becomes a constant. A
  hole holding a function or a handle is callable in the text (`${baked.sdf}(uv)`). A hole holding a string is refused:
  the template splices values, never text.

  ```ts
  const reach = param("reach", 1.2, { min: 0.8, max: 1.6 });
  const arm = wgsl`fn tip(m: f32) -> vec3f { return vec3(${reach} * m, 0.0, 0.0); }`;
  ```

  The first draft had a second way to declare params, WGSL `override reach: f32 = 1.2;`. It was cut: one way in, and a
  hole carries a range where `override` cannot.

### The subset (graph functions: twin, checks, `solve`, intervals, interpreter tier)

- **Types.** `f32`, `vec2f`/`vec3f`/`vec4f` (and the `vec3<f32>` spelling), `bool`, and structs of these. `mat2x2f`
  and `mat3x3f` are lowered at parse time to tuples of column vectors, so `m * v` becomes dot products and needs no new
  node type. Constant arrays may be indexed by any expression, which lowers to `pick`.
- **Literals.** Abstract numbers follow WGSL's rules. `1` in an f32 context is `1.0`, and `let n = 1; n * 0.5` is the
  WGSL error. The error message names the fix: write `1.0`.
- **Statements.** `let`, `const`, function-scope `var`, assignment, compound assignment, and swizzle reads and writes
  (`v.x = …` rebuilds the vector). `if`/`else` becomes `select` over SSA values. `for (var i = 0; i < N; i++)` with
  any `N` becomes `iterate`. The set of `var`s the body assigns becomes the loop state, `break` becomes the loop's
  `until`, and `continue` becomes a guard. An early `return` becomes a result variable and a done flag. `&&` and `||`
  evaluate both sides (the graph is pure, so the result is the same).
- **Operators and builtins.** Arithmetic, comparisons (vector comparisons with `all`/`any` are expanded into scalar
  `and`/`or` at parse time, so robustness margins stay exact), and the numeric builtins: `abs sign floor ceil fract
  trunc round sqrt inverseSqrt exp exp2 log log2 pow sin cos tan asin acos atan atan2 sinh cosh tanh min max clamp
  saturate mix step smoothstep length distance dot cross normalize reflect refract faceForward fma select radians
  degrees`.
- **WGSL text means WGSL semantics.** `round` is half-to-even, not Loft's `floor(x + 0.5)`. `%` is the truncated
  remainder, not Loft's floored `mod`. Each operator that differs gets its own graph op, so text never silently means
  something else. `fma` lowers to multiply-add in both backends, and the text says so.

**Outside the subset:** texture reads, `dpdx`/`fwidth`, atomics, pointers, storage and workgroup variables, `while` and
`loop` without a counter, `discard`, entry points, `f16`, and in phase 1 integer arithmetic and bit operations (u32 PCG
hashes). **A function that uses any of these is a raw function.** It is emitted verbatim, with holes replaced by
generated accessors, and graph code calls it as a GPU-only `LibFn`. The twin refuses it by name and line (`raw function
pcg at eel.ts:14: u32 bit operations`), and a ranged check over it returns `undecided` with that reason. The function
still renders. Phase 2 adds `u32`/`i32` graph types. The JS twin computes them with `Math.imul` and `>>> 0`. The
interval enclosure of a bit operation is the whole range of its type, which is sound and never false, though loose.

### Errors that point at the text

`wgsl` records the site of its own call once, with one `new Error().stack` per module, not per node. A template line
*k* is file line `site + k`, so every parse and lowering error prints `examples/eel.ts:9:22`, the line itself and a caret,
then the fix in the `SceneError` style: `unknown name "nois3"; did you mean noise3?`, `skin returns Skin, the paint slot
reads base: vec3f (given base: f32)`. A lowering error that depends on the call quotes both lines, the text line and the
TypeScript call that passed the bad argument. Text loaded from a file has no path. Its errors give the line within the
text and the site of the `wgsl(...)` call.

### Does the call form stay?

**Yes, as the meta-language.** TypeScript code builds graphs that are not one function. Examples: `.map` over legs,
station lists, generic library helpers over any vector type, `solve` specs, design numbers. The library itself stays
call form. `loft context` changes what it teaches. Maths inside a function is written in text, and structure is written
in TypeScript. Nothing in `expr.ts`'s public surface is deleted, because the parser lowers to those very constructors.
`wgslFn` is deleted: a raw function in a module replaces it, with the same GPU-only status, holes, and line errors.

### Cut after reading it cold

A stranger asked: "what is `eel`?", "how does `time` get in?", "how do I know which fields the paint slot reads?" and
"can I use my PCG hash?" The answers became rules above (a module of functions; `time` is in scope; slot-shape errors
list the fields; renders as raw until phase 2), not new names. Four things were cut: the `override` params; a typed
accessor `m.fn<"f32">("name")` (it adds a name, and lowering already checks types, with line numbers); a separate
`.wgsl` loader (`wgsl(text)` does it); and an `expr\`…\`` single-expression tag (a one-line `fn` is the same thing).

**Cost.** About 1,800–2,500 lines of TypeScript: tokenizer 250, recursive-descent parser 700, lowering and SSA 600,
about 18 new ops × 4 backends (WGSL emit, JS twin, interval enclosure, derivative) 400, and errors 150. That is 3–5
worker-days, mostly CPU, with one GPU test. The library's own `march`, `fold` and noise can move to text later, a file
at a time, if that reads better; nothing requires it.

**Migration.** None is required. Scenes are code, not persisted data. Artefacts are generated, and their format is
unchanged. Creative's allowlist gains `wgsl` and loses `wgslFn`.

**Risks.** (a) `if` becomes `select`, which computes both sides. An author who uses `if` to skip an expensive normal
pays for it. The `select` pass already behaves this way, so it is not a regression. The fix is branch-local emission
in `emitWgsl` (emit nodes used by only one side inside a real `if`), phase 2, measured. (b) Two ways to write maths
during the transition. `loft context` teaches one. (c) TypeScript stops checking the inside of the text, so mistakes
surface at `loft check`/`build` instead of `tsc`. Parse and lowering run at import, under 20 ms for a 300-line module
(target), so the loop stays short. (d) A new op without a sound enclosure is a refusal, never a guess.

## 3. Concern 4: primitives take shader code directly

**The general rule: a slot that takes a function of expressions takes a WGSL function, because a parsed WGSL function
is a function of expressions.** No primitive grows a text-specific option. Three more rules make the rule hold
everywhere:

1. **Record slots take struct functions.** The struct's field names are the record's keys. A missing required field
   or a field of the wrong type is an error that lists the slot's fields. Paint words are open: any field name is a
   channel, as any key is today.
2. **Slots never pass `time`.** `time` is in scope everywhere. Today `draw.field`'s field is `(p)`, `draw.volume`'s
   density is `(p, t)` and `specks`' grain is `(i, t)`, so a model must look up which. Passing the slot's own `time`
   adds nothing; an author who wants a warped clock calls their own `flight(i, 0.78 * time)`.
3. **Indices are f32** until phase 2 adds integers, because the graph's index has always been f32. Text writes
   `u32(i)` once phase 2 lands.

### The user's code: a dust burst (points and volume) and a PBR look

```ts
import { draw, pbr, plane, scene, wgsl } from "loft";

const fx = wgsl`
struct Flight { at: vec3f, fade: f32, h: f32 }
struct Grain { at: vec3f, radius: f32, colour: vec3f, opacity: f32 }
fn edge(u: f32) -> f32 { return smoothstep(0.0, 0.19, u * (1.0 - u)); }
fn flight(i: f32, t: f32) -> Flight {
  let h = hash22(vec2(i, 31.0));
  let k = hash22(vec2(i, 73.0));
  let age = max(0.0, t - 0.19 * k.x);
  let angle = mix(-1.25, 0.93, h.x);
  let reach = mix(0.7, 4.6, h.y) * (1.0 - exp(-2.2 * age));
  let height = mix(0.7, 4.3, k.y) * (1.0 - exp(-2.8 * age)) - 1.65 * age * age;
  let fade = edge(h.x) * edge(h.y) * smoothstep(0.0, 0.075, age) * (1.0 - smoothstep(0.72, 1.4, age));
  return Flight(vec3(cos(angle) * reach - 0.5, height + 0.06, sin(angle) * reach), fade, h.y);
}
fn grain(i: f32) -> Grain {
  let q = flight(i, time);
  let s = hash22(vec2(i, 111.0));
  let coarse = s.x * s.x * s.x;
  return Grain(q.at, mix(0.002, 0.012, coarse), mix(vec3(0.55, 0.18, 0.06), vec3(1.0, 0.56, 0.25), s.y),
               mix(0.18, 0.65, coarse) * q.fade);
}
fn dust(p: vec3f) -> f32 {                    // the cloud trails the grains
  let core = flight(7.0, 0.8 * time).at;
  let q = (p - core) / vec3(2.2, 1.2, 1.6);
  return 1.8 * (1.0 - smoothstep(0.5, 1.0, length(q))) * smoothstep(-0.1, 0.5, noise3(p * 1.7 - vec3(0.4 * time, 0.0, 0.0)));
}
fn film(v_h: f32, f0: vec3f) -> vec3f {       // a thin-film Fresnel for pbr's fresnel part
  let cos_t = sqrt(1.0 - (1.0 - v_h * v_h) / 2.4336);
  let phase = 4.0 * 3.14159265 * 1.56 * 380.0 * cos_t / vec3(650.0, 530.0, 450.0);
  return f0 + 0.25 * (1.0 + cos(phase));
}`;

const cloud = draw.volume("dust", fx.dust, { bounds: { min: [-2, 0, -5], max: [6, 5, 5] }, steps: 64, g: 0.6, albedo: [0.6, 0.25, 0.1] });
const grains = draw.points("grains", 100_000, fx.grain, { stretch: 0.004 });

export default scene({
  ground: plane, things: [cloud, grains], sun: [0.55, 0.38, -0.75],
  style: pbr({ fresnel: fx.film }),
  cameras: { game: { kind: "perspective", eye: [4.9, 3, 7.4], target: [0.5, 1.3, 0], fovY: 0.47 } },
});
```

Read cold, the code raised one question: "is `draw.points` the old instancer?" It is not, and §8 resolves the clash.
`draw.points` here is the spray lane's `specks` under the name the owner asked for, and the `(i)` slot no longer takes
`t`. Two limits are inherited from the running lanes and stated here, not hidden. The spray lane's `veil` reads a
`draw.density` medium's grid, so grains are not yet dimmed by a `draw.volume`; that is a follow-up in lane L5. The
volume lane's transparents need back-to-front order.

**Cost.** Lowering checks a returned struct against a slot's record type; about 100 lines in L1. The signature change
of the two running primitives (volume, specks) is under 50 lines plus their examples.

**Replaces.** The `t` argument of two slots. No primitive gains an option. **Risk.** A slot whose record has *function*
fields (paint's `scatter: (ctx, l) => …`) cannot be filled from a struct. Such fields stay TypeScript-only, or are given
as a module function: `{ ...fx.coat(m), scatter: fx.scatter }`.

## 4. Concern 3: one escape ladder

### The problem, in real code

Each escape today has its own contract. `wgslFn` takes a name string that must match the text. A raw `Drawable` needs
`name`, `skeletons: []`, `forms: []`, `draws: () => ({ shaders: { k: { code, libs: [] } }, draws: [drawSpec({...})] })`.
It reads params as `params[k]` and inputs as `inputs[k]`, where `k` is a slot index the author cannot know. The compute
lane's pass takes `reads`, `values` (only inputs or params, only in frame passes) and `entry`. Its own example
(`examples/compute_volume.ts`) raymarches the baked ring from a hard-coded ray, because the camera ray is not offered to
raw code. So its picture ignores the scene's camera, depth, ink and lighting.

### The ladder

| Rung | What the author writes | What they get | Honest limit |
|---|---|---|---|
| 1. Graph function | `fn` in the subset | Everything: GPU, twin, checks, `solve`, `gradient`, intervals, tier I, styles | the subset |
| 2. Raw function | `fn` outside the subset, in the same module | renders; callable from graph code; reads holes; calls graph functions | twin, checks, `solve` refuse by name; tier C |
| 3. Raw drawable | `@vertex fn vs` / `@fragment fn fs` in a module, given to `draw.shader` | own geometry and shading; Loft's camera, time, params, inputs, tables, compute outputs by name or hole; phases, depth, blend | unstyled; no `forms`, so checks cannot see it |
| 4. Raw compute | `@compute` entries, given to `compute` | build, load, frame or steps (the compute lane's sum type); outputs readable on every rung | GPU-only; frames stay pure; feedback only through `steps` |

**Calls in both directions.** Graph code calls a raw function as a GPU-only node. Raw code calls a graph function by name:
the compiler emits it as a standalone WGSL function, instantiating its body over variables of its declared parameter
types. Raw code reads `time` by name, the same word as in graph text; the compiler rewrites the identifier to
`frame.time.x`. Raw code reads everything else through holes. A hole in raw text becomes a generated zero-argument
accessor (`loft_hole_3()`) whose body is the hole's expression emitted by `emitWgsl`. So a raw function can read a param,
an input, a solved curve or any uniform expression without knowing a slot index. Two prelude functions are added because
rung 3 cannot work without them: `camera_ray(px: vec2f) -> Ray`, and `Ray` itself. The prelude already has
`view_dir`, `pixel_world`, `scene_depth`, `scene_position` and `frame`.

### The user's code: a compute bake read by an expression drawable

```ts
import { compute, draw, nothing, pbr, scene, wgsl } from "loft";

const shape = wgsl`
fn ring(p: vec3f) -> f32 {                        // graph: the twin can read this one
  let r = length(p.xz) - (0.51 + 0.07 * cos(5.0 * atan2(p.z, p.x)));
  return length(vec2(r, p.y + 0.1 * sin(3.0 * atan2(p.z, p.x)))) - 0.19;
}
@compute @workgroup_size(4, 4, 4)
fn fill(@builtin(global_invocation_id) id: vec3u) {          // raw: an entry point
  textureStore(sdf, id, vec4(ring(vec3f(id) / 47.0 * 2.0 - 1.0), 0.0, 0.0, 1.0));
}`;

const baked = compute("ring", {
  when: { kind: "build" },
  writes: { sdf: { kind: "texture", size: [48, 48, 48], format: "r32float" } },
  dispatch: [12, 12, 12],
  wgsl: shape,
});

const view = wgsl`fn field(p: vec3f) -> f32 { return ${baked.sdf}(p * 0.5 + 0.5).x; }`;

export default scene({
  ground: nothing, style: pbr(), sun: [-1, 2, -1],
  things: [draw.field("ring", view.field, { bounds: { kind: "box", min: [-1, -1, -1], max: [1, 1, 1] }, steps: 64, safety: 0.9 })],
  cameras: { front: { kind: "perspective", eye: [0, 2, 5], target: [0, 0, 0], fovY: 0.66 } },
});
```

Compared with today's example, the ring now gets the scene's camera, depth, outline and PBR lighting through
`draw.field`, and loses its hand-written marcher. `entry` is gone, since the module has one `@compute`; `steps` still
names `start` and `entry`. `reads` is gone, because the hole `${baked.sdf}` is the dependency and the tag records it.
`values` is gone, because a param is a hole. `draw.field`'s CPU Lipschitz check cannot read a texture, so it asks for
`safety` by name. That is the rung-2 limit stated where it bites. The baked texture and the source function differ, so a check reads
`shape.ring`, the source, which is still a graph function. The bake is for speed; the proof is about the source. A first
draft showed this with a check line over a made-up param. Read cold, a stranger asked what that param was, so the line
was cut and the idea kept in prose.

### Rung 3, for completeness

```ts
const sparks = draw.shader("sparks", wgsl`
struct Out { @builtin(position) clip: vec4f, @location(0) q: vec2f }
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> Out {
  let p = ${sim.state}(f32(i), 0.0).xyz;
  let c = array(vec2f(-1, -1), vec2f(1, -1), vec2f(-1, 1), vec2f(-1, 1), vec2f(1, -1), vec2f(1, 1))[v];
  let h = frame.view_proj * vec4(p, 1.0);
  return Out(vec4(h.xy + c * frame.viewport.zw * h.w * ${size}, h.zw), c);
}
@fragment fn fs(in: Out) -> @location(0) vec4f { return vec4(1.0, 0.7, 0.3, 1.0 - dot(in.q, in.q)); }`,
  { vertices: 6, instances: 2048, phase: "transparent", blend: "alpha" });
```

`draw.shader` takes a name, a module with `vs`/`fs` (fixed names, as today), and the draw's state (`drawSpec`'s fields,
with its defaults). It replaces 14 lines of `Drawable` boilerplate and the need to learn `Shader`, `Emitted` and
`drawSpec`. Library authors keep `Drawable`. For grains like these, rung 1 is better: `draw.points("sparks", 2048, (i) =>
({ at: sim.state(i, 0).xyz, … }))` is lit, veiled and depth-tested, and keeps sub-pixel grains. The ladder is a ladder:
step down only for what the rung above cannot do.

**Multi-pass and feedback.** `pass` (another camera's picture) and style `post` stay. Feedback across frames is
`compute` `steps`. Frames stay a pure function of time, so a trail or an accumulation that reads the previous frame is
not offered. **The honest limits:** a new frame stage (a pass kind the host does not know) is a library change. Raw code
is opaque to the CPU twin, so it is opaque to checks, and they say so by name. No rung lets raw code write to a
graph-owned table.

**Cost.** `draw.shader` takes about 80 lines. Hole accessors and the rewrite of `time` in raw text take about 150.
Emitting graph functions callable from raw text takes about 100. The compute spec trims (`reads`, `values`, single
`entry`) are a net deletion. `camera_ray` is 10 lines of prelude. **Migration.** Three compute examples and 17 files
using `drawSpec`; the library's drawables stay on `Drawable`. **Risk.** Generated accessor names must not collide with
the author's names; the compute lane already refuses `__` and `loft_` identifiers, which covers it.

## 5. Concern 2: generated code made visible

**The problem.** `draw.points`, `march`, `draw.field` and `draw.volume` build vertex and fragment shaders the author
never sees. The artefact has them only as hash-named chunks (`wgsl/<hash>.wgsl`, concatenated per module). `emitWgsl`
names values `e123`. There is a worse, real bug. `src/host/host.ts` reports every shader-module failure as
`EnvironmentError(transient: true)`: "a refusal here is the GPU stack's … never the scene's". That held while the
compiler wrote every line. Author WGSL now flows through `wgslFn`, raw drawables, posts and compute. A typo there is
retried on a fresh device, then reported with the environment exit code. The tool says something false.

**The solution.**

1. **Sites on generated statements.** The text parser stores each node's site in a side table (`WeakMap<Expr, Site>`,
   kept off the interned key). `emitWgsl` takes an optional comment callback, so a statement reads
   `let e812 = smoothstep(0.0, 0.19, e811);   // eel.ts:9 girth`. Call-form nodes get sites only when asked: `--sites`
   re-imports the scene with stack capture on, which is slow and used only for diagnosis.
2. **Chunk origins and line maps in the artefact.** Each chunk records `origin: "library" | "author"` and a map from
   chunk line to site. This is an additive manifest field; old readers ignore it.
3. **Errors.** When Tint refuses a module, the host maps its line to the chunk and the site. If the chunk is the
   author's, it raises a `SceneError` with the scene exit code: `examples/eel.ts:14: textureSample used in a vertex
   stage (Tint: …)`, the author's line quoted and the generated line beside it. If it is the library's, it stays an
   environment error. The confined compile child has no GPU, so Tint runs at load. Lane L4 checks whether bun-webgpu
   can open Dawn's null adapter in the child for earlier validation. If it cannot, load-time mapping is enough, and
   naga-in-wasm is the fallback, adopted only if the extra round trip shows up in the training numbers.
4. **`loft show <scene> [--thing eel] [--stage vertex|fragment|pose|compute]`** prints the assembled modules a thing
   emits, with site comments. The prelude is shown as its declarations (the names a raw author may use), not its bodies.
   It is one new verb. Its capability is reading the code without unpacking hashed chunks. `loft build` still writes
   the files.

**Cost.** About 500 lines across the manifest, host and CLI. **Replaces.** The blanket environment classification of
shader failures. **Risk.** Line maps must survive the f16 lane's two-variant modules. Map per variant.

## 6. Concern 5: one vocabulary for a condition over time

**The problem.** One idea, "this holds over that time", has three spellings today:

```ts
staysClear(tail, ship, { from: 1, until: 3, margin: 0.1 })                // sampled at 60 Hz; "pass" from samples
equal(vec2(x, y), [0, 0], { from: 0, to: 0.3, within: 1e-3 })               // solve hold; its own When type
always([time, 0, 4], ge(gap, 0.1))                                          // ranged; proved over every t
```

A model must learn three timing syntaxes and three result types. One of them, a sampled `pass: true`, is a claim about
240 samples printed as if it were a claim about the interval.

**The solution.** A condition is a boolean expression. Time is quantified by `always`/`eventually` over a range
`[leaf, lo, hi]`, and an instant is `[time, t, t]`. That is all.

- **Comparisons** are `ge`, `le`, `gt`, `lt`, `and`, `or`, `not` in call form, or `>=`, `&&`, `all(a <= b)` in text. The
  margin is the existing robustness semantics.
- **`equal(a, b, within = 1e-5)`** stays as one name, now returning a boolean expression: every component of `|a − b|`
  is at most `within`. `solve` treats an `equal` as a target, not merely an inequality. It knows the node from a
  registry `equal` writes, not by matching patterns, so `all(abs(a - b) <= vec2(w))` written in text is solved as the
  inequality it literally is. That is correct, only less pinned.
- **`solve`'s `hold`** takes the same formulas. A bare condition means the whole span. The span is the one place where a
  bare condition has an obvious range.
- **Domain checks become measures.** `clearance(body, target)` is an expression: the least, over a body's chain
  stations, of the target's distance minus the station's girth. `staysClear(…)` becomes
  `always([time, 1, 3], ge(clearance(tail, ship), 0.1))`, which is about five tokens longer and is now *proved*. The
  measure is also usable in `solve` (a jaw that must clear the wall). `stopsAt` becomes `eventually([time, a, b],
  le(clearance(…), contact))` plus an `always` margin. Lane L2 classifies each existing check once. A check whose
  quantity is an expression (clearance, screen size from the camera's projection, lit-by) becomes a measure. One that is
  not (a footprint area share in `landsInside`, `budget`'s triangle count, `expressionsRead`) stays a `sampled` variant
  of the tagged `Verdict` (the small lane built the tag). Its text says "no failure at 240 samples", never "holds".

### The user's code: a check

```ts
// jaw, wall and reach come from the scene; landed is any expression of time (a foot's height above the deck).
export const checks = [
  always([time, 0, 4], ge(clearance(jaw, wall), 0.05)),                  // proved over every t
  always([reach, 0.8, 1.2], eventually([time, 0, 2], ge(landed, 0))),     // for every reach, it lands
  ...lunge.holds,                                                        // the solve's own, re-proved
  budget({ triangles: 100_000 }),                                        // sampled: says so
];
```

**Cost.** Net deletion: `atLeast`, `When`, `Times`, the timing options of `equal`, and the timing code of seven sampled
checks; `clearance` is added. Lane L2 rewrites the 3 files using `atLeast`, 4 using `equal` and 7 using `staysClear`.
The heron must reproduce its 52/52 proofs and byte-identical spline controls. **Persisted data:** none. Verdict JSON
already changed shape in the small lane, and creative is pinned. **Risk.** `clearance` against a `skin` target is not an
expression today (ranged has a special `skin` body). It keeps that special case behind the same name.

## 7. Concern 6 and concern 7: tooling and identity

### `loft check --at … --explain`: confirmed, with two additions

The plan cut `measure`/`why`/`try` to `loft check --at t=…,param=… --explain`, which gives the margin at a setting, the
witness, and the graph nodes driving it. That is right: a `measure` is a check with no threshold, and `try` is `--at`.
Two additions make it worth reading. First, **nodes are named by their sites**: "margin −0.031 at t = 1.37: driven by
`eel.ts:20 distance(vec2(x, y), vec2(1.3, 0.5))` (82%) …". Text gives sites for free, and call form gets them through
`--sites`. Second, **every witness prints the command that shows it**: `loft render examples/eel.ts --time 1.37 --views
side`. A model then looks at the frame it failed on rather than guessing.

### One recorded identity

A frame is a function of more than the plan's tuple: the precision actually used (`mixed` may fall back), the variant
and the style override. A check is about the f64 twin with f32-encoded holds. Make it one record:

```ts
type Identity = {
  build:  { graph: string; library: string; tier: "compiled" | "interpreted"; precision: "f32" | "mixed"; variant: string; style: string | null };
  frame:  { camera: string; time: number; size: readonly [number, number]; params: Record<string, number>; inputs: Record<string, readonly number[]>;
            precision: "f32" | "mixed" | "f32-fallback" };
  device: { backend: string; adapter: string; driver: string };
};
```

`graph` is the exact structural hash of the interned graph, the same key the plan's step-8 cache uses. Raw text enters
the hash as text. `id = sha256(canonical JSON)`. It appears in four places: the manifest carries `build`; every serve
answer, `render`, `frames` and `clip` print `id` beside the `rgba` hash; and a verdict carries `{ graph, arithmetic:
"f64-twin/f32-encoded" }`. The contract is that the same `id` gives a byte-identical frame. Across devices, agreement is
within the plan's stated tolerances. Serve's separate `tier` and `precision` fields fold into `identity`. Serve is a
deployed API, so creative's reader changes in the same pin bump. **Cost:** about 200 lines. **Risk:** a field left out
of `id` makes two different frames look the same. The lane's check changes each field and sees `id` change.

## 8. Concern 8: other places a model writes more, guesses or is trapped

1. **Two import paths and deep imports.** Examples import from `src/index.ts`, `src/lib/index.ts`, `src/check.ts`,
   `src/grad.ts`, `src/geom/solve.ts`, `src/lib/sdf3.ts` and `src/ranged.ts`. The guide tells readers to import `always`
   from `src/ranged.ts` although `checks.ts` re-exports it. **Fix:** one entry point that exports every public name;
   creative's allowlist is generated from it already. Text shrinks imports further, since library maths is in the prelude.
2. **`draw.points` means two things.** The existing `draw.points(name, points, thing)` instances a thing at points (25
   callers) and nearly duplicates `draw.posed`. The owner asked for `draw.points(n, (i) => …)` for grains; the spray
   lane shipped `specks` to avoid the clash. **Fix:** fold the old `draw.points` into `draw.posed` (points are poses
   without turns), then rename `specks` to `draw.points`. This changes code only.
3. **Raw code reads inputs and params by slot index.** Fixed by holes (§4).
4. **Author WGSL errors exit as environment failures.** Fixed in §5.
5. **WGSL meaning differs from Loft's.** `round` and `%` differ, as covered in §2. A model that knows WGSL would
   otherwise be silently wrong.
6. **`design` boilerplate.** `examples/volume_dust.ts` ends with
   `export default design.build(design.resolve({ variant: "base", picks: [] })).scene;`. The guide's `beacon` returns
   `{ scene, checks: [], strikes: {}, holds: {}, waves: {} }` and declares `slots: {}`. **Fix:** `slots`, `variants`,
   `compare` and the empty maps of a build's result are optional, and the loader accepts a module whose only export is
   `design`. That removes a line a model must copy and cannot derive.
7. **Two places for checks.** `export const checks` beside a scene, or `checks` in a design build. Both stay, since
   designs need per-variant checks. `loft context` states the rule in one line.
8. **Option walls.** `draw.density` asks for `sun`, `sky`, `forward`, `shadowing`, `detail` and `haze` in the spray
   example. Each option of a primitive should have a default that looks right. Lane L5 audits the primitives and makes
   every option with a sensible default optional.

## 9. Work plan

The running lanes merge first, by the integrator, in this order: small, f16, pipecache, pbrsky, dof, spray, volume,
compute, rustplan. Compute goes late because it touches `expr.ts`, `ranged.ts`, `check.ts`, `solve.ts`, `interval.ts`,
the host and serve. Then one full suite on the merged revision. Workers run focused tests only. The integrator runs one
full suite per wave on the merged revision. No two lanes in a wave own the same file. Where a lane needs a one-line
registration in a file another lane owns (a CLI verb, a `draw` member), the integrator adds it at merge.

| Lane | Owner files | Depends on | GPU | Pass / fail check |
|---|---|---|---|---|
| **L1 text** | new `src/text/{tokens,parse,lower,prelude,index}.ts`; `src/expr.ts` (new ops; the `emitWgsl` comment hook); `src/interval.ts`; `src/grad.ts`; `src/core.ts` export | compute merged | light | (1) the five probe functions and six example functions rewritten in text intern to the **same structural hash** as their call form, so their artefacts are byte-identical; (2) twin and GPU agree within 1/255 on a paint using each new op; (3) each fixture outside the subset becomes a raw function with its named reason; (4) parse and lower a 300-line module in under 20 ms |
| **L2 conditions** | `src/geom/solve.ts`, `src/ranged.ts`, `src/check.ts`, `src/checks.ts`, `src/motion.ts`; the 14 example and test files using `equal`/`atLeast`/`staysClear`/`stopsAt`; the guide's solve and checks sections | small, compute merged | none | heron leap: 52/52 holds proved, spline controls byte-identical; every former check classified as a measure or `sampled`; no sampled verdict prints "holds" |
| **L3 identity** | new `src/identity.ts`; `src/cli/serve.ts`; `src/host/manifest.ts`; `src/cli/main.ts`; `docs/handoff_training.md`; creative reader (pin bump) | f16, small merged | light | the same request twice gives the same `id` and `rgba`; changing each field changes `id`; a mixed request without `shader-f16` records `f32-fallback` |
| **L4 visible** | `src/artefact.ts` (chunk origin, line maps); `src/host/host.ts`; new `src/cli/show.ts`; `src/cli/verdict.ts`; `docs/errors.md` | L1; pipecache merged | light | a raw-function typo exits with the scene code, quoting `file:line`; `loft show` on the dust burst gives text that compiles as one module, with every author-derived statement sited; a library-chunk failure still exits as environment |
| **L5 slots** | `src/draw/index.ts`, `src/draw/volume.ts`, `src/soft/specks.ts`; the 25 `draw.points` callers; delete `src/lib/wgsl.ts` (replace `fwidth`, `viewDir`, `eyePosition`, `pixelWorld` with text raw functions in their callers: `styles.ts`, `style/papercraft.ts`, `style/light.ts`, `style/whitewater.ts`, `geom/sea.ts`); `src/index.ts` single entry; `draw.density` option defaults; `veil` reading a `draw.volume` | L1; spray, volume merged | **heavy** | every example's artefact byte-identical except the files whose signatures changed, which are listed; `tools/check_snippets.ts` passes; grains dim inside a `draw.volume` cloud by a measured share |
| **L6 raw** | new `src/draw/shader.ts`; `src/compute.ts`, `src/compile/compute.ts` (holes, entry inference); `src/compile/scene_wgsl.ts` (`camera_ray`, `time` in raw text, accessors); `examples/compute_*.ts` | L1, L4, L5 | **heavy** | compute steps and frame examples are byte-identical to their pre-change frames; the ring example renders through `draw.field` with depth tested against a plane; a raw drawable reading a param through a hole moves when `--param` changes |
| **L7 explain** | `src/cli/check.ts` (`--at`, `--explain`, `--sites`) | L2, L4 | none | on the heron with the rail raised 0.2 m, `--explain` names the hold's line and a witness within 0.02 s of the sampled minimum, and prints a render command that runs |
| **L8 context** | `src/cli/api_preamble.md`, `src/cli/context.ts`, `docs/guide.md` (text-first), `DESIGN.md` §2 | all above | none | `loft context` includes the subset and the prelude list within its size budget; the guide's snippets pass |

**Order.** Wave 1: L1, L2 and L3 in parallel (no shared files; at most one heavy GPU lane). Wave 2: L4 and L5. Wave 3:
L6 and L7. Wave 4: L8, then the blind test. At most two GPU-heavy lanes are ever open at once, well under the limit of
four. The integrator's full suite runs after each wave, never per merge.

## 10. The blind test of the authoring change

This extends the plan's step 7 (the blind trial against three.js and raw shaders) with a third arm, so one run decides
both questions.

- **Arms.** (A) Loft call form; (B) Loft text, after L1–L6; (C) raw: three.js with hand-written WGSL/GLSL materials or
  passes, rendered headless at the same size.
- **Briefs.** 24 briefs, written before the run and kept from the authors: 8 straight-line (a creature's paint and
  shape), 8 loop-heavy (a marcher, a fold fractal, an iterated integrator, a particle settle), and 8 mixed (a creature
  with motion and a check). The split tests the theory in §1 that the gain sits in loops.
- **Protocol.** Same model, same context budget (each arm's own `loft context` or the three.js docs digest at an equal
  token cap), and at most 6 iterations with rendered frames as feedback. Two seeds per brief and arm, so 144 samples.
- **Measured.** Final scene tokens (o200k and Claude `count_tokens`); total tokens across iterations; compile errors per
  iteration, typed by source (tsc, `SceneError`, text parse/lower, Tint, three.js); iterations to the first accepted
  render; and compile CPU seconds.
- **Looks.** Blind pairwise judging of the same camera, times and size (four frames and a clip per sample), with arm
  labels hidden and order randomized. A fixed-prompt model judge rates every pair. The owner rates a random 25%, and
  their agreement with the judge is reported.

**The pass mark for text over call form (all four must hold).**

1. Compile errors per sample at most 0.7× arm A (predicted 0.55×).
2. Looks: pairwise win rate of B over A at least 0.5, with a lower one-sided 90% bound of at least 0.40 over 48 pairs,
   so text is not worse.
3. Final tokens at most 1.0× A (predicted 0.92× overall, 0.75× loop-heavy, 1.0× straight-line).
4. Median iterations to the first accepted render no more than A's.

Against raw (C), B must reach the plan's existing bar of 17 of 24 briefs won.

**If text fails 1–4:** keep `wgsl` as the unified raw form (rungs 2–4 with holes, line errors and `loft show`, which are
worth having regardless), keep teaching call form for maths, and do not move library code to text. **If B fails against
C**, the plan's step-7 fallback applies unchanged.

## 11. What is inherited and what is new here

Inherited: WGSL itself; SSA and if-conversion to `select` (standard compiler practice); loops as a fold with an `until`
(Loft's own `iterate`); robustness margins (Donzé and Maler); hash-consing (Loft's interner); source maps (as in
JavaScript tooling); tagged templates (as in GPU libraries such as TypeGPU and `wgsl-preprocessor`).

New here:
- lowering *per call* into the interned graph, so text and call form are provably the same program, checked by
  structural hash;
- a raw/graph split *per function*, with the refusal named by line;
- holes as the only way Loft values enter raw code, compiled to accessors;
- the rule that a slot of a function of expressions already accepts shader code, so no primitive grows an API;
- `equal` recognized by registry rather than pattern;
- "bake for speed, prove on the source".

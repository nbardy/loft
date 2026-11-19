

================================================================
# THE ASK
# file: docs/handoff/2026-11-20_syntax/00_SYNTAX_ASK.md
# what it is: the question, what we measured, our reflection, the ten questions, what to send back
================================================================

# Loft v1.0, "the polished syntax": the ask (20 Nov 2026)

You helped design the Loft meta engine (`docs/PHILOSOPHY.md`, `docs/proposals/2026-11-20_meta_engine_plan.md`, the
language draft `agent_notes/dump_manual_nov_19th_alpha_v1.md`, Astra's `docs/handoff/2026-11-20_spec/astra_spec_v1.md`).
The **semantics are settled** and two phases are built on them: kinds as the only unit of code identity, rows as data,
explicit staging, derived bounds, typed spaces with domain-map spaces, smoothness for `diff`, the six queries, an open
Bsdf, the visibility record, laws, solvers, constraints with honest verdicts, schedules owned by the compiler, Tier 0
for the dirty kind only. Phase 1 drew 1 and 20 hermit crabs with 8 shader modules either way.

What is **not** settled is the **surface**: what an author (mostly an LLM) types. Both earlier specs assumed "a WGSL
superset" without testing anything else. We are now testing, and we want your judgment before we freeze v1.0.

## What we measured (blind test, round 1)

Four one-page language cards with identical semantics; Sonnet and Haiku each wrote three tasks (a hermit crab with
mirrored eyes tracking a world target; 200 reeds in an ink look; a heron hop law with gaze lock, neck limit and body
turn) from the card alone; an Opus judge scored blind. Files: `agent_notes/2026-11-20_syntax_bakeoff/`.

| Syntax | Violations Sonnet / Haiku | Semantic checks | Readability | Avg tokens |
|---|---|---|---|---|
| A: WGSL superset, declarations (`kind { row {} }`, `comptime for`) | 4 / 4 | all pass (Haiku partial on gaze) | 4 | 6,320 |
| D: Python-like (`@kind`, `@comptime`) | 1 / 6 | all pass | 4 | 6,527 |
| B: WGSL superset, builder calls (Astra) | 3 / 6 | Haiku gaze fails | 3 | 6,751 |
| C: Slang/HLSL | 4 / 7 | all pass | 3 | 6,422 |

- Tokens were within 7%: all four still spell types and ceremony.
- A won on **forgiveness**: Haiku made no more errors than Sonnet, because `comptime for` marks the stage on the loop.
- Familiar syntax invites familiar mistakes: in the Python-like surface Haiku wrote ordinary module-level Python loops
  outside any build step (unstaged host loops, the root cause of the old engine's blowups).
- Errors common to every syntax: two laws writing the same kind (5 of 8 outputs); points from the wrong space passed
  to a shared material helper (both Sonnets).
- The judge's grafts: domain maps as values (`sym.p`); typed verdicts (`Verdict[Rad]`); field-level write ownership
  (`writes Heron.{neck, gaze_err}`); generic material helpers over spaces (`fn chitin[S](p: Pt[S])`).

Round 2, running now: **E** a terse expression language (inferred types, numeric promotion, implicit return, `[x,y,z]`
literals, pipes, unit suffixes, rows as constructor arguments); **F** a Lua-like lean surface with infix maths; **G** a
Clojure-like Lisp with macros as staging. Early sign: E/F/G outputs are about 40% smaller in bytes than A–D. The judge
of all seven has not reported yet; its file will be `VERDICT_SEVEN.md`.

## Our own reflection (the owner's ideas and the lead engineer's view)

- The owner leans away from a strict WGSL superset toward something as lean as Lua or Clojure, with TypeScript-style
  inference so types are rarely written.
- Agreed: a strict superset forces WGSL's verbosity (`vec3<f32>`, `1.0` vs `1`, `f32(...)`, `return`, `@` attributes)
  on every author. A raw WGSL block keeps hardware access without making the whole surface WGSL.
- Agreed: local inference, types only at module boundaries.
- Lisp has strong bones (macros as staging, code as data for agents that rewrite code, a tiny parser) but prefix
  maths (`(- (length (- p c)) r)`) is the "maths as nested calls" problem that cost old Loft 2–3× tokens and errors.
- The owner floated a chain: a Clojure/Lua-style Lisp → TypeScript → a TS-to-Rust step → wgpu. The lead engineer
  argued against: TypeScript returns (host tracing was the root cause of the old blowups), errors stop pointing at the
  author's line, two type systems must agree, every edit crosses more tools. Proposed instead: one surface parsed by
  the Rust compiler straight into the typed graph (a 1–2k-line parser), optionally plus a second, machine-facing
  s-expression form of the same IR for agents to transform.
- Forgiveness matters as much as tokens: the cheapest model that writes the scene must not be able to write the
  root-cause bugs (unstaged loops, shared writers, wrong-space points) without an error.

## The questions

1. **Superset or not.** Should v1.0 be a strict WGSL superset, or a lean surface with WGSL only in escape blocks? What
   do we lose either way?
2. **Which family.** A (WGSL declarations), E (terse expressions), F (Lua-like), G (Clojure-like), or something better
   you would design now. Use the cards and outputs; argue from the measured errors, not taste.
3. **Maths.** How should shader maths read? If a Lisp, how do you keep infix maths without two languages?
4. **Inference.** How far can types be inferred (spaces, units, smoothness, rows) and where must the author still
   write them so errors stay local and readable?
5. **Staging on the surface.** What syntax makes the build stage unmistakable even to a weak model (A's `comptime for`
   worked; D's decorators did not)?
6. **Ownership.** How does the syntax make two writers of the same field impossible to write (5 of 8 outputs erred)?
7. **Spaces in helpers.** How does a shared helper (a material) stay correct for points from any space?
8. **The transpile chain.** Do you agree it should be one surface parsed directly by Rust? Is there any case for
   compiling through TypeScript or another host?
9. **A second form.** Is a canonical s-expression (or other) form of the IR worth having for agents that rewrite code?
10. **The floor.** What is the token floor for the hermit crab task, and what does your v1.0 cost against A's 6,320?

## What we would like back

One Markdown document: **Loft v1.0 surface syntax**:
1. Decisions, one line each with the reason and what it rules out.
2. The grammar in EBNF, and the 10 rules that keep it unambiguous for a small parser.
3. The typing and inference rules that the surface relies on (what is inferred, what is written).
4. The three bakeoff tasks written in it, complete, with token counts.
5. Five diagnostics for the errors above (unstaged loop, shared writer, wrong-space helper, mirrored gaze, authored
   bound), each quoting the line and naming the fix in one sentence.
6. Which grafts you accept and why; whether to ship a second machine-facing form.
7. The parser size you predict.

Code in code blocks, numbers in tables, every number labelled measured (from the files) or predicted (yours). Be
ambitious; the semantics stay, the surface may change completely.


================================================================
# TASKS
# file: agent_notes/2026-11-20_syntax_bakeoff/TASKS.md
# what it is: the three bakeoff tasks, in plain words
================================================================

# Loft scene tasks

You have one Loft language card. Use only what it shows. If you need a helper function the card does not list, write it
yourself in the card's syntax. Write each task as one complete file: spaces, kinds or their equivalent, the rows the
build emits, materials, any laws or constraints, the lights, a camera and a schedule. Give metres and radians with
units. Do not explain your file; only write it.

## Task 1: a hermit crab

A hermit crab sits on wet sand.

- **The shell** is a coiled gastropod spiral. Build it by scanning frames along a spiral spine: start one frame from
  the spine's tangent, then transport it along the spine so the frames do not twist. Sweep a ribbed tube around the
  spine, and grow the tube's radius with the spiral. Rasterise the shell as a parametric surface.
- **The crusher claw** is a signed distance field: a rounded palm with a fixed finger and a hinged moving finger, with
  serrations along the moving finger. Its opening angle is per-instance data. Use the same claw definition for a
  smaller feeder claw on the other side, which differs only in its data.
- **The twelve legs** (six per side) are one kind with twelve rows. Each leg has its own segment lengths and socket on
  the body. The build step emits the rows in a loop. Adding a second crab must add rows and no new code.
- **Two eyes on stalks** come from a single mirrored definition. Each pupil looks at the same world-space target, a
  point that drifts slowly over time. Compute each eye's own world position correctly through the mirror, so the left
  eye and the right eye aim differently.
- **The material** is layered wet chitin: a red-orange base, a translucent subsurface layer, and a clear wet coat on
  top. Use a fine noise to vary the roughness. Keep the noise in the material, not in the distance field.

## Task 2: a reed bed in wind

Two hundred reeds stand in shallow water. Each reed is a thin, slightly tapered stalk with a seed head at the top.

- All 200 reeds are rows of one kind. Height, lean, stiffness and sway phase vary per reed. The build step places them
  in a loose, irregular cluster.
- Wind bends every reed over time. The bend grows toward the tip, a gust travels across the bed, and stiffer reeds bend
  less. The motion comes from each reed's own data and time; no reed has its own code.
- The look is not physically based: two flat inks on warm paper. Lit sides are paper. Shadowed sides take a dark ink,
  with a hard edge between the two. The water and the sky are plain paper with a faint second ink.

## Task 3: a heron's hop

A heron stands at the water's edge with a fish in view. Write the motion law and its checks, plus enough body geometry
to see the result: a trunk, two legs, an S-shaped neck, a head and a beak on the head's forward axis.

- The fish moves along a curve in world space. Its position is the gaze target.
- **The gaze lock comes first.** Each tick, the head keeps its eye aimed at the fish in world space, whatever the body
  is doing.
- **The neck has a limit.** The head may turn only so far from the trunk's heading. When the target passes beyond that
  limit, the head stops at the limit, and a constraint records the gaze as missed by the leftover angle. It must not
  report the gaze as held.
- **Then the body turns.** While the neck is at its limit, the heron makes a small hop: it leaves the ground, rotates
  its trunk toward the fish, and lands. Once the trunk has turned far enough, the neck comes back inside its limit and
  the gaze lock holds again.
- Add a second constraint that the head stays above the hips throughout. Schedule the laws in order: the body, then the
  neck and head, then the checks.


================================================================
# CARD A
# file: agent_notes/2026-11-20_syntax_bakeoff/card_A.md
# what it is: the one-page language card for candidate A
================================================================

# Loft language card

Loft is WGSL plus a few declarations. `comptime` code runs at build and emits rows. Each `kind` compiles once; the GPU
runs it over its rows. A kind is code; a row is one instance's data.

## Spaces and units

```
space World, Body, Leg, Claw, Face;
let p = Pt[Claw](0.0, 0.2, 0.9);           // components are metres
let w: Pt[World] = self.pose * p;   // pose: Xform[Claw -> World]
```

Pt - Pt is a Vec; Pt + Pt is an error; mixing spaces is E104. `Dir[S]` and `Norm[S]` have unit
length; normals map by inverse transpose. `inv(x)` inverts an Xform. Units: `Meters Rad Sec Kg`; suffixes `0.3m 1.2rad 0.5s 40deg`.

## Kind and row

```
kind CrabLeg {
  row { parent: RowId[CrabBody], pose: Xform[Leg -> World], femur: Meters, foot: Pt[World] }
  query trace(self, p: Pt[Leg]) -> Field[Leg] {
    return sd_capsule(p, Pt[Leg](0.0, 0.0, 0.0), Pt[Leg](self.femur, 0.0, 0.0), 0.07m);
  }
}
kind Claw[comptime serrated: bool] { ... }   // discrete only
```

Per-instance values go in `row`. `kind K[comptime len: Meters]` is E401; over 4 used variants is E402.

## Comptime: emitting rows

```
comptime fn crab(b: RowId[CrabBody], body: Xform[Body -> World]) {
  comptime for j in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg { parent: b, pose: body * leg_socket(j, side), femur: 0.40m + 0.05m * f32(j),
                     foot: body * Pt[Body](side, 0.0, 0.3 * f32(j)) };
    }
  }
}
```

`emit` appends a row and returns its `RowId`, never a shader. A `for` inside a kind is a GPU loop.

## Queries

A kind implements any of: `trace(self, p) -> Field[S]` (sphere-traced distance); `@grid(nu, nv) query surface(self,
u, v) -> Pt[S]` (rasterised patch); `visible(self, ro, rd) -> f32` (closed-form hit t, -1 on miss); `material`,
`light`, `sky`, `medium`. Any query may take `time: Sec`.

The compiler derives every `Field` bound. A divisor that can reach zero is E211: clamp it.
`diff(f, u)` needs C1; `diff` of `min`, `abs` or Voronoi is E307.

```
scan whorl(u: f32 in 0.0..1.0, steps: 256u) @schedule(bake_lut) -> Table[Frame[Shell]] {
  init: frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir[Shell](0.0, 1.0, 0.0));
  step(prev, u_i): transport_frame(prev, spine(u_i), normalize(diff(spine, u)(u_i)));
}
// let f = sample_c2(whorl, u);  f.origin, f.normal, f.binormal
```

## Domain maps

```
map sym = mirror_x(q) |> translate(-Vec[Face](0.15, 0.72, 0.42));
let eye_c = Pt['sym](0.03, 0.30, 0.07);
let globe = sd_sphere(sym - eye_c, 0.085m);
let eye_w = self.pose * (sym.to_parent * eye_c);
let look  = sym.from_parent * (inv(self.pose) * normalize(self.gaze - eye_w));
```

`map` makes a fresh space `'sym`; `sym.cell` is the branch (mirror side, sector). Also `rot_x`, `rot_z`, `repeat_polar(p, 7u)`.

## Materials

```
query material(self, p: Pt[Leg], n: Norm[Leg]) -> Bsdf {
  return ggx(albedo, 0.35, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}
@lobe fn ink(dark: vec3<f32>, cut: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> { // a: passed below
  return vec4(select(vec3(0.92, 0.85, 0.7), dark, dot(n, l) < cut), 0.0);
}
```

A material sums lobes, bottom first: `ggx(albedo, rough, metal)`, `clearcoat(weight, rough)`,
`thin_film(weight, ior, nm)`, `sss(color, radius)`, `emission(color)`, or any `@lobe` fn. 
Library: `sd_sphere sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2 ray_capsule hit_min`.

## Motion law

```
law crab_step(leg: CrabLeg, dt: Sec, bodies: Table[CrabBody]) writes CrabLeg {
  let b = bodies[leg.parent];
  let cp = b.com + b.vel * 0.28s;
  if distance(leg.foot, cp) > 0.5m { leg.foot = Pt[World](cp.x, 0.0, cp.z); }
}
```

A law runs once per row of the kind it writes, and only it writes those fields.

## Constraint

```
constraint upright(b: CrabBody) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let margin = (b.pose * b.head).y - 0.15m;
  if margin < 0.0m { return Verdict::Missed(-margin); }
  return Verdict::Held(margin);
}
```

## Schedule

```
schedule Main {
  pass rows: crabs();
  pass motion: run_laws(body_law -> crab_step);
  pass check: check(upright);
  pass camera: orbit(at: Pt[World](0.0, 0.4, 0.0), radius: 6m, fov: 40deg);
  pass cull: build_indirect(Ground, CrabLeg, Claw);
  pass vis: render_bundle(Vis64) { draw_indirect(Ground, CrabLeg, Claw); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

## Raw WGSL

```
fn hash(p: vec3<f32>) -> f32 {
  raw wgsl (p: vec3<f32>) -> f32 { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); }
}
```

Raw code is opaque: not differentiable, bound `Unbounded`.


================================================================
# CARD B
# file: agent_notes/2026-11-20_syntax_bakeoff/card_B.md
# what it is: the one-page language card for candidate B
================================================================

# Loft language card

Loft is WGSL plus modules (`import std::render::*;`), lambdas and declarations. `pub comptime fn scene() -> Scene`
runs at build.

## Spaces and units

```
space Body, Leg, Claw, Face, Shell;            // World is built in
let p = Pt[Claw](0, 0.2, 0.9);                 // components are metres
let q = xf * p;                                // xf: Xform[Claw, World]
```

Pt - Pt is a Vec; Pt + Pt, mixed spaces: E104. `Dir[S]` and `Norm[S]` have unit length; normals map by
inverse transpose; `inverse(xf)` inverts. Suffixes: `0.3m 1.2rad 0.5s`.

## Kinds and rows

`add` calls with the same geometry and material functions share one kind. A value a lambda captures (`femur`
below) is a row column; a literal in a function body is code. Law state is a declared kind,
`kind CrabLeg { row { parent: Row[CrabBody], foot: Pt[World], } }`; `emit` adds a row and returns a handle. Variants:
`kind K[comptime serrated: bool]`. `[comptime len: Meters]` is E401; over 4 used variants is E402.

## Comptime

```
pub comptime fn scene() -> Scene {
    let w = world();
    let crab = emit CrabBody { pose: rigid[Body, World](Vec[World](0, 0.4, 0)), ... };
    comptime for i in 0u..12u {
        let side = select(-1.0, 1.0, i % 2u == 1u);
        emit CrabLeg { parent: crab, foot: Pt[World](side, 0, 0.3 * f32(i / 2u)) };
        let at = attachment(crab, |r| r.pose * leg_socket(i / 2u, side));
        let femur = 0.40m + 0.05m * f32(i / 2u);
        add(w, field(box(Pt[Leg](-0.1, -0.1, -0.1), Pt[Leg](0.8, 0.1, 0.1)),
                     |p| sd_capsule(p, Pt[Leg](0, 0, 0), Pt[Leg](femur, 0, 0), 0.07m)), wet, at);
    }
    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));
    return frame(w, look_at(eye, target, up, 0.6rad), main);
}
```

## Geometry and queries

`field(box, |p| dist)` is sphere-traced; `surface(|uv| point, grid(nu, nv))` is rasterised; `capsule(a, b, r)`,
`sphere(r)`, `plane(n, offset)` are closed form. `time()` is the clock. Bounds
are derived. A divisor reaching zero is E211. `diff(f, u)` needs C1; `diff` of `min`, `abs`, Voronoi: E307.

```
scan whorl(u: Unit) @schedule(bake_lut, 256u) -> Table[Basis[Shell]] {
    init: frame_seed(spine(0), normalize(diff(spine, u)(0)), Dir[Shell](0, 1, 0));
    step(last, u_i): transport_step(last, spine(u_i), normalize(diff(spine, u)(u_i)));
}
// sample_c2(whorl, u) -> .origin .normal .binormal
```

## Domain maps

```
map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
let c = Pt['e](0.03, 0.30, 0.07);
let globe = length(e.p - c) - 0.085m;
let eye_w = xf * (e.to_parent * c);
let look = e.from_parent.dir(inverse(xf) * normalize(target - eye_w));
```

`map` makes a fresh space `'e`; `e.cell` is the branch. Also `rotate(axis, a)`, `repeat_polar(p, 7u)`.

## Materials

```
fn wet(h: Hit) -> Bsdf {
    return coat(sss(ggx(rgb(0.6, 0.14, 0.05), 0.35, 0.0), rgb(0.9, 0.3, 0.1), 0.02m), 0.7, 0.07, 1.34);
}
fn ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let src = light(w, h.p, k);
    return select(rgb(0.92, 0.85, 0.7), rgb(0.1, 0.1, 0.12), dot(h.n, src.wi) < 0.4);
}
```

Materials are `fn(Hit) -> Bsdf`; layers wrap: `ggx(base, rough, metal)`, `coat(b, weight, rough, ior)`,
`thin_film(b, ior, thickness)`, `sss(b, color, radius)`, `emission(b, color)`. Looks (`ink`) go to `deferred`. Also: `sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2`.

## Motion law

```
pub law crab_step(leg: CrabLeg, bodies: Table[CrabBody], dt: Sec) -> CrabLeg {
    let b = bodies[leg.parent];
    var next = leg;
    let cp = b.com + b.vel * 0.28s;
    if distance(leg.foot, cp) > 0.5m { next.foot = Pt[World](cp.x, 0, cp.z); }
    return next;
}
```

Each tick a law returns its row's replacement; only it writes that row.

## Constraint

```
pub constraint upright(b: CrabBody) -> Verdict[Meters] {
    if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
    let margin = (b.pose * b.head).y - 0.15m;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}
```

## Schedule

```
pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);                         // calls are passes, in order
    run_laws(s, w, [body_law, crab_step]);
    check(s, w, upright);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Raw WGSL

```
fn hash(p: vec3f) -> f32 {
    return raw wgsl(p: vec3f) -> f32 { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); };
}
```

Raw code is opaque: not differentiable, bound `Unbounded`.


================================================================
# CARD C
# file: agent_notes/2026-11-20_syntax_bakeoff/card_C.md
# what it is: the one-page language card for candidate C
================================================================

# Loft language card

Loft is a Slang/HLSL-style shader language. A `[Kind]` struct is code and compiles once; its fields are a row, one
instance's data. `[Comptime]` functions run at build and emit rows.

## Spaces and units

```hlsl
struct Claw : ISpace {};                       // likewise World, Body, Leg, Face, Shell
Pt<Claw> p = Pt<Claw>(0.0, 0.2, 0.9);          // components are metres
Pt<World> w = pose * p;                        // Xform<Claw, World> pose
```

Pt - Pt is a Vec; Pt + Pt is an error; mixing spaces is E104. `Dir<S>` and `Norm<S>` have unit length; normals map by
inverse transpose. `inverse(x)` inverts an Xform. Units: `Meters Rad Sec Kg`; suffixes `0.3m 1.2rad 0.5s 40deg`.

## Kind and row

```hlsl
[Kind]
struct CrabLeg : ITrace<Leg>
{
    RowId<CrabBody> parent;
    Xform<Leg, World> pose;
    Meters femur;
    Pt<World> foot;

    Field<Leg> trace(Pt<Leg> p) { return sd_capsule(p, Pt<Leg>(0, 0, 0), Pt<Leg>(femur, 0, 0), 0.07m); }
};
[Kind] struct Claw<let Serrated : bool> : ITrace<Claw> { ... };   // discrete only
```

`struct K<let Len : Meters>` is E401; over 4 used variants is E402.

## Comptime: emitting rows

```hlsl
[Comptime]
void crab(RowId<CrabBody> b, Xform<Body, World> body)
{
    for (uint j = 0; j < 6; j++)
        for (float side = -1.0; side <= 1.0; side += 2.0)
            emit(CrabLeg{ .parent = b, .pose = body * leg_socket(j, side), .femur = 0.40m + 0.05m * j,
                          .foot = body * Pt<Body>(side, 0.0, 0.3 * j) });
}
```

`emit` appends a row and returns its `RowId`, never a shader. Loops in a kind method are GPU loops.

## Queries

A kind implements any of: `ITrace<S>` (`Field<S> trace(Pt<S> p)`, sphere-traced distance); `ISurface<S>`
(`[Grid(nu, nv)] Pt<S> surface(float u, float v)`, rasterised patch); `IVisible<S>` (`float visible(Pt<S> ro, Dir<S>
rd)`, closed-form hit t, -1 on miss); `IMaterial<S>`, `ILight`, `ISky`, `IMedium`. Any method may take `Sec time`.

The compiler derives every `Field` bound. A divisor that can reach zero is E211: clamp it.
`diff(f, u)` needs C1; `diff` of `min`, `abs` or Voronoi is E307.

```hlsl
[Scan(steps = 256, range = (0.0, 1.0), BakeLut)]
struct Whorl : IScan<Frame<Shell>>
{
    Frame<Shell> init(float u) { return frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir<Shell>(0, 1, 0)); }
    Frame<Shell> step(Frame<Shell> prev, float u) { return transport_frame(prev, spine(u), normalize(diff(spine, u)(u))); }
};
// Frame<Shell> f = sample_c2<Whorl>(u);  f.origin, f.normal, f.binormal
```

## Domain maps

```hlsl
let sym = map(q).mirror_x().translate(-Vec<Face>(0.15, 0.72, 0.42));
let eye_c = sym.pt(0.03, 0.30, 0.07);                     // Pt<sym.Space>
float globe = sd_sphere(sym.p - eye_c, 0.085m);
Pt<World> eye_w = pose * sym.to_parent(eye_c);
let look = sym.from_parent(inverse(pose) * normalize(gaze - eye_w));
```

`map` makes a fresh space `sym.Space`; `sym.cell` is the branch. Also `.rot_x(a)`, `.rot_z(a)`,
`.repeat_polar(7)`.

## Materials

```hlsl
Bsdf material(Pt<Leg> p, Norm<Leg> n)
{
    return ggx(albedo, 0.35, 0.0) + sss(float3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}
[Lobe] float4 ink(float3 dark, float cut, float3 n, float3 v, float3 l)   // a: passed below
{
    return float4(dot(n, l) < cut ? dark : float3(0.92, 0.85, 0.7), 0.0);
}
```

A material sums lobes, bottom first: `ggx(albedo, rough, metal)`, `clearcoat(weight, rough)`,
`thin_film(weight, ior, nm)`, `sss(color, radius)`, `emission(color)`, or any `[Lobe]` function.
Library: `sd_sphere sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2 ray_capsule hit_min`.

## Motion law

```hlsl
[Law(Writes = CrabLeg)]
void crab_step(inout CrabLeg leg, Sec dt, Table<CrabBody> bodies)
{
    CrabBody b = bodies[leg.parent];
    Pt<World> cp = b.com + b.vel * 0.28s;
    if (distance(leg.foot, cp) > 0.5m) leg.foot = Pt<World>(cp.x, 0.0, cp.z);
}
```

A law runs once per row of the kind it writes, and only it writes those fields.

## Constraint

```hlsl
[Constraint]
Verdict<Meters> upright(CrabBody b)
{
    if (!is_finite(b.pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = (b.pose * b.head).y - 0.15m;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}
```

## Schedule

```hlsl
technique Main
{
    pass Rows     { crabs(); }
    pass Motion   { run_laws(body_law, crab_step); }
    pass Check    { check(upright); }
    pass Camera   { orbit(Pt<World>(0.0, 0.4, 0.0), 6m, 40deg); }
    pass Cull     { build_indirect<Ground, CrabLeg, Claw>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Ground, CrabLeg, Claw>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

## Raw WGSL

```hlsl
float hash(float3 p)
{
    __wgsl(p) { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); }
}
```

Raw code is opaque: not differentiable, bound `Unbounded`.


================================================================
# CARD D
# file: agent_notes/2026-11-20_syntax_bakeoff/card_D.md
# what it is: the one-page language card for candidate D
================================================================

# Loft language card

Loft has a Python-like surface that compiles to GPU code. An `@kind` class is code and compiles once; its annotated
fields are a row, one instance's data. `@comptime` functions run at build and emit rows. No Python runtime: no lists,
dicts, recursion or closures in GPU code; `float` is f32, `int` is i32.

## Spaces and units

```python
space("World", "Body", "Leg", "Claw", "Face", "Shell")
p = Pt["Claw"](0.0, 0.2, 0.9)          # components are metres
w: Pt["World"] = self.pose @ p         # pose: Xform["Claw", "World"]
```

Pt - Pt is a Vec; Pt + Pt is an error; mixing spaces is E104. `Dir[S]` and `Norm[S]` have unit length; normals map by
inverse transpose. `inv(x)` inverts an Xform. Units `Meters Rad Sec Kg`; write `0.3*m 1.2*rad 0.5*s 40*deg`.

## Kind and row

```python
@kind
class CrabLeg:
    parent: RowId["CrabBody"]
    pose: Xform["Leg", "World"]
    femur: Meters
    foot: Pt["World"]

    @query
    def trace(self, p: Pt["Leg"]) -> Field["Leg"]:
        return sd_capsule(p, Pt["Leg"](0, 0, 0), Pt["Leg"](self.femur, 0, 0), 0.07*m)

@kind
class Claw:
    serrated: Static[bool]      # a code variant, not a row field; discrete only
```

`Static[Meters]` is E401; over 4 used variants is E402.

## Comptime: emitting rows

```python
@comptime
def crab(b: RowId["CrabBody"], body: Xform["Body", "World"]):
    for j in range(6):
        for side in (-1.0, 1.0):
            emit(CrabLeg(parent=b, pose=body @ leg_socket(j, side), femur=0.40*m + 0.05*m*j,
                         foot=body @ Pt["Body"](side, 0.0, 0.3*j)))
```

`emit` appends a row and returns its `RowId`, never a shader. A loop inside a kind method is a GPU loop.

## Queries

A kind implements any `@query` method named: `trace(self, p) -> Field[S]` (sphere-traced distance);
`surface(self, u, v) -> Pt[S]` with `@query(grid=(nu, nv))` (rasterised patch); `visible(self, ro, rd) -> float`
(closed-form hit t, -1 on miss); `material`, `light`, `sky`, `medium`. Any query may take `time: Sec`.

The compiler derives every `Field` bound. A divisor that can reach zero is E211: clamp it.
`diff(f, "u")` needs C1; `diff` of `min`, `abs` or Voronoi is E307.

```python
@scan(over=(0.0, 1.0), steps=256, schedule="bake_lut")
class whorl:
    def init(u: float) -> Frame["Shell"]:
        return frame_from_tangent(spine(u), normalize(diff(spine, "u")(u)), Dir["Shell"](0, 1, 0))
    def step(prev: Frame["Shell"], u: float) -> Frame["Shell"]:
        return transport_frame(prev, spine(u), normalize(diff(spine, "u")(u)))
# f = sample_c2(whorl, u);  f.origin, f.normal, f.binormal
```

## Domain maps

```python
sym = mirror_x(q) | translate(-Vec["Face"](0.15, 0.72, 0.42))
eye_c = sym.pt(0.03, 0.30, 0.07)                 # Pt[sym]
globe = sd_sphere(sym.p - eye_c, 0.085*m)
eye_w = self.pose @ sym.to_parent(eye_c)
look = sym.from_parent(inv(self.pose) @ normalize(self.gaze - eye_w))
```

A map makes a fresh space `sym`; `sym.cell` is the branch. Also `rot_x(a)`, `rot_z(a)`, `repeat_polar(7)`.

## Materials

```python
@query
def material(self, p: Pt["Leg"], n: Norm["Leg"]) -> Bsdf:
    return ggx(albedo, 0.35, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07)

@lobe
def ink(dark: vec3, cut: float, n: vec3, v: vec3, l: vec3) -> vec4:   # a: passed below
    return vec4(dark if dot(n, l) < cut else vec3(0.92, 0.85, 0.7), 0.0)
```

A material sums lobes, bottom first: `ggx(albedo, rough, metal)`, `clearcoat(weight, rough)`,
`thin_film(weight, ior, nm)`, `sss(color, radius)`, `emission(color)`, or any `@lobe` function.
Library: `sd_sphere sd_box sd_round_box sd_capsule smin_poly ssub_poly fbm2 ray_capsule hit_min`.

## Motion law

```python
@law(writes=CrabLeg)
def crab_step(leg: CrabLeg, dt: Sec, bodies: Table["CrabBody"]):
    b = bodies[leg.parent]
    cp = b.com + b.vel * (0.28*s)
    if distance(leg.foot, cp) > 0.5*m:
        leg.foot = Pt["World"](cp.x, 0.0, cp.z)
```

A law runs once per row of the kind it writes, and only it writes those fields.

## Constraint

```python
@constraint
def upright(b: CrabBody) -> Verdict[Meters]:
    if not is_finite(b.pose):
        return Unknown("NonFinitePose")
    margin = (b.pose @ b.head).y - 0.15*m
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)
```

## Schedule

```python
@schedule
def main():                                # each call is a pass, in order
    rows(crabs)
    run_laws(body_law, crab_step)
    check(upright)
    orbit(at=Pt["World"](0.0, 0.4, 0.0), radius=6*m, fov=40*deg)
    build_indirect(Ground, CrabLeg, Claw)
    with render_bundle(Vis64):
        draw_indirect(Ground, CrabLeg, Claw)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```

## Raw WGSL

```python
@raw_wgsl
def hash(p: vec3) -> float:
    """return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5);"""
```

Raw code is opaque: not differentiable, bound `Unbounded`.


================================================================
# CARD E
# file: agent_notes/2026-11-20_syntax_bakeoff/card_E.md
# what it is: the one-page language card for candidate E
================================================================

# Loft language card

A `kind` is code, compiled once to WGSL; its parameters are a row, one instance's data. Module-level code runs at
build; queries run on the GPU. Types are inferred except on `pub` items and a law's or check's first parameter. A
block's value is its last expression.

## Spaces

```
space World Body Leg Claw Face Shell
p = [0,0.2,0.9]@Claw     # point, metres
w = pose * p               # pose: xf@Claw>World
```

Types: `p3 v3 d3 n3` (point, vector, direction, normal) `@S`, `xf@A>B`, `f u i bool rgb m rad s kg`, row id `K`,
table `[K]`. `[x,y,z]` takes space and type from context; add `@S` where the space changes. Numbers promote.
Point - point is a vector; point + point or mixed spaces: E104. `inv(x)` inverts.

## Kinds

```
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, foot: p3@World) {
  trace(p) = capsule(p, [0,0,0], [femur,0,0], 0.07m)
}
kind Claw[serrated: bool] @Claw (pose, open: rad) { ... }
```

`kind K[len: m]` is E401; over 4 used variants, E402.

## Build

```
fn crab(b, body: xf@Body>World) {
  for j in 0..6, side in [-1, 1] {
    CrabLeg(parent: b, pose: body * socket(j, side), femur: 0.40m + 0.05m * j, foot: body * [side, 0, 0.3 * j]@Body)
  }
}
at = place([0,0.4,0]@World)
crab(CrabBody(pose: at), at)
```

At build `K(..)` appends a row and returns its id; `for` unrolls. In a query `for` is a GPU loop.

## Queries

`trace(p)` sphere-traced, `surface(u, v) grid 64x32` rasterised, `visible(ro, rd)` hit t or -1, `material(p, n)`,
`light`, `sky`, `medium`; `time` is in scope. Bounds are derived; a divisor reaching zero is E211.
`f'(u)` needs C1; of `min`, `abs`, Voronoi: E307.

```
scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0,1,0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}
# whorl(u) samples C2: .o .n .b
```

## Maps

```
sym = q |> mirror_x |> move([-0.15,-0.72,-0.42])
eye_c = [0.03,0.30,0.07]@sym
globe = sphere(sym.p - eye_c, 0.085m)
eye_w = pose * sym.out(eye_c)
look = sym.in(inv(pose) * dir(gaze - eye_w))
```

`a |> f(x)` is `f(a, x)`; a bound map chain is a fresh space; `sym.cell` is the branch. Also `rot_x(a) rot_z(a)
repeat_polar(7)`.

## Materials

```
material(p, n) = ggx(albedo, 0.35, 0) + sss([0.9,0.3,0.1], 0.08) + coat(0.7, 0.07)
lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { [0.92,0.85,0.7] }
```

Lobes sum bottom first: `ggx(albedo, rough, metal) coat(w, rough) film(w, ior, nm) sss(color, radius) emit(color)`,
any `lobe`. Library: `sphere box round_box capsule smin ssub fbm2 ray_capsule hit_min`.

## Law

```
law crab_step(leg: CrabLeg, dt, bodies: [CrabBody]) {
  b = bodies[leg.parent]
  cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m { leg.foot = [cp.x,0,cp.z] }
}
```

Runs per row of its first parameter, which only it writes.

## Check

```
check upright(b: CrabBody) {
  if !finite(b.pose) { return unknown(NonFinitePose) }
  margin = (b.pose * b.head).y - 0.15m
  if margin < 0 { missed(-margin) } else { held(margin) }
}
```

## Schedule

```
schedule Main {        # a pass per line
  crabs()
  run_laws(body_law, crab_step)
  check(upright)
  orbit([0,0.4,0]@World, 6m, 40deg)
  build_indirect(Ground, CrabLeg, Claw)
  render_bundle(Vis64) { draw_indirect(Ground, CrabLeg, Claw) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```

## Raw WGSL

```
fn hash(p: v3) -> f = wgsl { return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); }
```

Opaque: no derivative, bound `Unbounded`.

## Parsing

1. Newline ends a statement unless the line ends in an operator or opener, or the next starts `|>`.
2. Declarations open with a keyword; other module lines are build code.
3. Types follow only `:` or `->`; there `>` joins spaces.
4. `@` only attaches a space: `[..]@S`, `T@S`, `kind K @S`.
5. A number touching letters takes a unit or is a lex error.
6. `'` only marks a derivative.
7. Values are lowercase; `K(` constructs a kind or tag, `K[..](` a variant.
8. `{` ends a header; only schedules have `call(..) {` blocks.
9. `x = e` binds a new name or sets a `var`; `a.f = e` only in a law.
10. `wgsl {` is raw text to its balanced `}`.


================================================================
# CARD F
# file: agent_notes/2026-11-20_syntax_bakeoff/card_F.md
# what it is: the one-page language card for candidate F
================================================================

# Loft language card

Loft is Lua-like, parsed by Rust to a typed graph. A `kind` is code; its columns are a row. Top-level code runs at
build; queries run on the GPU. Types are inferred except on `pub` items and a law's or check's first parameter.

## Spaces and units

```
space World Body Leg Claw Face Shell
local w = pose * {0, 0.2, 0.9}@Claw     -- pose: xf@Claw>World
```

`p3 v3 d3 n3` (point, vector, direction, normal) `@S`; `xf@A>B`; `f u i bool rgb`; `K` a row id, `[K]` a
table. `{x, y, z}` takes space from context. Point + point, mixed spaces: E104. WGSL maths, plus `and or not`.
`0.3m 1.2rad 0.5s 40deg`.

## Kinds and rows

```
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, foot: p3@World)
  trace(p) = capsule(p, {0, 0, 0}, {femur, 0, 0}, 0.07m)
end
kind Claw[serrated: bool] @Claw (open: rad) ... end

fn crab(b, body)
  for j in 0..6, side in {-1, 1}
    CrabLeg{parent = b, pose = body * socket(j, side), femur = 0.40m + 0.05m * j,
            foot = body * {side, 0, 0.3 * j}@Body}
  end
end
local at = place({0, 0.4, 0}@World)
crab(CrabBody{pose = at}, at)
```

Columns are in scope. `[..]` is a code variant (`[len: m]` E401; over 4, E402). At build `K{..}` appends a row,
returning its id, and `for` unrolls; in a query it loops on GPU.

## Queries

`trace(p)` sphere-traced, `surface(u, v) grid 64x32` rasterised, `visible(ro, rd)` hit t or -1, `material light
sky medium`; `time` in scope. Bounds are derived. E211: zero divisor. `f'(u)` needs C1 (E307).

```
scan whorl u in 0..1 by 256 on bake_lut
  init = frame(spine(u), dir(spine'(u)), {0, 1, 0})
  step prev = transport(prev, spine(u), dir(spine'(u)))
end
-- whorl(u): C2 sample .o .n .b
```

## Domain maps

```
local sym = q |> mirror_x |> move({-0.15, -0.72, -0.42})
local eye_c = {0.03, 0.30, 0.07}@sym
local eye_w = pose * sym.out(eye_c)
local look = sym.into(inv(pose) * dir(gaze - eye_w))
```

`a |> f(x)` is `f(a, x)`. A map chain is a space; `sym.cell` its branch. Also `rot_x rot_z repeat_polar`.

## Materials

```
material(p, n) = ggx(albedo, 0.35 + 0.1 * fbm2(p.xz * 40), 0) + sss({0.9, 0.3, 0.1}, 0.08) + coat(0.7, 0.07)
lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut then dark else {0.92, 0.85, 0.7} end
```

Lobes sum bottom first: `ggx coat film sss emit`, `lobe`. Also `box round_box smin ssub`.

## Laws and checks

```
law crab_step(leg: CrabLeg, dt, bodies: [CrabBody])
  local b = bodies[leg.parent]
  local cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m then leg.foot = {cp.x, 0, cp.z} end
end
```

A law runs per row of its first parameter's kind and alone writes it.

```
check upright(b: CrabBody)
  if not finite(b.pose) then return unknown(NonFinitePose) end
  local margin = (b.pose * b.head).y - 0.15m
  if margin < 0 then missed(-margin) else held(margin) end
end
```

## Schedule

```
schedule Main      -- in order
  crabs()
  run_laws(body_law, crab_step)
  check(upright)
  orbit({0, 0.4, 0}@World, 6m, 40deg)
  build_indirect(Ground, CrabLeg, Claw)
  render_bundle(Vis64) do
    draw_indirect(Ground, CrabLeg, Claw)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
end
```

## Raw WGSL

```
pub fn hash(p: v3) -> f = wgsl[[ return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5); ]]
```

Opaque, bound `Unbounded`.

## Parse rules

1. Newline ends a statement unless the line ends in an operator, `,` `(` `{` `[`, or the next starts `|>`.
2. `space kind fn lobe law check scan schedule pub` open declarations; the rest is build.
3. A header ends at its newline; `end` closes it; `=` gives a one-expression body. A last expression is the value.
4. `then` makes a one-line `if`, also an expression. `do` only opens a schedule call's block.
5. `local x = e` binds; `x = e` sets a local; `a.f = e` only in a law.
6. Braces never open blocks: `{a, b}` vector, `K{f = e}` row.
7. Types follow only `:` or `->`, where `>` joins spaces; `@` attaches a space.
8. `0.5m`: digits touching letters are a unit.
9. `--` comments; `'` marks a derivative; no strings.
10. Values lowercase, kinds capitalised; `wgsl[[` is raw to `]]`.


================================================================
# CARD G
# file: agent_notes/2026-11-20_syntax_bakeoff/card_G.md
# what it is: the one-page language card for candidate G
================================================================

# Loft language card

A lean Lisp read by the Rust compiler straight into its typed graph, one form per node. A `defkind` is code; its
columns are a row. `!` heads run at build. Types are inferred except on columns, `pub` items and a law's or
check's first argument.

## Spaces and units

`(defspace World Body Leg)`; `(p3 Claw 0 0.2 0.9)` is metres. Types: `p3 v3 d3 n3`, `(xf A B)`, `f u bool rgb m
rad s`; `K` a row id, `[K]` a table. `[x y z]` takes space and type from context. Point + point, mixed spaces: E104.

## Kinds and rows

```clojure
(defkind CrabLeg :in Leg {:parent CrabBody :pose (xf Leg World) :femur m :foot (p3 World)}
  (trace [p] (capsule p [0 0 0] [femur 0 0] 0.07m)))
```

Columns are in scope. `(defkind Claw [serrated bool] …)` is a code variant; `[len m]`: E401.

## Build

```clojure
(defn! crab [b body]
  (for! [j (range 6) side [-1 1]]
    (emit! CrabLeg {:parent b :pose (* body (move [side 0 (* 0.25 j)])) :femur (+ 0.40m (* 0.05m j))
                    :foot (* body (p3 Body side 0 (* 0.3 j)))})))
(let! [at (place [0 0.4 0])] (crab (emit! CrabBody {:pose at}) at))
```

`emit!` returns a row id. `for!` unrolls; query `for` loops on GPU. `defmacro` returns forms, never rows.

## Queries

`(trace [p])`, `(surface [u v] :grid [64 32])`, `(visible [ro rd])`, `(material [p n])`, `light sky medium`;
`time` in scope. Bounds are derived. Zero-reaching divisor: E211. `(d f u)` of `min abs` Voronoi: E307.

```clojure
(defscan whorl [u 0 1 256] :on bake-lut
  :init (frame (spine u) (dir (d spine u)) [0 1 0])
  :step [prev] (transport prev (spine u) (dir (d spine u))))   ; (whorl u) -> C2 frame :o :n :b
```

## Maths

`($ b.com + b.vel * 0.28s)` is infix (`* /`, `+ -`, comparisons; left to right) over symbols, numbers or prefix
forms with optional `.f`; the reader rewrites it to prefix.

## Domain maps

```clojure
(let [sym (-> q mirror-x (move [-0.15 -0.72 -0.42]))
      c   (p3 sym 0.03 0.30 0.07)]
  (in sym (* (inv pose) (dir (- gaze (* pose (out sym c)))))))
```

A map is a point-bound value and a fresh space: `(:p sym)` is the mapped point, `(:cell sym)` the branch. Also `rot-x rot-z repeat-polar`.

## Materials

```clojure
(defn chitin [p] (+ (ggx [0.6 0.14 0.05] ($ 0.3 + 0.1 * (fbm2 p)) 0) (sss [0.9 0.3 0.1] 0.08) (coat 0.7 0.07)))
(deflobe ink [dark cut n v l] (if (< (dot n l) cut) dark [0.92 0.85 0.7]))
(defraw hash [p :v3] :-> f "return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5);")
```

Lobes sum bottom first: `ggx coat film sss emit`, `deflobe`. Untyped helpers are space-generic. Raw: no
derivative, `Unbounded`.

## Motion law

```clojure
(deflaw crab-step [leg :CrabLeg dt bodies] :writes [:foot]
  (let [b (bodies (:parent leg)) cp ($ b.com + b.vel * 0.28s)]
    (when (> (dist (:foot leg) cp) 0.5m) (write leg :foot [(:x cp) 0 (:z cp)]))))
```

Runs per row of its first argument's kind; a kind's laws have disjoint `:writes`.

## Constraint

```clojure
(defcheck upright [b :CrabBody] :-> m
  (let [margin ($ (* (:pose b) (:head b)).y - 0.15m)]
    (if (neg? margin) (missed (- margin)) (held margin))))
   ; or (unknown :tag)
```

## Schedule

```clojure
(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]}) (emit! Sky {:zenith [0.4 0.5 0.6]})
(defschedule Main (crabs) (run-laws body-law crab-step) (check upright) (orbit [0 0.4 0] 6m 40deg)
  (draw Vis64 Ground CrabLeg) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```

## Parse rules

1. Forms are `()` lists, `[]` vectors, `{}` maps; no precedence outside `$`.
2. `def…` heads declare (`defn!` a build fn), `…!` heads run at build, others call.
3. In argument vectors a keyword after a symbol is a type.
4. A row is a `{}` of exactly its columns.
5. A number touching letters is a unit or a lex error.
6. Capitalised symbols are spaces, kinds, tags.
7. `($ …)` is the sole infix form.
8. `a.f` only inside `$`; elsewhere `(:f a)`.
9. `write` only in a `deflaw`, on its `:writes`.
10. A `defraw` string is opaque WGSL.


================================================================
# VERDICT ROUND 1 (A-D)
# file: agent_notes/2026-11-20_syntax_bakeoff/VERDICT.md
# what it is: the blind judge's scores for A to D
================================================================

# Syntax bakeoff verdict (2026-11-20)

The judge read all eight outputs in full. Syntaxes: **A** WGSL + `kind { row {} }` declarations; **B** WGSL + modules,
lambdas and `add(...)` (a kind is implied by lambda capture); **C** Slang/HLSL `[Kind] struct`; **D** Python-like
`@kind class`.

Token counts come from tiktoken `o200k_base`, a stand-in for Claude's tokenizer, over the code blocks only.
Each cell has one sample and one judge. Treat a difference of one or two violations as noise.

## Per-syntax tables

Semantic checks, in order: crab legs are 12 rows of one kind / reeds are 200 rows of one kind / gaze goes through
world space / the neck limit hands off to the body turning.

### A: WGSL + declarations

| | Sonnet | Haiku |
|---|---|---|
| Tokens T1 / T2 / T3 (total) | 2662 / 1092 / 1997 (5751) | 3062 / 1279 / 2549 (6890) |
| Violations | 4: `chitin(Pt[Leg])` called with Claw and Body points (E104); raw WGSL hash used only for comptime placement (raw escape not needed); `/ self.stiff` not clamped (E211); two laws `writes Heron` | 4: `map sym = mirror_x(q)` at file scope with no query point; `let sun_sky = SunSky{..}` struct literal instead of `emit` (both tasks); invented `clamp_ldr` pass; two laws `writes Heron` |
| Semantics | pass / pass / pass / pass | pass / pass / partial (conversion written correctly, but the map is unbound) / pass |
| Readability, cold | 4: one place per idea (`row`, `query`, `comptime for`, `pass`); only `'sym` is cryptic | |
| Haiku gap | **0** (4 vs 4); no staging errors, because `comptime for` marks the stage on the loop itself | |

### B: WGSL + lambdas / `add`

| | Sonnet | Haiku |
|---|---|---|
| Tokens (total) | 2776 / 1545 / 2571 (6892) | 2628 / 1330 / 2652 (6610) |
| Violations | 3: `/ stiffness` and `/ height` not clamped (E211); `ink` looks passed to `add` as materials, though looks go to `deferred`; `map h = translate(..)` has no domain argument | 6: `leg_socket` and `m_per_s_dt` used but never written; eye `map` at comptime with no `p`, which yields **one** eye; the gaze target `time()` is read at build so it never drifts; two laws return `Heron`; `Pt * k`; legs given the sand material |
| Semantics | pass / pass / pass / pass | pass / pass / **FAIL** (static, single eye) / pass |
| Readability, cold | 3: a kind is implicit (12 `add` calls with one lambda equal one kind with 12 rows). A stranger cannot see the kind boundary. State kind and render `add` are parallel books; schedule threads `s, w` everywhere | |
| Haiku gap | +3 plus the only semantic failure, on the hardest feature (mirrored gaze) | |

### C: Slang/HLSL

| | Sonnet | Haiku |
|---|---|---|
| Tokens (total) | 2958 / 1307 / 2587 (6852) | 2744 / 1050 / 2199 (5993) |
| Violations | 4: four socket helpers never written; `chitin(Pt<Body>)` called with Claw and Leg points; `__wgsl` hash only for comptime placement; CrabBody, Eye and Shell left out of the draw lists | 7: `Pt<Face> eye_c = sym.pt(..)` and stalk mixes Face with `sym.Space` (E104); `Radians` is not a card type; `0.8m * size` gives m^2 (several places); `emit` inside a `technique` pass; two laws write `Heron`; `wrap_angle` never written; no head geometry |
| Semantics | pass / pass / pass / pass | pass / pass / pass (path right, types wrong) / pass |
| Readability, cold | 3: familiar to shader programmers, but heavy (`.field =`, `ISpace` structs, `Pt<sym.Space>`); C `for` with float stepping at comptime | |
| Haiku gap | +3; unit and space typing breaks first | |

### D: Python-like

| | Sonnet | Haiku |
|---|---|---|
| Tokens (total) | 3242 / 1408 / 2748 (7398) | 2619 / 1006 / 2031 (5656) |
| Violations | 1: `sun_sky = SunSky` aliases a class, where a row was intended | 6: **unstaged** top-level calls `crab(..)`, `bed(200)`, `emit(..)`, `spawn()`; `random.Random` in comptime (Python runtime); `rows(Kind, ..)` instead of a build function; two laws write `Heron`; `sky_color` undefined; no material on legs or claws |
| Semantics | pass / pass / pass / pass (clean; split Trunk/Head kinds, E211 clamped) | pass / pass / pass / pass |
| Readability, cold | 4: the easiest to read line by line; `Pt["World"]` strings and `*m` units are noise; staging is invisible | |
| Haiku gap | **+5**, the largest. Python invites Python: script-level code and stdlib | |

## Cross-cutting findings

- **No output wrote** a continuous kind parameter or an authored Lipschitz value. The E401/E402 rules and derived
  bounds were not a problem in any syntax.
- **Five of the eight outputs had two laws write the same kind**: A×2, B-Haiku, C-Haiku and D-Haiku. It is the
  most common error, and none of the four syntaxes prevents it. Only the outputs that split the heron into Trunk and
  Head kinds avoided it.
- **Shared material helpers typed in one space** (`chitin(Pt[Leg])`) broke in A-Sonnet and C-Sonnet.
- **Helpers shown in card examples but never defined** (`leg_socket`, sockets) went missing in B-Haiku, C-Sonnet and
  C-Haiku. This is a gap in the cards, not in the syntaxes.
- Token totals agree within about 7% averaged over both models (A 6320, C 6422, D 6527, B 6751). Token count does
  not decide between them.

## Ranking

1. **A**: no Haiku gap, all semantics pass, cheapest, and staging is visible in the syntax.
2. **D**: best ceiling (Sonnet nearly clean), but the weaker model fails it in a predictable way.
3. **B**: fewest Sonnet errors after D, but kinds are implicit, and Haiku broke the gaze.
4. **C**: most verbose, the most unit and space typing errors, and the largest violation count for Haiku.

## Best syntax: A

A is the one to build. It is the only syntax where the weaker model made no more errors than the stronger one. Its
failures were local slips, not structural ones. The rule named in the rubric ("unstaged loops") is enforced by the
keyword on the loop. D shows that without that keyword, a model falls back to writing a script.

Ideas to graft onto A:

- **From C/D: the map as a value, `sym.p` / `sym.pt(..)`**, in place of the `'sym` tick-space and `sym - eye_c`.
  A-Haiku's map at file scope came from the declaration-like `map sym = ...`. Binding the map to the query point
  as a value makes that mistake hard to write.
- **From B/C/D: a typed verdict, `Verdict[Meters]` / `Verdict[Rad]`.** The A card shows a bare `Verdict`.
- **From D: keyword arguments when emitting a row** are already close to A's struct literal. Keep A's form.
- **New, for the most common error: field-level write ownership,** `law neck(h: Heron) writes Heron.{neck,
  gaze_err}`. Two laws on one kind become legal when their fields are disjoint, and the compiler can check the
  overlap. This is my own proposal; no syntax in the test had it.
- **New: generic material helpers,** `fn chitin[S](p: Pt[S])`, shown once on the card. This fixes the E104 slip
  that both Sonnets made.
- **For every card:** define `leg_socket` and show how `sun_sky` and `sky_color` are declared. Every output
  invented its own light and sky.


================================================================
# VERDICT ALL (A-E)
# file: agent_notes/2026-11-20_syntax_bakeoff/VERDICT_ALL.md
# what it is: the judge's scores including the terse candidate E (if finished)
================================================================

# Loft v1.0 syntax bake-off: verdict on all five cards (A–E)

Judged fresh on 2026-11-19. `VERDICT.md` was not read. Cards F and G and their outputs are out of scope.
Tokens are counted with tiktoken `o200k_base`, run through uv. Each output's body is split at its `Task N` headings.

| Card | Surface |
|---|---|
| A | WGSL plus declarations (`kind`/`row`, `comptime for`, `map 'sym`, `law … writes`, `schedule { pass … }`) |
| B | WGSL plus modules and lambdas: `add(w, field(box, \|p\| …), mat, attachment)`, where captured values become row columns |
| C | Slang/HLSL: `[Kind] struct`, interfaces, `technique`/`pass` |
| D | Python surface: `@kind class`, `Pt["Leg"]`, `0.3*m` |
| E | Terse custom syntax: `kind K @S (row) { q(p) = expr }`, `[x,y,z]@S`, `f'(u)`, module code runs at build, 10 parse rules |

## How each output was scored

- **Violations** are breaches of the output's own card: a needless raw escape, a missing or mistyped space conversion
  through the mirror, unstaged build code, a divisor on row data that can reach zero (E211), two laws writing one row
  where the card forbids it, a helper used but never written, unit or space mismatches, and Python runtime use.
- **Not found anywhere:** continuous kind parameters (E401) and authored Lipschitz bounds. Every variant parameter
  was a `bool`.
- **Semantics** scores four checks, each 0, ½ or 1:
  - **Legs:** 12 legs are one kind with rows, and a second crab adds rows without new code.
  - **Reeds:** 200 reeds are one kind.
  - **Gaze:** each eye's world position is computed through the mirror.
  - **Heron:** the neck limit records the gaze as missed and hands off to a body turn.

## Per-output scores

| Output | T1 tok | T2 tok | T3 tok | Total | Violations | Semantics /4 |
|---|---|---|---|---|---|---|
| A sonnet | 2667 | 1097 | 2002 | 5778 | 3 | 3.5 |
| A haiku | 3074 | 1292 | 2561 | 6952 | 2 | 4 |
| B sonnet | 2781 | 1550 | 2576 | 6919 | 1 | 3.5 |
| B haiku | 2639 | 1342 | 2663 | 6671 | 3 | 2 |
| C sonnet | 2965 | 1314 | 2594 | 6885 | 4 | 3.5 |
| C haiku | 2755 | 1062 | 2210 | 6039 | 4 | 3.5 |
| D sonnet | 3252 | 1419 | 2758 | 7441 | 0 | 4 |
| D haiku | 2630 | 1018 | 2042 | 5702 | 2 | 3.5 |
| E sonnet | 1876 | 998 | 1777 | 4670 | 1 | 4 |
| E haiku | 2309 | 823 | 1603 | 4747 | 2 | 3 |

## Per-syntax tables

### A: WGSL superset (card 1695 tokens)

| Measure | Result |
|---|---|
| Tokens per task | 2122 on average (both models) |
| Violations | Sonnet 3: raw `hash1` that plain WGSL could write; `chitin(Pt[Leg])` called with `Pt[Claw]` and `Pt[Body]`; `/self.stiff` and `/self.height` unclamped. Haiku 2: module-level `map sym = mirror_x(q)` with `q` unbound; `/self.size` in the claw. |
| Semantics | Legs, reeds and gaze correct in both. In Sonnet's heron the head geometry never follows `neck_yaw`, and the gaze is measured from the body origin, not the eye. |
| Cold readability | 4. Anyone who knows WGSL reads it at once. `Pt[Leg](…)` on every literal and named `pass x:` labels are noise. |
| Haiku–Sonnet gap | About zero. Haiku is slightly better on semantics but uses 20% more tokens. The syntax is forgiving. |
| Parser | WGSL (~250 productions) plus ~15 declaration forms, ~60 extra rules. Ambiguities: `Pt[S](…)` is a type constructor in WGSL's index position; the `'sym` space lexeme; unit suffixes on numbers. |

### B: WGSL + lambdas/`add` (card 1695)

| Measure | Result |
|---|---|
| Tokens per task | 2265 on average, the highest |
| Violations | Sonnet 1: E211 divisors. Haiku 3: two laws return the `Heron` row, which the card forbids; `m_per_s_dt` and `leg_socket` are never written; the eye map is evaluated at comptime with `p` unbound and `time()` frozen, so there is one eye position and no mirror conversion. |
| Semantics | Sonnet 3.5: the legs are captured per `add`, and a second crab means copying the scene code. Haiku 2: the gaze fails; the legs use the sand material; the reed bounding `box` is too small for the sway. |
| Cold readability | 2. Whether a value is a row column or code depends on what a lambda captures. That is invisible on the page, and `s, w` are threaded through every schedule call. |
| Haiku–Sonnet gap | The largest: violations 3 vs 1, semantics 2 vs 3.5. The least forgiving syntax. |
| Parser | WGSL plus lambdas plus modules is moderate. The cost is in the middle end: capture analysis decides which values become row columns and which kinds merge (the same functions share a kind), and authored `box` bounds are required. |

### C: Slang/HLSL (card 1733)

| Measure | Result |
|---|---|
| Tokens per task | 2154 on average |
| Violations | Sonnet 4: `__wgsl` hash that plain code could write; `chitin(Pt<Body>)` space mismatch; `leg_socket`, `claw_socket` and `eye_socket` never written; E211. Haiku 4: `0.8m * size` gives m²; `Pt<Face> eye_c = sym.pt(…)` mislabels the mirrored space; `sin(0.3 * time)` on a `Sec`; E211. |
| Semantics | Sonnet's crab never draws the body, shell or eye (they are missing from `build_indirect`), and its heron head ignores the yaw. Haiku's heron parts all have identity poses. |
| Cold readability | 3. Familiar to shader writers, but heavy: designated initialisers, `Pt<sym.Space>`, attributes. |
| Haiku–Sonnet gap | About zero, but both models are bad: the most violations in total. |
| Parser | The largest grammar: C declarators, `<…>` generics (the `a < b > c` type-table ambiguity), attributes, interfaces, `technique`. It still has to lower to WGSL. |

### D: Python surface (card 1680)

| Measure | Result |
|---|---|
| Tokens per task | 2190 on average |
| Violations | Sonnet 0: it clamps every divisor (`max(self.stiff, 0.0)`, `max(self.height, 0.2*m)`). Haiku 2: build code at module level (`bed(200)`, `crab(...)`, a bare `emit(Water())`), which is unstaged; and `random.Random(42)`, Python runtime that the card forbids. |
| Semantics | Sonnet 4/4, the best single output. Haiku's reed placement uses the Python runtime. |
| Cold readability | 4. Python reads easily. `Pt["Leg"]` strings, `0.07*m` and `num()` unit stripping are noise. |
| Haiku–Sonnet gap | Moderate: violations 2 vs 0 and 23% fewer tokens. Haiku's mistakes are false friends from Python, and the surface invites host-language habits. |
| Parser | Parsing is cheap (Python's `ast`). Defining and checking the subset is expensive: reject lists, dicts, closures, recursion and module RNG; units arrive as multiplication, so unit inference must run over dataflow. Spaces are strings. |

### E: terse custom syntax (card 1527, the smallest)

| Measure | Result |
|---|---|
| Tokens per task | **1570** on average: −26% vs A, −31% vs B. Both models land within 2% of each other. |
| Violations | Sonnet 1: E211 `p.y / height`. Haiku 2: `fish: Fish` used where the card's table type is `[Fish]`; E211 `q.x / size`. |
| Semantics | Sonnet 4/4. Haiku 3: it emits two `Eye` rows *and* mirrors each one, giving four eyes, and the trunk always turns +yaw whichever side the fish is on. |
| Cold readability | 4. `kind CrabLeg @Leg (parent: CrabBody, …) { trace(p) = … }` reads like the spec. A stranger asks what type `[x,y,z]` has, and what `spine'` means. |
| Haiku–Sonnet gap | Small to moderate: one violation and one semantic point apart; tokens equal. |
| Parser | The smallest grammar (~80 productions) with 10 explicit disambiguation rules (newline handling, `>` only after `:`/`->`, `'` only for derivatives, `wgsl {` raw to its balanced `}`). The cost moves to bidirectional type inference for context-typed `[..]` literals, spaces and units. |

## Ranking by tokens × violations

Tokens are the total over both outputs. Violations are the total over both. A (v+1) column is shown so that zero does not
dominate.

| Rank | Card | Tokens | Violations | T×V | T×(V+1) | Semantics /8 |
|---|---|---|---|---|---|---|
| 1 | **E** | 9417 | 3 | 28.3k | **37.7k** | 7 |
| 2 | D | 13143 | 2 | 26.3k | 39.4k | 7.5 |
| 3 | B | 13590 | 4 | 54.4k | 67.9k | 5.5 |
| 4 | A | 12730 | 5 | 63.7k | 76.4k | 7.5 |
| 5 | C | 12924 | 8 | 103.4k | 116.3k | 7 |

D and E are statistically tied on T×V. E wins on tokens by a margin a resampling would not erase (−28%); D wins on
violations by one.

## Recommendation for v1.0: **E**, with these grafts

E is the only card that cuts the token bill substantially: about 26% below the WGSL superset (A), for both the output
and the card itself. It does this without raising violations or the Haiku–Sonnet gap. On this evidence WGSL is the
verbose option. Make it the lowering target and the `wgsl {}` escape, not the surface.

Grafts:

1. **A kind-scoped `map`** (from A's named `map sym = …`). Declare a map once per kind, and every query sees the same
   `sym`. This removes the mirror chain recomputed in `trace` and in `material`, seen in D sonnet, D haiku, E sonnet
   and E haiku. Add a lint: two rows that differ only by a mirrored socket are an error. That catches E haiku's four eyes.
2. **A typed error for a bare kind used as a table** (A/C/D all spell `Table[...]`). In a law or check parameter, `x: K`
   with no `[K]` is an error, unless it is the first, written row. This catches E haiku's `fish: Fish`.
3. **Field-level write ownership** (from A/C/D, "only it writes those fields"), replacing E's whole-row rule. The heron
   naturally splits into body fields and neck fields. Field ownership keeps the clean A haiku and D haiku layouts legal,
   and B haiku's whole-row breach shows the row rule is the one models break.
4. **E211 with a remedy built in** (from D sonnet's guarded divisors). Let a row column carry a range,
   `stiff: f in 0.2..1.4`, so the compiler can prove the divisor nonzero. A bare row divisor is then an error with the
   fix named. Unclamped row divisors were the most common violation (7 of 10 outputs).
5. **Typed `Verdict` units** (from B/C/D's `Verdict[Meters]`/`Verdict[Rad]`). Write `check gaze(h: Head) -> rad`, so
   that a held margin and a missed angle cannot mix units.
6. **Keep E's staging rule: module code runs at build.** Keep it over D's decorators and A's `comptime fn` plus
   `pass rows:`. No E output staged a loop wrongly. D haiku did, and both A outputs spent a line on it.
7. **Drop:** B's capture-to-column inference (the widest gap, invisible on the page), C's declarator grammar, and A's
   and C's named passes.


================================================================
# SONNET OUTPUT A
# file: agent_notes/2026-11-20_syntax_bakeoff/out_A_sonnet.md
# what it is: the three tasks written by Sonnet in syntax A
================================================================

# Task 1

```
space World, Body, Leg, Claw, Face, Shell;

fn spine(u: f32) -> Pt[Shell] {
  let a = 14.0rad * u;
  let r = 0.02m * exp(1.6 * u);
  return Pt[Shell](r * cos(a), 0.05m * u * 8.0, r * sin(a));
}

scan whorl(u: f32 in 0.0..1.0, steps: 256u) @schedule(bake_lut) -> Table[Frame[Shell]] {
  init: frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir[Shell](0.0, 1.0, 0.0));
  step(prev, u_i): transport_frame(prev, spine(u_i), normalize(diff(spine, u)(u_i)));
}

kind ShellKind {
  row { pose: Xform[Shell -> World] }
  @grid(256u, 48u) query surface(self, u: f32, v: f32) -> Pt[Shell] {
    let f = sample_c2(whorl, u);
    let th = 6.2831853rad * v;
    let rad = 0.012m * exp(1.6 * u) * (1.0 + 0.12 * sin(24.0 * th));
    return f.origin + f.normal * (rad * cos(th)) + f.binormal * (rad * sin(th));
  }
  query material(self, p: Pt[Shell], n: Norm[Shell]) -> Bsdf {
    return ggx(vec3(0.85, 0.72, 0.6), 0.45, 0.0) + clearcoat(0.3, 0.2);
  }
}

kind Claw {
  row { pose: Xform[Claw -> World], open: Rad, size: Meters, palm: Meters }
  query trace(self, p: Pt[Claw]) -> Field[Claw] {
    let palm = sd_round_box(p, Vec[Claw](self.palm, self.palm * 0.6, self.palm * 0.5), 0.2 * self.palm);
    let fixed = sd_capsule(p, Pt[Claw](self.palm, 0.0, 0.0), Pt[Claw](self.palm + self.size, 0.0, 0.0), 0.3 * self.palm);
    let c = cos(self.open);
    let s = sin(self.open);
    let tip = Pt[Claw](self.palm + self.size * c, self.size * s, 0.0);
    var mv = sd_capsule(p, Pt[Claw](self.palm, 0.0, 0.0), tip, 0.28 * self.palm);
    let along = dot(p - Pt[Claw](self.palm, 0.0, 0.0), Vec[Claw](c, s, 0.0));
    let teeth = 0.02m * self.palm * abs(sin(along * 60.0));
    mv = mv + teeth * 0.5;
    return smin_poly(smin_poly(palm, fixed, 0.03m), mv, 0.03m);
  }
  query material(self, p: Pt[Claw], n: Norm[Claw]) -> Bsdf { return chitin(p); }
}

kind CrabBody {
  row { pose: Xform[Body -> World], com: Pt[World], vel: Vec[World], head: Pt[Body], gaze: Pt[World] }
  query trace(self, p: Pt[Body]) -> Field[Body] {
    return smin_poly(sd_round_box(p, Vec[Body](0.35, 0.12, 0.25), 0.05m),
                     sd_sphere(p - Vec[Body](0.3, 0.05, 0.0), 0.14m), 0.05m);
  }
  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf { return chitin(p); }
}

kind CrabLeg {
  row { parent: RowId[CrabBody], pose: Xform[Leg -> World], femur: Meters, tibia: Meters, foot: Pt[World] }
  query trace(self, p: Pt[Leg]) -> Field[Leg] {
    let k = Pt[Leg](self.femur, 0.1m, 0.0);
    let a = sd_capsule(p, Pt[Leg](0.0, 0.0, 0.0), k, 0.035m);
    let b = sd_capsule(p, k, Pt[Leg](self.femur, -self.tibia, 0.0), 0.022m);
    return smin_poly(a, b, 0.02m);
  }
  query material(self, p: Pt[Leg], n: Norm[Leg]) -> Bsdf { return chitin(p); }
}

kind EyeStalk {
  row { parent: RowId[CrabBody], pose: Xform[Face -> World], gaze: Pt[World] }
  query trace(self, p: Pt[Face]) -> Field[Face] {
    map sym = mirror_x(p) |> translate(-Vec[Face](0.15, 0.72, 0.42));
    let eye_c = Pt['sym](0.03, 0.30, 0.07);
    let stalk = sd_capsule(sym - eye_c, Pt['sym](0.0, -0.2, 0.0), Pt['sym](0.0, 0.0, 0.0), 0.02m);
    let globe = sd_sphere(sym - eye_c, 0.085m);
    let eye_w = self.pose * (sym.to_parent * eye_c);
    let look = sym.from_parent * (inv(self.pose) * normalize(self.gaze - eye_w));
    let pupil = sd_sphere(sym - (eye_c + look * 0.08m), 0.035m);
    return min(smin_poly(stalk, globe, 0.02m), pupil);
  }
  query material(self, p: Pt[Face], n: Norm[Face]) -> Bsdf {
    return ggx(vec3(0.05, 0.04, 0.04), 0.1, 0.0) + clearcoat(0.9, 0.03);
  }
}

fn chitin(p: Pt[Leg]) -> Bsdf {
  let rough = 0.30 + 0.15 * fbm2(vec2(p.x, p.z) * 90.0);
  return ggx(vec3(0.85, 0.28, 0.1), rough, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}

kind Sand {
  row { pose: Xform[World -> World] }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y + 0.004m * fbm2(vec2(p.x, p.z) * 30.0); }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.35, 0.28, 0.2), 0.25, 0.0) + clearcoat(0.5, 0.05);
  }
}

kind Sun {
  row { dir: Dir[World], color: vec3<f32> }
  query light(self, p: Pt[World]) -> vec3<f32> { return self.color; }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return mix(vec3(0.7, 0.8, 0.9), vec3(0.3, 0.5, 0.8), rd.y); }
}

comptime fn leg_socket(j: u32, side: f32) -> Xform[Leg -> Body] {
  return xform_translate(Vec[Body](0.12 * f32(j) - 0.25, 0.0, side * 0.22)) * xform_rot_y(side * 1.2rad);
}

comptime fn crab(origin: Pt[World], yaw: Rad) {
  let body = xform_translate(origin - Pt[World](0.0, 0.0, 0.0)) * xform_rot_y(yaw);
  let b = emit CrabBody { pose: body, com: origin, vel: Vec[World](0.0, 0.0, 0.0),
                          head: Pt[Body](0.4, 0.1, 0.0), gaze: Pt[World](1.0, 0.4, 1.0) };
  comptime for j in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg { parent: b, pose: body * leg_socket(j, side), femur: 0.14m + 0.01m * f32(j),
                     tibia: 0.18m - 0.01m * f32(j), foot: body * Pt[Body](0.12 * f32(j) - 0.25, 0.0, side * 0.5) };
    }
  }
  emit Claw { pose: body * xform_translate(Vec[Body](0.4, 0.0, -0.25)), open: 0.5rad, size: 0.22m, palm: 0.1m };
  emit Claw { pose: body * xform_translate(Vec[Body](0.4, 0.0, 0.25)), open: 0.2rad, size: 0.1m, palm: 0.05m };
  emit EyeStalk { parent: b, pose: body * xform_translate(Vec[Body](0.4, 0.1, 0.0)), gaze: Pt[World](1.0, 0.4, 1.0) };
  emit ShellKind { pose: body * xform_translate(Vec[Body](-0.15, 0.15, 0.0)) };
}

comptime fn scene() {
  emit Sand { pose: xform_identity() };
  emit Sun { dir: normalize(Dir[World](-0.4, 0.8, 0.3)), color: vec3(1.0, 0.95, 0.85) };
  crab(Pt[World](0.0, 0.0, 0.0), 0.0rad);
}

law gaze_drift(e: EyeStalk, t: Sec) writes EyeStalk {
  e.gaze = Pt[World](1.0 + 0.5 * sin(0.2 * f32(t)), 0.4, 1.0 + 0.5 * cos(0.15 * f32(t)));
}

schedule Main {
  pass rows: scene();
  pass motion: run_laws(gaze_drift);
  pass camera: orbit(at: Pt[World](0.0, 0.15, 0.0), radius: 1.2m, fov: 40deg);
  pass cull: build_indirect(Sand, ShellKind, Claw, CrabBody, CrabLeg, EyeStalk);
  pass vis: render_bundle(Vis64) { draw_indirect(Sand, ShellKind, Claw, CrabBody, CrabLeg, EyeStalk); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

# Task 2

```
space World, Reed;

kind Reed {
  row { pose: Xform[Reed -> World], height: Meters, lean: Rad, stiff: f32, phase: Rad }
  query trace(self, p: Pt[Reed], time: Sec) -> Field[Reed] {
    let h = clamp(p.y / self.height, 0.0, 1.0);
    let gust = sin(f32(time) * 1.7 - 0.8 * (self.pose * p).x + self.phase);
    let bend = (self.lean + 0.5rad * gust / self.stiff) * h * h;
    let q = Pt[Reed](p.x - bend * p.y, p.y, p.z);
    let r = 0.012m * (1.0 - 0.6 * h);
    let stalk = sd_capsule(q, Pt[Reed](0.0, 0.0, 0.0), Pt[Reed](0.0, self.height, 0.0), r);
    let head = sd_capsule(q, Pt[Reed](0.0, self.height - 0.12m, 0.0), Pt[Reed](0.0, self.height, 0.0), 0.025m);
    return min(stalk, head);
  }
  query material(self, p: Pt[Reed], n: Norm[Reed]) -> Bsdf { return ink(vec3(0.15, 0.1, 0.08), 0.0); }
}

@lobe fn ink(dark: vec3<f32>, cut: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> {
  return vec4(select(vec3(0.92, 0.85, 0.7), dark, dot(n, l) < cut), 0.0);
}

kind Water {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ink(vec3(0.92, 0.85, 0.7) * 0.9, -2.0) + emission(vec3(0.02, 0.03, 0.04) * fbm2(vec2(p.x, p.z) * 4.0));
  }
}

kind Sun {
  row { dir: Dir[World] }
  query light(self, p: Pt[World]) -> vec3<f32> { return vec3(1.0, 1.0, 1.0); }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return vec3(0.92, 0.85, 0.7) - 0.04 * fbm2(vec2(rd.x, rd.z) * 3.0); }
}

fn hash1(i: u32) -> f32 {
  raw wgsl (i: u32) -> f32 { return fract(sin(f32(i) * 12.9898) * 43758.5453); }
}

comptime fn bed() {
  comptime for i in 0u..200u {
    let a = 6.2831853 * hash1(i * 3u);
    let r = 1.5 * sqrt(hash1(i * 3u + 1u)) * (0.6 + 0.8 * hash1(i * 3u + 2u));
    emit Reed { pose: xform_translate(Vec[World](r * cos(a), 0.0, r * sin(a))),
                height: 1.1m + 0.5m * hash1(i * 7u), lean: 0.1rad * (hash1(i * 11u) - 0.5),
                stiff: 0.6 + 0.8 * hash1(i * 13u), phase: 6.2831853rad * hash1(i * 17u) };
  }
  emit Water { level: 0.0m };
  emit Sun { dir: normalize(Dir[World](-0.5, 0.8, 0.3)) };
}

schedule Main {
  pass rows: bed();
  pass camera: orbit(at: Pt[World](0.0, 0.7, 0.0), radius: 4m, fov: 35deg);
  pass cull: build_indirect(Water, Reed);
  pass vis: render_bundle(Vis64) { draw_indirect(Water, Reed); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

# Task 3

```
space World, Body, Neck, Head;

kind Heron {
  row { pose: Xform[Body -> World], hips: Pt[Body], head_pos: Pt[World], yaw: Rad, neck_yaw: Rad,
        limit: Rad, hop_t: Sec, hopping: bool, vel: Vec[World], gaze: Pt[World], head: Xform[Head -> World] }
  query trace(self, p: Pt[Body]) -> Field[Body] {
    let trunk = sd_capsule(p, Pt[Body](-0.15, 0.55, 0.0), Pt[Body](0.1, 0.6, 0.0), 0.11m);
    let legs = min(sd_capsule(p, Pt[Body](0.0, 0.0, 0.05), Pt[Body](0.0, 0.5, 0.05), 0.012m),
                   sd_capsule(p, Pt[Body](0.0, 0.0, -0.05), Pt[Body](0.0, 0.5, -0.05), 0.012m));
    let neck = min(sd_capsule(p, Pt[Body](0.1, 0.6, 0.0), Pt[Body](0.2, 0.75, 0.0), 0.03m),
                   min(sd_capsule(p, Pt[Body](0.2, 0.75, 0.0), Pt[Body](0.15, 0.9, 0.0), 0.025m),
                       sd_capsule(p, Pt[Body](0.15, 0.9, 0.0), Pt[Body](0.25, 1.0, 0.0), 0.025m)));
    let head = sd_sphere(p - Vec[Body](0.28, 1.02, 0.0), 0.04m);
    let beak = sd_capsule(p, Pt[Body](0.3, 1.02, 0.0), Pt[Body](0.45, 1.02, 0.0), 0.012m);
    return min(min(smin_poly(trunk, legs, 0.03m), neck), min(head, beak));
  }
  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf { return ggx(vec3(0.55, 0.6, 0.65), 0.6, 0.0); }
}

kind Fish {
  row { pos: Pt[World] }
  query trace(self, p: Pt[World]) -> Field[World] { return sd_sphere(p - (self.pos - Pt[World](0.0, 0.0, 0.0)), 0.05m); }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf { return ggx(vec3(0.8, 0.8, 0.85), 0.2, 1.0); }
}

kind Ground {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf { return ggx(vec3(0.3, 0.35, 0.3), 0.5, 0.0); }
}

kind Sun {
  row { dir: Dir[World] }
  query light(self, p: Pt[World]) -> vec3<f32> { return vec3(1.0, 0.95, 0.9); }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return vec3(0.6, 0.75, 0.9); }
}

fn fish_curve(t: Sec) -> Pt[World] {
  return Pt[World](1.5 * cos(0.4 * f32(t)) , 0.1, 1.0 + 1.5 * sin(0.4 * f32(t)));
}

fn wrap_pi(a: Rad) -> Rad {
  return atan2(sin(a), cos(a));
}

law fish_swim(f: Fish, h: Table[Heron], t: Sec) writes Fish {
  f.pos = fish_curve(t);
}

law heron_gaze(h: Heron, dt: Sec, fish: Table[Fish]) writes Heron {
  let target = fish[0u].pos;
  h.gaze = target;
  let to = target - h.pose * Pt[Body](0.0, 0.0, 0.0);
  let want = wrap_pi(atan2(to.z, to.x) - h.yaw);
  h.neck_yaw = clamp(want, -h.limit, h.limit);
}

law heron_hop(h: Heron, dt: Sec, fish: Table[Fish]) writes Heron {
  let to = h.gaze - h.pose * Pt[Body](0.0, 0.0, 0.0);
  let want = wrap_pi(atan2(to.z, to.x) - h.yaw);
  if abs(want) > h.limit && !h.hopping { h.hopping = true; h.hop_t = 0.0s; }
  if h.hopping {
    h.hop_t = h.hop_t + dt;
    let u = h.hop_t / 0.4s;
    let y = 0.12m * 4.0 * u * (1.0 - u);
    h.vel = Vec[World](0.0, y, 0.0);
    h.yaw = h.yaw + wrap_pi(want - h.limit * 0.5) * clamp(f32(dt) / 0.4, 0.0, 1.0);
    if u >= 1.0 { h.hopping = false; }
  }
  h.pose = xform_translate(h.vel) * xform_rot_y(h.yaw);
  h.head = h.pose * xform_rot_y(h.neck_yaw) * xform_translate(Vec[Body](0.28, 1.02, 0.0));
  h.head_pos = h.head * Pt[Head](0.0, 0.0, 0.0);
}

constraint gaze_locked(h: Heron) -> Verdict {
  if !is_finite(h.pose) { return Verdict::Unknown(NonFinitePose); }
  let to = h.gaze - h.pose * Pt[Body](0.0, 0.0, 0.0);
  let want = wrap_pi(atan2(to.z, to.x) - h.yaw);
  let over = abs(want) - h.limit;
  if over > 0.0rad { return Verdict::Missed(over); }
  return Verdict::Held(-over);
}

constraint head_above_hips(h: Heron) -> Verdict {
  if !is_finite(h.pose) { return Verdict::Unknown(NonFinitePose); }
  let margin = h.head_pos.y - (h.pose * h.hips).y;
  if margin < 0.0m { return Verdict::Missed(-margin); }
  return Verdict::Held(margin);
}

comptime fn scene() {
  emit Ground { level: 0.0m };
  emit Sun { dir: normalize(Dir[World](-0.4, 0.8, 0.3)) };
  emit Fish { pos: fish_curve(0.0s) };
  emit Heron { pose: xform_identity(), hips: Pt[Body](0.0, 0.5, 0.0), head_pos: Pt[World](0.28, 1.02, 0.0),
               yaw: 0.0rad, neck_yaw: 0.0rad, limit: 1.1rad, hop_t: 0.0s, hopping: false,
               vel: Vec[World](0.0, 0.0, 0.0), gaze: fish_curve(0.0s), head: xform_identity() };
}

schedule Main {
  pass rows: scene();
  pass fish: run_laws(fish_swim);
  pass body: run_laws(heron_hop);
  pass neck: run_laws(heron_gaze);
  pass check: check(gaze_locked, head_above_hips);
  pass camera: orbit(at: Pt[World](0.0, 0.6, 0.0), radius: 4m, fov: 40deg);
  pass cull: build_indirect(Ground, Heron, Fish);
  pass vis: render_bundle(Vis64) { draw_indirect(Ground, Heron, Fish); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```


================================================================
# SONNET OUTPUT B
# file: agent_notes/2026-11-20_syntax_bakeoff/out_B_sonnet.md
# what it is: the three tasks written by Sonnet in syntax B
================================================================

## Task 1

```
import std::render::*;

space Body, Leg, Claw, Face, Shell;

kind CrabBody { row { pose: Xform[Body, World], com: Pt[World], vel: Vec[World], head: Pt[Body], } }
kind CrabLeg  { row { parent: Row[CrabBody], foot: Pt[World], } }

fn tau() -> f32 { return 6.2831853; }

fn drift() -> Pt[World] {
    let t = time() / 1s;
    return Pt[World](0.6 * sin(0.21 * t), 0.35, 1.2 + 0.4 * cos(0.13 * t));
}

fn root() -> Xform[World, World] { return Xform[World, World]::identity(); }

fn leg_socket(k: u32, side: f32) -> Xform[Leg, Body] {
    return rigid[Leg, Body](Vec[Body](side * 0.22, 0.0, 0.18 - 0.11 * f32(k)));
}
fn claw_socket(side: f32) -> Xform[Claw, Body] {
    return rigid[Claw, Body](Vec[Body](side * 0.28, 0.05, 0.42));
}
fn face_socket() -> Xform[Face, Body] {
    return rigid[Face, Body](Vec[Body](0.0, 0.14, 0.40));
}
fn shell_socket() -> Xform[Shell, Body] {
    return rigid[Shell, Body](Vec[Body](0.0, 0.18, -0.30));
}

// ---------- shell: scanned frames along a spiral spine ----------
fn spine(u: Unit) -> Pt[Shell] {
    let a = 18.0 * u;
    let r = 0.015 * exp(0.13 * a);
    return Pt[Shell](r * cos(a), 0.012 * a, r * sin(a));
}
fn tube_r(u: Unit) -> f32 { return 0.004 + 0.045 * exp(2.3 * u) / exp(2.3); }

scan whorl(u: Unit) @schedule(bake_lut, 256u) -> Table[Basis[Shell]] {
    init: frame_seed(spine(0), normalize(diff(spine, u)(0)), Dir[Shell](0, 1, 0));
    step(last, u_i): transport_step(last, spine(u_i), normalize(diff(spine, u)(u_i)));
}

fn shell_point(uv: Vec2) -> Pt[Shell] {
    let f = sample_c2(whorl, uv.x);
    let phi = tau() * uv.y;
    let rib = 1.0 + 0.10 * sin(160.0 * uv.x);
    let r = tube_r(uv.x) * rib;
    return f.origin + (f.normal * cos(phi) + f.binormal * sin(phi)) * r;
}

// ---------- claw: one definition, differing only in data ----------
fn claw_sdf(p: Pt[Claw], open: Rad, s: f32) -> Meters {
    let palm = sd_round_box(p, Vec[Claw](0.16 * s, 0.11 * s, 0.09 * s), 0.03 * s);
    let fixed_finger = sd_capsule(p, Pt[Claw](0.10 * s, -0.06 * s, 0), Pt[Claw](0.46 * s, -0.04 * s, 0), 0.05 * s);
    map h = translate(Vec(-0.10 * s, 0.06 * s, 0)) |> rotate(Vec(0, 0, 1), open);
    let teeth = 0.010 * s * max(sin(70.0 * h.p.x / s), 0.0);
    let moving = sd_capsule(h.p, Pt['h](0, 0, 0), Pt['h](0.46 * s, 0, 0), 0.045 * s) - teeth;
    return smin_poly(smin_poly(palm, fixed_finger, 0.03 * s), moving, 0.02 * s);
}

comptime fn add_claw(w: World, crab: Row[CrabBody], side: f32, open: Rad, size: f32) {
    let at = attachment(crab, |r| r.pose * claw_socket(side));
    add(w, field(box(Pt[Claw](-0.3 * size, -0.4 * size, -0.2 * size), Pt[Claw](0.7 * size, 0.4 * size, 0.2 * size)),
                 |p| claw_sdf(p, open, size)), wet, at);
}

// ---------- eyes: one mirrored definition ----------
fn eye_stalk(p: Pt[Face], xf: Xform[Face, World]) -> Meters {
    map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
    let c = Pt['e](0.03, 0.30, 0.07);
    let stalk = sd_capsule(e.p, Pt['e](0.03, 0.0, 0.0), c, 0.025m);
    let globe = length(e.p - c) - 0.085m;
    return min(stalk, globe);
}
fn eye_pupil(p: Pt[Face], xf: Xform[Face, World], target: Pt[World]) -> Meters {
    map e = mirror_x(p) |> translate(Vec(-0.15, -0.72, -0.42));
    let c = Pt['e](0.03, 0.30, 0.07);
    let eye_w = xf * (e.to_parent * c);
    let look = e.from_parent.dir(inverse(xf) * normalize(target - eye_w));
    return length(e.p - (c + 0.075m * look)) - 0.03m;
}

// ---------- materials ----------
fn wet(h: Hit) -> Bsdf {
    let n = fbm2(vec2(h.p.x + h.p.z, h.p.y + h.p.z) * 90.0);
    let rough = 0.22 + 0.22 * n;
    return coat(sss(ggx(rgb(0.6, 0.14, 0.05), rough, 0.0), rgb(0.9, 0.3, 0.1), 0.02m), 0.7, 0.07, 1.34);
}
fn pupil_mat(h: Hit) -> Bsdf { return ggx(rgb(0.02, 0.02, 0.02), 0.1, 0.0); }
fn sand(h: Hit) -> Bsdf {
    return coat(ggx(rgb(0.42, 0.34, 0.24), 0.7, 0.0), 0.5, 0.2, 1.33);
}

// ---------- laws ----------
pub law body_law(b: CrabBody, dt: Sec) -> CrabBody { return b; }

pub law crab_step(leg: CrabLeg, bodies: Table[CrabBody], dt: Sec) -> CrabLeg {
    let b = bodies[leg.parent];
    var next = leg;
    let cp = b.com + b.vel * 0.28s;
    if distance(leg.foot, cp) > 0.5m { next.foot = Pt[World](cp.x, 0, cp.z); }
    return next;
}

pub constraint upright(b: CrabBody) -> Verdict[Meters] {
    if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
    let margin = (b.pose * b.head).y - 0.15m;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}

// ---------- build ----------
pub comptime fn scene() -> Scene {
    let w = world();
    let crab_xf = rigid[Body, World](Vec[World](0, 0.05, 0));
    let crab = emit CrabBody {
        pose: crab_xf, com: Pt[World](0, 0.05, 0), vel: Vec[World](0, 0, 0), head: Pt[Body](0, 0.3, 0.4),
    };

    // sand
    add(w, plane(Dir[World](0, 1, 0), 0m), sand, attachment(crab, |r| root()));

    // body
    add(w, sphere(0.2m), wet, attachment(crab, |r| r.pose * rigid[Body, Body](Vec[Body](0, 0.1, 0.1))));

    // shell
    add(w, surface(|uv| shell_point(uv), grid(512u, 48u)), wet, attachment(crab, |r| r.pose * shell_socket()));

    // legs: six per side
    comptime for i in 0u..12u {
        let side = select(-1.0, 1.0, i % 2u == 1u);
        let k = i / 2u;
        emit CrabLeg { parent: crab, foot: Pt[World](side * 0.5, 0, 0.2 - 0.12 * f32(k)) };
        let at = attachment(crab, |r| r.pose * leg_socket(k, side));
        let femur = 0.14m + 0.02m * f32(k);
        let tibia = 0.18m + 0.015m * f32(k);
        add(w, field(box(Pt[Leg](-0.5, -0.4, -0.1), Pt[Leg](0.5, 0.1, 0.1)),
                     |p| smin_poly(
                         sd_capsule(p, Pt[Leg](0, 0, 0), Pt[Leg](side * femur, 0.05, 0), 0.02m),
                         sd_capsule(p, Pt[Leg](side * femur, 0.05, 0), Pt[Leg](side * (femur + 0.4 * tibia), -tibia, 0), 0.014m),
                         0.01m)), wet, at);
    }

    // claws: same definition, different data
    add_claw(w, crab, -1.0, 0.55rad, 1.0);   // crusher
    add_claw(w, crab, 1.0, 0.25rad, 0.45);   // feeder

    // eyes
    let fxf = crab_xf * face_socket();
    let face_at = attachment(crab, |r| r.pose * face_socket());
    let bx = box(Pt[Face](-0.4, -0.1, -0.3), Pt[Face](0.4, 0.5, 0.3));
    add(w, field(bx, |p| eye_stalk(p, fxf)), wet, face_at);
    add(w, field(bx, |p| eye_pupil(p, fxf, drift())), pupil_mat, face_at);

    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));

    let eye = Pt[World](0.9, 0.55, 1.4);
    let target = Pt[World](0, 0.2, 0);
    let up = Dir[World](0, 1, 0);
    return frame(w, look_at(eye, target, up, 0.6rad), main);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [body_law, crab_step]);
    check(s, w, upright);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Task 2

```
import std::render::*;

space Reed;

kind Reed {
    row { xf: Xform[Reed, World], height: f32, lean: f32, stiffness: f32, phase: f32, }
}

kind Ground { row { } }

fn rnd(i: u32, k: u32) -> f32 {
    var x = i * 747796405u + k * 2891336453u;
    x = (x ^ (x >> 16u)) * 2246822519u;
    x = x ^ (x >> 13u);
    return f32(x & 16777215u) / 16777216.0;
}

fn root() -> Xform[World, World] { return Xform[World, World]::identity(); }

// bend offset (metres) of a reed at height y: grows toward the tip, a gust travels across the bed in +x
fn bend(y: f32, height: f32, lean: f32, stiffness: f32, phase: f32, bx: f32, t: f32) -> f32 {
    let k = (y / height) * (y / height);
    let gust = 0.5 + 0.5 * sin(0.9 * bx - 1.6 * t);
    let flutter = 0.7 + 0.3 * sin(2.3 * t + phase);
    return k * height * (lean + 0.35 * gust * flutter / stiffness);
}

fn stalk(p: Pt[Reed], height: f32, lean: f32, stiffness: f32, phase: f32, bx: f32) -> Meters {
    let t = time() / 1s;
    let y = clamp(p.y, 0.0, height);
    let off = bend(y, height, lean, stiffness, phase, bx, t);
    let r = 0.011 * (1.0 - 0.55 * y / height);
    let d = length(vec2(p.x - off, p.z)) - r;
    return max(d, max(-p.y, p.y - height));
}

fn seed_head(p: Pt[Reed], height: f32, lean: f32, stiffness: f32, phase: f32, bx: f32) -> Meters {
    let t = time() / 1s;
    let top = bend(height, height, lean, stiffness, phase, bx, t);
    let a = Pt[Reed](top, height - 0.02, 0);
    let b = Pt[Reed](top + 0.02 * lean, height + 0.13, 0);
    return sd_capsule(p, a, b, 0.022m);
}

fn paper() -> Rgb { return rgb(0.93, 0.87, 0.74); }
fn ink_a() -> Rgb { return rgb(0.13, 0.17, 0.20); }
fn ink_b() -> Rgb { return rgb(0.45, 0.50, 0.52); }

fn reed_ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let src = light(w, h.p, k);
    return select(paper(), ink_a(), dot(h.n, src.wi) < 0.35);
}
fn head_ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let src = light(w, h.p, k);
    return select(paper(), rgb(0.28, 0.12, 0.08), dot(h.n, src.wi) < 0.35);
}
fn water_ink(w: World, h: Hit, wo: DirW, k: Key) -> Rgb {
    let band = fract(h.p.z * 6.0 + 0.4 * sin(h.p.x * 1.7));
    return select(paper(), mix(paper(), ink_b(), 0.18), band < 0.06);
}

pub comptime fn scene() -> Scene {
    let w = world();
    let ground = emit Ground { };
    add(w, plane(Dir[World](0, 1, 0), 0m), water_ink, attachment(ground, |r| root()));
    comptime for i in 0u..200u {
        let ang = 6.2831853 * rnd(i, 1u);
        let rad = 1.8 * pow(rnd(i, 2u), 0.7);
        let x = rad * cos(ang) * 1.3;
        let z = rad * sin(ang);
        let height = 0.9 + 0.7 * rnd(i, 3u);
        let lean = 0.25 * (rnd(i, 4u) - 0.5);
        let stiffness = 0.6 + 1.0 * rnd(i, 5u);
        let phase = 6.2831853 * rnd(i, 6u);
        let reed = emit Reed {
            xf: rigid[Reed, World](Vec[World](x, 0, z)),
            height: height, lean: lean, stiffness: stiffness, phase: phase,
        };
        let at = attachment(reed, |r| r.xf);
        let bnd = 0.6 * height;
        let bx = x;
        let bounds = box(Pt[Reed](-bnd, 0, -0.1), Pt[Reed](bnd, height + 0.2, 0.1));
        add(w, field(bounds, |p| stalk(p, height, lean, stiffness, phase, bx)), reed_ink, at);
        add(w, field(bounds, |p| seed_head(p, height, lean, stiffness, phase, bx)), head_ink, at);
    }
    add_sun(w, normalize(Vec[World](-2, 4, 1.5)), rgb(3, 3, 3));
    environment(w, |d| select(paper(), mix(paper(), ink_b(), 0.08), d.y > 0.6 && fract(d.x * 9.0) < 0.1));
    let eye = Pt[World](0, 1.0, -5.0);
    let target = Pt[World](0, 0.7, 0);
    let up = Dir[World](0, 1, 0);
    return frame(w, look_at(eye, target, up, 0.5rad), main);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```

## Task 3

```
import std::render::*;

space Body, Neck, Head;

const NECK_LIMIT = 1.1rad;
const HOP_TIME = 0.42s;
const HOP_HEIGHT = 0.16m;
const TURN_RATE = 1.1;          // rad per second while airborne
const GAZE_CLEAR = 0.05;        // rad: hop starts when the neck is this far past its limit

kind HeronBody {
    row { pos: Pt[World], yaw: f32, hop_t: Sec, }
}
kind HeronHead {
    row {
        parent: Row[HeronBody],
        eye: Pt[World], hips: Pt[World],
        rel: f32,            // bearing of the fish relative to the trunk heading
        head_yaw: f32,       // head yaw relative to the trunk, inside the limit
        pitch: f32,
        missed: f32,         // leftover angle beyond the limit; 0 when inside
    }
}

fn fish_pos(t: f32) -> Pt[World] {
    return Pt[World](2.2 * sin(0.45 * t), 0.05 + 0.04 * sin(1.7 * t), 1.6 + 1.2 * cos(0.45 * t) - 1.2);
}
fn wrap(a: f32) -> f32 {
    return a - 6.2831853 * floor((a + 3.1415927) / 6.2831853);
}
fn bearing_rel(pos: Pt[World], yaw: f32, t: f32) -> f32 {
    let f = fish_pos(t);
    return wrap(atan2(f.x - pos.x, f.z - pos.z) - yaw);
}
fn quat_y(a: f32) -> Quat { return quat(0.0, sin(0.5 * a), 0.0, cos(0.5 * a)); }
fn quat_x(a: f32) -> Quat { return quat(sin(0.5 * a), 0.0, 0.0, cos(0.5 * a)); }
fn body_pose(pos: Pt[World], yaw: f32) -> Xform[Body, World] {
    return rigid[Body, World](Vec[World](pos.x, pos.y, pos.z), quat_y(yaw));
}
fn root() -> Xform[World, World] { return Xform[World, World]::identity(); }

// ---------- laws ----------
pub law body_law(b: HeronBody, dt: Sec) -> HeronBody {
    var next = b;
    let t = time() / 1s;
    let rel = bearing_rel(b.pos, b.yaw, t);
    let excess = abs(rel) - NECK_LIMIT / 1rad;
    if b.hop_t > 0s {
        let h = b.hop_t + dt;
        if h >= HOP_TIME {
            next.hop_t = 0s;
            next.pos.y = 0m;
        } else {
            let u = h / HOP_TIME;
            next.hop_t = h;
            next.pos.y = HOP_HEIGHT * 4.0 * u * (1.0 - u);
            let step = min(TURN_RATE * (dt / 1s), abs(rel));
            next.yaw = b.yaw + select(-step, step, rel > 0.0);
        }
    } else if excess > GAZE_CLEAR {
        next.hop_t = dt;
    }
    return next;
}

pub law head_law(h: HeronHead, bodies: Table[HeronBody], dt: Sec) -> HeronHead {
    let b = bodies[h.parent];
    var next = h;
    let t = time() / 1s;
    let pose = body_pose(b.pos, b.yaw);
    let eye = pose * Pt[Body](0, 1.25, 0.42);

    // gaze lock first: aim at the fish in world space from the current eye
    let f = fish_pos(t);
    let rel = wrap(atan2(f.x - eye.x, f.z - eye.z) - b.yaw);
    let horiz = length(vec2(f.x - eye.x, f.z - eye.z));
    let pitch = atan2(f.y - eye.y, horiz);

    // then the neck limit
    let lim = NECK_LIMIT / 1rad;
    next.eye = eye;
    next.hips = pose * Pt[Body](0, 0.55, 0);
    next.rel = rel;
    next.head_yaw = clamp(rel, -lim, lim);
    next.pitch = clamp(pitch, -0.8, 0.8);
    next.missed = max(abs(rel) - lim, 0.0);
    return next;
}

// ---------- constraints ----------
pub constraint gaze_held(h: HeronHead) -> Verdict[Rad] {
    if !is_finite(h.rel) { return Verdict::Unknown(NonFiniteGaze); }
    if h.missed > 0.0 { return Verdict::Missed(h.missed * 1rad); }
    return Verdict::Held((NECK_LIMIT / 1rad - abs(h.rel)) * 1rad);
}

pub constraint head_above_hips(h: HeronHead) -> Verdict[Meters] {
    if !is_finite(h.eye) { return Verdict::Unknown(NonFinitePose); }
    let margin = h.eye.y - h.hips.y - 0.3m;
    if margin < 0m { return Verdict::Missed(-margin); }
    return Verdict::Held(margin);
}

// ---------- materials ----------
fn feather(h: Hit) -> Bsdf { return ggx(rgb(0.52, 0.58, 0.64), 0.6, 0.0); }
fn beak_mat(h: Hit) -> Bsdf { return ggx(rgb(0.92, 0.7, 0.2), 0.35, 0.0); }
fn water(h: Hit) -> Bsdf { return coat(ggx(rgb(0.05, 0.12, 0.14), 0.4, 0.0), 0.8, 0.05, 1.33); }
fn fish_mat(h: Hit) -> Bsdf { return ggx(rgb(0.8, 0.45, 0.25), 0.3, 0.6); }

// ---------- build ----------
pub comptime fn scene() -> Scene {
    let w = world();
    let p0 = Pt[World](0, 0, 0);
    let body = emit HeronBody { pos: p0, yaw: 0.0, hop_t: 0s };
    let head = emit HeronHead {
        parent: body,
        eye: Pt[World](0, 1.25, 0.42), hips: Pt[World](0, 0.55, 0),
        rel: 0.0, head_yaw: 0.0, pitch: 0.0, missed: 0.0,
    };

    let body_at = attachment(body, |r| body_pose(r.pos, r.yaw));
    let head_at = attachment(head, |r| rigid[Head, World](
        Vec[World](r.eye.x, r.eye.y, r.eye.z),
        quat_y(bearing_world(r)) * quat_x(-r.pitch)));
    let neck_at = attachment(body, |r| body_pose(r.pos, r.yaw) * rigid[Neck, Body](Vec[Body](0, 0.75, 0.2)));
    let fixed = attachment(body, |r| root());

    // water
    add(w, plane(Dir[World](0, 1, 0), -0.02m), water, fixed);

    // trunk
    add(w, capsule(Pt[Body](0, 0.6, -0.25), Pt[Body](0, 0.72, 0.2), 0.13m), feather, body_at);

    // two legs
    comptime for i in 0u..2u {
        let sx = select(-0.07, 0.07, i == 1u);
        add(w, capsule(Pt[Body](sx, 0.55, 0), Pt[Body](sx, 0.0, 0.02), 0.014m), feather, body_at);
    }

    // S-shaped neck, in neck space
    add(w, field(box(Pt[Neck](-0.2, -0.1, -0.3), Pt[Neck](0.2, 0.7, 0.3)),
                 |p| smin_poly(
                     smin_poly(sd_capsule(p, Pt[Neck](0, 0.0, 0.0), Pt[Neck](0, 0.2, -0.12), 0.04m),
                               sd_capsule(p, Pt[Neck](0, 0.2, -0.12), Pt[Neck](0, 0.38, 0.1), 0.035m), 0.03m),
                     sd_capsule(p, Pt[Neck](0, 0.38, 0.1), Pt[Neck](0, 0.5, 0.0), 0.03m), 0.03m)),
        feather, neck_at);

    // head and beak, on the head's forward axis (+z)
    add(w, sphere(0.05m), feather, head_at);
    add(w, capsule(Pt[Head](0, 0, 0.04), Pt[Head](0, 0, 0.34), 0.018m), beak_mat, head_at);

    // fish
    add(w, field(box(Pt[World](-4, -0.2, -4), Pt[World](4, 0.5, 4)),
                 |p| length(p - fish_pos(time() / 1s)) - 0.05m), fish_mat, fixed);

    add_sun(w, normalize(Vec[World](-3, 5, 2)), rgb(3, 3, 2.5));
    environment(w, |d| rgb(0.4, 0.5, 0.6));

    let eye = Pt[World](3.5, 1.5, -3.5);
    let target = Pt[World](0, 0.7, 0.5);
    let up = Dir[World](0, 1, 0);
    return frame(w, look_at(eye, target, up, 0.6rad), main);
}

fn bearing_world(h: HeronHead) -> f32 {
    // absolute head yaw = trunk yaw + clamped relative yaw; the trunk yaw is recovered from the fish bearing
    return atan2(fish_pos(time() / 1s).x - h.eye.x, fish_pos(time() / 1s).z - h.eye.z) - (h.rel - h.head_yaw);
}

pub fn main(w: World, c: Camera) -> Schedule {
    let s = plan(w, c);
    run_laws(s, w, [body_law, head_law]);
    check(s, w, gaze_held);
    check(s, w, head_above_hips);
    let n = clear_counts(s, w);
    let v = record(s, w, c, cull(s, w, c, n), vis64());
    let hdr = deferred(s, w, v, classify(s, v, n), sun_and_sky);
    return output(s, hdr, aces);
}
```


================================================================
# SONNET OUTPUT C
# file: agent_notes/2026-11-20_syntax_bakeoff/out_C_sonnet.md
# what it is: the three tasks written by Sonnet in syntax C
================================================================

# Task 1

```hlsl
struct World : ISpace {};
struct Body : ISpace {};
struct Leg : ISpace {};
struct Claw : ISpace {};
struct Shell : ISpace {};
struct Face : ISpace {};

// ---- shell: scanned frames along a spiral spine ----
Pt<Shell> spine(float u)
{
    float a = 14.0 * u;                       // radians of coil
    float r = 0.02 * exp(0.9 * u);            // metres, grows with the spiral
    return Pt<Shell>(r * cos(a), 0.11 * u, r * sin(a));
}

[Scan(steps = 256, range = (0.0, 1.0), BakeLut)]
struct Whorl : IScan<Frame<Shell>>
{
    Frame<Shell> init(float u)
    {
        return frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir<Shell>(0, 1, 0));
    }
    Frame<Shell> step(Frame<Shell> prev, float u)
    {
        return transport_frame(prev, spine(u), normalize(diff(spine, u)(u)));
    }
};

[Kind]
struct ShellRow : ISurface<Shell>, IMaterial<Shell>
{
    RowId<CrabBody> parent;
    Xform<Shell, World> pose;
    float3 albedo;

    [Grid(256, 48)]
    Pt<Shell> surface(float u, float v)
    {
        Frame<Shell> f = sample_c2<Whorl>(u);
        float a = v * 6.2831853;
        float rib = 1.0 + 0.08 * sin(40.0 * u * 6.2831853) ;
        float radius = (0.012 + 0.07 * u) * rib;     // tube grows with the spiral
        return f.origin + (f.normal * cos(a) + f.binormal * sin(a)) * radius;
    }

    Bsdf material(Pt<Shell> p, Norm<Shell> n)
    {
        return ggx(albedo, 0.45, 0.0) + clearcoat(0.3, 0.2);
    }
};

// ---- claw: one definition, differences are data ----

[Kind]
struct ClawRow : ITrace<Claw>, IMaterial<Claw>
{
    RowId<CrabBody> parent;
    Xform<Claw, World> pose;
    Meters size;          // palm radius
    Rad open;             // hinge opening angle, per instance
    float serration;      // tooth amplitude, metres

    Field<Claw> trace(Pt<Claw> p)
    {
        Field<Claw> palm = sd_round_box(p, Vec<Claw>(size, 0.7 * size, 0.6 * size), 0.2 * size);

        // fixed finger
        Field<Claw> fixed = sd_capsule(p, Pt<Claw>(size, 0, 0), Pt<Claw>(2.2 * size, -0.2 * size, 0), 0.22 * size);

        // moving finger, hinged at the palm
        Pt<Claw> h = Pt<Claw>(size, 0.4 * size, 0);
        Vec<Claw> q = p - h;
        float c = cos(open);
        float s = sin(open);
        Pt<Claw> pm = Pt<Claw>(c * q.x + s * q.y, -s * q.x + c * q.y, q.z);
        float teeth = serration * sin(60.0 * pm.x);
        Field<Claw> mover = sd_capsule(pm, Pt<Claw>(0, 0, 0), Pt<Claw>(1.8 * size, -0.2 * size, 0), 0.2 * size) + teeth;

        return smin_poly(smin_poly(palm, fixed, 0.1 * size), mover, 0.05 * size);
    }

    Bsdf material(Pt<Claw> p, Norm<Claw> n)
    {
        return chitin(p);
    }
};

// ---- legs ----
[Kind]
struct CrabLeg : ITrace<Leg>, IMaterial<Leg>
{
    RowId<CrabBody> parent;
    Xform<Leg, World> pose;
    Meters femur;
    Meters tibia;
    Pt<World> foot;

    Field<Leg> trace(Pt<Leg> p)
    {
        Field<Leg> a = sd_capsule(p, Pt<Leg>(0, 0, 0), Pt<Leg>(femur, 0, 0), 0.03m);
        Field<Leg> b = sd_capsule(p, Pt<Leg>(femur, 0, 0), Pt<Leg>(femur, -tibia, 0), 0.022m);
        return smin_poly(a, b, 0.02m);
    }

    Bsdf material(Pt<Leg> p, Norm<Leg> n) { return chitin(p); }
};

// ---- body ----
[Kind]
struct CrabBody : ITrace<Body>, IMaterial<Body>
{
    Xform<Body, World> pose;
    Pt<World> com;
    Vec<World> vel;

    Field<Body> trace(Pt<Body> p)
    {
        return sd_round_box(p, Vec<Body>(0.14m, 0.05m, 0.18m), 0.04m);
    }

    Bsdf material(Pt<Body> p, Norm<Body> n) { return chitin(p); }
};

// ---- eyes: one mirrored definition ----
[Kind]
struct EyeRow : ITrace<Face>, IMaterial<Face>
{
    RowId<CrabBody> parent;
    Xform<Face, World> pose;
    Pt<World> gaze_world;     // written by eye_law

    Field<Face> trace(Pt<Face> q, Sec time)
    {
        let sym = map(q).mirror_x().translate(-Vec<Face>(0.05, 0.0, 0.0));
        let eye_c = sym.pt(0.0, 0.06, 0.0);
        Field<Face> stalk = sd_capsule(sym.p, Pt<sym.Space>(0, 0, 0), eye_c, 0.012m);
        Field<Face> globe = sd_sphere(sym.p - eye_c, 0.028m);

        // gaze target drifts over time, in world space
        Pt<World> target = Pt<World>(0.5 * sin(0.3 * time / 1.0s), 0.1, 0.8 + 0.2 * cos(0.2 * time / 1.0s));
        Pt<World> eye_w = pose * sym.to_parent(eye_c);
        let look = sym.from_parent(inverse(pose) * normalize(target - eye_w));
        Pt<sym.Space> pupil_c = eye_c + Vec<sym.Space>(look.x, look.y, look.z) * 0.024;
        Field<Face> pupil = sd_sphere(sym.p - pupil_c, 0.011m);

        return hit_min(smin_poly(stalk, globe, 0.01m), pupil);
    }

    Bsdf material(Pt<Face> p, Norm<Face> n)
    {
        return ggx(float3(0.02, 0.02, 0.02), 0.1, 0.0) + clearcoat(1.0, 0.03);
    }
};

// ---- material: layered wet chitin, noise only here ----
Bsdf chitin(Pt<Body> p)
{
    float r = 0.35 + 0.2 * fbm2(float2(p.x, p.z) * 90.0);
    return ggx(float3(0.75, 0.22, 0.08), r, 0.0) + sss(float3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07);
}

// ---- sand, light, sky ----
[Kind]
struct Ground : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p) { return p.y + 0.004 * fbm2(float2(p.x, p.z) * 20.0); }
    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ggx(float3(0.32, 0.26, 0.18), 0.2, 0.0) + clearcoat(0.5, 0.05);
    }
};

[Kind]
struct SunLight : ILight
{
    Dir<World> dir;
    float3 color;
};

[Kind]
struct SkyRow : ISky
{
    float3 zenith;
    float3 horizon;
};

// ---- laws ----
[Law(Writes = CrabBody)]
void body_law(inout CrabBody b, Sec dt)
{
    b.vel = Vec<World>(0.02, 0.0, 0.0);
    b.com = b.com + b.vel * dt;
}

[Law(Writes = CrabLeg)]
void crab_step(inout CrabLeg leg, Sec dt, Table<CrabBody> bodies)
{
    CrabBody b = bodies[leg.parent];
    Pt<World> cp = b.com + b.vel * 0.28s;
    if (distance(leg.foot, cp) > 0.5m) leg.foot = Pt<World>(cp.x, 0.0, cp.z);
}

// ---- rows the build emits ----
[Comptime]
void crab(RowId<CrabBody> b, Xform<Body, World> body)
{
    for (uint j = 0; j < 6; j++)
        for (float side = -1.0; side <= 1.0; side += 2.0)
            emit(CrabLeg{ .parent = b, .pose = body * leg_socket(j, side),
                          .femur = 0.10m + 0.012m * j, .tibia = 0.09m + 0.01m * j,
                          .foot = body * Pt<Body>(side * 0.25, 0.0, 0.05 * j - 0.12) });

    emit(ClawRow{ .parent = b, .pose = body * claw_socket(-1.0), .size = 0.05m, .open = 0.45rad, .serration = 0.004 });
    emit(ClawRow{ .parent = b, .pose = body * claw_socket(1.0),  .size = 0.025m, .open = 0.2rad, .serration = 0.0 });

    emit(EyeRow{ .parent = b, .pose = body * eye_socket(), .gaze_world = Pt<World>(0, 0, 0) });
    emit(ShellRow{ .parent = b, .pose = body * shell_socket(), .albedo = float3(0.85, 0.7, 0.55) });
}

[Comptime]
void crabs()
{
    Xform<Body, World> body = translate_xform<Body, World>(Vec<World>(0.0, 0.08, 0.0));
    RowId<CrabBody> b = emit(CrabBody{ .pose = body, .com = Pt<World>(0.0, 0.08, 0.0), .vel = Vec<World>(0, 0, 0) });
    crab(b, body);
    emit(Ground{});
    emit(SunLight{ .dir = normalize(Dir<World>(-0.4, -0.8, -0.3)), .color = float3(1.0, 0.95, 0.85) });
    emit(SkyRow{ .zenith = float3(0.35, 0.55, 0.85), .horizon = float3(0.8, 0.85, 0.9) });
}

[Constraint]
Verdict<Meters> crab_on_sand(CrabBody b)
{
    if (!is_finite(b.pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = b.com.y - 0.02m;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}

technique Main
{
    pass Rows     { crabs(); }
    pass Motion   { run_laws(body_law, crab_step); }
    pass Check    { check(crab_on_sand); }
    pass Camera   { orbit(Pt<World>(0.0, 0.1, 0.0), 0.9m, 25deg); }
    pass Cull     { build_indirect<Ground, CrabLeg, ClawRow>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Ground, CrabLeg, ClawRow>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

# Task 2

```hlsl
struct World : ISpace {};
struct Reed : ISpace {};

// wind bend angle at height fraction h along the stalk; gust travels along +x
Rad bend_angle(Pt<World> base, float h, Sec time, Rad lean, float stiffness, Rad phase)
{
    float gust = 0.5 + 0.5 * sin(1.2 * (base.x / 1.0m) - 1.5 * (time / 1.0s));
    float sway = sin(2.1 * (time / 1.0s) + phase / 1.0rad);
    return lean + (0.35rad * gust + 0.08rad * sway) * h * h / (0.5 + stiffness);
}

[Kind]
struct ReedRow : ITrace<Reed>, IMaterial<Reed>
{
    Xform<Reed, World> pose;
    Meters height;
    Rad lean;
    float stiffness;
    Rad phase;

    Field<Reed> trace(Pt<Reed> p, Sec time)
    {
        // bend: shear the point back by the arc displacement at its height
        float h = clamp(p.y / height, 0.0, 1.0);
        Rad a = bend_angle(pose * Pt<Reed>(0, 0, 0), h, time, lean, stiffness, phase);
        float c = cos(a);
        float s = sin(a);
        Pt<Reed> q = Pt<Reed>(c * p.x + s * p.y - s * 0.0, -s * p.x + c * p.y, p.z);
        Pt<Reed> tip = Pt<Reed>(0.0, height, 0.0);
        Meters r = 0.006m * (1.0 - 0.6 * h);
        Field<Reed> stalk = sd_capsule(q, Pt<Reed>(0, 0, 0), tip, r);
        Field<Reed> head = sd_capsule(q, tip, tip + Vec<Reed>(0, 0.09m, 0), 0.014m);
        return smin_poly(stalk, head, 0.01m);
    }

    Bsdf material(Pt<Reed> p, Norm<Reed> n)
    {
        return ink(float3(0.18, 0.2, 0.28), 0.15);
    }
};

[Lobe]
float4 ink(float3 dark, float cut, float3 n, float3 v, float3 l)
{
    return float4(dot(n, l) < cut ? dark : float3(0.92, 0.85, 0.7), 0.0);
}

[Kind]
struct Water : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p) { return p.y - 0.0; }
    Bsdf material(Pt<World> p, Norm<World> n)
    {
        return ink(float3(0.92, 0.85, 0.7), 2.0) + ink(float3(0.6, 0.66, 0.7), 0.0 + 0.9 * fbm2(float2(p.x, p.z) * 4.0));
    }
};

[Kind]
struct PaperSky : ISky
{
    float3 paper;
    float3 faint;
};

[Kind]
struct Sun : ILight
{
    Dir<World> dir;
    float3 color;
};

float hash1(float n)
{
    __wgsl(n) { return fract(sin(n * 91.3458) * 47453.5453); }
}

[Comptime]
void reeds()
{
    for (uint i = 0; i < 200; i++)
    {
        float fi = float(i);
        float ang = 6.2831853 * hash1(fi * 1.7);
        float rad = 1.6 * sqrt(hash1(fi * 3.1)) * (0.6 + 0.4 * hash1(fi * 5.3));
        Pt<World> base = Pt<World>(rad * cos(ang), 0.0, rad * sin(ang));
        emit(ReedRow{
            .pose = translate_xform<Reed, World>(Vec<World>(base.x, 0.0, base.z)),
            .height = 0.9m + 0.7m * hash1(fi * 7.7),
            .lean = (hash1(fi * 11.3) - 0.5) * 0.3rad,
            .stiffness = 0.3 + 1.2 * hash1(fi * 13.9),
            .phase = 6.2831853rad * hash1(fi * 17.1) });
    }
    emit(Water{});
    emit(PaperSky{ .paper = float3(0.92, 0.85, 0.7), .faint = float3(0.75, 0.72, 0.62) });
    emit(Sun{ .dir = normalize(Dir<World>(-0.5, -0.7, -0.4)), .color = float3(1, 1, 1) });
}

technique Main
{
    pass Rows     { reeds(); }
    pass Camera   { orbit(Pt<World>(0.0, 0.7, 0.0), 4m, 15deg); }
    pass Cull     { build_indirect<Water, ReedRow>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Water, ReedRow>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```

# Task 3

```hlsl
struct World : ISpace {};
struct Trunk : ISpace {};
struct Head : ISpace {};
struct Limb : ISpace {};

static const Rad NECK_LIMIT = 1.1rad;
static const Rad TURN_DONE = 0.15rad;

[Kind]
struct Fish : ITrace<World>, IMaterial<World>
{
    Pt<World> pos;
    Sec clock;

    Field<World> trace(Pt<World> p) { return sd_sphere(p - pos, 0.05m); }
    Bsdf material(Pt<World> p, Norm<World> n) { return ggx(float3(0.7, 0.7, 0.75), 0.2, 0.8); }
};

[Kind]
struct HeronTrunk : ITrace<Trunk>, IMaterial<Trunk>
{
    Xform<Trunk, World> pose;
    Pt<World> pos;
    Rad heading;          // yaw about +y
    float hop_t;          // 0 = grounded, (0,1] = airborne
    bool hopping;

    Field<Trunk> trace(Pt<Trunk> p)
    {
        Field<Trunk> body = sd_capsule(p, Pt<Trunk>(0, 0, -0.12m), Pt<Trunk>(0, 0.05m, 0.12m), 0.09m);
        Field<Trunk> legl = sd_capsule(p, Pt<Trunk>(-0.04m, 0, 0), Pt<Trunk>(-0.04m, -0.55m, 0), 0.012m);
        Field<Trunk> legr = sd_capsule(p, Pt<Trunk>(0.04m, 0, 0), Pt<Trunk>(0.04m, -0.55m, 0), 0.012m);
        return smin_poly(body, hit_min(legl, legr), 0.02m);
    }
    Bsdf material(Pt<Trunk> p, Norm<Trunk> n) { return ggx(float3(0.45, 0.5, 0.58), 0.6, 0.0); }
};

[Kind]
struct HeronNeck : ITrace<Trunk>, IMaterial<Trunk>
{
    RowId<HeronTrunk> parent;
    Xform<Head, World> head_pose;
    Rad yaw;              // head yaw relative to trunk heading
    Rad lag;              // leftover angle beyond the limit, 0 when inside
    bool at_limit;

    // S-shaped neck from shoulder to head, in trunk space; head sits on the end
    Field<Trunk> trace(Pt<Trunk> p)
    {
        Pt<Trunk> s0 = Pt<Trunk>(0, 0.10m, 0.12m);
        Pt<Trunk> s1 = Pt<Trunk>(0, 0.30m, 0.20m);
        Pt<Trunk> s2 = Pt<Trunk>(0, 0.38m, 0.08m);
        Pt<Trunk> s3 = Pt<Trunk>(0, 0.55m, 0.16m);
        Field<Trunk> n = smin_poly(sd_capsule(p, s0, s1, 0.02m),
                         smin_poly(sd_capsule(p, s1, s2, 0.02m), sd_capsule(p, s2, s3, 0.018m), 0.02m), 0.02m);
        Field<Trunk> beak = sd_capsule(p, s3, s3 + Vec<Trunk>(0, 0, 0.16m), 0.01m);
        Field<Trunk> skull = sd_sphere(p - s3, 0.03m);
        return smin_poly(n, smin_poly(skull, beak, 0.01m), 0.01m);
    }
    Bsdf material(Pt<Trunk> p, Norm<Trunk> n) { return ggx(float3(0.8, 0.8, 0.82), 0.5, 0.0); }
};

Rad wrap_pi(Rad a)
{
    return a - 6.2831853rad * floor((a + 3.1415927rad) / 6.2831853rad);
}

Pt<World> fish_at(Sec t)
{
    float s = t / 1.0s;
    return Pt<World>(1.2 + 0.9 * sin(0.4 * s), 0.0, 1.0 + 1.6 * sin(0.4 * s + 1.0) * cos(0.25 * s));
}

[Law(Writes = Fish)]
void fish_law(inout Fish f, Sec dt)
{
    f.clock = f.clock + dt;
    f.pos = fish_at(f.clock);
}

// bearing of a world point from the trunk, relative to trunk heading
Rad bearing(HeronTrunk b, Pt<World> target)
{
    Vec<World> d = target - b.pos;
    return wrap_pi(atan2(d.x, d.z) * 1.0rad - b.heading);
}

[Law(Writes = HeronTrunk)]
void body_law(inout HeronTrunk b, Sec dt, Table<HeronNeck> necks, Table<Fish> fish)
{
    HeronNeck n = necks[RowId<HeronNeck>(0)];
    Pt<World> target = fish[RowId<Fish>(0)].pos;
    Rad rel = bearing(b, target);

    // start a hop when the neck is pinned at its limit
    if (!b.hopping && n.at_limit) { b.hopping = true; b.hop_t = 0.0; }

    if (b.hopping)
    {
        b.hop_t = b.hop_t + (dt / 0.5s);
        float h = 0.12 * sin(3.1415927 * clamp(b.hop_t, 0.0, 1.0));    // metres
        b.pos = Pt<World>(b.pos.x, h, b.pos.z);
        // rotate the trunk toward the fish while airborne, fraction of the remaining angle
        b.heading = b.heading + rel * clamp(4.0 * (dt / 1.0s), 0.0, 1.0);
        if (b.hop_t >= 1.0)
        {
            b.hopping = false;
            b.hop_t = 0.0;
            b.pos = Pt<World>(b.pos.x, 0.0, b.pos.z);
        }
    }
    b.pose = translate_xform<Trunk, World>(Vec<World>(b.pos.x, b.pos.y + 0.6, b.pos.z)) * rot_y_xform<Trunk, Trunk>(b.heading);
}

[Law(Writes = HeronNeck)]
void neck_law(inout HeronNeck n, Sec dt, Table<HeronTrunk> trunks, Table<Fish> fish)
{
    HeronTrunk b = trunks[n.parent];
    Rad want = bearing(b, fish[RowId<Fish>(0)].pos);     // gaze lock: aim at the fish in world space

    if (abs(want) > NECK_LIMIT)
    {
        n.yaw = want > 0.0rad ? NECK_LIMIT : -NECK_LIMIT;
        n.lag = abs(want) - NECK_LIMIT;
        n.at_limit = true;
    }
    else
    {
        n.yaw = want;
        n.lag = 0.0rad;
        n.at_limit = abs(want) > NECK_LIMIT - TURN_DONE ? n.at_limit : false;
    }
    n.head_pose = b.pose * rot_y_xform<Head, Trunk>(n.yaw);
}

[Constraint]
Verdict<Rad> gaze(HeronNeck n, Table<HeronTrunk> trunks, Table<Fish> fish)
{
    if (!is_finite(n.head_pose)) return Verdict::Unknown(NonFinitePose);
    if (n.lag > 0.0rad) return Verdict::Missed(n.lag);
    return Verdict::Held(NECK_LIMIT - abs(n.yaw));
}

[Constraint]
Verdict<Meters> head_above_hips(HeronNeck n, Table<HeronTrunk> trunks)
{
    HeronTrunk b = trunks[n.parent];
    if (!is_finite(n.head_pose)) return Verdict::Unknown(NonFinitePose);
    Meters margin = (n.head_pose * Pt<Head>(0, 0, 0)).y + 0.55m - (b.pose * Pt<Trunk>(0, 0, 0)).y;
    if (margin < 0.0m) return Verdict::Missed(-margin);
    return Verdict::Held(margin);
}

[Kind]
struct Shore : ITrace<World>, IMaterial<World>
{
    Field<World> trace(Pt<World> p) { return p.y; }
    Bsdf material(Pt<World> p, Norm<World> n) { return ggx(float3(0.3, 0.27, 0.2), 0.5, 0.0); }
};

[Kind]
struct Sun : ILight { Dir<World> dir; float3 color; };

[Kind]
struct Sky : ISky { float3 zenith; float3 horizon; };

[Comptime]
void scene()
{
    RowId<HeronTrunk> t = emit(HeronTrunk{
        .pose = translate_xform<Trunk, World>(Vec<World>(0, 0.6, 0)), .pos = Pt<World>(0, 0, 0),
        .heading = 0.0rad, .hop_t = 0.0, .hopping = false });
    emit(HeronNeck{ .parent = t, .head_pose = translate_xform<Head, World>(Vec<World>(0, 1.15, 0)),
                    .yaw = 0.0rad, .lag = 0.0rad, .at_limit = false });
    emit(Fish{ .pos = fish_at(0.0s), .clock = 0.0s });
    emit(Shore{});
    emit(Sun{ .dir = normalize(Dir<World>(-0.3, -0.8, -0.4)), .color = float3(1.0, 0.96, 0.9) });
    emit(Sky{ .zenith = float3(0.4, 0.6, 0.9), .horizon = float3(0.85, 0.88, 0.92) });
}

technique Main
{
    pass Rows     { scene(); }
    pass Motion   { run_laws(fish_law, body_law, neck_law); }
    pass Check    { check(gaze); check(head_above_hips); }
    pass Camera   { orbit(Pt<World>(0.0, 0.7, 0.0), 4m, 20deg); }
    pass Cull     { build_indirect<Shore, HeronTrunk, HeronNeck>(); }
    pass Vis      { render_bundle(Vis64) { draw_indirect<Shore, HeronTrunk, HeronNeck>(); } }
    pass Classify { tile_bin_kinds(Vis64, 8); }
    pass Shade    { dispatch_material_tiles(Vis64, sun_sky); }
    pass Present  { tonemap_aces(ColorHdr, sky_color); }
}
```


================================================================
# SONNET OUTPUT D
# file: agent_notes/2026-11-20_syntax_bakeoff/out_D_sonnet.md
# what it is: the three tasks written by Sonnet in syntax D
================================================================

# Task 1: hermit crab

```python
space("World", "Body", "Leg", "Claw", "Face", "Shell")

TAU = 6.2831853*rad

def num(x: Meters) -> float:
    return x / (1.0*m)

def wet_chitin(noise: float) -> Bsdf:
    rough = 0.28 + 0.22*noise
    return ggx(vec3(0.85, 0.22, 0.07), rough, 0.0) + sss(vec3(0.9, 0.3, 0.1), 0.08) + clearcoat(0.7, 0.07)

# ---------------------------------------------------------------- shell

def spine(u: float) -> Pt["Shell"]:
    a = u * 3.0 * TAU
    r = 0.006 + 0.05*u
    return Pt["Shell"](r*cos(a), 0.16*u, r*sin(a))

@scan(over=(0.0, 1.0), steps=256, schedule="bake_lut")
class whorl:
    def init(u: float) -> Frame["Shell"]:
        return frame_from_tangent(spine(u), normalize(diff(spine, "u")(u)), Dir["Shell"](0, 1, 0))
    def step(prev: Frame["Shell"], u: float) -> Frame["Shell"]:
        return transport_frame(prev, spine(u), normalize(diff(spine, "u")(u)))

@kind
class Shell:
    pose: Xform["Shell", "World"]

    @query(grid=(512, 48))
    def surface(self, u: float, v: float) -> Pt["Shell"]:
        f = sample_c2(whorl, u)
        th = v * TAU
        rib = 1.0 + 0.07*sin(8.0*th) + 0.03*sin(u * 160.0*rad)
        r = (0.004 + 0.034*u) * rib
        return f.origin + (f.normal*cos(th) + f.binormal*sin(th)) * r

    @query
    def material(self, p: Pt["Shell"], n: Norm["Shell"]) -> Bsdf:
        band = 0.5 + 0.5*sin(num(p.y) * 300.0*rad)
        return ggx(mix(vec3(0.86, 0.78, 0.64), vec3(0.45, 0.28, 0.18), band), 0.45, 0.0) + clearcoat(0.5, 0.12)

# ---------------------------------------------------------------- body and legs

@kind
class CrabBody:
    pose: Xform["Body", "World"]
    vel: Vec["World"]

    @query
    def trace(self, p: Pt["Body"]) -> Field["Body"]:
        return sd_round_box(p - Pt["Body"](0.0, 0.0, 0.0), Vec["Body"](0.09, 0.04, 0.12), 0.03*m)

    @query
    def material(self, p: Pt["Body"], n: Norm["Body"]) -> Bsdf:
        return wet_chitin(fbm2(vec2(num(p.x), num(p.z)) * 120.0))

@kind
class CrabLeg:
    parent: RowId["CrabBody"]
    pose: Xform["Leg", "World"]
    femur: Meters
    tibia: Meters
    foot: Pt["World"]
    home: Pt["Body"]

    @query
    def trace(self, p: Pt["Leg"]) -> Field["Leg"]:
        foot = inv(self.pose) @ self.foot
        knee = Pt["Leg"](self.femur*0.8, self.femur*0.55, 0.0)
        d = foot - knee
        tip = knee + d * (self.tibia / max(length(d), 0.001*m))
        thigh = sd_capsule(p, Pt["Leg"](0, 0, 0), knee, 0.016*m)
        shin = sd_capsule(p, knee, tip, 0.011*m)
        return smin_poly(thigh, shin, 0.01*m)

    @query
    def material(self, p: Pt["Leg"], n: Norm["Leg"]) -> Bsdf:
        return wet_chitin(fbm2(vec2(num(p.x), num(p.y)) * 200.0))

@law(writes=CrabLeg)
def crab_step(leg: CrabLeg, dt: Sec, bodies: Table["CrabBody"]):
    b = bodies[leg.parent]
    home = b.pose @ leg.home
    target = home + b.vel * (0.28*s)
    if distance(leg.foot, target) > 0.04*m:
        leg.foot = Pt["World"](target.x, 0.0, target.z)

# ---------------------------------------------------------------- claws (one code, two rows)

@kind
class Claw:
    serrated: Static[bool]
    pose: Xform["Claw", "World"]
    size: Meters
    open: Rad

    @query
    def trace(self, p: Pt["Claw"]) -> Field["Claw"]:
        s = self.size
        palm = sd_round_box(p - Pt["Claw"](0.0, 0.0, 0.0), Vec["Claw"](0.5*s, 0.4*s, 0.35*s), 0.12*s)
        fixed = sd_capsule(p, Pt["Claw"](0.4*s, -0.12*s, 0.0), Pt["Claw"](1.3*s, -0.12*s, 0.0), 0.1*s)
        hinge = translate(p, -Vec["Claw"](0.45*s, 0.12*s, 0.0)) | rot_z(-self.open)
        mv = sd_capsule(hinge.p, hinge.pt(0.0, 0.0, 0.0), hinge.pt(0.9*s, 0.0, 0.0), 0.1*s)
        if self.serrated:
            pitch = max(0.14*num(s), 0.001)
            tooth = abs(fract(num(hinge.p.x) / pitch) - 0.5)
            mv = mv + 0.1*s*tooth
        return smin_poly(smin_poly(palm, fixed, 0.06*s), mv, 0.04*s)

    @query
    def material(self, p: Pt["Claw"], n: Norm["Claw"]) -> Bsdf:
        return wet_chitin(fbm2(vec2(num(p.x), num(p.z)) * 160.0))

# ---------------------------------------------------------------- eyes (one mirrored definition)

def gaze_target(time: Sec) -> Pt["World"]:
    a = 0.4*rad*(time/s)
    return Pt["World"](0.25*sin(a), 0.10 + 0.04*cos(1.7*a), 0.40 + 0.1*cos(a))

def stalk_d(p: Pt["Face"]) -> Field["Face"]:
    sym = mirror_x(p) | translate(-Vec["Face"](0.03, 0.0, 0.0))
    return sd_capsule(sym.p, sym.pt(0.0, 0.0, 0.0), sym.pt(0.0, 0.04, 0.0), 0.006*m)

def globe_d(p: Pt["Face"]) -> Field["Face"]:
    sym = mirror_x(p) | translate(-Vec["Face"](0.03, 0.0, 0.0))
    eye_c = sym.pt(0.0, 0.04, 0.0)
    return sd_sphere(sym.p - eye_c, 0.012*m)

def pupil_d(pose: Xform["Face", "World"], p: Pt["Face"], time: Sec) -> Field["Face"]:
    sym = mirror_x(p) | translate(-Vec["Face"](0.03, 0.0, 0.0))
    eye_c = sym.pt(0.0, 0.04, 0.0)
    eye_w = pose @ sym.to_parent(eye_c)
    look = sym.from_parent(inv(pose) @ normalize(gaze_target(time) - eye_w))
    return sd_sphere(sym.p - (eye_c + look*(0.009*m)), 0.0045*m)

@kind
class CrabEyes:
    pose: Xform["Face", "World"]

    @query
    def trace(self, p: Pt["Face"], time: Sec) -> Field["Face"]:
        return min(min(stalk_d(p), globe_d(p)), pupil_d(self.pose, p, time))

    @query
    def material(self, p: Pt["Face"], n: Norm["Face"], time: Sec) -> Bsdf:
        if pupil_d(self.pose, p, time) < 0.0004*m:
            return ggx(vec3(0.02, 0.02, 0.02), 0.1, 0.0) + clearcoat(0.9, 0.03)
        if globe_d(p) < 0.0004*m:
            return ggx(vec3(0.9, 0.85, 0.7), 0.2, 0.0) + clearcoat(0.9, 0.03)
        return wet_chitin(fbm2(vec2(num(p.x), num(p.y)) * 300.0))

# ---------------------------------------------------------------- world

@kind
class Ground:
    level: Meters

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_box(p - Pt["World"](0.0, self.level - 0.1*m, 0.0), Vec["World"](4.0*m, 0.1*m, 4.0*m))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        grain = fbm2(vec2(num(p.x), num(p.z)) * 60.0)
        return ggx(vec3(0.30, 0.24, 0.17) * (0.8 + 0.4*grain), 0.55, 0.0) + clearcoat(0.4, 0.15)

@kind
class SunSky:
    dir: Dir["World"]
    color: vec3
    zenith: vec3
    horizon: vec3

    @query
    def light(self, p: Pt["World"]) -> Light:
        return directional(self.dir, self.color)

    @query
    def sky(self, rd: Dir["World"]) -> vec3:
        return mix(self.horizon, self.zenith, clamp(rd.y, 0.0, 1.0))

sun_sky = SunSky
sky_color = SunSky

# ---------------------------------------------------------------- build: rows

@comptime
def leg_socket(j: int, side: float) -> Xform["Leg", "Body"]:
    yaw = (0.1*(j - 2.5) + (0.0 if side > 0.0 else 3.1416))*rad
    return Xform["Leg", "Body"](t=Vec["Body"](side*0.07, -0.01, 0.035*j - 0.08), yaw=yaw)

@comptime
def crab(b: RowId["CrabBody"], body: Xform["Body", "World"]):
    for j in range(6):
        for side in (-1.0, 1.0):
            emit(CrabLeg(parent=b, pose=body @ leg_socket(j, side),
                         femur=0.07*m + 0.008*m*j, tibia=0.09*m + 0.01*m*j,
                         foot=body @ Pt["Body"](side*0.14, -0.05, 0.03*j - 0.07),
                         home=Pt["Body"](side*0.14, -0.05, 0.03*j - 0.07)))
    emit(Shell(pose=body @ Xform["Shell", "Body"](t=Vec["Body"](0.0, 0.07, -0.06), yaw=0.3*rad)))
    emit(Claw(serrated=True, pose=body @ Xform["Claw", "Body"](t=Vec["Body"](0.11, 0.02, 0.15), yaw=-1.2*rad),
              size=0.05*m, open=0.45*rad))
    emit(Claw(serrated=True, pose=body @ Xform["Claw", "Body"](t=Vec["Body"](-0.11, 0.02, 0.15), yaw=-1.9*rad),
              size=0.028*m, open=0.2*rad))
    emit(CrabEyes(pose=body @ Xform["Face", "Body"](t=Vec["Body"](0.0, 0.04, 0.12), yaw=0.0*rad)))

@comptime
def crabs():
    body = Xform["Body", "World"](t=Vec["World"](0.0, 0.05, 0.0), yaw=0.0*rad)
    b = emit(CrabBody(pose=body, vel=Vec["World"](0.0, 0.0, 0.0)))
    crab(b, body)

@comptime
def lights():
    emit(Ground(level=0.0*m))
    emit(SunSky(dir=normalize(Vec["World"](-0.4, -0.8, -0.3)), color=vec3(3.0, 2.8, 2.5),
                zenith=vec3(0.35, 0.55, 0.9), horizon=vec3(0.8, 0.85, 0.9)))

@schedule
def main():
    rows(crabs)
    rows(lights)
    run_laws(crab_step)
    orbit(at=Pt["World"](0.0, 0.08, 0.0), radius=0.7*m, fov=40*deg)
    build_indirect(Ground, CrabBody, CrabLeg, Claw, CrabEyes, Shell)
    with render_bundle(Vis64):
        draw_indirect(Ground, CrabBody, CrabLeg, Claw, CrabEyes, Shell)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```

# Task 2: reed bed in wind

```python
space("World", "Reed")

TAU = 6.2831853*rad
PAPER = vec3(0.92, 0.85, 0.70)
INK = vec3(0.14, 0.12, 0.20)

def num(x: Meters) -> float:
    return x / (1.0*m)

@lobe
def flat(c: vec3, n: vec3, v: vec3, l: vec3) -> vec4:
    return vec4(c, 0.0)

@lobe
def ink(dark: vec3, cut: float, n: vec3, v: vec3, l: vec3) -> vec4:
    return vec4(dark if dot(n, l) < cut else PAPER, 0.0)

@kind
class Reed:
    pose: Xform["Reed", "World"]
    height: Meters
    lean: Rad
    stiff: float
    phase: Rad

    @query
    def trace(self, p: Pt["World"], time: Sec) -> Field["World"]:
        q = inv(self.pose) @ p
        h = max(self.height, 0.2*m)
        t = clamp(q.y / h, 0.0, 1.0)
        base = self.pose @ Pt["Reed"](0.0, 0.0, 0.0)
        ph = self.phase + (2.2*num(base.x) + 1.3*num(base.z) - 1.6*(time/s))*rad
        gust = 0.6 + 0.4*sin(ph)
        bend = 0.30*m * (t*t) * gust / (0.4 + max(self.stiff, 0.0))
        wind = normalize(inv(self.pose) @ Vec["World"](1.0, 0.0, 0.3))
        shift = Vec["Reed"](sin(self.lean)*q.y, 0.0, 0.0) + wind*bend
        r = q - shift
        radius = 0.0055*m * (1.0 - 0.45*t)
        stalk = sd_capsule(r, Pt["Reed"](0.0, 0.0, 0.0), Pt["Reed"](0.0, h, 0.0), radius)
        head = sd_capsule(r, Pt["Reed"](0.0, 0.86*h, 0.0), Pt["Reed"](0.0, h + 0.03*m, 0.0), 0.014*m)
        return min(stalk, head)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ink(INK, 0.12)

@kind
class Water:
    level: Meters

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_box(p - Pt["World"](0.0, self.level - 0.1*m, 0.0), Vec["World"](8.0*m, 0.1*m, 8.0*m))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        band = step(0.92, fract(num(p.z)*14.0 + 0.3*sin(num(p.x)*5.0*rad)))
        return flat(mix(PAPER, vec3(0.62, 0.72, 0.76), 0.35*band))

@kind
class SunSky:
    dir: Dir["World"]
    color: vec3

    @query
    def light(self, p: Pt["World"]) -> Light:
        return directional(self.dir, self.color)

    @query
    def sky(self, rd: Dir["World"]) -> vec3:
        return mix(PAPER, vec3(0.80, 0.84, 0.84), 0.5*clamp(rd.y, 0.0, 1.0))

sun_sky = SunSky
sky_color = SunSky

@comptime
def rnd(i: int, k: float) -> float:
    return fract(sin(i*12.9898 + k*78.233) * 43758.5453)

@comptime
def bed():
    for i in range(200):
        a = (i * 2.39996)*rad
        blob = 1.0 + 0.35*sin(3.0*a + rnd(i, 9.0)*TAU)
        r = 0.09 * sqrt(i + 0.5) * (0.75 + 0.5*rnd(i, 1.0)) * blob
        x = r*cos(a) + 0.15*(rnd(i, 2.0) - 0.5)
        z = r*sin(a) + 0.15*(rnd(i, 3.0) - 0.5)
        emit(Reed(pose=Xform["Reed", "World"](t=Vec["World"](x, 0.0, z), yaw=rnd(i, 4.0)*TAU),
                  height=(0.9 + 0.7*rnd(i, 5.0))*m,
                  lean=(rnd(i, 6.0) - 0.5)*0.30*rad,
                  stiff=0.4 + 0.6*rnd(i, 7.0),
                  phase=rnd(i, 8.0)*TAU))

@comptime
def scene():
    emit(Water(level=0.0*m))
    emit(SunSky(dir=normalize(Vec["World"](-0.6, -0.7, -0.2)), color=vec3(1.0, 1.0, 1.0)))

@schedule
def main():
    rows(bed)
    rows(scene)
    orbit(at=Pt["World"](0.0, 0.6, 0.0), radius=4.0*m, fov=40*deg)
    build_indirect(Water, Reed)
    with render_bundle(Vis64):
        draw_indirect(Water, Reed)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```

# Task 3: heron's hop

```python
space("World", "Trunk")

TAU = 6.2831853*rad
NECK_LIMIT = 1.0*rad
HOP = 0.5*s
HOP_H = 0.22*m
STAND = 0.55*m

def wrap(a: Rad) -> Rad:
    return a - TAU*floor((a + 0.5*TAU)/TAU)

def trunk_pose(pos: Pt["World"], heading: Rad, y: Meters) -> Xform["Trunk", "World"]:
    return Xform["Trunk", "World"](t=Vec["World"](pos.x, pos.y + STAND + y, pos.z), yaw=heading)

def pivot_of(pose: Xform["Trunk", "World"]) -> Pt["World"]:
    return pose @ Pt["Trunk"](0.0, 0.40, 0.18)

# signed yaw from the trunk heading to the target, seen from the head pivot
def aim_yaw(pose: Xform["Trunk", "World"], heading: Rad, target: Pt["World"]) -> Rad:
    d = target - pivot_of(pose)
    return wrap(atan2(d.x, d.z) - heading)

def aim_pitch(pose: Xform["Trunk", "World"], target: Pt["World"]) -> Rad:
    d = target - pivot_of(pose)
    return atan2(d.y, length(Vec["World"](d.x, 0.0, d.z)))

def fish_curve(t: Sec) -> Pt["World"]:
    a = 0.35*rad*(t/s)
    r = 1.2 + 0.15*sin(2.0*a)
    return Pt["World"](r*sin(a), 0.0, r*cos(a))

# ---------------------------------------------------------------- kinds

@kind
class Fish:
    t: Sec
    pos: Pt["World"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_capsule(p, self.pos - Vec["World"](0.0, 0.0, 0.04), self.pos + Vec["World"](0.0, 0.0, 0.04), 0.02*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.7, 0.75, 0.8), 0.25, 0.8) + clearcoat(0.6, 0.08)

def leg_d(q: Pt["Trunk"], side: float, y: Meters) -> Field["Trunk"]:
    hip = Pt["Trunk"](side*0.045, -0.05, -0.05)
    ankle = Pt["Trunk"](side*0.045, -0.30 - 0.1*num_m(y), -0.08)
    foot = Pt["Trunk"](side*0.045, -STAND - 0.2*y, -0.03)
    return min(sd_capsule(q, hip, ankle, 0.012*m), sd_capsule(q, ankle, foot, 0.009*m))

def num_m(x: Meters) -> float:
    return x / (1.0*m)

@kind
class Trunk:
    pos: Pt["World"]
    heading: Rad
    y: Meters
    hop_t: Sec
    fish: RowId["Fish"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        q = inv(trunk_pose(self.pos, self.heading, self.y)) @ p
        body = sd_capsule(q, Pt["Trunk"](0.0, 0.0, -0.14), Pt["Trunk"](0.0, 0.02, 0.12), 0.085*m)
        legs = min(leg_d(q, 1.0, self.y), leg_d(q, -1.0, self.y))
        return smin_poly(body, legs, 0.02*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.55, 0.58, 0.62), 0.5, 0.0)

def beak_d(pivot: Pt["World"], fwd: Dir["World"], p: Pt["World"]) -> Field["World"]:
    return sd_capsule(p, pivot + fwd*(0.08*m), pivot + fwd*(0.32*m), 0.012*m)

@kind
class Head:
    parent: RowId["Trunk"]
    fish: RowId["Fish"]
    yaw: Rad
    pitch: Rad
    over: Rad
    trunk: Xform["Trunk", "World"]
    fwd: Dir["World"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        n0 = self.trunk @ Pt["Trunk"](0.0, 0.10, 0.20)
        n1 = self.trunk @ Pt["Trunk"](0.0, 0.20, 0.34)
        n2 = self.trunk @ Pt["Trunk"](0.0, 0.30, 0.10)
        n3 = pivot_of(self.trunk)
        neck = min(sd_capsule(p, n0, n1, 0.02*m), min(sd_capsule(p, n1, n2, 0.018*m), sd_capsule(p, n2, n3, 0.016*m)))
        skull = sd_capsule(p, n3, n3 + self.fwd*(0.09*m), 0.028*m)
        return min(smin_poly(neck, skull, 0.02*m), beak_d(n3, self.fwd, p))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        if beak_d(pivot_of(self.trunk), self.fwd, p) < 0.002*m:
            return ggx(vec3(0.85, 0.65, 0.15), 0.35, 0.0)
        return ggx(vec3(0.55, 0.58, 0.62), 0.5, 0.0)

@kind
class Water:
    level: Meters

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_box(p - Pt["World"](0.0, self.level - 0.1*m, 0.0), Vec["World"](4.0*m, 0.1*m, 4.0*m))

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.1, 0.25, 0.3), 0.08, 0.0) + clearcoat(0.8, 0.03)

@kind
class SunSky:
    dir: Dir["World"]
    color: vec3
    zenith: vec3
    horizon: vec3

    @query
    def light(self, p: Pt["World"]) -> Light:
        return directional(self.dir, self.color)

    @query
    def sky(self, rd: Dir["World"]) -> vec3:
        return mix(self.horizon, self.zenith, clamp(rd.y, 0.0, 1.0))

sun_sky = SunSky
sky_color = SunSky

# ---------------------------------------------------------------- laws

@law(writes=Fish)
def fish_step(f: Fish, dt: Sec):
    f.t = f.t + dt
    f.pos = fish_curve(f.t)

# the body decides to hop from the same aim the neck uses, so both agree on "at the limit"
@law(writes=Trunk)
def body_step(t: Trunk, dt: Sec, fish: Table["Fish"]):
    f = fish[t.fish]
    want = aim_yaw(trunk_pose(t.pos, t.heading, t.y), t.heading, f.pos)
    if t.hop_t > 0.0*s:
        t.hop_t = t.hop_t + dt
        if t.hop_t >= HOP:
            t.hop_t = 0.0*s
            t.y = 0.0*m
        else:
            u = t.hop_t / HOP
            t.y = HOP_H * (4.0 * u * (1.0 - u))
            reach = (5.0*rad/s) * dt
            t.heading = t.heading + clamp(want, -reach, reach)
    elif abs(want) > NECK_LIMIT:
        t.hop_t = dt

@law(writes=Head)
def neck_step(h: Head, dt: Sec, trunks: Table["Trunk"], fish: Table["Fish"]):
    t = trunks[h.parent]
    f = fish[h.fish]
    pose = trunk_pose(t.pos, t.heading, t.y)
    aim = aim_yaw(pose, t.heading, f.pos)
    yaw = clamp(aim, -NECK_LIMIT, NECK_LIMIT)
    pitch = clamp(aim_pitch(pose, f.pos), -1.0*rad, 0.5*rad)
    yw = t.heading + yaw
    h.yaw = yaw
    h.pitch = pitch
    h.over = abs(aim) - abs(yaw)
    h.trunk = pose
    h.fwd = normalize(Vec["World"](sin(yw)*cos(pitch), sin(pitch), cos(yw)*cos(pitch)))

# ---------------------------------------------------------------- constraints

@constraint
def gaze_held(h: Head) -> Verdict[Rad]:
    if not is_finite(h.yaw):
        return Unknown("NonFiniteYaw")
    if h.over > 0.0*rad:
        return Missed(h.over)
    return Held(NECK_LIMIT - abs(h.yaw))

@constraint
def head_above_hips(h: Head) -> Verdict[Meters]:
    if not is_finite(h.trunk):
        return Unknown("NonFinitePose")
    hips = h.trunk @ Pt["Trunk"](0.0, -0.05, -0.08)
    head = pivot_of(h.trunk) + h.fwd*(0.05*m)
    margin = head.y - hips.y
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)

# ---------------------------------------------------------------- build and schedule

@comptime
def scene():
    f = emit(Fish(t=0.0*s, pos=fish_curve(0.0*s)))
    t = emit(Trunk(pos=Pt["World"](0.0, 0.0, 0.0), heading=0.0*rad, y=0.0*m, hop_t=0.0*s, fish=f))
    emit(Head(parent=t, fish=f, yaw=0.0*rad, pitch=0.0*rad, over=0.0*rad,
              trunk=trunk_pose(Pt["World"](0.0, 0.0, 0.0), 0.0*rad, 0.0*m),
              fwd=Dir["World"](0.0, 0.0, 1.0)))
    emit(Water(level=0.0*m))
    emit(SunSky(dir=normalize(Vec["World"](-0.4, -0.8, -0.3)), color=vec3(3.0, 2.8, 2.5),
                zenith=vec3(0.35, 0.55, 0.9), horizon=vec3(0.8, 0.85, 0.9)))

@schedule
def main():
    rows(scene)
    run_laws(fish_step)
    run_laws(body_step)
    run_laws(neck_step)
    check(gaze_held)
    check(head_above_hips)
    orbit(at=Pt["World"](0.0, 0.6, 0.5), radius=3.5*m, fov=40*deg)
    build_indirect(Water, Trunk, Head, Fish)
    with render_bundle(Vis64):
        draw_indirect(Water, Trunk, Head, Fish)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=sun_sky)
    tonemap_aces(ColorHdr, sky=sky_color)
```


================================================================
# SONNET OUTPUT E
# file: agent_notes/2026-11-20_syntax_bakeoff/out_E_sonnet.md
# what it is: the three tasks written by Sonnet in syntax E
================================================================

# out_E_sonnet

## Task 1

```
space World Body Leg Claw Shell

# ---------- helpers ----------
fn clamp(x, lo, hi) = if x < lo { lo } else if x > hi { hi } else { x }
fn phase(rate) = rate * (time / 1s)
fn gaze() = [0.45m * sin(phase(0.31rad)), 0.10m, 0.55m + 0.20m * cos(phase(0.23rad))]@World
fn mount(x, y, z, yaw) = place([x, y, z]@Body) * rot_y(yaw)

# ---------- shell spiral ----------
fn growth(u) = exp(1.7 * u)
fn ang(u) = 6.2832rad * 3.5 * u
fn spine(u) = [0.016m * growth(u) * cos(ang(u)), 0.075m * u * growth(u), 0.016m * growth(u) * sin(ang(u))]@Shell
fn tube_r(u) = 0.010m * growth(u)

scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0,1,0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}

kind Shell @Shell (pose: xf@Shell>World) {
  surface(u, v) grid 64x32 = {
    fr = whorl(u)
    a = 6.2832rad * v
    rib = 1 + 0.07 * cos(14 * a) + 0.05 * sin(6.2832rad * 40 * u)
    fr.o + tube_r(u) * rib * (cos(a) * fr.n + sin(a) * fr.b)
  }
  material(p, n) = ggx([0.80,0.68,0.55], 0.55, 0) + coat(0.25, 0.30)
}

# ---------- claw ----------
kind Claw[serrated: bool] @Claw (pose: xf@Claw>World, open: rad, k: m) {
  trace(p) = {
    palm = round_box(p, [0.45 * k, 0.30 * k, 0.25 * k], 0.10 * k)
    fixed = capsule(p, [0.40 * k, -0.10 * k, 0], [1.35 * k, -0.20 * k, 0], 0.12 * k)
    q = p |> move([-0.45 * k, 0.20 * k, 0]) |> rot_z(-open)
    teeth = if serrated { 0.025 * k * abs(sin(6.2832rad * 7 * q.p.x / (0.9 * k))) } else { 0m }
    moving = capsule(q.p, [0,0,0], [0.9 * k, 0.1 * k, 0], 0.11 * k) + teeth
    smin(smin(palm, fixed, 0.05 * k), moving, 0.02 * k)
  }
  material(p, n) = chitin(p, n)
}

# ---------- legs ----------
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World) {
  trace(p) = {
    knee = [femur, 0, 0]@Leg
    tip = knee + tibia * dir(inv(pose) * foot - knee)
    min(capsule(p, [0,0,0]@Leg, knee, 0.006m), capsule(p, knee, tip, 0.005m))
  }
  material(p, n) = chitin(p, n)
}

# ---------- body and eyes ----------
kind CrabBody @Body (pose: xf@Body>World) {
  trace(p) = {
    sym = p |> mirror_x
    eye_c = [0.03m, 0.10m, 0.09m]@sym
    shell_c = capsule(p, [0, 0.02m, -0.02m], [0, 0.03m, 0.06m], 0.05m)
    stalk = capsule(sym.p, [0.03m, 0.045m, 0.07m]@sym, eye_c, 0.007m)
    globe = sphere(sym.p - eye_c, 0.014m)
    min(shell_c, min(stalk, globe))
  }
  material(p, n) = {
    sym = p |> mirror_x
    eye_c = [0.03m, 0.10m, 0.09m]@sym
    eye_w = pose * sym.out(eye_c)
    look = sym.in(inv(pose) * dir(gaze() - eye_w))
    if dist(sym.p, eye_c) < 0.02m && dot(sym.in(n), look) > 0.8 {
      ggx([0.02,0.02,0.02], 0.1, 0) + coat(1.0, 0.02)
    } else {
      chitin(p, n)
    }
  }
}

# ---------- materials ----------
fn chitin(p, n) = ggx([0.85,0.28,0.10], 0.18 + 0.12 * fbm2([p.x / 0.003m, (p.y + p.z) / 0.003m]), 0) + sss([0.9,0.3,0.1], 0.004m) + coat(0.7, 0.04)

kind Ground @World () {
  trace(p) = p.y
  material(p, n) = ggx([0.22,0.17,0.12], 0.25, 0) + coat(0.5, 0.05)
}

# ---------- build ----------
fn socket(j, side) = mount(side * 0.035m, 0.0m, -0.04m + 0.016m * j, side * 1.4rad)

fn crab(b, body: xf@Body>World) {
  for j in 0..6, side in [-1, 1] {
    CrabLeg(parent: b, pose: body * socket(j, side), femur: 0.05m + 0.004m * j, tibia: 0.06m + 0.003m * j, foot: body * [side * 0.11m, 0, -0.05m + 0.02m * j]@Body)
  }
  Shell(pose: body * mount(0.0m, 0.07m, -0.07m, 0rad))
  Claw[true](pose: body * mount(0.05m, 0.03m, 0.07m, 0.3rad), open: 0.5rad, k: 0.09m)
  Claw[false](pose: body * mount(-0.05m, 0.03m, 0.07m, -0.3rad), open: 0.2rad, k: 0.05m)
}

Ground()
at = place([0,0,0]@World)
crab(CrabBody(pose: at), at)

# ---------- light, camera, schedule ----------
light = sun(dir([-0.5,-1,-0.4]@World), [1.0,0.93,0.82] * 3)
sky(rd) = [0.55,0.70,0.90] * (1 - 0.4 * (1 - rd.y)) + [0.9,0.85,0.8] * 0.4 * (1 - rd.y)

schedule Main {
  orbit([0,0.05,0]@World, 0.8m, 0.45rad)
  build_indirect(Ground, CrabBody, CrabLeg, Claw, Shell)
  render_bundle(Vis64) { draw_indirect(Ground, CrabBody, CrabLeg, Claw, Shell) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```

## Task 2

```
space World Reed

paper = [0.93,0.88,0.76]
ink_dark = [0.17,0.12,0.16]
ink_faint = [0.86,0.80,0.70]
wind = dir([0.8,0,0.6]@World)

fn clamp(x, lo, hi) = if x < lo { lo } else if x > hi { hi } else { x }
fn rnd(i, k) = fract(sin((i * 12.9 + k * 78.2) * 1rad) * 43758.5)
fn gust(x, z) = 0.5 + 0.5 * sin(2.4rad * (time / 1s) - 1.3rad * ((0.8 * x + 0.6 * z) / 1m))

kind Reed @Reed (pose: xf@Reed>World, height: m, lean: rad, stiff: f, phase: rad) {
  trace(p) = {
    o = pose * [0,0,0]@Reed
    h = clamp(p.y / height, 0, 1)
    amp = 0.30m * gust(o.x, o.z) * (1 - 0.8 * stiff) + 0.03m * sin(1.7rad * (time / 1s) + phase)
    off = [sin(lean) * height * h + amp * h * h, 0m, 0m]@Reed + (inv(pose) * wind) * (amp * h * h)
    q = p - off
    s0 = capsule(q, [0,0,0], [0, 0.33 * height, 0], 0.012m)
    s1 = capsule(q, [0, 0.33 * height, 0], [0, 0.66 * height, 0], 0.009m)
    s2 = capsule(q, [0, 0.66 * height, 0], [0, 0.90 * height, 0], 0.006m)
    head = capsule(q, [0, 0.88 * height, 0], [0, height, 0], 0.022m)
    min(min(s0, s1), min(s2, head))
  }
  material(p, n) = ink(ink_dark, 0.0)
}

lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { paper }

kind Water @World () {
  trace(p) = p.y
  material(p, n) = emit(if fract(p.z / 0.08m + 0.3 * sin((p.x / 0.4m) * 1rad)) < 0.12 { ink_faint } else { paper })
}

light = sun(dir([-0.5,-0.8,-0.3]@World), [1,1,1])
sky(rd) = if fract(rd.y * 18) < 0.06 { ink_faint } else { paper }

# 200 reeds, one kind, loose irregular cluster
for i in 0..200 {
  a = 6.2832rad * rnd(i, 1)
  r = 1.4m * rnd(i, 2) * rnd(i, 2)
  x = r * cos(a) + 0.25m * (rnd(i, 3) - 0.5)
  z = r * sin(a) + 0.25m * (rnd(i, 8) - 0.5)
  Reed(pose: place([x, 0, z]@World), height: 1.1m + 0.7m * rnd(i, 4), lean: 0.24rad * (rnd(i, 5) - 0.5), stiff: 0.2 + 0.7 * rnd(i, 6), phase: 6.2832rad * rnd(i, 7))
}
Water()

schedule Main {
  orbit([0,0.7,0]@World, 4m, 0.25rad)
  build_indirect(Water, Reed)
  render_bundle(Vis64) { draw_indirect(Water, Reed) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```

## Task 3

```
space World Body Head

neck_limit = 1.0rad
hop_len = 0.5s
hop_h = 0.18m
turn_rate = 2.4rad

fn clamp(x, lo, hi) = if x < lo { lo } else if x > hi { hi } else { x }
fn wrap_pi(a) = a - 6.2832rad * floor(a / 6.2832rad + 0.5)
fn bearing(a, b) = atan2(b.x - a.x, b.z - a.z)
fn fish_curve(u) = [2.0m * sin(u * 1rad), 0.03m, 0.8m + 0.6m * cos(2 * u * 1rad)]@World

kind Fish @World (u: f, pos: p3@World) {
  trace(p) = sphere(p - pos, 0.05m)
  material(p, n) = ggx([0.7,0.75,0.8], 0.2, 0.6)
}

kind Heron @Body (pose: xf@Body>World, pos: p3@World, heading: rad, hop: s, lift: m, target: Fish) {
  trace(p) = {
    sym = p |> mirror_x
    trunk = capsule(p, [0, 0.04m, -0.20m], [0, 0.10m, 0.12m], 0.10m)
    legs = capsule(sym.p, [0.05m, -0.05m, -0.08m]@sym, [0.05m, -0.72m, -0.05m]@sym, 0.012m)
    min(trunk, legs)
  }
  material(p, n) = ggx([0.55,0.6,0.65], 0.5, 0)
}

kind Head @Head (parent: Heron, target: Fish, pose: xf@Head>World, yaw: rad, pitch: rad, leftover: rad, eye: p3@World, hips: p3@World) {
  trace(p) = {
    neck = min(capsule(p, [0, -0.30m, -0.12m], [0, -0.18m, 0.08m], 0.02m),
           min(capsule(p, [0, -0.18m, 0.08m], [0, -0.08m, -0.04m], 0.02m),
               capsule(p, [0, -0.08m, -0.04m], [0, 0, 0.02m], 0.02m)))
    skull = sphere(p - [0,0,0]@Head, 0.035m)
    beak = capsule(p, [0, -0.005m, 0.03m], [0, -0.01m, 0.25m], 0.012m)
    min(neck, min(skull, beak))
  }
  material(p, n) = ggx([0.9,0.85,0.6], 0.4, 0)
}

kind Water @World () {
  trace(p) = p.y
  material(p, n) = ggx([0.1,0.2,0.25], 0.1, 0) + coat(0.6, 0.02)
}

law fish_swim(f: Fish, dt) {
  f.u = f.u + 0.5 * (dt / 1s)
  f.pos = fish_curve(f.u)
}

# 1. body: hop and turn while the neck is at its limit
law heron_body(b: Heron, dt, fish: [Fish]) {
  want = wrap_pi(bearing(b.pos, fish[b.target].pos) - b.heading)
  if b.hop < 0s && abs(want) > neck_limit { b.hop = 0s }
  if b.hop >= 0s {
    k = b.hop / hop_len
    step = turn_rate * (dt / 1s)
    b.heading = b.heading + clamp(want, -step, step)
    b.lift = 4 * hop_h * k * (1 - k)
    b.hop = b.hop + dt
    if b.hop >= hop_len {
      b.hop = -1s
      b.lift = 0m
    }
  }
  b.pose = place([b.pos.x, 0.75m + b.lift, b.pos.z]@World) * rot_y(b.heading)
}

# 2. neck and head: gaze lock first, then the limit
law neck_head(h: Head, dt, bodies: [Heron], fish: [Fish]) {
  b = bodies[h.parent]
  anchor = b.pose * [0, 0.5m, 0.28m]@Body
  to = fish[h.target].pos
  want = wrap_pi(bearing(anchor, to) - b.heading)
  h.yaw = clamp(want, -neck_limit, neck_limit)
  h.leftover = abs(want - h.yaw)
  h.pitch = clamp(atan2(to.y - anchor.y, dist(anchor, [to.x, anchor.y, to.z]@World)), -0.9rad, 0.5rad)
  h.pose = b.pose * place([0, 0.5m, 0.28m]@Body) * rot_y(h.yaw) * rot_x(-h.pitch)
  h.eye = h.pose * [0, 0.025m, 0.05m]@Head
  h.hips = b.pose * [0, -0.05m, -0.08m]@Body
}

# 3. checks
check gaze(h: Head) {
  if !finite(h.pose) { return unknown(NonFinitePose) }
  if h.leftover > 0.001rad { missed(h.leftover) } else { held(neck_limit - abs(h.yaw)) }
}

check head_above_hips(h: Head) {
  if !finite(h.eye) { return unknown(NonFiniteEye) }
  margin = h.eye.y - h.hips.y - 0.2m
  if margin < 0m { missed(-margin) } else { held(margin) }
}

# build
fish = Fish(u: 0.0, pos: fish_curve(0.0))
body = Heron(pose: place([0, 0.75m, 0]@World), pos: [0,0,0]@World, heading: 0rad, hop: -1s, lift: 0m, target: fish)
Head(parent: body, target: fish, pose: place([0, 1.25m, 0.28m]@World), yaw: 0rad, pitch: 0rad, leftover: 0rad, eye: [0, 1.27m, 0.33m]@World, hips: [0, 0.7m, -0.08m]@World)
Water()

light = sun(dir([-0.4,-0.8,-0.3]@World), [1.0,0.95,0.85] * 3)
sky(rd) = [0.6,0.72,0.88]

schedule Main {
  run_laws(fish_swim, heron_body, neck_head)
  check(gaze, head_above_hips)
  orbit([0,0.8m,0.5m]@World, 3.5m, 0.35rad)
  build_indirect(Water, Heron, Head, Fish)
  render_bundle(Vis64) { draw_indirect(Water, Heron, Head, Fish) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
}
```


================================================================
# SONNET OUTPUT F
# file: agent_notes/2026-11-20_syntax_bakeoff/out_F_sonnet.md
# what it is: the three tasks written by Sonnet in syntax F
================================================================

## Task 1

```
space World Body Leg Claw Shell Eye Sand

kind Sand @Sand (pose: xf@Sand>World)
  trace(p) = p.y + 0.01m * fbm2(p.xz * 6)
  material(p, n) = ggx({0.45, 0.36, 0.25}, 0.2, 0) + coat(0.5, 0.05)
end

kind CrabBody @Body (pose: xf@Body>World, vel: v3@World, com: p3@World, head: p3@Body, gaze: p3@World)
  trace(p) = round_box(p, {0.18m, 0.07m, 0.14m}, 0.04m)
  material(p, n) = chitin(p, n)
end

kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m)
  trace(p) = smin(capsule(p, {0, 0, 0}, {femur, 0, 0}, 0.025m),
                  capsule(p, {femur, 0, 0}, {femur + tibia, -0.05m, 0}, 0.018m), 0.02m)
  material(p, n) = chitin(p, n)
end

kind Claw[serrated: bool] @Claw (parent: CrabBody, pose: xf@Claw>World, size: f, open: rad)
  trace(p) =
    local q = p / size
    local palm = round_box(q, {0.10, 0.05, 0.07}, 0.03)
    local fixed = capsule(q, {0.08, 0, 0.02}, {0.26, 0, 0.0}, 0.025)
    local qm = rot_z(q - {0.08, 0, -0.02}, open)
    local moving = capsule(qm, {0, 0, 0}, {0.18, 0, 0.0}, 0.022)
    local teeth = if serrated then 0.006 * sin(qm.x * 120) else 0 end
    smin(smin(palm, fixed, 0.02), moving + teeth, 0.01) * size
  end
  material(p, n) = chitin(p, n)
end

kind Eye @Eye (parent: CrabBody, pose: xf@Eye>World, pupil: d3@Eye)
  trace(p) = min(capsule(p, {0, 0, 0}, {0, 0.10m, 0}, 0.008m), length(p - {0, 0.11m, 0}) - 0.02m)
  material(p, n) = ggx({0.05, 0.05, 0.05}, 0.1, 0) + coat(0.9, 0.02)
end

kind Shell @Shell (parent: CrabBody, pose: xf@Shell>World)
  surface(u, v) grid 64x32 =
    local s = whorl(u)
    local th = v * 6.2832rad
    local rib = 1 + 0.08 * sin(th * 14 + u * 40)
    local r = 0.02m * exp(1.6 * u) * rib
    s.o + (s.n * cos(th) + s.b * sin(th)) * r
  end
  material(p, n) = ggx({0.85, 0.72, 0.55}, 0.4, 0) + coat(0.4, 0.1)
end

fn spine(u) = {0.12m * exp(1.1 * u) * cos(u * 18rad), 0.05m * u * 4, 0.12m * exp(1.1 * u) * sin(u * 18rad)}

scan whorl u in 0..1 by 256 on bake_lut
  init = frame(spine(u), dir(spine'(u)), {0, 1, 0})
  step prev = transport(prev, spine(u), dir(spine'(u)))
end

fn chitin(p, n) = ggx({0.85, 0.28, 0.08}, 0.35 + 0.1 * fbm2(p.xz * 40), 0) + sss({0.9, 0.3, 0.1}, 0.08) + coat(0.7, 0.07)

fn socket(j, side) = place({side * 0.12m, -0.02m, 0.05m * j - 0.12m}@Body)

fn crab(at)
  local body = CrabBody{pose = at, vel = {0, 0, 0}, com = at * {0, 0, 0}@Body, head = {0, 0.1m, 0.15m}@Body, gaze = {0.6m, 0.3m, 0.8m}@World}
  for j in 0..6, side in {-1, 1}
    CrabLeg{parent = body, pose = at * socket(j, side), femur = 0.10m + 0.01m * j, tibia = 0.09m + 0.008m * j}
  end
  Claw[true]{parent = body, pose = at * place({0.16m, 0, 0.16m}@Body), size = 1.0, open = 0.5rad + 0.2rad * sin(time * 1s)}
  Claw[false]{parent = body, pose = at * place({-0.16m, 0, 0.14m}@Body), size = 0.55, open = 0.3rad}
  local q = at |> mirror_x |> move({-0.05m, 0.06m, 0.14m})
  local gaze = {0.6m + 0.2m * sin(time * 0.3s), 0.3m, 0.8m}@World
  local eye_c = {0.03, 0.0, 0.0}@q
  local eye_w = at * q.out(eye_c)
  local look = q.into(inv(at) * dir(gaze - eye_w))
  Eye{parent = body, pose = at * q.out(eye_c), pupil = look}
  Shell{parent = body, pose = at * place({-0.05m, 0.10m, -0.05m}@Body)}
  body
end

local ground = Sand{pose = place({0, 0, 0}@World)}
local at = place({0, 0.07m, 0}@World)
crab(at)

light sun(p) = {3.2, 3.0, 2.6} * 1.0
sky = {0.55, 0.65, 0.8}

schedule Main
  orbit({0, 0.1m, 0}@World, 0.9m, 25deg)
  build_indirect(Sand, CrabLeg, Claw)
  render_bundle(Vis64) do
    draw_indirect(Sand, CrabLeg, Claw)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky)
end
```

## Task 2

```
space World Reed Wet

kind Reed @Reed (base: p3@World, height: m, lean: rad, stiff: f, phase: f, seed: f)
  bend(h) = lean + (1 - stiff) * 0.35rad * h * h * (0.6 + 0.4 * sin(time * 1.3s + phase + base.x * 0.8 - base.z * 0.5))
  trace(p) =
    local q = p - base
    local h = clamp(q.y / height, 0, 1)
    local a = bend(h)
    local tip = {sin(a) * height * h, cos(a) * height * h, 0}
    local stalk = capsule(q, {0, 0, 0}, tip, 0.012m * (1 - 0.6 * h))
    local head = capsule(q, tip, tip + {sin(a) * 0.12m, cos(a) * 0.12m, 0}, 0.02m)
    min(stalk, head)
  end
  material(p, n) = ink(n)
end

kind Water @Wet (pose: xf@Wet>World)
  trace(p) = p.y
  material(p, n) = {0.93, 0.89, 0.8} + 0.04 * fbm2(p.xz * 3) * {0.3, 0.5, 0.6}
end

lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut then dark else {0.93, 0.89, 0.78} end

fn hash1(x: f) -> f = wgsl[[ return fract(sin(x * 127.1) * 43758.5); ]]

fn bed()
  for i in 0..200
    local a = hash1(i * 1.0) * 6.2832rad
    local r = 1.2m * sqrt(hash1(i * 3.7)) * (0.7 + 0.5 * hash1(i * 9.1))
    Reed{base = {r * cos(a), 0, r * sin(a)}@World, height = 0.9m + 0.6m * hash1(i * 5.3),
         lean = (hash1(i * 7.7) - 0.5) * 0.3rad, stiff = hash1(i * 2.9) * 0.8,
         phase = hash1(i * 4.1) * 6.2832, seed = i * 1.0}
  end
end

Water{pose = place({0, 0, 0}@World)}
bed()

light sun(p) = {2.8, 2.6, 2.2}
sky = {0.93, 0.89, 0.8}

schedule Main
  orbit({0, 0.6m, 0}@World, 4m, 15deg)
  build_indirect(Water, Reed)
  render_bundle(Vis64) do
    draw_indirect(Water, Reed)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky)
end
```

## Task 3

```
space World Body Neck Head Ground

kind Heron @Body (pose: xf@Body>World, heading: rad, yaw: rad, head: rad, hop: f, air: m, hips: p3@Body,
                  eye: p3@Head, fwd: d3@Head, neck_limit: rad, turn_rate: rad, hop_t: f)
  trace(p) = smin(round_box(p, {0.10m, 0.07m, 0.22m}, 0.03m),
                  capsule(p, {0, 0.1m, 0.2m}, {0, 0.35m, 0.28m}, 0.02m), 0.04m)
  material(p, n) = ggx({0.55, 0.6, 0.65}, 0.5, 0)
end

kind Leg @Body (parent: Heron, side: f)
  trace(p) = capsule(p, {side * 0.05m, -0.05m, 0}, {side * 0.05m, -0.5m, 0.02m}, 0.012m)
  material(p, n) = ggx({0.8, 0.7, 0.3}, 0.5, 0)
end

kind HeadPart @Head (parent: Heron)
  trace(p) = min(length(p) - 0.04m, capsule(p, {0, 0, 0.03m}, {0, 0, 0.18m}, 0.012m))
  material(p, n) = ggx({0.9, 0.75, 0.2}, 0.4, 0)
end

kind Fish @World (t: f)
  path(t) = {1.5m * cos(t * 0.4), 0.0, 1.2m * sin(t * 0.4) + 1.0m}@World
  trace(p) = length(p - path(t)) - 0.04m
  material(p, n) = ggx({0.7, 0.75, 0.8}, 0.2, 0) + coat(0.6, 0.05)
end

fn wrap(a: rad) -> rad = a - 6.2832rad * floor((a + 3.1416rad) / 6.2832rad)

fn gaze_target(f: Fish) -> p3@World = {1.5m * cos(f.t * 0.4), 0.0, 1.2m * sin(f.t * 0.4) + 1.0m}@World

fn bearing(h: Heron, tgt: p3@World) -> rad =
  local d = inv(h.pose) * tgt
  atan2(d.x, d.z)
end

law neck_head(h: Heron, dt, fish: [Fish])
  local tgt = gaze_target(fish[0])
  local want = bearing(h, tgt)
  local off = wrap(want - h.yaw)
  h.head = clamp(off, -h.neck_limit, h.neck_limit)
end

law body_turn(h: Heron, dt, fish: [Fish])
  local tgt = gaze_target(fish[0])
  local want = wrap(bearing(h, tgt))
  local over = abs(want) - h.neck_limit
  if over > 0rad and h.hop_t <= 0 then h.hop_t = 1 end
  if h.hop_t > 0 then
    h.hop_t = h.hop_t - dt / 0.5s
    h.air = 0.12m * sin(clamp(1 - h.hop_t, 0, 1) * 3.1416)
    h.heading = h.heading + sign(want) * min(abs(over), h.turn_rate * dt)
    if h.hop_t <= 0 then h.air = 0m; h.hop_t = 0 end
  end
  h.pose = place({0, 0.5m + h.air, 0}@World) * rot_y(h.heading)
end

check gaze(h: Heron, fish: [Fish])
  if not finite(h.pose) then return unknown(NonFinitePose) end
  local want = bearing(h, gaze_target(fish[0]))
  local left = abs(wrap(want - h.head)) 
  local over = abs(wrap(want)) - h.neck_limit
  if over > 0rad then missed(over) else held(h.neck_limit - abs(wrap(want))) end
end

check head_above_hips(h: Heron)
  local margin = (h.pose * h.eye).y - (h.pose * h.hips).y
  if margin < 0m then missed(-margin) else held(margin) end
end

local bird = Heron{pose = place({0, 0.5m, 0}@World), heading = 0rad, yaw = 0rad, head = 0rad, hop = 0, air = 0m,
                   hips = {0, 0, -0.1m}@Body, eye = {0, 0.02m, 0.03m}@Head, fwd = dir({0, 0, 1}),
                   neck_limit = 70deg, turn_rate = 4rad, hop_t = 0}
Leg{parent = bird, side = -1}
Leg{parent = bird, side = 1}
HeadPart{parent = bird}
Fish{t = 0}

light sun(p) = {3.0, 2.8, 2.4}
sky = {0.6, 0.7, 0.85}

schedule Main
  run_laws(body_law, body_turn)
  run_laws(neck_law, neck_head)
  check(gaze)
  check(head_above_hips)
  orbit({0, 0.5m, 0.5m}@World, 4m, 30deg)
  build_indirect(Ground, Heron, Leg, HeadPart, Fish)
  render_bundle(Vis64) do
    draw_indirect(Ground, Heron, Leg, HeadPart, Fish)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky)
end
```


================================================================
# SONNET OUTPUT G
# file: agent_notes/2026-11-20_syntax_bakeoff/out_G_sonnet.md
# what it is: the three tasks written by Sonnet in syntax G
================================================================

# Task 1

```clojure
(defspace World Body Leg Shell Claw Eyes)

(defn spine [t]
  (let [a ($ t * 18.0)
        r ($ 0.012m * (exp ($ 0.11 * a)))]
    (p3 Shell (* r (cos a)) ($ 0.006m * a) (* r (sin a)))))

(defscan whorl [u 0 1 256] :on bake-lut
  :init (frame (spine u) (dir (d spine u)) [0 1 0])
  :step [prev] (transport prev (spine u) (dir (d spine u))))

(defn shell-radius [u]
  ($ 0.010m * (exp ($ 0.11 * u * 18.0)) * (+ 1.0 (* 0.10 (sin ($ u * 400.0))))))

(defn hash1 [x y z] (hash [x y z]))

(defn chitin [p]
  (+ (ggx [0.85 0.28 0.10] ($ 0.22 + 0.18 * (fbm2 p)) 0)
     (sss [0.9 0.3 0.1] 0.08)
     (coat 0.7 0.07)))

(defn sand [p] (+ (ggx [0.45 0.38 0.28] 0.35 0) (coat 0.4 0.12)))

(defkind ShellK :in Shell {:pose (xf Shell World)}
  (surface [u v] :grid [64 32]
    (let [f (whorl u)
          ang ($ v * 6.2832rad)
          r (shell-radius u)]
      (* pose (+ (:o f) (* (* r (cos ang)) (:n f)) (* (* r (sin ang)) (:b f))))))
  (material [p n] (chitin p)))

(defkind CrabBody :in Body {:pose (xf Body World) :head (p3 Body)}
  (trace [p] (capsule p [-0.12m 0 0] [0.12m 0 0] 0.10m))
  (material [p n] (chitin p)))

(defkind CrabLeg :in Leg {:parent CrabBody :pose (xf Leg World) :femur m :tibia m :foot (p3 World)}
  (trace [p]
    (min (capsule p [0 0 0] [femur 0 0] 0.012m)
         (capsule p [femur 0 0] [femur (* -1 tibia) 0] 0.009m)))
  (material [p n] (chitin p)))

(defkind Claw :in Claw {:pose (xf Claw World) :open rad :scale f}
  (trace [p]
    (let [q (* (inv pose) p)
          s (/ q scale)
          palm (capsule s [0 0 0] [0.02m 0 0] 0.035m)
          fixed (capsule s [0.03m 0 0] [0.11m 0.01m 0] 0.012m)
          h (-> s (move [0.03m 0 0]) (rot-z (* -1 open)))
          hp (:p h)
          moving (+ (capsule hp [0 0 0] [0.08m 0 0] 0.011m)
                    ($ 0.003m * (sin ($ (:x hp) * 900.0))))]
      (* scale (min palm fixed moving))))
  (material [p n] (chitin p)))

(defn target [t]
  (p3 World ($ 0.4m * (sin ($ t * 0.3))) 0.15m ($ 0.6m + 0.2m * (cos ($ t * 0.21)))))

(defkind EyesK :in Eyes {:pose (xf Eyes World)}
  (trace [p]
    (let [sym (-> (* (inv pose) p) mirror-x (move [-0.03m 0.0m 0.0m]))
          c (p3 sym 0 0.04m 0)
          stalk (capsule (:p sym) [0 0 0] [0 0.04m 0] 0.005m)
          ball (capsule (:p sym) [0 0.04m 0] [0 0.04m 0] 0.012m)
          g (in sym (* (inv pose) (dir (- (target time) (* pose (out sym c))))))
          pupil (capsule (:p sym) [0 0.04m 0] (+ [0 0.04m 0] (* 0.011m g)) 0.006m)]
      (min stalk ball pupil)))
  (material [p n] (+ (ggx [0.05 0.05 0.05] 0.2 0) (coat 0.8 0.05))))

(defkind Ground :in World {}
  (trace [p] (:y p))
  (material [p n] (sand p)))

(defn! crab [b body]
  (for! [j (range 6) side [-1 1]]
    (emit! CrabLeg {:parent b
                    :pose (* body (move [(* side 0.08m) 0 (* 0.025m j)]))
                    :femur (+ 0.040m (* 0.005m j))
                    :tibia (+ 0.060m (* 0.004m j))
                    :foot (* body (p3 Body (* side 0.2m) 0 (* 0.03m j)))})))

(let! [at (place [0 0.05m 0])
       b (emit! CrabBody {:pose at :head (p3 Body 0.12m 0.04m 0)})]
  (crab b at)
  (emit! ShellK {:pose (* at (move [-0.05m 0.1m 0]))})
  (emit! Claw {:pose (* at (move [0.14m 0.02m 0.10m])) :open 0.5rad :scale 1.0})
  (emit! Claw {:pose (* at (move [0.14m 0.02m -0.10m])) :open 0.2rad :scale 0.6})
  (emit! EyesK {:pose (* at (move [0.12m 0.08m 0]))}))

(emit! Ground {})
(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]})
(emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main (run-laws)
  (orbit [0 0.05m 0] 0.8m 40deg)
  (draw Vis64 Ground CrabBody CrabLeg Claw EyesK ShellK)
  (shade Vis64 Sun Sky)
  (tonemap-aces Sky))
```

# Task 2

```clojure
(defspace World Reed)

(deflobe ink [dark cut n v l]
  (if (< (dot n l) cut) dark [0.92 0.85 0.7]))

(defn paper [p n] (ink [0.12 0.10 0.30] 0.0))
(defn water-mat [p n] (+ (ink [0.55 0.58 0.62] -2.0)))
(defn sky-ink [p n] (ink [0.75 0.78 0.82] -2.0))

(defn gust [x t] (+ 0.5 (* 0.5 (sin ($ x * 1.3 - t * 1.6)))))

(defkind Reed :in Reed {:pose (xf Reed World) :height m :lean rad :stiff f :phase f}
  (trace [p]
    (let [q (* (inv pose) p)
          y (:y q)
          k ($ y / height)
          w ($ (gust (:x q) time) * (+ 0.6 (* 0.4 (sin ($ time * 1.1 + phase)))) * 0.25m / stiff)
          bend ($ (+ w (* height (sin lean))) * k * k)
          qb (p3 Reed (- (:x q) bend) y (:z q))
          stalk (capsule qb [0 0 0] [0 (* 0.92 height) 0] ($ 0.006m * (- 1.0 (* 0.5 k))))
          head (capsule qb [0 (* 0.92 height) 0] [0 height 0] 0.014m)]
      (min stalk head)))
  (material [p n] (paper p n)))

(defkind Water :in World {}
  (trace [p] (:y p))
  (material [p n] (water-mat p n)))

(defn! reeds []
  (for! [j (range 200)]
    (let! [a (* 6.2832 (hash [(* 1.0 j) 1.0 2.0]))
           r (* 1.5m (sqrt (hash [(* 1.0 j) 3.0 4.0])))
           h (+ 0.9m (* 0.6m (hash [(* 1.0 j) 5.0 6.0])))]
      (emit! Reed {:pose (place [(* r (cos a)) 0 (* r (sin a))])
                   :height h
                   :lean (* 0.12rad (- (hash [(* 1.0 j) 7.0 8.0]) 0.5))
                   :stiff (+ 0.6 (* 1.2 (hash [(* 1.0 j) 9.0 1.0])))
                   :phase (* 6.2832 (hash [(* 1.0 j) 2.0 9.0]))}))))

(defraw hash [p :v3] :-> f "return fract(sin(dot(p, vec3(12.9, 78.2, 37.7))) * 43758.5);")

(reeds)
(emit! Water {})
(emit! Sun {:dir (dir [-3 5 2]) :color [1 1 1]})
(emit! Sky {:zenith [0.92 0.88 0.8]})

(defschedule Main (run-laws)
  (orbit [0 0.6m 0] 4m 15deg)
  (draw Vis64 Water Reed)
  (shade Vis64 Sun Sky)
  (tonemap-aces Sky))
```

# Task 3

```clojure
(defspace World Trunk Head)

(defn wrap [a] (- (mod (+ a 3.1416rad) 6.2832rad) 3.1416rad))
(defn bearing [from to] (atan2 (- (:x to) (:x from)) (- (:z to) (:z from))))
(defn clampa [a lim] (max (* -1 lim) (min lim a)))

(defkind Trunk :in Trunk {:pose (xf Trunk World) :yaw rad :y m :vy f :air bool}
  (trace [p]
    (let [q (* (inv pose) p)
          body (capsule q [0 0.55m 0.12m] [0 0.60m -0.12m] 0.07m)
          legl (capsule q [0.04m 0.5m 0] [0.04m 0 0] 0.012m)
          legr (capsule q [-0.04m 0.5m 0] [-0.04m 0 0] 0.012m)]
      (min body legl legr)))
  (material [p n] (+ (ggx [0.5 0.55 0.6] 0.5 0))))

(defkind Neck :in Trunk {:pose (xf Trunk World)}
  (trace [p]
    (let [q (* (inv pose) p)]
      (min (capsule q [0 0.62m 0.12m] [0 0.80m 0.20m] 0.02m)
           (capsule q [0 0.80m 0.20m] [0 0.95m 0.12m] 0.02m)
           (capsule q [0 0.95m 0.12m] [0 1.05m 0.16m] 0.02m))))
  (material [p n] (+ (ggx [0.5 0.55 0.6] 0.5 0))))

(defkind HeronHead :in Head
  {:pose (xf Head World) :rel rad :over rad :limit rad :hips (p3 World)}
  (trace [p]
    (let [q (* (inv pose) p)
          skull (capsule q [0 0 0] [0 0 0.03m] 0.03m)
          beak (capsule q [0 0 0.03m] [0 0 0.20m] 0.008m)]
      (min skull beak)))
  (material [p n] (+ (ggx [0.8 0.7 0.2] 0.4 0))))

(defkind Fish :in World {:pos (p3 World) :s f}
  (trace [p] (capsule p (:pos (this)) (+ (:pos (this)) [0.06m 0 0]) 0.02m))
  (material [p n] (+ (ggx [0.7 0.7 0.8] 0.2 0) (coat 0.8 0.05))))

(deflaw fish-move [f :Fish dt] :writes [:pos :s]
  (let [s (+ (:s f) (* 0.12 dt))
        a ($ s * 6.2832)]
    (write f :s s)
    (write f :pos [(* 1.2m (cos a)) 0.05m (+ 1.0m (* 0.8m (sin a)))])))

(deflaw hop [t :Trunk dt fishes heads] :writes [:yaw :y :vy :air]
  (let [h (heads 0)
        fish (:pos (fishes 0))
        want (bearing [0 0 0] fish)
        err (wrap (- want (:yaw t)))]
    (cond
      (and (not (:air t)) (> (:over h) 0.0rad))
        (do (write t :vy 1.8) (write t :air true))
      (:air t)
        (let [vy (- (:vy t) (* 9.8 dt))
              y (+ (:y t) (* vy dt))]
          (write t :yaw (+ (:yaw t) (clampa err (* 6.0 dt))))
          (if (< y 0.0m)
            (do (write t :y 0.0m) (write t :vy 0.0) (write t :air false))
            (do (write t :y y) (write t :vy vy))))
      true nil)))

(deflaw gaze [h :HeronHead dt trunks fishes] :writes [:rel :over :pose :hips]
  (let [t (trunks 0)
        top (* (:pose t) (p3 Trunk 0 1.05m 0.16m))
        fish (:pos (fishes 0))
        want (wrap (- (bearing top fish) (:yaw t)))
        rel (clampa want (:limit h))
        over (max 0.0rad (- (abs want) (:limit h)))]
    (write h :rel rel)
    (write h :over over)
    (write h :hips (* (:pose t) (p3 Trunk 0 0.55m 0)))
    (write h :pose (* (:pose t) (move [0 1.05m 0.16m]) (rot-y rel)))))

(defcheck gaze-held [h :HeronHead] :-> rad
  (if (> (:over h) 0.0rad)
    (missed (:over h))
    (held (- (:limit h) (abs (:rel h))))))

(defcheck head-above-hips [h :HeronHead] :-> m
  (let [margin ($ (:y (* (:pose h) (p3 Head 0 0 0))) - (:y (:hips h)))]
    (if (neg? margin) (missed (- margin)) (held margin))))

(let! [at (place [0 0 0])]
  (emit! Trunk {:pose at :yaw 0.0rad :y 0.0m :vy 0.0 :air false})
  (emit! Neck {:pose at})
  (emit! HeronHead {:pose (* at (move [0 1.05m 0.16m])) :rel 0.0rad :over 0.0rad
                    :limit 1.0rad :hips (p3 World 0 0.55m 0)})
  (emit! Fish {:pos (p3 World 1.2m 0.05m 1.0m) :s 0.0}))

(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]})
(emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main
  (run-laws fish-move hop gaze)
  (check gaze-held head-above-hips)
  (orbit [0 0.6m 0.5m] 3m 30deg)
  (draw Vis64 Trunk Neck HeronHead Fish)
  (shade Vis64 Sun Sky)
  (tonemap-aces Sky))
```


================================================================
# HAIKU OUTPUT A
# file: agent_notes/2026-11-20_syntax_bakeoff/out_A_haiku.md
# what it is: the three tasks written by Haiku in syntax A
================================================================

# Card A (Haiku 5.5) outputs

## Task 1: a hermit crab

```loft
space World, Body, Leg, Claw, Face, Shell;

// ---- sockets: where each part sits on the body (Body -> World is set per crab) ----
fn leg_socket(j: u32, side: f32) -> Xform[Leg -> Body] {
  return translate_xf(Vec[Body](side * 0.25m, 0.05m, 0.12m * f32(j) - 0.3m)) * rot_y(side * 0.8rad);
}
fn claw_socket(side: f32) -> Xform[Claw -> Body] {
  return translate_xf(Vec[Body](0.30m, 0.02m, 0.25m * side)) * rot_y(0.0rad);
}
fn eye_socket() -> Xform[Face -> Body] {
  return translate_xf(Vec[Body](0.0m, 0.0m, 0.30m));
}
fn shell_socket() -> Xform[Shell -> Body] {
  return translate_xf(Vec[Body](-0.10m, 0.10m, 0.0m));
}
fn body_pose() -> Xform[Body -> World] {
  return translate_xf(Vec[World](0.0m, 0.12m, 0.0m)) * rot_y(0.3rad);
}

// ---- wet chitin: one material, noise only in the material ----
fn wet_roughness(p: vec3<f32>) -> f32 {
  return 0.25 + 0.2 * fbm2(p.xz * 60.0);
}
fn wet_chitin(p: vec3<f32>) -> Bsdf {
  let r = wet_roughness(p);
  return ggx(vec3(0.75, 0.22, 0.08), r, 0.0)      // red-orange base, bottom
       + sss(vec3(0.9, 0.3, 0.1), 0.08)           // translucent subsurface
       + clearcoat(0.7, 0.07);                    // clear wet coat on top
}

// ---- shell: a coiled spiral spine with transported frames ----
fn spine(u: f32) -> Pt[Shell] {
  let turn = 6.2831853 * 2.5 * u;
  let r = 0.55 * (1.0 - 0.35 * u);
  return Pt[Shell](r * cos(turn), 0.9 * u, r * sin(turn));
}

scan whorl(u: f32 in 0.0..1.0, steps: 256u) @schedule(bake_lut) -> Table[Frame[Shell]] {
  init: frame_from_tangent(spine(u), normalize(diff(spine, u)(u)), Dir[Shell](0.0, 1.0, 0.0));
  step(prev, u_i): transport_frame(prev, spine(u_i), normalize(diff(spine, u)(u_i)));
}

kind Whorl {
  row { parent: RowId[CrabBody], pose: Xform[Shell -> World], tube: Meters, ribs: f32 }
  @grid(256u, 24u) query surface(self, u: f32, v: f32) -> Pt[Shell] {
    let f = sample_c2(whorl, u);
    let ang = 6.2831853 * v;
    let grow = 1.0 + 0.9 * u;                                      // radius grows with the spiral
    let rib = 1.0 + 0.12 * sin(self.ribs * 6.2831853 * 14.0 * u);  // ribs run along the whorl
    let r = self.tube * grow * rib;
    return f.origin + (f.normal * cos(ang) + f.binormal * sin(ang)) * r;
  }
}

// ---- claw: one definition; crusher and feeder differ only in row data ----
kind Claw {
  row { parent: RowId[CrabBody], pose: Xform[Claw -> World], size: Meters, open: Rad, teeth: f32 }
  query trace(self, p: Pt[Claw]) -> Field[Claw] {
    let palm  = sd_round_box(p - Pt[Claw](0.0m, 0.0m, 0.0m),
                             Vec[Claw](self.size, 0.8 * self.size, 0.7 * self.size), 0.25 * self.size);
    let hinge = Pt[Claw](self.size, 0.0m, 0.0m);
    let fixed = sd_capsule(p, Pt[Claw](self.size, -0.1 * self.size, 0.0m),
                              Pt[Claw](1.6 * self.size, -0.1 * self.size, 0.0m), 0.035 * self.size);
    // rotate the query point back about the hinge, so the moving finger stays a plain capsule
    let d = p - hinge;
    let c = cos(self.open);
    let s = sin(self.open);
    let q = hinge + Vec[Claw](c * d.x + s * d.y, -s * d.x + c * d.y, d.z);
    let tip = hinge + Vec[Claw](1.6 * self.size, 0.0m, 0.0m);
    let teeth = 0.006m * sin(self.teeth * 6.2831853 * (q.x - hinge.x) / self.size);   // serrations
    let mover = sd_capsule(q, hinge, tip, 0.03 * self.size) + teeth;
    return smin_poly(palm, min(fixed, mover), 0.02 * self.size);
  }
  query material(self, p: Pt[Claw], n: Norm[Claw]) -> Bsdf {
    return wet_chitin(vec3<f32>(p.x / 1.0m, p.y / 1.0m, p.z / 1.0m));
  }
}

// ---- legs: one kind, one row per leg ----
kind CrabLeg {
  row { parent: RowId[CrabBody], pose: Xform[Leg -> World], femur: Meters, tibia: Meters, foot: Pt[World] }
  query trace(self, p: Pt[Leg]) -> Field[Leg] {
    let knee  = Pt[Leg](self.femur, 0.0m, 0.0m);
    let ankle = knee + Vec[Leg](self.tibia * 0.6, -self.tibia * 0.8, 0.0m);
    let thigh = sd_capsule(p, Pt[Leg](0.0m, 0.0m, 0.0m), knee, 0.07m);
    let shank = sd_capsule(p, knee, ankle, 0.05m);
    return smin_poly(thigh, shank, 0.03m);
  }
  query material(self, p: Pt[Leg], n: Norm[Leg]) -> Bsdf {
    return wet_chitin(vec3<f32>(p.x / 1.0m, p.y / 1.0m, p.z / 1.0m));
  }
}

// ---- body ----
kind CrabBody {
  row { pose: Xform[Body -> World], com: Pt[World], vel: Vec[World] }
  query trace(self, p: Pt[Body]) -> Field[Body] {
    return sd_round_box(p - Pt[Body](0.0m, 0.0m, 0.0m), Vec[Body](0.35m, 0.12m, 0.45m), 0.10m);
  }
  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf {
    return wet_chitin(vec3<f32>(p.x / 1.0m, p.y / 1.0m, p.z / 1.0m));
  }
}

// ---- eyes: one mirrored definition; the mirror cell gives the two eyes ----
map sym = mirror_x(q) |> translate(-Vec[Face](0.15, 0.72, 0.42));
let eye_c = Pt['sym](0.03, 0.30, 0.07);

fn gaze_target(time: Sec) -> Pt[World] {
  // a point in world space that drifts slowly
  return Pt[World](0.6 * sin(0.2 * f32(time)), 0.25, 1.4 + 0.3 * cos(0.13 * f32(time)));
}

kind Eye {
  row { parent: RowId[CrabBody], pose: Xform[Face -> World] }
  query trace(self, p: Pt[Face]) -> Field[Face] {
    let stalk = sd_capsule(sym, Pt['sym](0.0, -0.25, 0.0), Pt['sym](0.0, 0.0, 0.0), 0.03m);
    return smin_poly(sd_sphere(sym - eye_c, 0.085m), stalk, 0.02m);
  }
  query material(self, p: Pt[Face], n: Norm[Face], time: Sec) -> Bsdf {
    let target = gaze_target(time);
    let eye_w = self.pose * (sym.to_parent * eye_c);                  // each eye's own world position
    let look = sym.from_parent * (inv(self.pose) * normalize(target - eye_w));
    let pupil = select(vec3(0.02, 0.02, 0.03), vec3(0.9, 0.85, 0.2), dot(n, look) > 0.6);
    return ggx(vec3(0.85, 0.8, 0.7), 0.2, 0.0) + emission(pupil);
  }
}

// ---- sand, light, sky ----
kind Sand {
  row { wetness: f32 }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.42, 0.36, 0.30), mix(0.5, 0.15, self.wetness), 0.0);
  }
}

kind SunSky {
  row { dir: Dir[World], power: f32 }
  query light(self, p: Pt[World], n: Norm[World]) -> vec3<f32> {
    return vec3(self.power) * max(dot(n, self.dir), 0.0);
  }
}

kind Sky {
  row { zenith: vec3<f32> }
  query sky(self, rd: Dir[World]) -> vec3<f32> {
    return mix(vec3(0.7, 0.8, 0.9), self.zenith, rd.y);
  }
}

let sun_sky = SunSky { dir: normalize(Dir[World](-0.4, 0.8, -0.3)), power: 1.1 };
let sky_color = Sky { zenith: vec3(0.35, 0.55, 0.85) };

// ---- build: rows only; the crab is emitted by a loop over legs ----
comptime fn crab(b: RowId[CrabBody], body: Xform[Body -> World]) {
  comptime for j in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg { parent: b, pose: body * leg_socket(j, side), femur: 0.40m + 0.05m * f32(j),
                     tibia: 0.30m - 0.02m * f32(j), foot: body * Pt[Body](side, 0.0, 0.3 * f32(j)) };
    }
  }
  // crusher on one side, feeder on the other: same kind, different data
  emit Claw { parent: b, pose: body * claw_socket(-1.0), size: 0.20m, open: 0.6rad, teeth: 5.0 };
  emit Claw { parent: b, pose: body * claw_socket(1.0),  size: 0.08m, open: 0.3rad, teeth: 3.0 };
  emit Whorl { parent: b, pose: body * shell_socket(), tube: 0.07m, ribs: 3.0 };
  emit Eye { parent: b, pose: body * eye_socket() };
}

comptime fn crabs() {
  let b = emit CrabBody { pose: body_pose(), com: Pt[World](0.0, 0.1, 0.0), vel: Vec[World](0.0, 0.0, 0.0) };
  crab(b, body_pose());
  emit Sand { wetness: 0.8 };
}

schedule Main {
  pass rows: crabs();
  pass camera: orbit(at: Pt[World](0.0, 0.1, 0.0), radius: 2.4m, fov: 40deg);
  pass cull: build_indirect(Sand, CrabBody, CrabLeg, Claw, Whorl, Eye);
  pass vis: render_bundle(Vis64) { draw_indirect(Sand, CrabBody, CrabLeg, Claw, Whorl, Eye); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```

## Task 2: a reed bed in wind

```loft
space World;

// ---- ink: two flat inks, a hard edge between lit and shadowed ----
@lobe fn ink(dark: vec3<f32>, cut: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>) -> vec4<f32> {
  return vec4(select(vec3(0.92, 0.85, 0.7), dark, dot(n, l) < cut), 0.0);
}

let sun_dir = normalize(Dir[World](-0.5, 0.8, -0.3));
let view_dir = Dir[World](0.0, 0.0, 1.0);

// ---- placement: 200 reeds in a loose, irregular cluster; built once ----
comptime fn hash01(n: u32) -> f32 {
  return fract(sin(f32(n) * 12.9898) * 43758.5453);
}

comptime fn reeds() {
  comptime for i in 0u..200u {
    let k = 7u * i;
    let a = 6.2831853 * hash01(k);
    let r = 3.0 * sqrt(hash01(k + 1u));
    emit Reed {
      root: Pt[World](r * cos(a), 0.0, r * sin(a)),
      height: 0.9m + 0.5m * hash01(k + 2u),
      lean: 0.4rad * (hash01(k + 3u) - 0.5),
      stiff: 0.2 + 0.8 * hash01(k + 4u),
      phase: 6.2831853 * hash01(k + 5u),
      base_r: 0.012m + 0.004m * hash01(k + 6u),
      seed_r: 0.035m,
    };
  }
}

// ---- reed: a tapered stalk with a seed head; sway from its own data and time ----
kind Reed {
  row { root: Pt[World], height: Meters, lean: Rad, stiff: f32, phase: f32, base_r: Meters, seed_r: Meters }

  query trace(self, p: Pt[World], time: Sec) -> Field[World] {
    let t = f32(time);
    let k = 1.0 / (1.0 + 2.0 * self.stiff);                      // stiffer reeds bend less
    let gust = sin(0.9 * f32(self.root.x) - 2.4 * t) * 0.5 + 0.5; // a gust travels across the bed
    let sway = f32(self.lean) + 0.5 * sin(self.phase + 1.7 * t) + 0.6 * gust;
    let bend = sway * k * f32(self.height);                      // displacement scale, metres
    var d = 1.0e3m;
    var a = self.root;
    for s in 1u..7u {
      let v = f32(s) / 6.0;
      // the bend grows toward the tip: offset is quadratic in height
      let b = Pt[World](self.root.x + bend * v * v, self.root.y + v * self.height, self.root.z);
      let r = self.base_r * (1.0 - 0.6 * v);
      d = smin_poly(d, sd_capsule(p, a, b, r), 0.01m);
      a = b;
    }
    return smin_poly(d, sd_sphere(p - a, self.seed_r), 0.02m);
  }

  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ink(vec3(0.10, 0.08, 0.07), 0.0, n, view_dir, sun_dir);
  }
}

// ---- water and sky: plain paper with a faint second ink ----
kind Water {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ink(vec3(0.80, 0.76, 0.66), -0.6, n, view_dir, sun_dir);
  }
}

kind Sky {
  row { paper: vec3<f32> }
  query sky(self, rd: Dir[World]) -> vec3<f32> { return self.paper; }
}

kind SunSky {
  row { dir: Dir[World], power: f32 }
  query light(self, p: Pt[World], n: Norm[World]) -> vec3<f32> {
    return vec3(self.power) * max(dot(n, self.dir), 0.0);
  }
}

let sun_sky = SunSky { dir: sun_dir, power: 1.0 };
let sky_color = Sky { paper: vec3(0.96, 0.92, 0.84) };

comptime fn reeds_scene() {
  reeds();
  emit Water { level: 0.0m };
}

schedule Main {
  pass rows: reeds_scene();
  pass camera: orbit(at: Pt[World](0.0, 0.5, 0.0), radius: 7m, fov: 40deg);
  pass cull: build_indirect(Water, Reed, Sky);
  pass vis: render_bundle(Vis64) { draw_indirect(Water, Reed); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: clamp_ldr(ColorHdr, sky: sky_color);
}
```

## Task 3: a heron's hop

```loft
space World, Body, Fish;

// ---- fish: moves along a curve in world space; its position is the gaze target ----
fn fish_path(t: Sec) -> Pt[World] {
  let s = f32(t);
  return Pt[World](1.4 * sin(0.25 * s), 0.02, 1.6 + 0.4 * cos(0.18 * s));
}

kind Fish {
  row { clock: Sec, pos: Pt[World] }
  query trace(self, p: Pt[World]) -> Field[World] { return sd_sphere(p - self.pos, 0.06m); }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.9, 0.5, 0.2), 0.3, 0.0) + emission(vec3(0.3, 0.12, 0.02));
  }
}

law swim(f: Fish, dt: Sec) writes Fish {
  f.clock = f.clock + dt;
  f.pos = fish_path(f.clock);
}

// ---- heron: trunk, two legs, S-shaped neck, head and beak, in Body space ----
// the neck pivots at the top of the trunk; its S-curve is laid out in Body space
fn neck_pt(s: f32) -> Pt[Body] {
  return Pt[Body](0.0, 0.62 + 0.55 * s, 0.15 + 0.2 * sin(6.2831853 * s));
}

fn pose_at(origin: Pt[World], yaw: Rad) -> Xform[Body -> World] {
  return translate_xf(Vec[World](origin.x, origin.y, origin.z)) * rot_y(yaw);
}

fn wrap_angle(a: f32) -> Rad {
  return Rad(atan2(sin(a), cos(a)));
}

kind Heron {
  row {
    base: Pt[World], pose: Xform[Body -> World], yaw: Rad, lift: Meters,
    hop: Sec, hopping: f32, target: RowId[Fish],
    neck: Rad, limit: Rad, want: Rad, gaze_err: Rad, at_limit: f32,
    head_y: Meters, hip_y: Meters
  }

  query trace(self, p: Pt[Body]) -> Field[Body] {
    // trunk, fixed in the body
    var d = sd_capsule(p, Pt[Body](0.0m, 0.45m, -0.15m), Pt[Body](0.0m, 0.62m, 0.15m), 0.12m);

    // neck and head turn about the trunk top by the neck angle: evaluate in the neck's frame
    let pivot = neck_pt(0.0);
    let c = cos(self.neck);
    let s = sin(self.neck);
    let r = p - pivot;
    let q = pivot + Vec[Body](c * r.x - s * r.z, r.y, s * r.x + c * r.z);
    for i in 0u..8u {
      let a = neck_pt(f32(i) / 8.0);
      let b = neck_pt(f32(i + 1u) / 8.0);
      d = smin_poly(d, sd_capsule(q, a, b, 0.035m), 0.02m);
    }
    let head_c = neck_pt(1.0) + Vec[Body](0.0m, 0.04m, 0.0m);
    let skull = sd_sphere(q - head_c, 0.07m);
    // beak runs along the head's forward axis (+z in the neck frame)
    let beak = sd_capsule(q, head_c + Vec[Body](0.0m, 0.0m, 0.05m), head_c + Vec[Body](0.0m, -0.01m, 0.30m), 0.02m);
    d = smin_poly(d, smin_poly(skull, beak, 0.01m), 0.02m);

    // two legs
    for k in 0u..2u {
      let side = f32(k) * 2.0 - 1.0;
      let hip  = Pt[Body](side * 0.08m, 0.45m, 0.0m);
      let knee = Pt[Body](side * 0.10m, 0.22m, 0.10m);
      let foot = Pt[Body](side * 0.08m, 0.0m, 0.0m);
      let leg = smin_poly(sd_capsule(p, hip, knee, 0.02m), sd_capsule(p, knee, foot, 0.012m), 0.01m);
      d = smin_poly(d, leg, 0.01m);
    }
    return d;
  }

  query material(self, p: Pt[Body], n: Norm[Body]) -> Bsdf {
    return ggx(vec3(0.62, 0.66, 0.68), 0.5, 0.0) + clearcoat(0.1, 0.2);
  }
}

// ---- laws, in order: body first ----
// the body hops while the neck is at its limit, turns the trunk toward the fish, and lands
law body_law(b: Heron, dt: Sec) writes Heron {
  if b.hopping < 0.5 && b.at_limit > 0.5 {
    b.hopping = 1.0;
    b.hop = 0.0s;
  }
  if b.hopping > 0.5 {
    b.hop = b.hop + dt;
    let phase = b.hop / 0.6s;
    b.lift = 0.22m * sin(3.14159265 * phase);
    // trunk turns toward the fish's world heading, at most 1.5 rad/s
    let step = 1.5rad * dt / 1.0s;
    b.yaw = b.yaw + clamp(b.want - b.yaw, -step, step);
    if phase >= 1.0 {
      b.hopping = 0.0;
      b.lift = 0.0m;
    }
  }
  b.pose = pose_at(b.base + Vec[World](0.0m, b.lift, 0.0m), b.yaw);
}

// then the neck and head: gaze lock first (aim at the fish in world space), then clamp the neck to its limit
law neck_law(b: Heron, dt: Sec, fish: Table[Fish]) writes Heron {
  let f = fish[b.target];
  let head = b.pose * neck_pt(1.0);
  let rel = f.pos - head;
  let want_abs = Rad(atan2(rel.x, rel.z));
  let rel_yaw = wrap_angle(f32(want_abs - b.yaw));
  let kept = clamp(rel_yaw, -b.limit, b.limit);
  b.neck = kept;
  b.gaze_err = rel_yaw - kept;               // the part of the gaze the neck cannot reach
  b.at_limit = f32(abs(rel_yaw) > b.limit);
  b.want = want_abs;
  b.head_y = head.y;
  b.hip_y = (b.pose * Pt[Body](0.0m, 0.45m, 0.0m)).y;
}

// checks: the gaze is held only when the neck reaches the target; otherwise it is missed by the leftover angle
constraint gaze_lock(b: Heron) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let miss = abs(b.gaze_err);
  if miss > 0.01rad { return Verdict::Missed(miss); }
  return Verdict::Held(0.01rad - miss);
}

constraint head_above_hips(b: Heron) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let margin = b.head_y - b.hip_y;
  if margin < 0.0m { return Verdict::Missed(-margin); }
  return Verdict::Held(margin);
}

// ---- build, light, camera, water ----
comptime fn heron_scene() {
  let fish = emit Fish { clock: 0.0s, pos: fish_path(0.0s) };
  emit Heron {
    base: Pt[World](0.0, 0.0, 0.0), pose: pose_at(Pt[World](0.0, 0.0, 0.0), 0.0rad),
    yaw: 0.0rad, lift: 0.0m, hop: 0.0s, hopping: 0.0, target: fish,
    neck: 0.0rad, limit: 0.6rad, want: 0.0rad, gaze_err: 0.0rad, at_limit: 0.0,
    head_y: 1.17m, hip_y: 0.45m
  };
  emit Water { level: 0.0m };
}

kind Water {
  row { level: Meters }
  query trace(self, p: Pt[World]) -> Field[World] { return p.y - self.level; }
  query material(self, p: Pt[World], n: Norm[World]) -> Bsdf {
    return ggx(vec3(0.35, 0.42, 0.45), 0.08, 0.0);
  }
}

kind SunSky {
  row { dir: Dir[World], power: f32 }
  query light(self, p: Pt[World], n: Norm[World]) -> vec3<f32> {
    return vec3(self.power) * max(dot(n, self.dir), 0.0);
  }
}

kind Sky {
  row { zenith: vec3<f32> }
  query sky(self, rd: Dir[World]) -> vec3<f32> {
    return mix(vec3(0.7, 0.8, 0.9), self.zenith, rd.y);
  }
}

let sun_sky = SunSky { dir: normalize(Dir[World](-0.4, 0.8, -0.3)), power: 1.1 };
let sky_color = Sky { zenith: vec3(0.35, 0.55, 0.85) };

schedule Main {
  pass rows: heron_scene();
  pass fish: run_laws(Fish -> swim);
  pass body: run_laws(Heron -> body_law);
  pass neck: run_laws(Heron -> neck_law);
  pass check: check(gaze_lock);
  pass check2: check(head_above_hips);
  pass camera: orbit(at: Pt[World](0.0, 0.6, 0.6), radius: 4.0m, fov: 40deg);
  pass cull: build_indirect(Water, Heron, Fish);
  pass vis: render_bundle(Vis64) { draw_indirect(Water, Heron, Fish); };
  pass classify: tile_bin_kinds(Vis64, tile: 8u);
  pass shade: dispatch_material_tiles(Vis64, light: sun_sky);
  pass present: tonemap_aces(ColorHdr, sky: sky_color);
}
```


================================================================
# HAIKU OUTPUT D
# file: agent_notes/2026-11-20_syntax_bakeoff/out_D_haiku.md
# what it is: the three tasks written by Haiku in syntax D
================================================================

# Task 1: a hermit crab

```python
space("World", "Body", "Leg", "Claw", "Face", "Shell", "Ground")

TAU = 6.2831853
PI = 3.14159265

# Shell spine: a spiral in metres, 2.5 turns, radius grows with u
def spine(u: float) -> Pt["Shell"]:
    a = TAU*2.5*u
    r = 0.05 + 0.15*u
    return Pt["Shell"](r*cos(a), r*sin(a), 0.3*u)

# Tube radius grows along the spiral
def tube_radius(u: float) -> float:
    return 0.03 + 0.07*u

@scan(over=(0.0, 1.0), steps=256, schedule="bake_lut")
class whorl:
    def init(u: float) -> Frame["Shell"]:
        return frame_from_tangent(spine(u), normalize(diff(spine, "u")(u)), Dir["Shell"](0, 1, 0))
    def step(prev: Frame["Shell"], u: float) -> Frame["Shell"]:
        return transport_frame(prev, spine(u), normalize(diff(spine, "u")(u)))

@kind
class Shell:
    parent: RowId["CrabBody"]
    pose: Xform["Shell", "World"]
    ribs: float

    @query(grid=(96, 24))
    def surface(self, u: float, v: float) -> Pt["Shell"]:
        f = sample_c2(whorl, u)
        t = TAU*v
        ribbing = 1.0 + 0.08*cos(TAU*self.ribs*u)
        r = tube_radius(u)*ribbing
        return f.origin + r*(cos(t)*f.normal + sin(t)*f.binormal)

@kind
class Claw:
    parent: RowId["CrabBody"]
    pose: Xform["Claw", "World"]
    opening: Rad          # per-instance hinge angle
    scale: float          # feeder claw is the same definition at half scale

    @query
    def trace(self, p: Pt["Claw"]) -> Field["Claw"]:
        s = self.scale
        palm = sd_round_box(p - Pt["Claw"](0.0, 0.0, 0.0), Vec["Claw"](0.10*s, 0.08*s, 0.06*s), 0.02*m*s)
        fixed = sd_capsule(p, Pt["Claw"](0.08*s, 0.0, 0.0), Pt["Claw"](0.20*s, 0.02*s, 0.0), 0.012*m*s)
        hinge = Pt["Claw"](0.08*s, -0.03*s, 0.0)
        moving = rot_z(self.opening)(p - hinge)
        teeth = 0.002*m*s*abs(sin(60.0*moving.x))
        finger = sd_round_box(moving, Vec["Claw"](0.14*s, 0.012*s, 0.02*s), 0.008*m*s) - teeth
        return smin_poly(smin_poly(palm, fixed, 0.02*m*s), finger, 0.02*m*s)

@kind
class CrabBody:
    pose: Xform["Body", "World"]
    com: Pt["World"]
    vel: Vec["World"]
    head: Pt["Body"]

    @query
    def material(self, p: Pt["Body"], n: Norm["Body"]) -> Bsdf:
        rough = 0.25 + 0.15*fbm2(vec2(p.x, p.z)*900.0)
        base = vec3(0.78, 0.26, 0.10)
        return ggx(base, rough, 0.0) + sss(vec3(0.9, 0.4, 0.2), 0.02) + clearcoat(0.8, 0.04)

@kind
class CrabLeg:
    parent: RowId["CrabBody"]
    pose: Xform["Leg", "World"]
    femur: Meters
    tibia: Meters
    foot: Pt["World"]

    @query
    def trace(self, p: Pt["Leg"]) -> Field["Leg"]:
        a = Pt["Leg"](0.0, 0.0, 0.0)
        knee = Pt["Leg"](self.femur, 0.0, 0.0)
        ankle = Pt["Leg"](self.femur + self.tibia*0.5, -self.tibia*0.8, 0.0)
        d1 = sd_capsule(p, a, knee, 0.07*m)
        d2 = sd_capsule(p, knee, ankle, 0.05*m)
        return smin_poly(d1, d2, 0.03*m)

@kind
class Eye:
    parent: RowId["CrabBody"]
    pose: Xform["Face", "World"]
    gaze: Pt["World"]
    clock: Sec

    @query
    def trace(self, p: Pt["Face"]) -> Field["Face"]:
        sym = mirror_x(p) | translate(-Vec["Face"](0.15, 0.72, 0.42))
        eye_c = sym.pt(0.03, 0.30, 0.07)
        globe = sd_sphere(sym.p - eye_c, 0.085*m)
        stalk = sd_capsule(p, Pt["Face"](0.15, 0.55, 0.42), sym.to_parent(eye_c), 0.012*m)
        return smin_poly(globe, stalk, 0.02*m)

    @query
    def material(self, p: Pt["Face"], n: Norm["Face"]) -> Bsdf:
        sym = mirror_x(p) | translate(-Vec["Face"](0.15, 0.72, 0.42))
        eye_c = sym.pt(0.03, 0.30, 0.07)
        eye_w = self.pose @ sym.to_parent(eye_c)
        look = sym.from_parent(inv(self.pose) @ normalize(self.gaze - eye_w))
        pupil = dot(normalize(sym.p - eye_c), look)
        albedo = vec3(0.02, 0.02, 0.02) if pupil > 0.9 else vec3(0.85, 0.80, 0.70)
        return ggx(albedo, 0.25, 0.0) + clearcoat(0.7, 0.05)

@kind
class Ground:
    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return p.y

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        wet = 0.5 + 0.5*fbm2(vec2(p.x, p.z)*6.0)
        return ggx(vec3(0.42, 0.36, 0.28)*(1.0 - 0.2*wet), 0.15 + 0.2*(1.0 - wet), 0.0)

@kind
class Sun:
    dir: Dir["World"]

# Socket transforms: leg sockets, claw mounts, shell and face mounts on the body
def leg_socket(j: int, side: float) -> Xform["Leg", "Body"]:
    return rot_y(side*(0.5 + 0.15*j)*rad)

CLAW_MOUNT: Xform["Claw", "Body"] = translate(Vec["Body"](0.18, 0.10, 0.25))
FEEDER_MOUNT: Xform["Claw", "Body"] = translate(Vec["Body"](-0.18, 0.08, 0.25))
SHELL_MOUNT: Xform["Shell", "Body"] = translate(Vec["Body"](0.0, 0.25, -0.20))
FACE_MOUNT: Xform["Face", "Body"] = translate(Vec["Body"](0.0, 0.10, 0.35))

@comptime
def crab(body_pose: Xform["Body", "World"]):
    b = emit(CrabBody(pose=body_pose, com=body_pose @ Pt["Body"](0.0, 0.0, 0.0),
                      vel=Vec["World"](0.0, 0.0, 0.0), head=Pt["Body"](0.0, 0.25, 0.35)))
    for j in range(6):
        for side in (-1.0, 1.0):
            emit(CrabLeg(parent=b, pose=body_pose @ leg_socket(j, side),
                         femur=0.40*m + 0.05*m*j, tibia=0.35*m + 0.03*m*j,
                         foot=body_pose @ Pt["Body"](side*0.3, 0.0, 0.3*j)))
    emit(Claw(parent=b, pose=body_pose @ CLAW_MOUNT, opening=0.5*rad, scale=1.0))
    emit(Claw(parent=b, pose=body_pose @ FEEDER_MOUNT, opening=0.3*rad, scale=0.5))
    emit(Shell(parent=b, pose=body_pose @ SHELL_MOUNT, ribs=9.0))
    emit(Eye(parent=b, pose=body_pose @ FACE_MOUNT, gaze=Pt["World"](0.0, 0.3, 1.0), clock=0.0*s))
    emit(Ground())
    emit(Sun(dir=Dir["World"](0.4, 0.8, 0.3)))

crab(translate(Vec["World"](0.0, 0.0, 0.0)) @ rot_y(0.0*rad))

@law(writes=CrabBody)
def body_law(b: CrabBody, dt: Sec):
    b.com = b.com + b.vel*dt

@law(writes=Eye)
def eye_track(e: Eye, dt: Sec):
    e.clock = e.clock + dt
    t = e.clock/s
    e.gaze = Pt["World"](0.3*sin(0.2*t), 0.25, 0.9 + 0.1*cos(0.13*t))

@law(writes=CrabLeg)
def crab_step(leg: CrabLeg, dt: Sec, bodies: Table["CrabBody"]):
    b = bodies[leg.parent]
    cp = b.com + b.vel * (0.28*s)
    if distance(leg.foot, cp) > 0.5*m:
        leg.foot = Pt["World"](cp.x, 0.0, cp.z)

@constraint
def upright(b: CrabBody) -> Verdict[Meters]:
    if not is_finite(b.pose):
        return Unknown("NonFinitePose")
    margin = (b.pose @ b.head).y - 0.15*m
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)

@schedule
def main():
    rows(CrabBody, CrabLeg, Claw, Shell, Eye, Ground, Sun)
    run_laws(body_law, crab_step, eye_track)
    check(upright)
    orbit(at=Pt["World"](0.0, 0.1, 0.0), radius=1.5*m, fov=40*deg)
    build_indirect(Ground, CrabLeg, Claw, Shell, Eye, CrabBody)
    with render_bundle(Vis64):
        draw_indirect(Ground, CrabLeg, Claw, Shell, Eye, CrabBody)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=Sun)
    tonemap_aces(ColorHdr, sky=sky_color)
```

# Task 2: a reed bed in wind

```python
space("World", "Reed")

TAU = 6.2831853
PAPER = vec3(0.96, 0.92, 0.82)
INK = vec3(0.14, 0.12, 0.10)
WATER_INK = vec3(0.80, 0.77, 0.70)

@lobe
def ink(dark: vec3, cut: float, n: vec3, v: vec3, l: vec3) -> vec4:
    return vec4(dark if dot(n, l) < cut else PAPER, 0.0)

@kind
class Reed:
    root: Pt["World"]
    height: Meters
    lean: Rad
    stiffness: float
    phase: float
    bend: Rad          # written by the wind law
    clock: Sec

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        a1 = self.lean*0.3 + self.bend*0.2
        a2 = self.lean*0.7 + self.bend*0.8
        h = self.height
        mid = self.root + Vec["World"](0.5*h*sin(a1), 0.5*h*cos(a1), 0.0*m)
        tip = mid + Vec["World"](0.5*h*sin(a2), 0.5*h*cos(a2), 0.0*m)
        stalk_low = sd_capsule(p, self.root, mid, 0.010*m)
        stalk_high = sd_capsule(p, mid, tip, 0.006*m)
        seed = sd_sphere(p - (tip + Vec["World"](0.0, 0.03*m, 0.0)), 0.012*m)
        return smin_poly(smin_poly(stalk_low, stalk_high, 0.004*m), seed, 0.006*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ink(INK, 0.0)

@kind
class Water:
    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return p.y

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ink(WATER_INK, 0.0)

@kind
class Sky:
    @query
    def sky(self, d: Dir["World"]) -> vec3:
        return PAPER*(1.0 - 0.1*abs(fbm2(vec2(d.x, d.z)*3.0)))

@kind
class Sun:
    dir: Dir["World"]

@comptime
def bed(n: int):
    rng = random.Random(42)
    for i in range(n):
        x = rng.gauss(0.0, 1.2)
        z = rng.gauss(0.0, 0.8)
        emit(Reed(root=Pt["World"](x*m, -0.1*m, z*m),
                  height=(0.9 + 0.5*rng.random())*m,
                  lean=(rng.random() - 0.5)*0.3*rad,
                  stiffness=0.5 + rng.random(),
                  phase=rng.random()*TAU,
                  bend=0.0*rad,
                  clock=0.0*s))

bed(200)
emit(Water())
emit(Sky())
emit(Sun(dir=Dir["World"](0.5, 0.8, 0.3)))

@law(writes=Reed)
def wind(r: Reed, dt: Sec):
    r.clock = r.clock + dt
    t = r.clock/s
    gust = 0.5 + 0.5*sin(0.9*r.root.x/m - 1.4*t)
    flutter = 0.15*sin(3.0*t + r.phase)
    r.bend = (0.9*gust + flutter)*rad / (1.0 + r.stiffness)

@schedule
def main():
    rows(Reed, Water, Sky, Sun)
    run_laws(wind)
    orbit(at=Pt["World"](0.0, 0.4, 0.0), radius=5*m, fov=35*deg)
    build_indirect(Water, Reed)
    with render_bundle(Vis64):
        draw_indirect(Water, Reed)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=Sun)
    tonemap_aces(ColorHdr, sky=Sky)
```

# Task 3: a heron's hop

```python
space("World", "Body", "Head", "Fish", "Ground")

PI = 3.14159265
LIMIT = 55*deg          # neck limit from the trunk heading
TURN_RATE = 1.5*rad/s   # trunk turn rate during the hop
HOP_T = 0.5*s
NECK_BASE = Pt["Body"](0.0, 0.92*m, 0.1*m)

@kind
class Fish:
    pos: Pt["World"]
    clock: Sec

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.7, 0.75, 0.8), 0.3, 0.0)

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return sd_sphere(p - self.pos, 0.06*m)

@kind
class Heron:
    origin: Vec["World"]
    trunk: Xform["Body", "World"]
    heading: Rad
    hop: Meters             # height of the hop, 0 on the ground
    hop_t: Sec              # time into the current hop
    neck_yaw: Rad           # head yaw relative to the trunk heading
    leftover: Rad           # gaze miss angle past the limit, 0 when held
    margin: Rad             # room left inside the limit, when held
    target: RowId["Fish"]

    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        q = inv(self.trunk) @ p
        trunk = sd_capsule(q, Pt["Body"](0.0, 0.6*m, 0.0), Pt["Body"](0.0, 0.9*m, 0.0), 0.12*m)
        leg_l = sd_capsule(q, Pt["Body"](0.08*m, 0.6*m, 0.0), Pt["Body"](0.08*m, 0.0, 0.05*m), 0.01*m)
        leg_r = sd_capsule(q, Pt["Body"](-0.08*m, 0.6*m, 0.0), Pt["Body"](-0.08*m, 0.0, 0.05*m), 0.01*m)
        # S-shaped neck: shoulder, bend, head
        mid = Pt["Body"](0.06*m*sin(self.neck_yaw*0.5), 1.05*m, 0.12*m)
        head_c = Pt["Body"](0.25*m*sin(self.neck_yaw), 1.15*m, 0.2*m*cos(self.neck_yaw) + 0.1*m)
        neck_lo = sd_capsule(q, NECK_BASE, mid, 0.03*m)
        neck_hi = sd_capsule(q, mid, head_c, 0.02*m)
        head = sd_sphere(q - head_c, 0.05*m)
        beak_tip = head_c + Vec["Body"](0.2*m*sin(self.neck_yaw), -0.02*m, 0.2*m*cos(self.neck_yaw))
        beak = sd_capsule(q, head_c, beak_tip, 0.012*m)
        body = smin_poly(trunk, smin_poly(leg_l, leg_r, 0.02*m), 0.05*m)
        neck = smin_poly(neck_lo, neck_hi, 0.02*m)
        return smin_poly(smin_poly(body, neck, 0.03*m), smin_poly(head, beak, 0.01*m), 0.02*m)

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.55, 0.58, 0.60), 0.45, 0.0)

@kind
class Ground:
    @query
    def trace(self, p: Pt["World"]) -> Field["World"]:
        return p.y

    @query
    def material(self, p: Pt["World"], n: Norm["World"]) -> Bsdf:
        return ggx(vec3(0.42, 0.36, 0.28), 0.35, 0.0)

@kind
class Sun:
    dir: Dir["World"]

@comptime
def spawn():
    f = emit(Fish(pos=Pt["World"](2.0, 0.2, 3.0), clock=0.0*s))
    emit(Heron(origin=Vec["World"](0.0, 0.0, 0.0),
               trunk=rot_y(0.0*rad),
               heading=0.0*rad, hop=0.0*m, hop_t=0.0*s,
               neck_yaw=0.0*rad, leftover=0.0*rad, margin=LIMIT,
               target=f))
    emit(Ground())
    emit(Sun(dir=Dir["World"](0.4, 0.8, 0.3)))

spawn()

@law(writes=Fish)
def fish_swim(f: Fish, dt: Sec):
    f.clock = f.clock + dt
    t = f.clock/s
    f.pos = Pt["World"](2.0*cos(0.3*t), 0.2, 3.0 + 1.0*sin(0.2*t))

# The body goes first: it hops and turns while the neck is at its limit
@law(writes=Heron)
def body_hop(h: Heron, dt: Sec, fish: Table["Fish"]):
    f = fish[h.target]
    stuck = h.leftover > 0*rad
    if stuck:
        h.hop_t = h.hop_t + dt
        if h.hop_t > HOP_T:
            h.hop_t = 0*s
        h.hop = 0.12*m*sin(PI*(h.hop_t/HOP_T))
    else:
        h.hop_t = 0*s
        h.hop = 0*m
    if h.hop > 0.005*m:
        aim = atan2((inv(h.trunk) @ f.pos - Pt["Body"](0.0, 0.0, 0.0)).x,
                    (inv(h.trunk) @ f.pos - Pt["Body"](0.0, 0.0, 0.0)).z)*rad
        step = TURN_RATE*dt
        if aim > step:
            h.heading = h.heading + step
        elif aim < -step:
            h.heading = h.heading - step
        else:
            h.heading = h.heading + aim
    h.trunk = translate(h.origin + Vec["World"](0.0, h.hop, 0.0)) @ rot_y(h.heading)

# Then the neck and head: the head aims at the fish in world space, clamped to the limit
@law(writes=Heron)
def neck_head(h: Heron, dt: Sec, fish: Table["Fish"]):
    f = fish[h.target]
    base = h.trunk @ NECK_BASE
    d = f.pos - base
    want = atan2(d.x, d.z)*rad - h.heading
    if want > LIMIT:
        h.neck_yaw = LIMIT
        h.leftover = want - LIMIT
        h.margin = 0*rad
    elif want < -LIMIT:
        h.neck_yaw = -LIMIT
        h.leftover = -want - LIMIT
        h.margin = 0*rad
    else:
        h.neck_yaw = want
        h.leftover = 0*rad
        h.margin = LIMIT - abs(want)

@constraint
def gaze_held(h: Heron) -> Verdict[Rad]:
    if h.leftover > 0*rad:
        return Missed(h.leftover)
    return Held(h.margin)

@constraint
def head_above_hips(h: Heron) -> Verdict[Meters]:
    head_p = h.trunk @ Pt["Body"](0.25*m*sin(h.neck_yaw), 1.15*m, 0.2*m*cos(h.neck_yaw) + 0.1*m)
    hips = h.trunk @ Pt["Body"](0.0, 0.6*m, 0.0)
    margin = head_p.y - hips.y
    if margin < 0*m:
        return Missed(-margin)
    return Held(margin)

@schedule
def main():
    rows(Fish, Heron, Ground, Sun)
    run_laws(fish_swim)
    run_laws(body_hop, neck_head)
    check(gaze_held)
    check(head_above_hips)
    orbit(at=Pt["World"](0.0, 0.6, 0.0), radius=3.0*m, fov=40*deg)
    build_indirect(Ground, Heron, Fish)
    with render_bundle(Vis64):
        draw_indirect(Ground, Heron, Fish)
    tile_bin_kinds(Vis64, tile=8)
    dispatch_material_tiles(Vis64, light=Sun)
    tonemap_aces(ColorHdr, sky=sky_color)
```


================================================================
# HAIKU OUTPUT E
# file: agent_notes/2026-11-20_syntax_bakeoff/out_E_haiku.md
# what it is: the three tasks written by Haiku in syntax E
================================================================

## Task 1: a hermit crab

```
space World Body Leg Claw Shell Eye

sun = dir([0.35, 1.0, 0.25]@World)
chitin = [0.72, 0.22, 0.08]
sss_tint = [0.9, 0.3, 0.1]
sand = [0.42, 0.36, 0.28]

# ---- shell: spiral spine, transported frame, ribbed tube ----
fn spine(u: f) -> p3@Shell {
  a = 6.2831853rad * 3.0 * u
  r = 0.05m * (1.0 - 0.4 * u)
  [r * cos(a), 0.12m * u, r * sin(a)]
}

fn tube_r(u: f) -> m = 0.004m + 0.012m * u

fn rib(u: f) -> m = 0.0009m * sin(6.2831853rad * 14.0 * u)

scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0,1,0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}

kind Shell @Shell (pose: xf@Shell>World) {
  surface(u, v) grid 64x32 = whorl(u).o + (tube_r(u) + rib(u)) * (cos(v) * whorl(u).n + sin(v) * whorl(u).b)
  material(p, n) = ggx(chitin, 0.3, 0) + sss(sss_tint, 0.02m) + coat(0.7, 0.05)
}

# ---- claw: rounded palm, fixed finger, hinged serrated finger ----
fn palm(p: p3@Claw, size: m) -> m = round_box(p, [0.5 * size, 0.4 * size, 0.3 * size], 0.1 * size)

fn fixed_finger(p: p3@Claw, size: m) -> m =
  capsule(p, [0.2 * size, 0.3 * size, 0.0m], [1.1 * size, 0.1 * size, 0.0m], 0.08 * size)

fn moving_finger(p: p3@Claw, size: m, open: rad) -> m {
  q = p |> move([-0.2 * size, 0.15 * size, 0.0m]) |> rot_z(-open)
  teeth = 0.03 * size * max(0.0, sin(90.0 * q.x / size))
  capsule(q, [0.0m, 0.0m, 0.0m], [1.0 * size, 0.1 * size, 0.0m], 0.07 * size) - teeth
}

fn chitin_rough(p: p3@Claw) -> f = 0.22 + 0.15 * fbm2(p * 180.0)

kind Claw @Claw (pose: xf@Claw>World, open: rad, size: m) {
  trace(p) = min(smin(palm(p, size), fixed_finger(p, size), 0.02m), moving_finger(p, size, open))
  material(p, n) = ggx(chitin, chitin_rough(p), 0) + sss(sss_tint, 0.02m) + coat(0.7, 0.04)
}

# ---- legs: one kind, twelve rows ----
kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World) {
  trace(p) = min(capsule(p, [0,0,0], [femur,0,0], 0.07m), capsule(p, [femur,0,0], [femur,-tibia,0], 0.04m))
  material(p, n) = ggx(chitin, 0.3, 0) + coat(0.7, 0.05)
}

# ---- body ----
kind CrabBody @Body (pose: xf@Body>World, head: p3@Body, vel: v3@World, com: p3@World) {
  trace(p) = round_box(p, [0.22m, 0.08m, 0.16m], 0.06m)
  material(p, n) = ggx(chitin, 0.3, 0) + coat(0.7, 0.05)
}

# ---- eyes: one mirrored definition ----
kind Eye @Eye (pose: xf@Eye>World) {
  trace(q) = {
    sym = q |> mirror_x |> move([-0.15,-0.72,-0.42])
    eye_c = [0.03,0.30,0.07]@sym
    globe = sphere(sym.p - eye_c, 0.085m)
    stalk = capsule(sym.p, [0.0m,0.0m,0.0m]@sym, [0.0m,0.1m,0.0m]@sym, 0.02m)
    min(globe, stalk)
  }
  material(q, n) = {
    sym = q |> mirror_x |> move([-0.15,-0.72,-0.42])
    eye_c = [0.03,0.30,0.07]@sym
    eye_w = pose * sym.out(eye_c)
    gaze = [0.3m * sin(0.2 * time), 0.4m, 1.0m]@World
    look = sym.in(inv(pose) * dir(gaze - eye_w))
    pupil = dot(normalize(sym.p - eye_c), look) > 0.93
    ink = if pupil { [0.02,0.02,0.02] } else { [0.9,0.85,0.8] }
    ggx(ink, 0.15, 0) + coat(0.8, 0.02)
  }
}

# ---- sand and sky ----
kind Sand @World (level: m) {
  trace(p) = p.y - level
  material(p, n) = ggx(sand, 0.12, 0) + coat(0.5, 0.02)
}

light(p, n) = [1.0, 0.94, 0.85] * max(0.0, dot(n, sun))
sky(rd) = [0.78, 0.84, 0.92]

# ---- laws and checks ----
law crab_step(leg: CrabLeg, dt, bodies: [CrabBody]) {
  b = bodies[leg.parent]
  cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m { leg.foot = [cp.x,0,cp.z] }
}

check upright(b: CrabBody) {
  if !finite(b.pose) { return unknown(NonFinitePose) }
  margin = (b.pose * b.head).y - 0.15m
  if margin < 0m { missed(-margin) } else { held(margin) }
}

# ---- build ----
fn socket(j: f, side: f) -> xf@Leg>Body = frame([0.12m * side, 0.05m, 0.12m * j - 0.3m]@Body, [side, 0, 0]@Body, [0,1,0]@Body)

fn shell_socket() -> xf@Shell>Body = frame([0, 0.06m, -0.05m]@Body, [0,0,1]@Body, [0,1,0]@Body)

fn claw_socket(side: f) -> xf@Claw>Body = frame([0.25m * side, 0.05m, 0.4m]@Body, [0,0,1]@Body, [0,1,0]@Body)

fn eye_socket(side: f) -> xf@Eye>Body = frame([0.06m * side, 0.1m, 0.2m]@Body, [0,0,1]@Body, [0,1,0]@Body)

fn crab(b, body: xf@Body>World) {
  for j in 0..6, side in [-1, 1] {
    CrabLeg(parent: b, pose: body * socket(j, side), femur: 0.40m + 0.05m * j, tibia: 0.30m - 0.02m * j, foot: body * [side, 0, 0.3 * j]@Body)
  }
  Shell(pose: body * shell_socket())
  Claw(pose: body * claw_socket(1), open: 0.6rad, size: 0.5m)
  Claw(pose: body * claw_socket(-1), open: 0.4rad, size: 0.25m)
  for side in [-1, 1] {
    Eye(pose: body * eye_socket(side))
  }
}

Sand(level: 0m)
at = place([0,0.4,0]@World)
crab(CrabBody(pose: at, head: [0,0.1m,0.15m]@Body, vel: [0,0,0]@World, com: [0,0.4,0]@World), at)

# ---- schedule ----
schedule Main {
  run_laws(crab_step)
  check(upright)
  orbit([0,0.25,0]@World, 0.9m, 30deg)
  build_indirect(Sand, CrabLeg, CrabBody, Shell, Claw, Eye)
  render_bundle(Vis64) { draw_indirect(Sand, CrabLeg, CrabBody, Shell, Claw, Eye) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sky)
  tonemap_aces(ColorHdr, sky)
}
```

## Task 2: a reed bed in wind

```
space World

paper = [0.95, 0.91, 0.82]
ink_dark = [0.12, 0.10, 0.09]
ink_faint = [0.78, 0.80, 0.86]
sun = dir([0.4, 1.0, 0.3]@World)

lobe flat_ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { paper }

# ---- one reed's centreline at parameter t in 0..1 ----
fn reed_point(base: p3@World, height: m, lean: rad, stiff: f, phase: rad, t: f, now: s) -> p3@World {
  sway = 0.35rad * sin(0.9/m * base.x - 2.2/s * now + phase) / (1.0 + 3.0 * stiff)
  dx = height * (t * sin(lean) + t * t * sin(sway))
  dy = height * t * cos(lean)
  base + [dx, dy, 0.0m]
}

kind Reed @World (base: p3@World, height: m, lean: rad, stiff: f, phase: rad) {
  trace(p) = {
    a = reed_point(base, height, lean, stiff, phase, 0.0, time)
    b = reed_point(base, height, lean, stiff, phase, 0.5, time)
    c = reed_point(base, height, lean, stiff, phase, 1.0, time)
    min(min(capsule(p, a, b, 0.008m), capsule(p, b, c, 0.005m)), capsule(p, c - [0,0.05m,0], c, 0.009m))
  }
  material(p, n) = flat_ink(ink_dark, 0.15, n, dir([0,0,1]@World), sun)
}

kind Water @World (level: m) {
  trace(p) = p.y - level
  material(p, n) = flat_ink(ink_faint, -0.2, n, dir([0,0,1]@World), sun)
}

sky(rd) = paper * 0.97 + ink_faint * 0.03

# ---- build: 200 reeds in a loose cluster ----
fn jit(i: f, k: f) -> f = fract(sin(i * 12.9898 + k * 78.233) * 43758.5453)

Water(level: 0m)

for i in 0..200 {
  r = 1.0m * sqrt(jit(i, 2))
  th = 6.2831853rad * jit(i, 3)
  Reed(
    base: [r * cos(th), -0.05m, r * sin(th)]@World,
    height: 0.6m + 0.4m * jit(i, 4),
    lean: 0.5rad * jit(i, 5) - 0.25rad,
    stiff: jit(i, 6),
    phase: 6.2831853rad * jit(i, 7)
  )
}

# ---- schedule ----
schedule Main {
  orbit([0,0.5,0]@World, 3m, 20deg)
  build_indirect(Water, Reed)
  render_bundle(Vis64) { draw_indirect(Water, Reed) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sky)
  tonemap_aces(ColorHdr, sky)
}
```

## Task 3: a heron's hop

```
space World Body Head

sun = dir([0.4, 1.0, 0.3]@World)
neck_top = [0.0m, 0.95m, 0.10m]@Body
neck_limit = 0.9rad

# ---- fish: a curve in world space, its position is the gaze target ----
fn fish_path(t: s) -> p3@World =
  [1.2m * cos(0.3rad/s * t), -0.1m, 1.2m + 0.4m * sin(0.5rad/s * t)]

kind Fish @World (pos: p3@World, t: s) {
  trace(p) = sphere(p - pos, 0.06m)
  material(p, n) = ggx([0.6, 0.7, 0.8], 0.2, 0) + coat(0.5, 0.03)
}

law fish_swim(f: Fish, dt) {
  f.t = f.t + dt
  f.pos = fish_path(f.t)
}

# ---- how far the head is inside its neck limit (rad, negative when past it) ----
fn neck_room(pose: xf@Body>World, fish: p3@World) -> rad {
  head_at = pose * neck_top
  trunk = pose * dir([0,0,1]@Body)
  want = dir(fish - head_at)
  neck_limit - acos(clamp(dot(want, trunk), -1.0, 1.0))
}

# ---- body: turns and hops while the neck is at its limit ----
kind Heron @Body (anchor: p3@World, yaw: rad, air: m, phase: f, pose: xf@Body>World) {
  trace(p) = {
    trunk = capsule(p, [0,0.35m,0]@Body, [0,0.6m,0]@Body, 0.12m)
    leg_l = capsule(p, [0.08m,0.35m,0]@Body, [0.08m,0.0m,0]@Body, 0.012m)
    leg_r = capsule(p, [-0.08m,0.35m,0]@Body, [-0.08m,0.0m,0]@Body, 0.012m)
    n1 = capsule(p, [0,0.6m,0]@Body, [0.0m,0.72m,-0.06m]@Body, 0.03m)
    n2 = capsule(p, [0.0m,0.72m,-0.06m]@Body, [0.0m,0.84m,0.06m]@Body, 0.025m)
    n3 = capsule(p, [0.0m,0.84m,0.06m]@Body, neck_top, 0.02m)
    min(min(min(trunk, leg_l), min(leg_r, n1)), min(n2, n3))
  }
  material(p, n) = ggx([0.55,0.58,0.6], 0.4, 0) + coat(0.3, 0.1)
}

law heron_body(b: Heron, dt, fish: Fish) {
  stuck = neck_room(b.pose, fish.pos) < 0rad
  b.phase = if stuck { fract(b.phase + dt * 0.8/s) } else { 0.0 }
  b.air = 0.05m * sin(3.14159rad * b.phase)
  b.yaw = if stuck { b.yaw + 0.6rad/s * dt } else { b.yaw }
  b.pose = frame(b.anchor + [0, b.air, 0]@World, dir([sin(b.yaw), 0, cos(b.yaw)]@World), [0,1,0]@World)
}

# ---- neck and head: aim at the fish, clamped to the neck limit ----
kind Head @Head (parent: Heron, pose: xf@Head>World, aim: d3@World, room: rad) {
  trace(p) = min(sphere(p, 0.05m), capsule(p, [0,0,0.02m], [0,0,0.22m], 0.012m))
  material(p, n) = ggx([0.55,0.58,0.6], 0.4, 0) + coat(0.3, 0.1)
}

law head_aim(h: Head, dt, bodies: [Heron], fish: Fish) {
  b = bodies[h.parent]
  head_at = b.pose * neck_top
  trunk = b.pose * dir([0,0,1]@Body)
  want = dir(fish.pos - head_at)
  room = neck_room(b.pose, fish.pos)
  perp = dir(want - trunk * dot(want, trunk))
  limit_dir = dir(trunk * cos(neck_limit) + perp * sin(neck_limit))
  h.room = room
  h.aim = if room < 0rad { limit_dir } else { want }
  h.pose = frame(head_at, h.aim, [0,1,0]@World)
}

# ---- checks ----
check gaze_held(h: Head) {
  if !finite(h.pose) { return unknown(NonFinitePose) }
  if h.room < 0rad { missed(-h.room) } else { held(h.room) }
}

check above_hips(h: Head, bodies: [Heron]) {
  b = bodies[h.parent]
  hip = (b.pose * [0.0m, 0.45m, 0.0m]@Body).y
  margin = (b.pose * neck_top).y - hip
  if margin < 0m { missed(-margin) } else { held(margin) }
}

# ---- build ----
at = [0,0,0]@World
h = Heron(anchor: at, yaw: 0rad, air: 0m, phase: 0.0, pose: frame(at, [0,0,1]@World, [0,1,0]@World))
Head(parent: h, pose: frame(at + neck_top, [0,0,1]@World, [0,1,0]@World), aim: [0,0,1]@World, room: 0rad)
Fish(pos: fish_path(0s), t: 0s)

# ---- schedule: body, then neck and head, then checks ----
schedule Main {
  run_laws(fish_swim, heron_body, head_aim)
  check(gaze_held)
  check(above_hips)
  orbit([0,0.5,0]@World, 3m, 35deg)
  build_indirect(Heron, Head, Fish)
  render_bundle(Vis64) { draw_indirect(Heron, Head, Fish) }
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sky)
  tonemap_aces(ColorHdr, sky)
}
```


================================================================
# HAIKU OUTPUT F
# file: agent_notes/2026-11-20_syntax_bakeoff/out_F_haiku.md
# what it is: the three tasks written by Haiku in syntax F
================================================================

## Task 1: hermit crab

```
space World Body Leg Claw Face Shell

-- Shell spine: a coiled spiral, metres
fn spine(u) = {0.14m * u * cos(22.0 * u), 0.10m * u, 0.14m * u * sin(22.0 * u)}@Shell

-- Rib modulation on the tube radius, v in 0..6.2832
fn rib(u, v) = 0.004m * (0.5 + 0.5 * cos(9.0 * v + 40.0 * u))

-- Frames transported along the spine so they do not twist
scan whorl u in 0..1 by 256 on bake_lut
  init = frame(spine(u), dir(spine'(u)), {0, 1, 0}@Shell)
  step prev = transport(prev, spine(u), dir(spine'(u)))
end

-- Shell as a ribbed tube whose radius grows with the spiral
surface(u, v) grid 64x32
  local fr = whorl(u)
  local r = 0.012m + 0.02m * u + rib(u, v)
  fr.o + r * (cos(v) * fr.n + sin(v) * fr.b)
end

-- Rounded palm, fixed finger, hinged moving finger with serrations
fn palm(p, s) = round_box(p, {s, s * 0.6, s * 0.5}, 0.2m * s)
fn fixed_finger(p, s) = round_box(p - {0, 0, s * 0.9}, {s * 0.25, s * 0.2, s * 0.8}, 0.1m * s)
fn teeth(p, s) = box(p - {0, 0, s * 1.4}, {s * 0.3, s * 0.1, s * 0.5})
fn moving_finger(p, s, open) = ssub(teeth(p, s), round_box(p |> move(-{0, 0, s * 0.9}) |> rot_z(open), {s * 0.25, s * 0.2, s * 0.8}, 0.1m * s))

kind Claw @Claw (open: rad, size: m, pose: xf@Claw>World)
  trace(p) = smin(palm(p, size), min(fixed_finger(p, size), moving_finger(p, size, open)))
end

kind CrabBody @Body (pose: xf@Body>World, head: p3@Body, com: p3@World, vel: v3@World)
  trace(p) = round_box(p, {0.12m, 0.04m, 0.10m}, 0.03m)
end

kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World)
  trace(p) = smin(capsule(p, {0, 0, 0}, {femur, 0, 0}, 0.07m), capsule(p, {femur, 0, 0}, {femur + tibia * 0.5, -tibia, 0}, 0.05m))
end

-- Eyes on stalks: one definition, mirrored by side
kind Eye @Face (side: f, pose: xf@Face>World, look: v3@Face)
  trace(p) = capsule(p, {0, 0, 0}, {0, 0.30m, 0}, 0.02m)
end

fn socket(j, side) = move({0.10m * side, 0, 0.04m * (j - 2.5)}@Body)

fn crab(b, body)
  for j in 0..6, side in {-1, 1}
    CrabLeg{parent = b, pose = body * socket(j, side), femur = 0.40m + 0.05m * j,
            tibia = 0.30m - 0.02m * j, foot = body * {side, 0, 0.3 * j}@Body}
  end
end

-- Gaze target that drifts slowly in world space
fn gaze_at(t) = {0.3m * sin(0.2 * t), 0.2m, 1.2m}@World

law crab_step(leg: CrabLeg, dt, bodies: [CrabBody])
  local b = bodies[leg.parent]
  local cp = b.com + b.vel * 0.28s
  if dist(leg.foot, cp) > 0.5m then leg.foot = {cp.x, 0, cp.z} end
end

law aim_eyes(e: Eye, dt)
  local sym = e.pose |> mirror_x
  local eye_c = {0.03m, 0.30m, 0.07m}@sym
  local eye_w = e.pose * sym.out(eye_c)
  e.look = sym.into(inv(e.pose) * dir(gaze_at(time) - eye_w))
end

-- Layered wet chitin: red-orange base, subsurface, clear wet coat
material(p, n) = ggx({0.75, 0.22, 0.08}, 0.35 + 0.1 * fbm2(p.xz * 40), 0) + sss({0.9, 0.3, 0.1}, 0.08) + coat(0.7, 0.07)

-- Wet sand: a plain plane; noise lives in the material, not the distance field
fn ground(p) = p.y

-- Build: one crab, its rows, its eyes
local at = place({0, 0.4, 0}@World)
local body = CrabBody{pose = at, head = {0, 0.04, 0.10}@Body, com = {0, 0.4, 0}@World, vel = {0, 0, 0}@World}
crab(body, at)
Eye{side = 1, pose = at, look = {0, 0, 1}@Face}
Eye{side = -1, pose = at, look = {0, 0, 1}@Face}

-- Shell and claws as rows
Claw{open = 0.5rad, size = 0.09m, pose = at}
Claw{open = 0.3rad, size = 0.05m, pose = at}

schedule Main
  run_laws(crab_step)
  run_laws(aim_eyes)
  orbit({0, 0.4, 0}@World, 6m, 40deg)
  build_indirect(World, CrabLeg, Claw, Eye)
  render_bundle(Vis64) do
    draw_indirect(World, CrabLeg, Claw, Eye)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
end
```

## Task 2: reed bed in wind

```
space World

kind Reed @World (root: p3@World, height: m, taper: f, lean: rad, stiff: f, phase: f, bend: rad)
  trace(p) = smin(seg(p, 0.0, 1.0 / 3.0), smin(seg(p, 1.0 / 3.0, 2.0 / 3.0), seg(p, 2.0 / 3.0, 1.0)))
end

-- One stalk segment, from fraction a to b of the height; radius tapers toward the tip
fn seg(p, a, b) = capsule(p, pt(a), pt(b), 0.012m * (1.0 - 0.5 * a))

-- Point on the stalk at fraction s: lean at base, bend grows toward the tip
fn pt(s) = root_at + {0, height * s, 0} + {height * lean_at(s), 0, 0}

-- Build-time pseudo-random, deterministic per (i, k)
fn rnd(i, k) = fract(sin(i * 12.9898 + k * 78.233) * 43758.5453)

-- Two hundred reeds in a loose irregular cluster
for i in 0..200
  local ang = 6.2832 * rnd(i, 1)
  local rad = 3.0m * sqrt(rnd(i, 2))
  Reed{root = {rad * cos(ang), 0, rad * sin(ang)}@World,
       height = 0.8m + 0.5m * rnd(i, 3),
       taper = 0.6 + 0.3 * rnd(i, 4),
       lean = 0.05rad * (rnd(i, 5) - 0.5),
       stiff = rnd(i, 6),
       phase = 6.2832 * rnd(i, 7),
       bend = 0rad}
end

-- Wind: a gust travels across the bed; the bend grows toward the tip; stiff reeds bend less
law sway(r: Reed, dt)
  local gust = sin(1.5 * time - 0.6 * r.root.x + r.phase)
  local amp = 0.35rad * (0.6 + 0.4 * gust) / (1.0 + 3.0 * r.stiff)
  r.bend = amp
end

-- Look: two flat inks on warm paper, hard shadow edge
material(p, n) = lobe ink(paper_dark, 0.0, n, v, l)
lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut then dark else {0.96, 0.92, 0.82} end

-- Water and sky: plain paper with a faint second ink
fn water(p) = p.y
material_water(p, n) = {0.96, 0.92, 0.82} + 0.03 * {0.2, 0.2, 0.3}

schedule Main
  run_laws(sway)
  build_indirect(World, Reed)
  render_bundle(Vis64) do
    draw_indirect(World, Reed)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, paper_ink)
  tonemap_aces(ColorHdr, sky_color)
end
```

## Task 3: heron's hop

```
space World Body Neck Head Leg

-- Fish: a row whose position is the gaze target, moving on a curve in world space
kind Fish @World (pos: p3@World)
  trace(p) = sphere(p, 0.05m)
end

fn fish_path(t) = {0.8m * cos(0.3 * t), 0.15m, 0.6m * sin(0.3 * t)}@World

-- Trunk: the body, with its heading and hip height
kind Heron @Body (pose: xf@Body>World, heading: rad, hips: p3@Body, head: K, hop: f, hopping: bool)
  trace(p) = round_box(p, {0.25m, 0.10m, 0.18m}, 0.06m)
end

kind HeronLeg @Leg (parent: Heron, pose: xf@Leg>World, femur: m, tibia: m, foot: p3@World)
  trace(p) = smin(capsule(p, {0, 0, 0}, {0, -femur, 0}, 0.02m), capsule(p, {0, -femur, 0}, {0, -femur - tibia, 0}, 0.015m))
end

-- Neck: an S-shaped chain, its curve in neck space
fn neck_pt(s) = {0.10m * sin(3.0 * s), 0.35m * s, 0.02m * (1.0 - s)}@Neck

kind Neck @Neck (parent: Heron, pose: xf@Neck>World)
  trace(p) = smin(capsule(p, neck_pt(0.0), neck_pt(0.5), 0.03m), capsule(p, neck_pt(0.5), neck_pt(1.0), 0.025m))
end

-- Head with eye, forward axis, and the neck's yaw limit
kind Head @Head (parent: Heron, pose: xf@Head>World, eye: p3@Head, fwd: d3@Head, yaw: rad, limit: rad, leftover: rad)
  trace(p) = smin(sphere(p, 0.06m), capsule(p, {0, 0, 0.04m}, {0, 0, 0.22m}, 0.012m))
end

-- Build: one heron, its legs, neck and head
local hd = Head{parent = K0, pose = place({0, 0.6m, 0}@World), eye = {0, 0.02m, 0.03m}@Head,
                fwd = {0, 0, 1}@Head, yaw = 0rad, limit = 1.1rad, leftover = 0rad}
local hr = Heron{pose = place({0, 0.6m, 0}@World), heading = 0rad, hips = {0, 0, 0}@Body,
                 head = hd, hop = 0.0, hopping = false}
HeronLeg{parent = hr, pose = place({0.08m, 0.6m, 0}@World), femur = 0.35m, tibia = 0.30m, foot = {0.08m, 0, 0}@World}
HeronLeg{parent = hr, pose = place({-0.08m, 0.6m, 0}@World), femur = 0.35m, tibia = 0.30m, foot = {-0.08m, 0, 0}@World}
Neck{parent = hr, pose = place({0, 0.6m, 0}@World)}
Fish{pos = fish_path(0)}

-- Law 1, the body: while the neck is at its limit, hop and turn the trunk toward the fish
law body_hop(b: Heron, dt, heads: [Head], fish: [Fish])
  local h = heads[b.head]
  local target = fish[0].pos
  if h.leftover > 0rad and not b.hopping then
    b.hopping = true
    b.hop = 0.0
  end
  if b.hopping then
    b.hop = b.hop + dt / 0.6s
    b.heading = b.heading + (dir(target - b.pose.pos).x) * 0.2 * dt / 0.6s
    if b.hop >= 1.0 then b.hopping = false end
  end
end

-- Law 2, the neck and head: the gaze lock holds each tick in world space; the head stops at its limit
law head_gaze(h: Head, dt, fish: [Fish], bodies: [Heron])
  local b = bodies[h.parent]
  local eye_w = h.pose * h.eye
  local aim = inv(h.pose) * dir(fish[0].pos - eye_w)
  local rel = dot(aim, h.fwd)
  local angle = acos(clamp(rel, -1.0, 1.0))
  local over = max(angle - h.limit, 0rad)
  h.leftover = over
  h.yaw = min(angle, h.limit)
end

-- Check 1: gaze is missed by the leftover angle when beyond the limit, else held
check gaze(h: Head)
  if h.leftover > 0rad then missed(h.leftover) else held(h.limit - h.yaw) end
end

-- Check 2: the head stays above the hips throughout
check head_above_hips(b: Heron, heads: [Head])
  local hd = heads[b.head]
  local margin = (hd.pose * hd.eye).y - (b.pose * b.hips).y
  if margin < 0 then missed(-margin) else held(margin) end
end

schedule Main
  run_laws(body_hop)
  run_laws(head_gaze)
  check(gaze)
  check(head_above_hips)
  orbit({0, 0.4, 0}@World, 3m, 30deg)
  build_indirect(World, Heron, HeronLeg, Neck, Head)
  render_bundle(Vis64) do
    draw_indirect(World, Heron, HeronLeg, Neck, Head)
  end
  tile_bin_kinds(Vis64, 8)
  dispatch_material_tiles(Vis64, sun_sky)
  tonemap_aces(ColorHdr, sky_color)
end
```


================================================================
# HAIKU OUTPUT G
# file: agent_notes/2026-11-20_syntax_bakeoff/out_G_haiku.md
# what it is: the three tasks written by Haiku in syntax G
================================================================

```clojure
; Task 1: hermit crab
(defspace World Body Leg Eye Claw Shell)

(defn spine [u]
  (let [r (* 0.06m (- 1 (* 0.5 u)))
        th (* 18.85 u)]
    (p3 World (* r (cos th)) (* r (sin th)) (* 0.05m u))))

(defn shell-r [u] (* 0.008m (+ 1 (* 2 u))))
(defn rib [u] (+ 1 (* 0.15 (sin (* 88 u)))))

(defscan whorl [u 0 1 256] :on bake-lut
  :init (frame (spine u) (dir (d spine u)) [0 1 0])
  :step [prev] (transport prev (spine u) (dir (d spine u))))

(defn chitin [p]
  (+ (ggx [0.75 0.22 0.06] ($ 0.25 + 0.08 * (fbm2 (* 60 p))) 0)
     (sss [0.9 0.35 0.15] 0.1)
     (coat 0.8 0.04)))

(defn sand-mat [p] (+ (ggx [0.3 0.27 0.22] 0.35 0) (coat 0.9 0.03)))

(defn claw-sdf [p open size]
  (let [palm (sphere p [0 0 0] (* 0.05m size))
        fixed (capsule p [0.02m 0m 0m] [0.09m 0m 0.02m] (* 0.012m size))
        q (-> p (rot-z open))
        serr (* 0.002m (abs (sin (* 2600 (/ (:x q) 1m)))))
        moving (- (capsule q [0m 0m 0m] [0.10m 0m 0m] (* 0.010m size)) serr)]
    (smin palm (min fixed moving) 0.01m)))

(defkind CrabBody :in Body {:pose (xf Body World) :head (p3 Body)}
  (trace [p] (sphere p [0 0 0] 0.07m))
  (material [p n] (chitin p)))

(defkind CrabLeg :in Leg {:parent CrabBody :pose (xf Leg World) :socket (p3 Body) :femur m :tibia m}
  (trace [p] (min (capsule p [0 0 0] [femur 0 0] 0.035m)
                  (capsule p [femur 0 0] [(+ femur tibia) 0 0] 0.025m)))
  (material [p n] (chitin p)))

(defkind Claw :in Claw {:parent CrabBody :pose (xf Claw World) :open rad :size f}
  (trace [p] (claw-sdf p open size))
  (material [p n] (chitin p)))

(defkind Eye :in Eye {:parent CrabBody :pose (xf Eye World) :socket (p3 Body) :aim (dir [0 0 1])}
  (trace [p] (min (capsule p [0 0 0] [0 0 0.08m] 0.008m)
                  (sphere p (+ [0 0 0.09m] (* 0.017m aim)) 0.02m)))
  (material [p n] (chitin p)))

(defkind Shell :in Shell {:pose (xf Shell World)}
  (surface [u v] :grid [256 32]
    (let [f (whorl u)
          a (* 6.28318 v)
          r (* (shell-r u) (rib u))]
      (+ (:o f) (* r (cos a) (:n f)) (* r (sin a) (:b f)))))
  (material [p n] (ggx [0.85 0.8 0.7] 0.3 0)))

(defkind Sand :in World {}
  (trace [p] (:y p))
  (material [p n] (sand-mat p)))

(defn! crab [b body]
  (for! [j (range 6) side [-1 1]]
    (emit! CrabLeg {:parent b
                    :pose (* body (move [(* side 0.02m) 0m (* 0.04m j)]))
                    :socket (p3 Body (* side 0.06m) 0m (* 0.04m j))
                    :femur (+ 0.40m (* 0.05m j))
                    :tibia (+ 0.30m (* 0.04m j))})))

(defn! claws [b body]
  (emit! Claw {:parent b :pose (* body (move [0.2m 0.1m 0.1m])) :open 0.6rad :size 1.0})
  (emit! Claw {:parent b :pose (* body (move [-0.2m 0.1m 0.1m])) :open 0.3rad :size 0.5}))

(defn! eyes [b body]
  (for! [side [-1 1]]
    (emit! Eye {:parent b
                :pose (* body (move [(* side 0.12m) 0.2m 0.1m]))
                :socket (p3 Body (* side 0.12m) 0.2m 0.1m)
                :aim (dir [0 0 1])})))

(deflaw eye-aim [eye :Eye] :writes [:aim]
  (let [pose (:pose eye)
        sym (-> (:socket eye) mirror-x)
        c (p3 sym 0m 0m 0.07m)
        gaze (p3 World (* 0.2m (sin (* 0.1 time))) 0.1m 0.4m)]
    (write eye :aim (in sym (* (inv pose) (dir (- gaze (* pose (out sym c)))))))))

(defcheck upright [b :CrabBody] :-> m
  (let [top (* (:pose b) (:head b))
        margin ($ top.y - 0.15m)]
    (if (neg? margin) (missed (- margin)) (held margin))))

(let! [at (place [0 0.4 0])
       b (emit! CrabBody {:pose at :head (p3 Body 0m 0.1m 0.11m)})]
  (crab b at)
  (claws b at)
  (eyes b at)
  (emit! Shell {:pose at})
  (emit! Sand {}))

(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]}) (emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main
  (run-laws eye-aim) (check upright) (orbit [0 0.2 0] 1.5m 30deg)
  (draw Vis64 Sand CrabBody CrabLeg Claw Eye Shell) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```

```clojure
; Task 2: reed bed in wind
(defspace World Reed)

(defkind Reed :in Reed {:root (p3 World) :height m :lean rad :stiff f :phase f}
  (trace [p]
    (let [now (/ time 1s)
          g (+ 0.6 (* 0.4 (sin (- (* 1.3 now) (* 1.7 (/ (:x root) 1m))))))
          theta (+ lean (* (/ 0.5rad (+ 1 stiff)) g) (* 0.1rad (sin (+ phase (* 2.2 now)))))
          mid [(* 0.5 height (sin (* 0.5 theta))) (* 0.5 height (cos (* 0.5 theta))) 0m]
          tip [(* height (sin theta)) (* height (cos theta)) 0m]]
      (min (capsule p [0m 0m 0m] mid 0.007m)
           (capsule p mid tip 0.003m)
           (capsule p (- tip [0m 0.01m 0m]) (+ tip [0m 0.01m 0m]) 0.008m))))
  (material [p n] (ink [0.12 0.09 0.14] 0.05)))

(defkind Water :in World {}
  (trace [p] (:y p))
  (material [p n] (ink [0.86 0.82 0.72] -0.5)))

(defn! bed []
  (for! [k (range 200)]
    (emit! Reed {:root (p3 World (* 1.6m (sin (* 12.9 k))) 0m (* 1.2m (cos (* 7.3 k))))
                 :height (+ 0.9m (* 0.5m (fract (* 3.7 k))))
                 :lean (* 0.08rad (sin (* 5.1 k)))
                 :stiff (+ 0.2 (* 0.8 (fract (* 2.3 k))))
                 :phase (* 6.28 (fract (* 0.61 k)))})))

(bed)
(emit! Water {})
(emit! Sun {:dir (dir [-1 2 1]) :color [1 1 1]}) (emit! Sky {:zenith [0.98 0.94 0.84]})

(defschedule Main
  (orbit [0 0.5m 0] 4m 15deg)
  (draw Vis64 Water Reed) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```

```clojure
; Task 3: heron's hop
(defspace World Body)

(defn fish-pos [t]
  (p3 World (* 2.0m (cos (* 0.2 (/ t 1s)))) 0.05m (* 2.0m (sin (* 0.2 (/ t 1s))))))

(defn wrap-angle [a] (- (mod (+ a 3.14159rad) 6.28318rad) 3.14159rad))

(defn yaw-to [from to] (atan2 (- (:x to) (:x from)) (- (:z to) (:z from))))

(defn approach [cur goal step]
  (+ cur (clamp (wrap-angle (- goal cur)) (- 0rad step) step)))

(defkind Ground :in World {}
  (trace [p] (:y p))
  (material [p n] (ggx [0.35 0.32 0.27] 0.5 0)))

(defkind Fish :in World {}
  (trace [p] (sphere p (fish-pos time) 0.06m))
  (material [p n] (ink [0.9 0.5 0.1] -0.5)))

(defkind Heron :in Body
  {:pose (xf Body World) :pos (p3 World) :yaw rad :lift m :hop s
   :turn rad :limit rad :limited bool :gap rad}
  (trace [p]
    (let [q (-> p (move [0.08m -1.0m -0.2m]) (rot-y (- 0rad turn)))]
      (min (sphere p [0 0.3m 0] 0.13m)
           (capsule p [-0.08m 0.2m 0m] [-0.08m -0.9m 0.05m] 0.02m)
           (capsule p [0.08m 0.2m 0m] [0.08m -0.9m 0.05m] 0.02m)
           (capsule p [0m 0.4m 0.05m] [0.1m 0.8m 0.12m] 0.03m)
           (capsule p [0.1m 0.8m 0.12m] [-0.08m 1.0m 0.2m] 0.025m)
           (sphere q [0m 0m 0m] 0.07m)
           (capsule q [0m 0m 0.02m] [0m 0m 0.22m] 0.018m))))
  (material [p n] (ggx [0.45 0.48 0.5] 0.4 0)))

(deflaw trunk-hop [h :Heron dt] :writes [:pose :yaw :lift :hop]
  (let [air (if (>= (:hop h) 0s) (+ (:hop h) dt) (if (:limited h) 0s -1s))
        hop (if (< air 0.4s) air -1s)
        lift (if (>= hop 0s) (* 0.12m (sin (* 3.14159 (/ hop 0.4s)))) 0m)
        yaw (if (>= hop 0s)
              (approach (:yaw h) (yaw-to (:pos h) (fish-pos time)) (* 1.5rad (/ dt 1s)))
              (:yaw h))]
    (write h :hop hop)
    (write h :lift lift)
    (write h :yaw yaw)
    (write h :pose (* (move [(:x (:pos h)) (+ 0.9m lift) (:z (:pos h))]) (rot-y yaw)))))

(deflaw head-gaze [h :Heron] :writes [:turn :limited :gap]
  (let [rel (wrap-angle (- (yaw-to (:pos h) (fish-pos time)) (:yaw h)))
        turn (clamp rel (- 0rad (:limit h)) (:limit h))]
    (write h :turn turn)
    (write h :limited (> (abs rel) (:limit h)))
    (write h :gap (- rel turn))))

(defcheck gaze-held [h :Heron] :-> rad
  (let [gap (abs (:gap h))]
    (if (> gap 0.001rad) (missed gap) (held (- 0.001rad gap)))))

(defcheck head-above-hips [h :Heron] :-> m
  (let [head (* (:pose h) (p3 Body -0.08m 1.0m 0.2m))
        hips (* (:pose h) (p3 Body 0m 0.3m 0m))
        margin ($ head.y - hips.y)]
    (if (neg? margin) (missed (- margin)) (held margin))))

(emit! Ground {}) (emit! Fish {})
(emit! Heron {:pose (move [0m 0.9m 0m]) :pos (p3 World 0m 0m 2m)
              :yaw 0rad :lift 0m :hop -1s :turn 0rad :limit 1.2rad :limited false :gap 0rad})
(emit! Sun {:dir (dir [-3 5 2]) :color [3 3 2.5]}) (emit! Sky {:zenith [0.4 0.5 0.6]})

(defschedule Main
  (run-laws trunk-hop head-gaze)
  (check gaze-held) (check head-above-hips)
  (orbit [0 0.5m 0] 4m 30deg)
  (draw Vis64 Ground Heron Fish) (shade Vis64 Sun Sky) (tonemap-aces Sky))
```


================================================================
# BACKGROUND (read in the repo, not inlined)
================================================================

- docs/PHILOSOPHY.md, CORE.md, docs/proposals/2026-11-20_meta_engine_plan.md
- agent_notes/dump_manual_nov_19th_alpha_v1.md (the language draft)
- docs/handoff/2026-11-20_spec/astra_spec_v1.md (Astra's spec v1)
- agent_notes/2026-11-20_meta/phase1.md (what is built, and where it differs from Astra)
- engines/*.loft, lib/*.loft (the built Phase 1 scenes in syntax A)

# State: a recurrence that stays a function of time

Written 2026-11-13. A design proposal: nothing here is built. It answers the "largest box" of `docs/legacy/open_design.md`.

**In one paragraph.** The author writes one step function over entries (a list or a grid) in Loft's own expressions.
Loft runs it on the GPU once, from `from` to `until`, and keeps the result as a table. That table is the truth: it is
written into the artefact's files, and both the drawing and the CPU checks read it. A small state is kept at every
step. A grid is kept at checkpoints, and the frame rebuilds the step it needs by replaying at most K steps from the
nearest checkpoint. Either way the state is a function of time, like everything else in Loft. Any frame draws alone,
checks measure the state at any instant, and a loop's seam is measured, not asserted. One export is added: `recur`.

## 1. What an author writes

Three scenes, written before the design and read cold. Each question a stranger would ask is answered in the scene
or by the rule in brackets after it.

### A skin that patterns itself (a grid, settled before the scene starts)

```ts
// A grub whose spots come from reaction and diffusion (Gray-Scott). The pattern settles for 150 s before t = 0, and
// only the last state is kept, so the skin is a still texture made by a step.
import { add, lt, mix, mul, recur, scene, plane, select, strand, sub, hash2, toon, vec2, vec3, vec4, sin, time, TAU, type F } from "../src/index.ts";

const N = 128;
const skin = recur({
  name: "skin",
  size: [N, N],                         // cells; prev() wraps at the edges (a torus)
  rate: 60, from: -150, until: 0,       // 9,000 steps, all before the scene starts
  start: (c) => [vec4(1, select(lt(hash2(c), 0.02), 1, 0), 0, 0)],    // u = 1 everywhere, v seeded in 2% of cells
  step: (prev, c) => {
    const [s] = prev(c);
    const n = (dx: number, dy: number) => prev(add(c, [dx, dy]))[0];
    const lap = sub(add(add(n(1, 0), n(-1, 0)), add(n(0, 1), n(0, -1))), mul(s, 4));
    const uvv = mul(mul(s.x, s.y), s.y);
    return [vec4(
      add(add(s.x, mul(lap.x, 0.16)), add(mul(uvv, -1), mul(sub(1, s.x), 0.037))),
      add(add(s.y, mul(lap.y, 0.08)), sub(uvv, mul(s.y, 0.097))), 0, 0)];
  },
});

const grub = strand({
  name: "grub",
  spine: (m: F) => vec3(mul(m, 4), add(0.5, mul(sin(add(mul(m, 6), mul(time, TAU / 4))), 0.15)), 0),  // one wave per loop
  girth: (m: F) => mul(sin(mul(m, Math.PI)), 0.5),
  paint: (m, v) => mix([0.85, 0.75, 0.55], [0.25, 0.12, 0.08], skin.on(vec2(m, v))[0].y.mul(3).saturate()),
});

export default scene({ ground: plane, things: [grub], style: toon(), sun: [-0.4, 0.8, 0.45], period: 4,
  cameras: { whole: { kind: "perspective", eye: [2, 2, 5], target: [2, 0.5, 0], fovY: 0.6 } } });
```

Questions a stranger would ask, with the rule that answers each:

- *What are `c` and `prev`?* For a grid, `c` is the cell (a vec2 of whole numbers). `prev(j)` returns the lanes of
  cell `j` from the state before this step. Indices wrap (the rule).
- *Why `[ ... ]`?* An entry is one or more vec4 lanes, as everywhere else in Loft's tables (`fetch(table, i, lane)`).
  `start` fixes how many lanes there are, and `step` must return the same number.
- *What does `on(vec2(m, v))` read?* The state at the scene's `time`, bilinear in cells, with coordinates 0 to 1
  across the grid. With `until: 0` it is the same at every time.

### A flock (a list, every step kept, no loop)

```ts
// 200 motes: separation, cohesion and alignment within 2 units, chasing a lure that is a closed form of time.
import { add, and, cos, div, dot, draw, f, hash33, iterate, lt, max, mul, ne, points, recur, scene, nothing, select,
  sin, strand, sub, through, time, toon, vec3, vec4, type F } from "../src/index.ts";

const n = 200, dt = 1 / 30;
const lure = vec3(mul(sin(mul(time, 0.4)), 6), 3, mul(cos(mul(time, 0.3)), 6));   // drawn too; inside step, time is the step's
const flock = recur({
  name: "flock", size: n, rate: 30, until: 30,
  start: (i) => [vec4(mul(sub(hash33(vec3(i, 1, 2)), 0.5), 8), 0), vec4(0)],      // lanes: position, velocity
  step: (prev, i) => {
    const [p, v] = prev(i);
    const [sep, mid, vel, k] = iterate(n, [vec3(0), vec3(0), vec3(0), f(0)] as const, ([s, c, a, m], j) => {
      const [q, w] = prev(j);
      const d = sub(q.xyz, p.xyz), d2 = add(dot(d, d), 1e-4), near = select(and(lt(d2, 4), ne(j, i)), 1, 0);
      return [sub(s, mul(d, div(mul(near, 0.3), d2))), add(c, mul(d, near)), add(a, mul(w.xyz, near)), add(m, near)] as const;
    });
    const k1 = max(k, 1);
    const acc = add(add(sep, mul(div(mid, k1), 0.4)), add(mul(sub(div(vel, k1), v.xyz), 0.6), mul(sub(lure, p.xyz), 0.15)));
    const v2 = add(v.xyz, mul(acc, dt));
    return [vec4(add(p.xyz, mul(v2, dt)), 0), vec4(v2, 0)];
  },
});

const mote = strand({ name: "mote", spine: (m: F) => through(m, [[-0.1, 0, 0], [0.1, 0, 0]]), stations: 4,
  girth: (m: F) => mul(sin(mul(m, Math.PI)), 0.08), paint: [0.95, 0.85, 0.6], detail: { along: 6, around: 8 } });

export default scene({
  ground: nothing, style: toon(), sun: [-0.4, 0.8, 0.45],
  things: [draw.points("motes", points(n, (i) => ({ at: flock.at(i)[0].xyz, scale: 1 })), mote)],
  cameras: { whole: { kind: "perspective", eye: [0, 6, 18], target: [0, 3, 0], fovY: 0.7 } },
});
```

- *What is `time` inside `step`?* The time of the step being taken. So `lure` means the lure where it was then, and
  the same expression drawn outside means the lure now. (This is the rule. It is how a step reads the scene's
  closed forms.)
- *What happens after 30 s?* The state holds at its last value. `loft clip` refuses a clip longer than the shortest
  `until` and names the fix.
- *Why no `period`?* A flock is chaotic, so it never returns to its start. `loft check` would measure the seam and
  fail (section 5).

### A rope on a moving hand (a list, periodic by settling)

```ts
// A 40-point rope hangs from an anchor that swings twice in the 8 s loop. Run for three loops before t = 0, the damped
// rope settles into the swing's periodic answer, so the loop closes by physics, and `loft check` measures the seam.
import { add, dot, eq, f, iterate, length, lt, max, mul, recur, scene, plane, select, sin, strand, sub, time, toon,
  vec3, vec4, TAU, type Check, type F, type Probe } from "../src/index.ts";

const anchor = vec3(mul(sin(mul(time, TAU / 4)), 1.5), 4, 0);     // whole turns in 8 s
const N = 40, REST = 0.1, W2 = 150 * 150, dt = 1 / 240;
const rope = recur({
  name: "rope", size: N, rate: 240, from: -24, until: 8,
  start: (i) => [vec4(0, sub(4, mul(i, REST)), 0, 0), vec4(0)],   // position, velocity
  step: (prev, i) => {
    const [p, v] = prev(i);
    const pull = (j: F) => {                                        // a stiff spring to entry j
      const d = sub(prev(j)[0].xyz, p.xyz), l = max(length(d), 1e-6);
      return mul(d, mul(sub(l, REST), W2).div(l));
    };
    const force = add(add(pull(sub(i, 1)), select(lt(i, N - 1), pull(add(i, 1)), vec3(0))), vec3(0, -9.8, 0));
    const v2 = mul(add(v.xyz, mul(force, dt)), 0.995);
    const pinned = eq(i, 0);
    return [vec4(select(pinned, anchor, add(p.xyz, mul(v2, dt))), 0), vec4(select(pinned, vec3(0), v2), 0)];
  },
});

const cord = strand({ name: "cord", spine: (m: F) => rope.on(m)[0].xyz, girth: 0.03, tips: "open", paint: [0.8, 0.3, 0.2] });

// Energy per unit mass: the bug it catches is an explicit step that blows up (stiffness times dt past 2).
const energy = iterate(N, [f(0)] as const, ([e], j) => {
  const [p, v] = rope.at(j);
  return [add(e, add(mul(dot(v.xyz, v.xyz), 0.5), mul(p.y, 9.8)))] as const;
})[0];
export const checks: Check[] = [{
  name: "the rope's energy stays inside its first second's band",
  run: (probe: Probe) => {
    const es = Array.from({ length: 81 }, (_, k) => probe.scalar(energy, k * 0.1));
    const lo = Math.min(...es.slice(0, 10)), hi = Math.max(...es.slice(0, 10)), worst = Math.max(...es);
    return { name: "rope energy", pass: worst <= hi + 0.2 * (hi - lo + 1), value: worst, measured: `most ${worst.toFixed(2)}`, limit: `<= ${(hi + 0.2 * (hi - lo + 1)).toFixed(2)}` };
  },
}];

export default scene({ ground: plane, things: [cord], style: toon(), sun: [-0.4, 0.8, 0.45], period: 8,
  cameras: { whole: { kind: "perspective", eye: [0, 3, 9], target: [0, 2.5, 0], fovY: 0.6 } } });
```

- *Why does `pull(sub(i, 1))` at `i = 0` work?* Index -1 wraps to entry 39, and the pinned entry ignores the force.
  Wrapping is the one indexing rule. Anything else is the author's `clamp` or `select`.
- *Why 240 steps a second?* Stability: the spring's angular rate times dt must stay below 2 (150 / 240 = 0.63). The
  rate is the author's choice, and the energy check catches a wrong one.
- *How does the check read the rope?* `rope.at(j)` is an expression of time like any other, so `probe.scalar(e, t)`
  evaluates it on the CPU from the stored table.

Defects these scenes exposed, and how the design fixed them: a separate `t` argument to `step` was removed, because
`time` already means it. A per-shape signature was removed: `size` is a number or `[w, h]`, and the index is a number
or a vec2. Reading at "any other time" is limited by a rule stated in section 4. An event keyed to a time on the step
grid is fragile: in a throwaway run, a kick written as `t >= 3 && t < 3 + dt` was skipped, because 720/240 rounded
below 3. Key events to the step index, or better, use the closed form (section 6).

## 2. What exists (verified in the code)

| Fact | Where |
|---|---|
| `time` is an input like any other (`input("time", "f32", [0])`); every motion is an expression of it. | `src/expr.ts:320` |
| `iterate(count, init, step)` is a real loop in both backends. `count` may be an expression, so a few numbers *can* be stepped over time today: `iterate(floor(time * 240), [x, v], ...)`. | `src/expr.ts:355` |
| The pendulum's swing is the sum of closed-form responses (`sway` steady state, `jolt` impulse response). `swing`, `response` and `impact` are the same spring with chosen rates. | `src/geom/pendulum.ts:146`, `src/geom/lively.ts:268`, `src/curve.ts:164` |
| Delay with no state: `dragged` (t - along * lag), `echo` (f(t - delay) * gain). | `src/geom/lively.ts:294,315` |
| `looping(T, build)` makes `alive` loop by rounding its rates to whole turns. Nothing else is made to loop. | `src/geom/lively.ts:347` |
| `Scene.period` becomes `Loop { kind: "every" }`. It is asserted by the author; no code compares the state at P with the state at 0. `loopTime` wraps the clock. | `src/scene.ts:77`, `src/artefact.ts:386` |
| Table programs: `map` (one invocation per entry), `scan` (one invocation loops, carrying a small state), `cpu` (named, unported). Tables are scheduled in dependency levels, one dispatch per level. | `src/expr.ts:108`, `src/compile/tables.ts` |
| Static tables are filled once on the CPU at compile and uploaded once (`Artefact.statics`). Storage limits: one binding holds at most 128 MiB (`BINDING_VEC4S`), and 5 table buffers per stage. | `src/compile/tables.ts:36-40,463` |
| `tabulate(name, fn, n, "static" \| "frame")` returns an interpolant. Distance bricks sample a static field once. `varianceOf` classifies an expression as static, changing (names what it reads) or unevaluable (GPU-only). | `src/map/table.ts`, `src/map/brick.ts` |
| Rates: build < frame < copy < item. Per-frame work read by faster programs is lifted into frame-values tables. | `src/compile/rates.ts`, `docs/legacy/gpu_native.md` "Rates" |
| Checks evaluate expressions at a time through `Probe.eval` → `compiled.frame(state)`, which fills every table on the CPU. | `src/check.ts:52` |
| The file format: a fixed list of dispatches per frame; a loader refuses `cpu` tables. | `docs/legacy/artefact_format.md` |
| `draw.points(name, points(n, i => ({ at, scale })), thing)` draws n copies at expressions of the index (and of time). | `src/draw/index.ts:83`, `src/map/chart.ts:84` |

So nothing in Loft is simulated, with one exception the docs do not mention: a state of a few numbers can already be
stepped inside one expression. A throwaway check (`out/state_exp/spring.ts`) stepped a spring kicked twice at 240
steps a second. It matched the closed-form sum of two impulse responses to 4e-5, and took 0.2 to 0.8 ms per CPU
evaluation at t = 2 to 120 s (the cost grows with t). A list or a grid cannot be stepped this way, because `iterate`'s
state is a small tuple.

## 3. Options

| Family | What a user writes | What it cannot express | What it costs the library |
|---|---|---|---|
| Special types | `flock({ count, separation })`, `cloth(...)`, `reactionDiffusion({ feed, kill })` | anything not on the menu: a fourth flocking rule, a coupled grid and list, a new automaton | a module per kind (about 300 lines each), each with its own GPU code and limits; a context line each; violates rule 1 |
| Parameters only | one `particles({ count, gravity, drag, attract, emit })` | interactions, grids, growth; any law not among the parameters | about 500 lines and a parameter list that only grows |
| Functions only (**recommended**) | `recur({ size, rate, from, until, start, step })`, with the step in expressions | scatter writes; host inputs; more than the memory budget at the chosen rate; mutual coupling of differently shaped states except inside one recurrence | about 600 lines, 1 export |
| Shaders only | `recurWgsl({ size, wgsl })` | nothing on the GPU | no CPU twin, so no drift certificate; errors arrive as WGSL compile errors. Not needed as an entry point: a step may call a `lib` function marked `"gpu-only"` (or `wgslFn`), which is the level below the functions-only option (rule 2) |
| Closed forms only (today) | a cookbook: `jolt`, `dragged`, modes | anything that truly steps | 0 lines |
| Stepping inside an expression (exists) | `iterate(floor(time * rate), ...)` | lists and grids; cost grows with t every frame | 0 lines |
| Live state on the GPU | a step run once per frame from last frame | nothing; it can read host inputs | gives up: any frame drawn alone, checks at an instant, clips rendered in any order or in parallel, the loop's proof, the same pixels in a fresh process (they depend on frame history) |
| Live state with an input log (hybrid) | the same step; the host logs inputs per step | nothing | everything above is recovered *given the log*. Bigger host contract. Later, on a second user (section 4) |

The options split on one question: **where does the trajectory live?** For special types and parameters it lives in
library code. For live state it lives in the host's memory and history. For the recommendation it lives in a table
computed once. That last choice is what keeps Loft's three properties (time is an argument, any frame alone, the
description is measurable).

## 4. The recommendation

### 4.1 The theory, to the number

A recurrence S(n+1) = step(S(n), t_n), with t_n = from + n / rate, is a function of time once it has been run: S(t) =
S(floor((t - from) * rate)). The only questions are where it is run, what is kept, and how a frame gets back the step
it needs.

**Keep every step, or keep checkpoints and replay.** If every K-th state is kept, the frame at step n is
step^(n mod K)(C[floor(n / K)]). This is exact, not an approximation, and it costs at most K steps per frame. This is
checkpointing as in reverse-mode differentiation (inherited), and the I-frames of a video codec (inherited). The
compiler picks K from size alone: K = 1 when the whole history fits the budget (64 MiB per recurrence), otherwise
K = ceil(history / 64 MiB), capped at 120 (above the cap, a refusal names the fix: a lower rate, a smaller size or a
shorter span). Lists fall on K = 1 and grids on checkpoints, so the shapes sort themselves.

**The GPU is the engine; the checkpoints are the truth.** Measured on this machine (Apple M4, Dawn through
bun-webgpu 0.1.7, load average 4 to 7, best of 3; scripts under `out/state_exp/`, not committed), with GPU wall time
including submission:

| Step | CPU, JavaScript | GPU | Ratio |
|---|---|---|---|
| Gray-Scott 256 x 256 | 2.7 ms per step (hand-written and `compileJs` per cell agree) | 5.6 us per step over 7,200 steps (41 ms); 20 us over 240 | about 480 |
| Flock of 200, all pairs | 0.38 ms | 27 us | 14 |
| Flock of 2,000 | 2.7 ms (600 steps) | 259 us | 10 |
| Rope of 60, 8 constraint passes | 0.019 ms | dispatch-bound, about 5 us | 4 |
| An empty, early-exit dispatch | | 5 to 15 us | |

A grid needs the GPU. A list does not, but one engine per trajectory matters more than speed. A chaotic step (a
flock) run on two engines gives two different flocks after a few seconds, and then a CPU check measures a flock
nobody drew. So the GPU runs the bake once, and the checkpoints go into the files. Every reader, whether the drawing
on any loader or the CPU checks, starts from the same numbers and drifts for at most K steps. On the machine that
baked, replay from a checkpoint repeats the bake's own arithmetic, so it is bitwise the bake's states. On another
GPU it differs by K steps of rounding. A load-time bake would move the whole trajectory onto each player's GPU, so it
is rejected.

**Per step, each dispatch needs its own step index.** It cannot be written with `writeBuffer` between dispatches:
writes are ordered with submits, so every dispatch of the bake would see the last write. The state therefore carries
a hidden clock entry, which invocation 0 advances into the next buffer of the ping-pong pair. Checkpoints keep the
clock too. (A comment at that site will say this.)

**Reading.** Between steps n and n + 1, a read is linear in time, by the fraction of a step (so 30 steps a second
draws smoothly at 60 frames a second). `on(u)` is linear (a list, u in 0 to 1) or bilinear (a grid, wrapping) in
space. For K = 1, a read is a closed form over a static table: no per-frame stage at all. For checkpoints, the frame
replays into two per-frame scratch tables (n and n + 1). The loader computes n from the time and issues exactly
k + 1 step dispatches. It writes n into a hidden input slot, so GPU and CPU agree on n exactly.

### 4.2 The memory and time table (question 1)

Entry sizes in vec4 lanes of 16 bytes: a kicked spring 1 x 1 (16 B per step); a flock of 200 x 2 lanes (6.4 KB); a
rope of 60 x 2 lanes (1.9 KB); a grid of 256 x 256 x 1 lane (1 MiB; two of its four floats are used). MB = 10^6
bytes. GPU times are interpolated between measured points. The rope's GPU time and the accretion row are estimates.

| Shape | Period | 30 steps/s | 60 steps/s | Kept as | GPU bake | CPU bake (for scale) |
|---|---|---|---|---|---|---|
| Spring (a few numbers) | 8 / 30 / 120 s | 3.8 KB / 14 KB / 58 KB | 7.7 KB / 29 KB / 115 KB | every step | needs none: closed form (section 6) | |
| Flock of 200 | 8 s | 1.5 MB | 3.1 MB | every step | 7 ms / 13 ms | 0.09 s / 0.18 s |
| | 30 s | 5.8 MB | 11.5 MB | every step | 24 / 49 ms | 0.34 / 0.69 s |
| | 120 s | 23 MB | 46 MB | every step | 96 / 191 ms | 1.4 / 2.8 s |
| Rope of 60 | 8 / 30 / 120 s | 0.46 / 1.7 / 6.9 MB | 0.92 / 3.5 / 13.8 MB | every step | under 40 ms in every case | at most 0.14 s |
| Grid 256 x 256, everything kept | 8 / 30 / 120 s | 252 MB / 944 MB / 3.8 GB | 503 MB / 1.9 GB / 7.5 GB | does not fit (one binding is 128 MiB) | | |
| Grid 256 x 256, checkpoints | 8 s | K = 4, 64 MB | K = 8, 64 MB | checkpoints | 5 / 7 ms | 0.65 / 1.3 s |
| | 30 s | K = 15, 64 MB | K = 29, 64 MB | checkpoints | 9 / 14 ms | 2.4 / 4.9 s |
| | 120 s | K = 57, 64 MB | K = 113, 64 MB | checkpoints | 22 / 41 ms | 9.7 / 19 s |
| Grid 256 x 256, settled then still (`until: 0`) | any | 1 MiB | 1 MiB | the last state only | 9,000 settling steps: about 50 ms | 24 s |
| Accretion, capacity 2,000 | any | 32 KB | 32 KB | the final state plus birth times (below) | about 1 to 2 s (estimate: a 200-step walk against every particle, per append) | about 4 s |

Replay cost per frame for a checkpointed grid, at the measured 5 to 10 us per dispatch: worst case K steps (0.04 ms
at K = 4, 0.15 ms at 15, 0.3 ms at 29, 0.6 ms at 57, 1.1 ms at 113). The average is half that. A host that keeps the
last replayed state ("resume") pays 1 or 2 steps per frame while playing forward, with bitwise the same result.
Checkpoints add to load time what it takes to read and upload them: about 30 to 80 ms per 64 MB (estimate).

Growth by accretion needs no history: it is append-only. Run it entirely before t = 0 (`from: -T, until: 0`). Entry
i is born at step i, and the closed form is "draw entry i when `time` is past its birth": `select(le(i, (time + T)
* rate), ...)`. Memory is the capacity, whatever the duration.

### 4.3 Exact types

```ts
// src/map/recur.ts (new). Exported from src/core.ts.
type Cells = number | readonly [number, number];
/** An entry's index: a whole number for a list, a cell (vec2 of whole numbers) for a grid. */
type Index<C extends Cells> = C extends number ? In<"f32"> : In<"vec2">;
type Lanes = readonly Expr<"vec4">[];

export type RecurSpec<C extends Cells> = {
  /** An identifier, unique in the scene (it names tables). */
  readonly name: string;
  /** Entries: a count (a list) or [w, h] (a grid). prev() wraps every index into range. */
  readonly size: C;
  /** Steps per second of scene time. Inside `step`, `time` is the time of the step being taken. */
  readonly rate: number;
  /** When the first step is taken, seconds (default 0; negative settles before the scene starts). */
  readonly from?: number;
  /** The last time kept, seconds. After it, reads hold the last state. */
  readonly until: number;
  /** The state at `from`: entry i's lanes, a closed form in i. Fixes how many lanes there are. */
  readonly start: (i: Expr<C extends number ? "f32" : "vec2">) => Lanes;
  /** Entry i after one step, from the whole state before it. Gather only: it writes entry i and nothing else. */
  readonly step: (prev: (j: Index<C>) => Lanes, i: Expr<C extends number ? "f32" : "vec2">) => Lanes;
};

export type Recurrence<C extends Cells> = {
  /** Entry i at `when` (default `time`), linear between steps. */
  readonly at: (i: Index<C>, when?: In<"f32">) => Lanes;
  /** The state at coordinates 0..1 (list: linear along it; grid: bilinear, wrapping), at `when` (default `time`). */
  readonly on: (u: C extends number ? In<"f32"> : In<"vec2">, when?: In<"f32">) => Lanes;
};

export function recur<const C extends Cells>(spec: RecurSpec<C>): Recurrence<C>;
```

**What a step may read** (classified once, at `recur`, by a walk like `varianceOf`, after `time` is replaced by the
step's time): constants; `time`; static tables; `prev`; other recurrences (one-way coupling, baked in lockstep in
dependency order: a recurrence is a function of time, so another step reads it at the step's time for free); `lib`
functions, including GPU-only ones. Refused, each with a `SceneError` naming the read and the fix: host inputs (see
4.6), per-frame tables (a posed body's stations: read the closed-form spine instead), curve rows (`tabulate` the curve
as static), and params outside `loft dev`. In `loft dev`, a param edit re-bakes (41 ms for the largest grid above), so
sliders keep working.

**What a reader may ask.** With every step kept, `when` may be anything, even per vertex. A trail is
`flock.at(i, sub(time, mul(m, 0.5)))` along a strand. With checkpoints, `when` must be a per-frame expression (rates
already classify it), and each distinct `when` costs one more replay. A per-item `when` on a checkpointed state is
refused with the bytes that keeping every step would need.

### 4.4 Where it plugs in

| Piece | File and function | Lines (estimate) |
|---|---|---|
| `recur`: spec checks, read classification, `at`/`on` built from `fetch` and `mix`, the four-part comment | `src/map/recur.ts` (new) | 200 |
| A table program kind for a recurrence's stored states, and the compiler's choice of what is kept, a sum type: `Kept = { kind: "every"; states } \| { kind: "checkpoints"; every: number; states }` | `src/expr.ts` (`TableProgram`), `src/compile/tables.ts` (`classify`, `schedule`, `poseStages`): the step lowered twice (ping and pong), the replay as a repeated map stage | 150 |
| Bake on the GPU (ping-pong pipelines, clock entry, checkpoint copies, readback); per-frame replay count and the hidden step input | `src/host/host.ts`, `src/artefact.ts` (optional `recurrences` field) | 120 |
| CPU reading: from the stored table directly (K = 1), or the `compileJs` twin replaying at most K steps from a checkpoint (Probe and `Artefact.pose`) | `src/compile/tables.ts` (`cpuFill`) | 40 |
| Format: a dispatch may carry `repeat: { recurrence }`; the loader computes k from time, rate, `from` and K, and writes n to the named input slot. Manifest version 2 only when present | `src/host/manifest.ts`, `docs/legacy/artefact_format.md`, `rust/loft-loader` | 30 TS + 40 Rust |
| The seam and drift checks, the clip-length refusal, `loft profile` lines (bake ms, replay ms, MB) | `src/cli/check.ts`, `src/cli/video.ts`, `src/host/profile.ts` | 60 |

**What changes for existing scenes: nothing.** A scene without `recur` compiles to byte-identical WGSL and a
byte-identical version 1 manifest, and draws the same pixels. `compile` stays synchronous and CPU-only: a compiled
artefact with recurrences carries them unbaked, and the first host to load it bakes (`loft build` writes the result as
blobs, so a file loader never bakes).

### 4.5 Loops (question 5)

When a scene states `period: P`, `scene()` refuses any recurrence whose kept span does not cover [0, P], and names
the fix. `loft check` then always measures the **seam**, max |S(P) - S(0)| per lane relative to that lane's range
over the loop, and fails above a typed tolerance (1e-3 relative by default). Loft asserts a period today and checks it
nowhere. With a recurrence, the seam becomes the first measured proof of a loop.

How a recurrence can close, from honest to not:

1. **Settle into the forced periodic answer (honest, physical, measured).** A damped system driven by a closed form
   of period P converges to its periodic steady state, the same steady state `sway` uses for the pendulum. Start it
   several periods early. The rope above damps velocity by e^(-1.2 s^-1), so 24 s of settling leaves a seam of about
   e^-29: below f32 rounding.
2. **Still by construction.** `until: 0`: a settled pattern is the same at every time.
3. **A blend (an edit, honest only when labelled).** Fading the last B seconds toward S(t - P) closes any loop, but
   during the blend the state obeys no step. That is visible: a flock's motes slide, and a sharp pattern goes soft
   midway. It is not a library feature. With every step kept, an author writes it in two lines with `at(i, when)`,
   labels it, and excludes the blend window from law checks.
4. **No loop.** A chaotic system (the flock) does not return. Leave `period` out.

Not offered: playing forward then backward (motion visibly runs backward), and searching for a near return
(astronomically unlikely in 1,200 numbers). A shooting method for a periodic orbit (Newton on S0 = Phi_P(S0)) is a
relative for small conservative states.

### 4.6 Host inputs (question 4)

A baked recurrence cannot depend on an input a game changes at run time, because its trajectory was computed before
the input existed. v1 refuses it. The honest alternatives:

- **Re-bake on change.** At 41 ms for 7,200 grid steps, this suits an input that changes rarely (a level's setting, a
  seed). It is a hitch, not a frame.
- **Live state.** Step once per frame from last frame. What is lost, exactly: a frame cannot be drawn alone (frame n
  needs every earlier frame); a check at an instant has nothing to evaluate (the state depends on input history it
  does not have); clips cannot be rendered out of order or by parallel workers; the loop's proof is gone; and the
  pixels depend on the frame timing unless the step is fixed-rate.
- **The hybrid: live state with an input log.** Steps are fixed-rate on the scene clock. The host records each input
  per step into a table that the step reads, and keeps checkpoints as it goes. Then state(t) is replay from the log,
  so a check, a clip renderer or a second machine reproduces any frame given the log. This is deterministic lockstep
  (inherited from networked games). It is the same `step`, with the log as one more read.

**Recommendation:** bake only, until a second unrelated scene needs a live input. Then build the hybrid, never plain
live state. **Never:** a step that reads the frame's dt; a host that writes into state buffers; scatter writes with
atomics (float adds in racing order break determinism); or quietly dropping "any frame alone" for a scene that
states a period.

## 5. Closed forms that look like state (question 6)

Use these first. Keep `recur` for what truly steps.

| Looks like state | Is a closed form | Exists |
|---|---|---|
| A spring kicked at known times | a sum of impulse responses, one per kick | `jolt`, `swing`, `impact`, `response` |
| A spring kicked at times a game sends | the same sum, with the last few kick times as inputs | `jolt` with `after: time - kickTime` |
| Any linear response to a known drive | a convolution: steady sines plus impulses; a keyed path is a jolt at each key | `pendulum`'s `answer` |
| A trail, a wake, a banner following a moving tip | the thing's own position at earlier times: point m is p(time - m * lag) | `dragged`, `echo` |
| A hanging chain, a string or a banner swinging a little | a sum of modes, each a pendulum (linear) | write it with `pendulum` per mode |
| A ballistic particle, sparks with birth times | each particle from its own birth: an arc | `arc`, per-index birth times |
| Distance travelled, a phase under a changing rate | an integral; closed if the rate integrates, otherwise `tabulate` the cumulative sum once | `tabulate` |
| A smoothed follow (a lagging camera) | exponential smoothing is a convolution; closed for sums of sines | |
| Additive cellular automata (rule 90) | Pascal's triangle mod 2 (Lucas' theorem): cell(x, t) from binomials | |
| A wander | noise of time | `alive`, `noise2` |

What truly needs a recurrence: nonlinear coupling between many entries (a flock, large swings, cloth with contact),
diffusion with reaction, erosion, nonadditive automata, growth that depends on what grew (accretion).

## 6. Checks (question 7)

The state at any instant is a table, so any expression of it is measurable by `probe.scalar(e, t)`. Two checks run
automatically for a scene with a recurrence, and add no export:

- **The seam** (section 4.5), when the scene states a period.
- **The drift certificate.** For a checkpointed state, the CPU twin replays checkpoint j for K steps and compares the
  result with checkpoint j + 1. This is the bound on how far a CPU check can be from what is drawn. It fails above
  the tolerance. A step that uses a GPU-only function is reported as "not certified", not failed.

One check for authors, written with no new export (the rope scene): **energy stays in its band**. It catches the most
common bug of a step, an explicit integrator past its stability limit, which shows as energy growing without bound
within a second. A second check worth writing: **a count stays in range** (accretion's born count equals
(time - from) * rate, and every entry stays inside its bounds).

## 7. The lines in `loft context`

Replace "no simulation;" in the first paragraph with "state is baked (`recur`), never live;" (6 words for 2).

API map, from the comment's first line (33 words):

> `recur({ name, size, rate, from, until, start, step })`: state stepped by your `step(prev, i)` (inside it `time` is
> the step's time), baked once, read as a function of time: `.at(i, when)`, `.on(u)`.

Craft, added under "Motion that reads" (48 words):

> **State: closed form first.** A kick is a `jolt`, a trail is the thing at earlier times (`dragged`), a small swing
> is a sum of modes. `recur` only what truly steps (a grid, a flock, contact). It is baked: no host inputs, and a
> loop's seam is measured.

The total added to the context is about 85 words, inside the 2,200-word budget's one-or-two-line rule for each
part.

## 8. Limits and relatives (the component's comment)

**The law.** S(n+1) = step(S(n)), run once on the GPU from `start` at `from` at `rate` steps a second, kept to
`until`. Read linearly between steps in time and linearly or bilinearly between entries in space. Every step is kept
when the history fits 64 MiB; otherwise every K-th, with at most K steps replayed per frame (exact on the baking GPU,
within K steps of rounding elsewhere).

**Parameters.** `size`, `rate`, `from` and `until` are fixed numbers (they decide memory). `start` and `step` are
functions in expressions. Inside `step`, `time` is the step's time.

**What it cannot do, because of the law:**
- read a host input, or a value that changes after the bake: the trajectory exists before the game does.
- scatter: a step writes only its own entry. A particle depositing into a grid is the grid gathering from the
  particles (65,536 cells x 1,000 particles is about 0.3 ms per GPU step).
- go past `until`: the state holds there.
- read a checkpointed state at a per-pixel or per-vertex time.
- couple two differently shaped states both ways, except as one recurrence whose entries hold both.
- grow its size: a growing structure has a fixed capacity and birth times.
- substeps that see each other within a step (Gauss-Seidel): use Jacobi at a higher rate.
- stay within 64 MiB at any size and rate: a 256 x 256 grid for 120 s at 60 steps a second already replays up to 113
  steps.

**Going past it.** The step is expressions over `prev`, so the next level down is a `lib` function with its own WGSL
(gpu-only), which loses only the drift certificate. Relatives:
- a few numbers: `iterate(floor(time * rate), ...)` inside one expression; no table, cost grows with t.
- anything linear: the closed forms of section 5.
- a static pattern: `until: 0` (settle, keep the last state).
- a periodic loop: settle into the forced answer by starting several periods early.
- temporal blocking (a tile with a halo of k cells, k steps in workgroup memory): a later optimisation for stencil
  steps, exact.

## 9. Predictions a build will be scored against

| Quantity | Prediction | Wrong if |
|---|---|---|
| Rope scene (40 points, 240 steps/s, 7,680 steps, 1,921 kept) | kept 2.5 MB; GPU bake 30 to 60 ms; seam under 1e-5 relative; no frame-time change beyond one GPU timer tick (0.0655 ms) | bake over 150 ms; seam over 1e-3 |
| Flock of 200, 30 s at 30 steps/s | 5.8 MB; GPU bake 20 to 50 ms; draw unchanged (copies read a static table) | bake over 150 ms |
| Grid 128 x 128 settled for 9,000 steps | 256 KiB kept; GPU bake 40 to 100 ms; the CPU twin bake (for comparison) about 6 s | GPU bake over 300 ms |
| Grid 256 x 256, 30 s at 30 steps/s | K = 15, 64 MB; worst replay 0.1 to 0.25 ms per frame, average about half; with resume 0.01 to 0.03 ms; load +30 to 80 ms | worst replay over 0.5 ms |
| Replay on the baking GPU | bitwise equal to the bake's own states | any bit differs |
| Drift, CPU twin against GPU over K = 15 Gray-Scott steps | under 1e-5 relative | over 1e-4 |
| Library code | about 600 lines of TypeScript and 40 of Rust | over 900 |
| Exported names added | 1 (`recur`; its types are inferred) | more than 2 |
| Existing scenes | byte-identical WGSL, manifests and pictures | any byte differs |

## 10. How it meets the other three proposals

**Light (a light is a function of a world point).** I need light functions to be allowed to read tables (`fetch`)
like any expression. I give a baked grid as a light's pattern (a cookie, a pool of light from a pattern) and a
baked field of glow density. Two hundred motes cannot be 200 lights (each light is a term in every shader); a light
summed from a baked density grid is the honest form.

**Materials (paint channels and a wrap).** I need channels to be any expression, which they are by that design. I give
`on(uv)` as a channel's natural source: `wet: skin.on(vec2(m, v))[0].y`. A grid is a texture whose content was
computed by a law.

**Geometry and rays (meshes made by code, functions of an index on the GPU, tracers).** I need the mesh drawable to
take vertex positions as expressions of the index that may read tables, not only arrays fixed at build. Then a cloth
of 32 x 32 entries is a mesh with `at(i)` per vertex, and copies whose placements change per frame (today's
`draw.copies` are static) can be a flock. I give `at(i)` as the per-vertex or per-copy source, and a grid as a height
function for a traced relief (an eroding terrain). Accretion gives a list of spheres with birth times, which their
copies or a distance field can draw.

## 11. Staged build plan

1. **The smallest picture (2 days).** `recur` with every step kept, baked on the CPU twin at compile as an ordinary
   static table: no host or format change. The rope and the flock draw. The refusals: inputs, lane count, budget.
2. **The GPU as engine (2 days).** Bake in the JS host: ping-pong pipelines, the clock entry, readback. `loft build`
   writes the result as blobs. Measure against section 9.
3. **Checkpoints for grids (4 days).** `Kept` with checkpoints, the repeat dispatch and step input, manifest version 2
   when present, the Rust loader, the `when` refusal. The skin scene and a creeping pattern.
4. **Measured loops (1 day).** Seam and drift checks, the clip-length refusal, `loft profile` lines, the context lines,
   `docs/legacy/artefact_format.md`.
5. **Resume (1 day, optional).** A host keeps its last replayed state.

About 10 days in all.

**Stop rule.** Abandon, and keep closed forms plus `iterate`, if:
- after stage 1, two of three unrelated scenes need what a gather step cannot say (scatter, a live input): the shape
  is wrong.
- after stage 2, replay on the baking GPU is not bitwise the bake: the design rests on that.
- after stage 3, the 256 x 256 x 7,200 bake is over 1 s, or worst replay at K = 113 is over 2 ms: the cost model is
  wrong by 2x.
- the context lines cannot stay under about 90 words.

## 12. What is inherited and what is new

Inherited: checkpointing with recomputation (reverse-mode differentiation's checkpoint schedules), keyframes plus
replay (video codecs), deterministic lockstep with input logs (networked games), ping-pong GPU stepping, settling into
a forced periodic answer and the shooting method, Gray-Scott (Pearson, 1993), boids (Reynolds, 1987), mass-spring
ropes.

New in this proposal:
- a recurrence presented as a function of time, with the GPU-computed checkpoints as the one truth carried in the
  files, so a trajectory does not depend on the loading GPU and checks measure what is drawn;
- `time` inside a step meaning the step's time, so a step reads the scene's closed forms with no new concept;
- one-way coupling for free between recurrences;
- the compiler choosing to keep every step or to keep checkpoints, tied to the rate rule for reads;
- the loader computing the replay count from time;
- append-only growth as a static table with a birth mask;
- the seam as the first measured proof of a loop.

On the rules: all nine hold. Rule 6 ("every default can be replaced") is argued against for K alone. K is a schedule
that never changes the answer, like the compiler's placement of a table in a level. An author changes its cost
through `rate`, `size` and `until`. Rule 9 holds the design open until a second, unrelated scene uses it.

## 13. Open questions for the owner

1. **Must checks have a GPU?** Recommended: yes, for scenes with a recurrence (exit 6 without one), because a CPU bake
   of a chaotic step measures a different trajectory. The CPU twin serves only the drift certificate.
2. **The budget:** 64 MiB per recurrence, K capped at 120? Recommended: yes. Both fit one binding and keep the worst
   replay near 1 ms.
3. **Live inputs:** refuse in v1, re-bake params in `loft dev` only, and build the input-log hybrid on a second
   user's need? Recommended: yes, and never plain live state.
4. **Seam failure:** a check failure (exit 5), with a span mismatch refused at `scene()`? Recommended: yes.
5. **The name:** `recur` (short, says "recurrence") over `stepped` or `simulate` (which promises physics the library
   does not own)? Recommended: `recur`.

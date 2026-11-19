# Motion architecture: one machinery for adaptive locomotion and performance (19 Nov 2026)

Status: proposal. Nothing here is built. It consolidates three designs into one:

- **F**: Fable, `agent_notes/2026-11-19_design/motion_fable_step{1,2}.md` and `motion_fable_phase1b.py`.
- **A**: Astra, `motion_astra_step{1,2}.md`.
- **P**: Astra with the full philosophy, `philosophy_pass/motion_step{1,2}.md`.

Evidence: the audit (`agent_notes/2026-11-19_audits/motion_audit_astra.md`), the look rounds
(`agent_notes/2026-11-19_motion.md` on `feat/motion`) and the Rust draws note (`agent_notes/2026-11-19_lean_rust_draws.md`).

My reading of the task: one motion sub meta engine whose example engines give both Spore-like adaptive locomotion
and Disney-style performance. The owner liked the S-neck of constant length, the level head with a glance capped at
15°, and feet that step because the weight asks them to; those become defaults. GPU frame time decides every runtime
choice.

## 0. The answer on one page

1. **The core gains no motion concept.** It uses the five nouns the render design already names (function, table,
   kind, pass, rate) and gains six small runtime facts that a fluid or a flock would need too (§2.6).
2. **The motion sub meta engine** is three table seams (BODY, STATE, POSE), four end kinds (`foot`, `head`, `hand`,
   `loose`), a library of laws written in WGSL, and three TypeScript names: `body`, `move` and `clip`.
3. **An engine is one WGSL module with two functions**: `step` advances a tick, `present` evaluates the pose at a
   fraction of one. The schedule is the order of calls in `step`. Loft ships two example engines, `roam` (adaptive
   locomotion) and `act` (performance: `roam` with one law replaced and two added), ordinary files anyone can copy.
4. **All motion is one dispatch per frame.** Each invocation advances its creature by the ticks it owes, presents its
   pose and measures its holds on the drawn sites. The source designs each spent three. On the Rust path today the
   five-crab lineup spends 97 ms a frame across 4,905 per-creature compute pipelines. With motion as rows and draws
   per kind, it should fall to about 24 pipelines and at most 4.3 ms (§8.1).
5. **Truth.** Every hold is measured on the drawn site, in its support's frame, naming what gave way and the trace.

| The owner asked for | Where it comes from |
|---|---|
| Spore-like locomotion that adapts at once to body, player and reversal | `roam`: no cycles; feet step from capture-point strain once their load is low; legs and neighbours come from the body |
| Disney-style performance | `act`: beats with authority, anticipation from the boundary conditions of a plan, follow-through driven by actual acceleration, squash from load, stillness by withholding motion |
| The neck and the gaze-fixed heron | The `head` kind's defaults: an S of two arcs of fixed length, a level skull, a glance capped at 15°, hold then thrust |

## 1. The consumer's code first

Tokens are `o200k_base` on each block, imports included. `shell`, `titan` and `human` are defined elsewhere;
`input`, `wgsl`, `scene` and `time` exist today.

**1. A crab that scrambles under a player's stick.**

```ts
import { body, move, foot, roam } from "loft/motion"

const legs = [[1.35, 2.8, 6.6], [0.5, 1.1, 7.4], [-0.4, -0.9, 7.3], [-1.3, -2.7, 6.4]]
const crab = body({
  trunk: { size: [4.8, 1.05, 5], height: 2.5 },
  chains: [-1, 1].flatMap((s) => legs.map(([x, fx, fz]) =>
    ({ root: [x, 0.1, 2.5 * s], lengths: [3, 2.9, 1.2], end: foot([fx, 0, fz * s]) }))),
})
const scuttle = move(crab, roam(), { go: input("stick", "vec2") })

export default scene({ things: shell(scuttle), ground: plane, cameras })
export const checks = scuttle.holds
```

**2. A heron that tracks a fish while hopping.** The neck has a range. When the fish leaves that range the body
turns, but only while a foot is down. In flight nothing can turn the body.

```ts
import { body, move, foot, head, roam, hop } from "loft/motion"

const heron = body({
  trunk: { size: [1.6, 0.6, 0.5], height: 1.9 },
  chains: [
    ...[-1, 1].map((s) => ({ root: [-0.1, -0.2, 0.18 * s], lengths: [1.05, 1.1], end: foot([0, 0, 0.18 * s]) })),
    { root: [0.5, 0.2, 0], lengths: [1.55], end: head([0.5, 1.18, 0], { turn: [1.9, 0.6], glance: 0.26 }) },
  ],
})
const bird = move(heron, roam({ lift: hop }), { go: input("stick", "vec2"), look: input("fish", "vec3") })
```

`bird.holds` reports in this form (1.58 rad is derived in §4.7; the times are illustrative):

```text
gaze   held      0 over the take until 3.10 s; neck settled at 1.58 rad with the fish circling at 1 rad/s
gaze   gave way  from 3.10 s for 0.21 s, up to 0.27 rad: neck at its 1.90 rad range in flight, no foot down to turn the body
neck   held      length error at most 0.2 mm on the drawn S
```

**3. A boss hit in the middle of an attack.** The claw keeps its beat, the trunk recoils and may step, the antennae
whip because the trunk moved, and the face stays still because nothing loose hangs from it.

```ts
import { move, act } from "loft/motion"
import { titan } from "./titan"          // six feet, two claws (hands), a head, two antennae (loose)

const slam = { clawR: [[0.7, [4, 0, 1.2], [0, -20, 0]], [1.3]] }   // be there at 0.7 s moving 20 m/s down; let go at 1.3 s
const boss = move(titan, act(slam), {
  at: input("at", "vec3"),           // the game owns where the boss stands
  since: input("attack"),            // seconds since the game began this attack
  hits: input("hits", "events"),     // point and impulse in world space, each applied once, on its own tick
})
```

No key asks for a wind-up: starting at rest 3 m above its mark and arriving at 20 m/s, the claw first rises 0.69 m
(§4.6).

**4. Motion from outside in, poses out.**

```ts
import { move, act, clip } from "loft/motion"

const walk = clip("walk.glb", human, { LeftFoot: "footL", RightFoot: "footR", Head: "head", Hips: "trunk" })
const walker = move(human, act(walk), { since: time })
export const checks = walker.holds   // where the take gave way to the body: site, ticks, metres
export const pose = walker.pose      // joint frames each frame: drawn, read back, or written by `loft export --gltf`

const live = clip(input("generated", "pose"), human, map)   // a model run elsewhere writes one source pose per tick
```

**5. Replace one law.** This heron starts turning its body at half its neck's range.

```ts
const early = wgsl`
fn turn(c: Creature, b: Body, k: u32) -> f32 {   // yaw rate head k asks of the trunk
  let neck = bearing(c, b, k);                    // the head's turn from the trunk, unwrapped, radians
  let ease = 0.5 * b.chain[k].turn.x;             // start turning the body at half the neck's range
  return sign(neck) * max(abs(neck) - ease, 0.0) / b.tc;
}`
const bird = move(heron, roam({ lift: hop, turn: early.turn }), { go: stick, look: fish })
```

A body a model invents needs no motion code. F's five-legged star with a head on a stalk is the crab's shape of
file with other numbers: 222 tokens, 9 lines.

**6. Replace the whole engine.** The body drifts with its limbs hanging, in the manner of Rain World.

```ts
const drift = wgsl`
struct Creature { at: vec3f, vel: vec3f }
fn step(c: Creature, b: Body, in: Inputs) -> Creature {
  let vel = c.vel + ((vec3f(in.go.x, 0, in.go.y) - c.vel) * 4.0 + vec3f(0, -9.81, 0)) * DT;
  var at = c.at + vel * DT;
  at.y = max(at.y, b.height);
  return Creature(at, vel);
}
fn present(c: Creature, b: Body, f: f32, s: u32) -> Frame {   // every segment hangs at rest under the moving trunk
  return Frame(c.at + c.vel * f * DT + b.rest[s].t, b.rest[s].q);
}`
const drifter = move(crab, drift, { go: input("stick", "vec2") })
```

| Case | Tokens | Lines | The same thing today |
|---|---|---|---|
| 1 Crab scramble: body, motion, scene | 235 | 10 | 2,434 tokens for the look round's path, stepper and scene (F), with no player |
| 2 Heron, fish and hop | 221 | 9 | 3,957 tokens for the gaze round's two files (F), on a fixed clock, with no fish |
| 3 Boss hit in mid-attack | 185 | 8 | Cannot be written: `recur` refuses host inputs |
| 4 Clip in, retarget, poses out, live generator | 151 | 6 | No import exists |
| 5 One law replaced | 144 | 7 | Edit the scene's expressions |
| 6 Engine replaced | 200 | 12 | Not applicable |

A reader learns three verbs (`body`, `move`, `clip`), four kinds (`foot`, `head`, `hand`, `loose`) and two example
engines (`roam`, `act`); `hop` is one of `roam`'s laws, exported so it can be named. The seams' WGSL structs are
printed by `loft context` from the code.

## 2. Architecture

```text
engines                roam · act · a consumer's own              WGSL modules; examples; any size
motion sub meta engine BODY · STATE · POSE · end kinds · laws     body(), move(), clip(); library written to be rewritten
core                   function · table · kind · pass · rate      knows nothing of feet; compiler, runtime, twin, checks, solve
```

### 2.1 Three seams

```text
build:  body plan ──► BODY rows (per plan)            score or clip ──► BEATS rows
frame:  for each creature, in ONE dispatch:
          load STATE; for n in owed ticks: STATE = step(STATE, BODY, inputs[n]); store STATE
          POSE = present(STATE, BODY, fraction); margins of every hold on the drawn sites ──► POSE
        POSE ──► draws (vertex stage) · a host that reads poses back · `loft check`
```

- **BODY**: segments with rest frames, twist and joint limits; sites (a pad point, an eye ray, a palm frame); one row
  per chain with its end kind; the trunk's derived numbers (mass, inertia, h, T_c, R, μ, strength); neighbour edges.
- **STATE**: one record per creature, a WGSL struct the engine owns, built from library parts (trunk, a pin per end, a
  row per loose chain, a plan). It holds the **span** of its current tick (start values and the acceleration chosen),
  so `present` can evaluate the pose anywhere inside the tick.
- **POSE**: one rigid frame per segment (32 B: translation and quaternion) and a margin row per held site. It feeds
  everything that draws, exports or checks.

Each seam is a plain table with a documented layout. A host can write POSE from its own solver, and Loft draws and
checks it.

### 2.2 One dispatch, ticks fused

Every source design spent three dispatches a frame: two steps and one pose. A dispatch in a short burst costs 8.5
to 33 µs (DESIGN.md §14.1, correction 2), more than the few microseconds of arithmetic in a creature's tick.

A creature's step reads its own state, BODY and its inputs, never another creature's rows. So one invocation runs
every tick it owes in a loop, then presents, with no barrier between ticks. A static bound of 64 covers a 30 Hz frame
(4 ticks), a 50 ms stall (6) and a seek from a checkpoint (at most 59). State is read and written once per frame, not
per tick. The compiler sees from a kernel's reads whether it stays in its own rows; a flock's step, which reads
others, keeps a dispatch per tick. No author flag.

The idea is the audit's XPBD alternative (an island's ticks in one dispatch), made the default.

### 2.3 Present at a fraction, not a mix of two states

F drew a mix of the last two states, up to 8.3 ms behind. I take A's form, `present(S[k], t − t[k])`:

- The trunk is integrated exactly for constant acceleration over a tick:
  `x' = x + v·dt + ½a·dt²`, `v' = v + a·dt`.
- `present` evaluates `x + v·Δ + ½a·Δ²` at Δ ∈ [0, dt). At Δ = dt this equals the next state, so nothing pops at a
  tick boundary.
- Swings are closed forms in their share s, and plans are closed forms in time.
- The attitude is interpolated between the tick's two ends.

The presented tick's decision used only inputs up to its start, so no response comes before its cause.

### 2.4 Three homes, one function

| Home | When | Mechanism | What it gives |
|---|---|---|---|
| Closed | A part has full authority from a score and reads no state (a keyed hand) | An expression of time inside `present` | Interval proofs; any frame alone |
| Baked | Every input the step reads is a script (an input whose default is an expression of time) | The same `advance` entry with `Keep::Every`, run once at load | Scrubbing; training's 16 frames |
| Live | A host writes an input | `Keep::Latest`, a per-tick input log, checkpoints | Player control; seek and replay from the log |

`move` chooses the home from what the step reads, and `loft show` prints it. An instance stays baked until its host
writes an input, then continues live from the baked state at that tick (F's "one continues the other"). Live and baked
call one compiled entry, `advance(from, to, keep)`, because Metal may contract `a*b+c` differently in two entries and
bit equality is a phase 1 gate.

### 2.5 Kinds of end

The end of a chain is a **kind** in the render design's sense: a WGSL struct plus the functions written for it
(`strain`, `repin`, `swing`, `present`). The library ships four:

| Kind | What it holds | Its re-pin when the slack runs out | Default giver |
|---|---|---|---|
| `foot` | A point in a support's frame | A step | trunk |
| `head` | Head place, skull attitude and eye ray, separately | A thrust of the head, then a turn of the body | gaze |
| `hand` | A place and arrival velocity from a score | The score's next beat | tip (`gives: "body"` makes a lunge) |
| `loose` | Nothing; it follows its root with weight | None | none |

A consumer adds a kind (a sucker that holds walls, a wheel) by writing one struct and its functions. The compiler
writes the dispatcher over the kinds present; with one kind there is no switch. Until the render lane lands open
functions, `move` writes the switch; the phase 1 crab needs none. The other sum types, each with one dispatcher, are
support (`Flight | Point | Segment | Polygon`), pin mode (`Planted | Swinging | Scored`), input
(`Level | Script | Events`) and home (`Closed | Baked | Live`).

### 2.6 What the core gains

| # | Change | Where | Why |
|---|---|---|---|
| 1 | Inputs are a sum type: `Level`, `Script(expr of time)`, `Events { slots }`. Each instance keeps a per-tick log ring, and an event is consumed once, by sequence number | `loft-ir` `InputDecl`, `loft-gpu` | Catch-up ticks now replay today's input on yesterday's ticks; a hit held over six of them is six hits |
| 2 | An instance owns its live state, tick cursor and checkpoints (every 60 ticks) | `loft-gpu` `instance()`, `prepare` | `instance()` refuses `Keep::Latest`, so reactive creatures cannot share a pass |
| 3 | Fused ticks: a row-local step runs `advance(from, to)` in one dispatch (static bound 64) | `loft-core` kernel and schedule, `loft-gpu` encode | One dispatch per frame for all motion (§2.2) |
| 4 | Steps run before the frame kernels and draws that read them | `loft-gpu` `encode_into` | Frame kernels are encoded before live steps, so readers see the last frame's pose |
| 5 | `Scope::Trace { ticks, inputs: digest, by: Gpu \| Twin }` | `loft-core` check | A verdict on stepped state. Today it is `Undecided(State)` |
| 6 | Frame identity includes the checkpoint, the input-prefix digest, the tick and the fraction | `loft-ir` identity | Two histories that end with the same stick draw different crabs, so their keys must differ |

None of the six names a foot or a gait; a fluid or a flock needs the same six. The grep gate
`rg -i -w 'foot|gait|creature|trunk' rust/crates` stays empty, as it is today.

F's records in rows are not needed: the library's WGSL loads and stores its structs from vec4 rows. `body()` runs
at record time in TypeScript, as the scene's own arithmetic, so Rust receives only rows and modules.

## 3. Where the designs differ, and what this design takes

| Question | F | A | P | Taken, and why |
|---|---|---|---|---|
| What an engine is | A WGSL module between table seams | Composable verbs (`walk`, `track`, `recover`, …) | Plain functions in order | **F's seams; P's schedule as plain calls in `step`.** One module per engine keeps cost independent of creature count and kind. A's verbs survive as laws, not names |
| Public names | 7 | About 11 | About 13 | **3 verbs, 4 kinds, 2 engines.** A law is replaced by its WGSL name in a slot (`roam({ turn })`) |
| Dispatches per frame | 3 | 3 | 3 | **1, ticks fused** (§2.2): the largest frame-time lever here |
| When the pose is drawn | A mix of the last two states, up to a tick behind | The state's span at the fraction | — | **A**: no lag, no pop, half the reads (§2.3) |
| Contact and load | Least-norm shares; can go negative | LP extremes: a share interval | Hull margin without a foot | **Each where strongest** (§4.2): hull for feasibility, nonnegative least-norm shares for the visible load (continuous, unlike LP extremes), the interval as a query |
| Who may lift | Jacobi, ties by index | After a balance witness | One decision per snapshot | **Greedy in strain order inside the invocation.** Jacobi lets two feet each lift assuming the other carries, and F's staggered first lifts (feet 0, 2, 5 at 1.033, 1.058, 1.092 s) come from chain order, not the body. Here load staggers them (§4.3) |
| Where a hold is true | Residuals from `step` | Drawn pad, deck space | The observed site | **P**: margins on the drawn site after IK; F's residuals stay as diagnostics; P's false plant is the gate |
| Gaze | Head pin; neck strain turns the body | Eye ray, skull and head place separate; unwrap | Each independently editable | **F's pin carrying A's three quantities**, with the unwrap. In flight no foot can turn the body |
| Body description | Chains | Segments, sites, surfaces | Segments, sites | **F's chains over A and P's segment and site table**, which keeps twist, rest frames and limits |
| Directed motion at build | Shooting through the step | Eliminate flight, solve near keys | Keys as preference | **Rust `solve` near hand keys** (panel c, owner-approved) with A's elimination; shooting is a phase 5 experiment |
| Authority | A number per part | One writer per degree of freedom | One writer, by order | **One rule**: one writer per degree of freedom per tick; F's authority is the default blend, in WGSL |
| Secondary motion | Exact per-tick spring on the root's acceleration | — | — | **F**; the audit's closed-form spans become a phase 5 lowering |
| Parallel layout | Invocation per creature | Workgroup per creature | Invocation | **Invocation**: a crab's serial work is about one dispatch (§8); A's layout is a lowering for heroes |
| Phase 1 | Crab; picture on TS | Weight transfer, four pads, Rust draw | Moving deck, swing edit, false plant | **The crab, live and drawn by Rust**, with P's false plant and edit and A's history identity; four pads and decks in phase 2 |

## 4. The default laws

Units are metres, seconds and radians. h is the trunk's height, T_c = √(h/g), and R is the strain radius, 0.18 of
the leg's span.

### 4.1 The pin (F)

A planted foot and a held gaze are one constraint: an end fixed in the world, a chain as slack, and a re-pin when the
slack runs out.

- **Hold.** A planted pin's point is copied from tick to tick, so its slide in state is exactly 0.
- **Strain** is the slack used over the slack available. For a foot it is
  |p − (home + v·T_c)| / R, the distance from where the capture point says the foot should be. For a head it is the
  neck's turn over its range.
- **Strain flows to the root; pose flows to the tip.** A strained neck asks the trunk to turn, which moves the homes,
  which strains the feet, which step.

### 4.2 Support and load

- The **support** is a sum type over the planted feet: `Flight | Point | Segment | Polygon`. A polygon is the convex
  hull of up to 16 points, built by monotone chain. Collinear and coincident sets fall into `Segment` and `Point`.
- **The clamp.** Holding acceleration a needs the pressure point p\* = c − (h/g)·a (the linear inverted pendulum). If
  p\* lies outside the support, project it to the nearest point q′ and take a′ = (g/h)(c − q′) (A). Then
  |a′| ≤ μg, no pull (vertical a ≥ −g) and push ≤ strength·g. In `Flight` the only force is gravity, so a flight is a
  parabola by construction.
- **Loads.** w = argmin ‖w − w_pref‖² subject to Σw = 1, Σw·p = p\* and w ≥ 0. With w_pref = 1/n and no active
  bound this is F's closed form, w_i = 1/n + (p_i − p̄)ᵀS⁻¹(p\* − p̄); a negative entry drops out and the form is
  solved again (at most n passes, usually one). A strictly convex QP's answer is continuous in p\*. An author who wants
  one foot to look loaded shifts w_pref. A's `shareRange(i)` returns foot i's feasible interval: [0.25, 0.50] in A's
  four-pad case (pads at (±0.30, ±0.20) m, q = (0.15, 0)), where the least-norm answer is 0.375.

### 4.3 Who lifts

A foot lifts when all of these hold:

- it is planted;
- its strain exceeds 1 and it has been down 0.12 s;
- no Cruse neighbour is swinging or was chosen this tick;
- its load share is at most 1.1/n;
- the remaining planted feet still contain p\* with an inset of 0.25 m.

Candidates are taken greedily in order of strain from the tick's starting snapshot. Neighbour edges (adjacent feet
along each side, plus the mirror partner) are derived from geometry by an editable library function, so five or six
legs need no index arithmetic. The load cap makes the stagger physical: under sideways acceleration p\* moves up to
1.5 m toward the trailing side, so the leading feet unload first. Evaluated (§10), the first four still lift together
from the symmetric start and the reversal breaks into staggered pairs, with swings of 0.120 to 0.334 s; a pure strain
rank gives only 0.21 to 0.27 s.

### 4.4 Swing (F, with round 4's shaping by need)

The swing is p = from + (aim − from)·m(s) + up·lift·16s²(1 − s)², with the least-jerk profile
m(s) = 10s³ − 15s⁴ + 6s⁵.

- The aim follows the body until s = 0.5 and is committed by s = 0.85.
- Duration and lift are set at lift-off from the step's length and urgency (round 4).
- The foot lands with zero vertical speed and under 0.08 m/s horizontally; today's half sine arrives at π·lift/T.

### 4.5 Trunk

- The chosen horizontal acceleration is a = (go − v)/τ with τ = T_c/2, so a larger animal answers more slowly.
- After the clamp, the trunk is integrated exactly (§2.3).
- **Lean** is a spring (2 Hz, ζ = 0.7) toward atan(a/g).
- **Leash.** A planted chain longer than 0.97 of its length moves the trunk back. The leg is never longer than it
  is, and the foot never slides.
- **Hit.** An impulse J at point r on its tick gives Δv = J/m and Δω = I⁻¹((r − c) × J). The capture point jumps by
  Δv·T_c. Feet step if that passes R, so a body absorbs hits below R/T_c without stepping. For the crab that is
  2.57 m/s. A second hit adds to the state the first one left.
- **Yaw** is clamped by the friction torque the planted feet can give. It is zero in flight, where spin is conserved.

### 4.6 Plans: anticipation is a boundary condition (F)

A move with a deadline is stated by where it must arrive, at what velocity and when: (D, v₁, T). The least-jerk
quintic from rest is

  p(s) = D(10s³ − 15s⁴ + 6s⁵) + V(−4s³ + 7s⁴ − 3s⁵),  V = v₁T, s = t/T.

Its s³ coefficient is 10D − 4V, so **the motion goes backward first exactly when V > 2.5D**. Nobody asks for the
wind-up.

- **The boss.** The slam (D = −3.0 m, v₁ = −20 m/s, T = 0.7 s) gives p = 26s³ − 53s⁴ + 24s⁵, which rises 0.69 m at
  s = 0.522 (0.37 s).
- **A jump.** With D = 0 the lowest acceleration is −1.949 v₁/T. Feet cannot pull, so the stance T = 1.949 v₁/g, the
  crouch is 0.385 v₁²/g at two thirds of the stance, and the peak net push is 2.02 g.
- **The heron's hop** (v₁ = 3.876 m/s) gives a 0.770 s stance and a 0.590 m crouch at 0.513 s. The keys asked for
  0.60 m at 0.35 s, which needs the feet to pull: free fall alone takes 0.35 s to drop 0.60 m and arrives still
  moving.
- **Strength.** A body weaker than 2.02 g takes T = 3.94 v₁/(strength·g) and crouches deeper. One number per body
  sets the timing of every jump it makes.

Plans store end conditions and a start tick, evaluate in closed form, and replan only on a hit, a new aim or a clamp
(F: replanning every tick loses 5% of the take-off speed).

### 4.7 Head, neck and gaze

These are the defaults the owner approved, made causal.

- **Head place.** The head is held in the world. When its offset from rest passes 0.15 of the neck's length, it
  thrusts back with a critically damped follower: ω = 2π·2 Hz, with velocity feed-forward 1/ω (round 4's
  best-measured lead). A strain trigger replaces round 4's fixed 0.2 s clock, so the bob rate is v/(0.15L): 2.6 Hz
  at 0.6 m/s for a 1.55 m neck.
- **Skull and eye.** The skull is level; its pitch toward a target is capped by `glance` (15°), and it yaws toward
  the target within the neck's range. The eye ray is the skull's forward direction.
- **Unwrap.** The head's yaw is unwrapped and remembers its side, so a fish crossing behind cannot flip a saturated
  head between its stops in one tick (A found a 0.115° target change giving a 57.3° jump on `main`).
- **Turn.** The trunk's yaw rate is the stick's plus `turn`: sign(neck)·max(|neck| − 0.6·range, 0)/T_c. With a fish
  circling at 1 rad/s the neck settles at 0.6·1.9 + 1·0.44 = 1.58 rad, inside 1.9. The body can keep up to
  1.73 rad/s. Past that, or in flight, the gaze gives way and says so.
- **Neck.** `present` draws the neck as round 4's S of two circular arcs of fixed length L, with
  sin θ/θ = d/L solved by four Newton steps. A chord longer than L is a reported failure, not a clamp.

### 4.8 Loose chains and authority

- **Loose chains** are damped springs driven by the root's actual acceleration, stepped by an exact 2×2 transition
  computed at build, stable at any stiffness. The rate comes from length and gravity (`pendulum.ts`) unless `hz` is
  given, so follow-through is caused by what happened, hits included.
- **Authority** is the score's writer blended with the law's writer by an explicit function, one per degree of
  freedom.
  - The default for the trunk is 1 − shock, with shock = e^(−(t − t_hit)/T_c)·min(|Δv|·T_c/R, 1).
  - Ends with a beat keep authority 1.
  - Deliberate stillness, as in A and P's face held through an antenna's swing, is authority 1 with nothing loose
    attached.

## 5. Two example engines

**`roam`** is §4's laws with `intend` reading `go` (a wanted velocity) or `at` (a place the game owns), plus `look`
and `hits`. It has no cycle, clip or gait table: it adapts to a body because every law reads BODY, and to a player
because strain reads the velocity now. Its slots: `lift`, `strain`, `swing`, `turn`, `lean`, `intend`.

**`act`** is `roam` with `intend` replaced by a score, plus `shaped` (squash from load, stretch from speed) and
`shock`.

- A score is beats per chain: `[time, place, arrival velocity]`, or `[time]` to let go.
- A pin with a beat arrives on the beat's tick exactly: a swing's arrival is a parameter, so "lands at 1.2 s" is
  true by construction for ends.
- Trunk beats are a prior followed inside the clamp, and `holds` says where they could not be: "trunk behind its keys
  from 0.25 to 0.35 s: a 0.60 m crouch by 0.35 s needs the feet to pull".
- A directed hop splits at take-off and landing. The flight is a parabola because no foot is down; the take-off
  velocity follows from the landing beat, v_y = (y₁ − y₀)/T + gT/2 (A), and the stance's plan from that.

| Disney principle | Where it comes from in `act` |
|---|---|
| Anticipation | The plan's boundary condition (§4.6), and load leaving a foot before it lifts |
| Slow in and out; arcs | Least-jerk swings and plans; swings about a fixed root; ballistic flight |
| Follow-through and overlap | Loose chains driven by the parent's actual acceleration |
| Squash and stretch | Load share and speed (`shaped`) |
| Timing | Exact at beats for ends; T_c and strength for the trunk |
| Exaggeration | Gains on impulse, strength and gravity, not on poses |
| Staging, holds | Authority 1 and nothing loose: a dead stop is a law like any other |
| Appeal, solid drawing | The author's |

## 6. Interop

**In.** `clip(source, body, map)` turns joint tracks into a score:

1. **Normalise at the edge**: metres, a right-handed y-up basis, continuous quaternion signs, twist kept. An
   unmapped required joint is a typed error naming the map entry.
2. **Retarget rotations.** For each mapped joint, q_t(t) = q_t,rest·B·(q_s,rest⁻¹·q_s(t))·B⁻¹, where B takes the
   source joint's rest basis to the target's (A). Scale the root's path by the ratio of hip heights.
3. **Ends become beats** in body-relative form (Spore), scaled by the ratio of chain lengths.
4. **Contacts are proposed.** A foot is labelled planted where its speed is under 0.15 m/s·scale and its height under
   0.02 m·scale for 3 samples. Labelled intervals become pins, so they are exact. A generator's own labels are
   proposals too: where they disagree, the report lists it.
5. **One writer per segment.** The engine owns the chains whose ends are pinned or beat. The clip writes every other
   segment, such as a spine or tail, directly.

Generated clips (MotionBricks, Kimodo, diffusion) take the same path. Only the corrected result in POSE is measured;
a clip is never reported as meeting a hold.

**In, live.** An input of type `"pose"` is a per-tick packet of source rotations and root. The step retargets it each
tick (about 1.5k operations for 33 joints). A missing tick holds the last pose and reports its age.

**Out.** POSE is the posed skeleton. A host may map it asynchronously; Keel's frame loop maps nothing.
`loft export --gltf` samples POSE:

- local TRS (parent⁻¹·child) and inverse binds from BODY's rest frames, so the export keeps twist;
- keys split at contact changes and hits;
- a check that interpolated keys at mid-sample times stay within 1 mm of POSE on every held site, or the span is
  subdivided.

The take's identity records the source hash, the map and the engine's hash.

## 7. Truth

- **A hold is a signed margin on a drawn site, in its support's frame, over a domain.** The plant margin is
  ε − ‖D(t)⁻¹·F(t)·p − a‖ (P): D is the support's transform, F the drawn segment's frame after IK, p the site and a
  the anchor. A deck moving at 0.3 m/s carries a correct foot 6 cm in 0.2 s, and the check rightly sees no error.
- **Where it is measured.** `present` writes the margins into POSE in the same dispatch that draws, so the check reads
  what the picture shows. The library also measures reach, joint limits, the neck's length on the drawn S, gaze error
  from the final eye ray, support margin, least share and timing at beats. A consumer adds a hold as a WGSL function
  of POSE that returns metres or radians.
- **Givers.** Each hold names what gave way (trunk, tip or release) and states it as a sentence: "gaze gave way
  0.27 rad for 0.21 s from 3.10 s: neck at its range, in flight".
- **Verdicts.** `Scope::Trace` says "held over 480 ticks of take `stick` (digest …), on the GPU". Interval proofs
  stay with closed homes; a law outside the `wgsl` subset gets a GPU trace or a named `Undecided`. A verdict carries
  the hash of every law it read, so replacing a law retires it (A, P).

## 8. Performance theory

GPU frame time comes first. The constants:

| Constant | Value | Band | Source |
|---|---|---|---|
| d, one dispatch in a frame's short burst | 10 µs | 8.5 to 33 | repo: DESIGN.md §14.1, correction 2 |
| The look round's eight-foot step, warm, including its dispatch | 6.5 µs | — | repo: 480 steps in 3.1 ms of GPU |
| Timestamp quantum | 0.065 ms | — | repo |
| One dependent scalar operation on an otherwise idle lane | 2 ns | 1 to 3 | conjecture |
| Throughput for branchy scalar WGSL across the GPU | 300 Gop/s | 150 to 500 | conjecture (F assumed 150) |
| Effective memory bandwidth for scattered rows | 60 GB/s | 30 to 100 | conjecture (F assumed 30) |
| One pipeline; one WGSL line through the driver | 25 ms; 60 µs | 5 to 45; 13 to 74 | the render design's constants |

**Work per crab**, counted from the laws:

- **Per tick, about 2,500 operations:** strain 240, hull 300, shares 360, lifts 400, four swings 400, trunk and lean
  220, leash 320, diagnostics 60.
- **`present`, about 2,400:** span 80, eight three-link IK at 130, 25 frames at 40, eight margins at 30.
- **Per frame at 60 Hz:** two ticks and one present, about 7,400 operations.
- **Bytes per frame:** STATE read and written once (2 × 672 B), POSE written (800 B of frames and 128 B of margins),
  and two input packets (64 B). That is about 2.3 KB. BODY (2.8 KB per distinct plan) is shared and cached.

**Model:** T(N) = d + max(L, N·w/F, N·m/B). L is one creature's serial time, w its operations, m its bytes, F the
throughput and B the bandwidth.

| Case | L | Throughput term | Memory term | Predicted motion time |
|---|---|---|---|---|
| 1 crab, 60 Hz | 7 to 22 µs | — | — | **17 to 32 µs** (cold dispatch: up to 55): below one quantum |
| 1 crab, 30 Hz (4 ticks) or a 6-tick stall | 12 to 52 µs | — | — | 22 to 62 µs, still one dispatch |
| 5 or 64 crabs | 7 to 22 µs | ≤ 3 µs | ≤ 5 µs | the same as one crab |
| 1,000 crabs | 7 to 22 µs | 15 to 49 µs | 23 to 78 µs | **33 to 88 µs** |
| 10,000 crabs | — | 0.15 to 0.49 ms | 0.23 to 0.78 ms | 0.25 to 0.8 ms |
| One heron (2 feet, head, S-neck) | about 3,800 operations | — | — | at most a crab's |
| One boss (6 feet, 2 hands, head, 2 loose chains) | about 11,000 operations | — | — | 22 to 43 µs |
| Seek: replay 59 ticks from a checkpoint | 0.15 to 0.44 ms | — | — | one dispatch |
| Bake of a 4 s scripted take (480 ticks) | 1.2 to 3.6 ms | — | — | one dispatch for every creature at once; today 3.1 ms took 480 dispatches |

The source designs predicted 0.02 ms (F), 99 µs (A) and 75 to 195 µs (P) for one crab, and 0.35 ms for 1,000 (F),
414 µs for 64 (A) and up to 0.8 ms for 64 (P). Fusing removes two dispatches and halves the state traffic.

**Memory.**

| Item | Size |
|---|---|
| Crab STATE (trunk span 6 lanes, eight pins of 4, diagnostics and cursor 4) | 42 lanes, 672 B; F's untrimmed record was 1,424 B |
| POSE | 928 B |
| Kept 4 s take | 481 × 672 B = 323 KB |
| Live seek window of W seconds | 2W checkpoints of 672 B plus 3,840·W B of input log: about 5.2 KB per second of window, per creature |
| 1,000 crabs with a 4 s window | 21 MB |

**Load.** Motion adds one module per engine, with two entries (`advance`, and `present` alone for baked scrubbing).
That is two pipelines, about 770 WGSL lines (library and `roam`). By the render design's formula, load is
0.05 s + 0.046 s ≈ **0.1 s**, whatever the number or kind of bodies: five crabs with different bodies differ only
in BODY rows.

**Build and training.** `body()` takes tens of microseconds of CPU. Recording and the Rust compile of the motion
module stay under 50 ms, against the 2 s cap. In training (512² × 16 frames, 50 ms a frame), a scripted take bakes
once in at most 5 ms, and each frame is one present-only dispatch.

### 8.1 The five-crab lineup on the Rust path

The lineup was measured on `main` at `4e8ee7b` (`agent_notes/2026-11-19_lean_rust_draws.md`, 960×540):

| | GPU p50 / p95 | Load | Modules | Pipelines (render / compute) |
|---|---|---|---|---|
| Lineup | 97.2 / 109.1 ms | 23.1 s | 5,538 | 653 / 4,905 |
| Titan | 6.79 / 7.20 ms | 35.1 s | 990 | 116 / 889 |

The TS engine draws it at 4.26 ms.

**Reading** (derived): 97.2 ms over 4,905 compute pipelines is 19.8 µs each, inside the measured 8.5 to 33 µs per
dispatch. The frame is consistent with being bound by dispatch count, not arithmetic. Each crab brings about 981 fill
kernels and 131 draw pipelines, because its pose and its draws are code per creature. Phase 1 first counts the
per-frame dispatches. If most of the 4,905 run only at load, this reading is wrong.

| | Today (Rust) | With POSE rows and per-kind draws | Who removes it |
|---|---|---|---|
| Pose and motion fills | Most of the 4,905 (counted in phase 1), one per kernel per crab | 1 module, 2 pipelines, 1 dispatch a frame for all five | this design |
| Draw pipelines | 653 | about 22, one per kind, the same for 1 or 5 crabs (render design §4.4) | the render lane |
| GPU frame p50 | 97.2 ms | dispatch overhead falls to at most 24 × 33 µs = 0.8 ms; motion ≤ 0.065 ms; the rest is draw work, which TS does in 4.26 ms. **At most 4.3 ms** | both |
| Load | 23.1 s | a + bP + κL with P ≈ 24: **about 1.5 s** (render's 1.45 s plus motion's 0.1 s) | both |

Neither half reaches these numbers alone. Motion turns every per-creature pose fill into rows of one table. The
render lane's rule that "holes are fields" turns the per-creature draws into one draw per kind. The Titan's
712-line rig, recoil and antenna whip become one `act` kernel and its rows: predicted about 25 pipelines and 1.5 s
of load, from 990 and 35.1 s.

**What would change these numbers.**

- **State spills** to memory per law, not per frame: the memory term grows up to six-fold and 1,000 crabs pass 0.3 ms.
  Then keep hot state in workgroup memory (A's layout, under 4 KiB) or split the record.
- **A hero creature** passes a quantum at two ticks: use A's workgroup-per-creature lowering.

## 9. Inherited and invented

**Inherited** (attributions recalled from the source designs): the linear inverted pendulum and capture point
(Kajita; Pratt) for T_c, strain and the pressure point; Raibert's three small laws for §4.5; least jerk (Flash and
Hogan) for swings and plans; Cruse's rules for neighbour inhibition; least-norm force distribution and feasible
contact wrenches (Caron et al.) for shares; position as primary state (Jakobsen, PBD) and exact linear springs; lockstep
netcode for the live home; Spore's ends by role, body-relative motion and grid of bodies × motions; the small leading
proxy of Rain World, Overgrowth and Euphoria; the pigeon's hold and thrust (Troje and Frost); spacetime constraints
(Witkin and Kass) as Loft's `solve`; rest-relative retargeting and glTF skinning; and Loft's own `recur`, `Steps`
kernels, `wgsl`, verdicts, typed refusals, the look rounds' stepper and neck, and render's kinds and rates.

**Invented in the source designs, kept:**

- F: the pin; chosen acceleration clamped by what the pins can give; three homes on one kernel; anticipation from
  boundary conditions, with its constants (2.5D, 1.949 v/g, 0.385 v²/g, 2.02 g, R/T_c).
- A: the load interval as a quantity a director can control; presenting a span at a fraction; one writer per degree
  of freedom.
- P: the observed site as the output of a construction, the input to its check and the root of a dependency cut; the
  false-plant gate; span compression of kept takes.

**This consolidation's own:**

- fused ticks with `present` in one dispatch;
- greedy lifting inside the invocation, gated by load share and support;
- one QP that unifies F's closed form and A's interval;
- end kinds as render kinds, so render and motion share one dispatch mechanism;
- one compiled entry for baked and live, chosen for bit equality;
- yaw tied to support, so flight explains a lost gaze.

Each ingredient is old; the combination, with truth, replaceability and frame time as the interface, is new.

## 10. Phased plan

Each phase is a lane (`tools/lane.sh`) in its own worktree. The TS engine stays frozen: TypeScript only records body
rows, WGSL text, input declarations and explicit draws (`src/record/decls.ts`). Runtime and compiler changes are Rust
in `rust/crates`, and the library and engines are WGSL text the Rust compiler consumes unchanged, so motion leaves no
TS runtime to port. Budgets are scc code lines per layer; more than 20% over needs a note in the lane.

### Phase 1: the crab lives (vertical slice, one implementer)

**Build.**

- Core changes 1 to 6 of §2.6; `body()`, `move()`, the `foot` kind, the laws of §4.1 to §4.5, and `roam` without
  head, plans, hits or loose chains.
- A crab scene drawn by Rust from explicit draws that read POSE: capsule legs, an ellipsoid shell, coloured toe tips,
  a marked ground. One draw per kind, instanced over POSE rows.
- A headless Rust harness that feeds the stick per tick through the log at 30, 60 and 120 Hz presentations.
- Before building: count the lineup's per-frame dispatches with `loft-draw profile`, to confirm §8.1's reading.

**Scene.** The look round's crab dimensions, 4 s, 120 ticks a second. The stick is +1.2 m/s from 0.2 to 1.7 s,
−2.1 m/s from 1.7 to 3.2 s, then 0. Other values: τ = T_c/2 = 0.252 s, μ = 0.6, R = 1.3, share cap 1.1/n, support
inset 0.25 m, swings shaped as in round 4.

**Owner files and budgets.**

| Layer | Files | Budget |
|---|---|---|
| Core, Rust | `loft-ir/src/{bundle,identity}.rs` (inputs, identity); `loft-core/src/{kernel,schedule,check}.rs` (fused ticks, order, `Scope::Trace`); `loft-gpu/src/lib.rs` (log ring, instance state, checkpoints, encode order) | ≤ 120 + 180 + 300 |
| Recorder, TS | `src/compute.ts` (keep latest, the input sum type) | ≤ 60 |
| Sub meta engine, TS | `src/motion/{body,move,index}.ts` | ≤ 400 |
| Library, WGSL | `src/motion/wgsl/{seams,support,pin,trunk,present}.wgsl` | ≤ 650 |
| Example engine | `src/motion/wgsl/roam.wgsl`, `src/motion/roam.ts` | ≤ 150 + 40 |
| Scene and checks | `examples/motion/crab_live.ts`, `rust/crates/loft-gpu/tests/it/motion_live.rs` | ≤ 120 TS, ≤ 250 Rust |

**Predicted, before building.** Motion rows come from these laws evaluated at f64 (F's script with greedy lifting,
the load and support gates, nonnegative shares and the exact integrator; scratch, not kept). An f32 threshold may move
a lift by a tick.

| # | Check | Predicted | Pass |
|---|---|---|---|
| 1 | Live against baked, same GPU and script, at 30, 60 and 120 Hz presentations and with a 6-tick stall | 0 of 481 states differ in any bit | 0 |
| 2 | Steps | 22; per foot 2, 3, 2, 3, 3, 3, 3, 3 | 20 to 24 |
| 3 | First lifts | 1.042 s (feet 0, 2, 5; foot 7 one tick later); all first lifts by 1.392 s | ±2 ticks |
| 4 | Reversal | The stick flips at 1.700 s, v crosses 0 at 1.900 s, the next lift is at 2.283 s | ±3 ticks |
| 5 | Clamp and lean | Peak acceleration 5.886 m/s² (friction); most lean 32.0° at 2.025 s | exact; ±0.5°, ±2 ticks |
| 6 | Support | Least share 0.060; p\* at least 1.62 m inside the hull; fewest feet down 4; the support gate binds 0 times on this wide body | all shares ≥ 0, margin > 0 |
| 7 | Reach | Margin at least 0.17 m; the leash never engages | margin > 0 |
| 8 | Plant on the drawn toe, after IK, in the deck frame | At most 0.03 mm (f32 rounding at 7 m); neighbours swinging together 0 | ≤ 0.1 mm; 0 |
| 9 | False plant: 2 cm added after IK to foot 3 | Fails at foot 3 only, 20.0 mm, over each of its stances, while the controller still says planted | exactly that |
| 10 | Landing | Last tick's travel at most 0.65 mm horizontally, 0 vertically; swings 0.120 to 0.334 s; lifts 0.250 to 0.985 | ±0.01 |
| 11 | At 4.0 s | z = −1.076 m, v = −0.086 m/s | ±0.02 |
| 12 | History | The same stick from 1.0 s after two different openings: feet differ by up to 0.26 m at 4 s, and the frame identities differ. The same history gives the same identity and RGBA | as stated |
| 13 | A law replaced: P's lower arch for urgent steps | One module's text changes. Lift ticks are identical, because decisions read only xz. Swing heights scale | as stated |
| 14 | GPU, motion span | One dispatch per frame whatever N and the ticks owed. 1 crab and 256 crabs: p95 below one quantum. 1,000 crabs: 0.03 to 0.09 ms | ≤ 0.065 ms; ≤ 0.2 ms |
| 15 | GPU, whole frame | Within one quantum of the same draws fed a recorded POSE table | ≤ 0.065 ms difference |
| 16 | Bake | 480 ticks in one dispatch, at most 3.6 ms | ≤ 5 ms |
| 17 | Modules and load | One crab and five crabs of different bodies build identical module hashes: 1 motion module and 2 pipelines, at most 8 modules in all. Frame times differ by at most a quantum. Motion's cold setup is 0.1 s | identical; ≤ 0.45 s |
| 18 | Bytes | STATE 672 B, POSE 928 B, kept take 323 KB | exact |
| 19 | Tokens | The crab file, 235 | ≤ 250 |
| 20 | Layers | `rg -i -w 'foot\|gait\|creature\|trunk' rust/crates` is empty | empty |
| 21 | The picture | Beside the look round's panels (a) and (c), at game distance, by the owner: it reads as weight at least as well as (c), the reversal reads as braking on planted feet, and no foot moonwalks | the owner |

A miss fixes the theory first. If the picture loses to (c), stop and derive the laws again. If Rust cannot yet draw
the declared pass, checks 1 to 20 score headless and only check 21 waits for the draws lane.

### Phases 2 to 5

| Phase | Adds | Owner files and budget per layer | Rust | Predicted checks |
|---|---|---|---|---|
| **2 The heron** | `head` kind (§4.7), `turn`, plans, `hop`, yaw clamped by support, decks (pins in the deck frame), A's four-pad test | `src/motion/wgsl/{head,plan,neck,deck}.wgsl` +700; `roam` +60; `src/motion/kinds.ts` +100; core 0 | None: tests that phase 1's core suffices | Hop: stance 0.770 s, crouch 0.590 m at 0.513 s, push 2.02 g at 0.583 s, flight residual 0. Fish at 1 rad/s: neck 1.58 rad, gaze error 0, held to 1.73 rad/s; beyond, or with no foot down, the gaze gives way and says so. Neck error ≤ 0.2 mm; head pitch ≤ 15°, 0° after take-off; head jerk ≤ 430 m/s³ (round 4: 428); 0 rear-crossing flips. Four pads: [0.25, 0.50], least-norm 0.375. A 0.5 m/s deck carries a held pad 0.50 ± 0.001 m |
| **3 `act`, the boss, the lineup** | Scores, beats, authority, `shock`, `shaped`; `hand` and `loose`; hits as events; five instances in one pass (Keel step 8); the directed rail hop, its free channels solved near keys by the existing Rust `solve` after A's elimination; the lineup on POSE with the render lane's per-kind draws | Library +600 WGSL; `act` +200; TS +150; core +80 in `loft-gpu` | Keel's contract: inputs in, pixels out | The claw arrives at 0.700 s on the same tick with and without a hit at 0.5 s; wind-up 0.69 m at 0.37 s. Δv < R/T_c moves no foot. Antennae at ζ = 0.45 overshoot 20.5%. Two hits 15 ticks apart apply once each; replays bit-identical at 30, 60, 120 Hz. Hitting one of five bosses leaves four POSE buffers bit-identical. Lineup: ≤ 25 pipelines (from 5,538 modules), p50 ≤ 4.3 ms (from 97.2), load ≤ 2 s (from 23.1), judged by eye against the TS lineup |
| **4 Interop** | `clip()` (glTF, BVH, arrays); live `"pose"` input; asynchronous POSE readback; `loft export --gltf` | `src/motion/clip.ts` +500 TS; library +150; core +40 (named-table readback) | Readback never in Keel's frame loop | A captured walk on `human` and on the heron keeps every labelled stance ≤ 0.1 mm on the drawn foot, beside the clip's own slide. Exported keys match POSE within 0.1 mm at 61 samples, ≤ 1 mm between, or split. Disagreeing contact labels listed |
| **5 Any body, crowds, retirement** | Spore's grid, 12 bodies × 6 motions; 1,000 mixed bodies; lowerings kept only if they win (workgroup per creature, closed-form springs, P's span compression, A's dependency cut); F's shooting solve as an experiment; `gait`, `footing`, `replants`, stances, `lookAt`, `lively.ts` weight curves and `pendulum` drives retired by codemod | Library +800; core +100 (emit); −1,500 TS | Lowerings are Rust emit choices | Every truth hold held on every cell, the picture rate reported per cell; 1,000 mixed ≤ 0.2 ms; the workgroup lowering at least halves a hero's span; shooting ≤ 0.3 s. Walking bipeds stay a conjecture until the grid scores them |

Over all phases the core gains about 900 Rust lines, the front door about 1,150 TS, the library about 2,900 WGSL and
the example engines about 450. Lines live in the library and the engines, as `PHILOSOPHY.md` §0 asks.

## 11. Standing of the numbers, and what would change the design

**Standing of the numbers:**

- **Measured** in the repo: the 6.5 µs step; 8.5 to 33 µs per dispatch; the 0.065 ms quantum; the look round's
  0.59 ms frame and 3.1 ms bake; round 4's 0.17 mm neck and 428 m/s³ head jerk; the 7.6 s near-keys solve; the Rust
  lineup and Titan costs (`agent_notes/2026-11-19_lean_rust_draws.md`) and the TS lineup's 4.26 ms.
- **Derived** here: the jump and slam constants, 1.58 and 1.73 rad/s, the four-pad interval, 19.8 µs per lineup
  pipeline, and the operation, lane and byte counts. **Evaluated**: phase 1's motion rows, from the f64 script.
  Nothing has run on a GPU. **Assumed**: the constants marked conjecture in §8. Tokens: `o200k_base` on §1's blocks.

**What would change the design:**

- **Live and baked differ** in bits on one GPU: pin the arithmetic before anything else.
- **The crab reads mechanical** beside panel (c): change the load cap and the lift law, not add parameters.
- **The lineup stays dispatch-bound** after motion is rows: the remaining kernels are draws, and the render lane's
  per-kind rule is the next lever.
- **Laws fall outside the `wgsl` subset** (structs with arrays, bounded loops): no twin verdicts; `loft check` needs a GPU.
- **Two phases of fixes go into one law** without the picture improving: stop and write its failure class.

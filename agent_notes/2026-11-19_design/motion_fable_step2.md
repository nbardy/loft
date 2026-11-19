# Motion, step 2: what we add to get motion right

19 November 2026 · Fable · invention and architecture. Nothing here is built.

Read after `motion_fable_step1.md`. My reading of the task: say what Loft adds so that authored and adaptive motion
are one machinery, show the consumer's code before the library, and predict phase 1 to the number. Numbers marked
*evaluated* come from running this note's own discrete equations in a throwaway f64 script, not from a GPU. Section 10
gives the standing of every number.

## 0. The answer

Step 1 ended with six shared causes of failure. We add one thing per cause.

| Cause (step 1, §5) | What we add |
|---|---|
| Motion is stored after its reasons are discarded | State holds the reasons: the trunk's momentum, what each end is holding (its **pin**), and plans. The pose is computed from them each frame and is never stored. |
| Kinematics and dynamics are two programs | One step function. Direction enters as a prior with an authority per part. Physics is a clamp on what the planted pins can deliver, and the clamp reports. |
| A constraint has no verdict | Each hold has a residual lane and a named giver. A report is measured on a trace and says what gave way, by how much and for how long. |
| The body plan is welded in | A body is data: a trunk and chains, each ending in a foot, a head, a hand or nothing. Laws read it from a table. |
| Dynamics are resolved at the wrong level | Only the trunk (six degrees of freedom) and loose parts are integrated. A contact is an exact fact about position. There is no contact solver and no iteration. |
| Reacting and replaying are exclusive | One step, three homes. A step whose inputs are all scripted is baked and scrubs. A step a host drives is live. Both are the same kernel and give the same bits. |

The motion sub meta engine is **two kernels and three tables** on the existing core. An engine is a WGSL module in
the library, and its `step` function is the schedule. Loft ships two, `roam` (adaptive locomotion) and `act`
(performance), and `act` is `roam` with one law replaced and one added.

## 1. Consumer code first

Seven cases. Tokens are counted with `o200k_base` on the text shown. `shell`, `titan` and `human` are a look and two
bodies defined elsewhere. Every other name is explained in sections 2 to 7.

**1. A crab that scrambles under a player's stick.**

```ts
const legs = [[1.35, 2.8, 6.6], [0.5, 1.1, 7.4], [-0.4, -0.9, 7.3], [-1.3, -2.7, 6.4]]
const crab = body({
  trunk: { size: [4.8, 1.05, 5], height: 2.5 },
  chains: [-1, 1].flatMap((s) => legs.map(([x, fx, fz]) =>
    ({ root: [x, 0.1, 2.5 * s], lengths: [3, 2.9, 1.2], end: { kind: "foot", home: [fx, 0, fz * s] } }))),
})
const scuttle = move(crab, roam(), { go: input("stick", "vec2") })

export default scene({ things: shell(scuttle.trunk, scuttle.tip), ground: plane, cameras })
export const checks = scuttle.holds
```

**2. A heron that tracks a fish while hopping.** The neck has a range. When the fish leaves it, the body turns.

```ts
const heron = body({
  trunk: { size: [1.6, 0.6, 0.5], height: 1.9 },
  chains: [
    ...[-1, 1].map((s) => ({ root: [-0.1, -0.2, 0.18 * s], lengths: [1.05, 1.1], end: { kind: "foot", home: [0, 0, 0.18 * s] } })),
    { root: [0.5, 0.2, 0], lengths: [1.55], turn: [1.9, 0.6], end: { kind: "head", rest: [0.5, 1.18, 0] } },
  ],
})
const bird = move(heron, roam({ lifts: hop }), { go: input("stick", "vec2"), look: input("fish", "vec3") })
```

**3. A boss hit in the middle of an attack.**

```ts
const slam = { clawR: [[0.7, [4, 0, 1.2], [0, -14, 0]], [1.3]] }   // arrive there at 0.7 s moving 14 m/s down; let go at 1.3
const boss = move(titan, act(), {
  at: input("at", "vec3"),                    // the game owns where the boss stands
  score: slam, since: input("attack"),        // seconds since the game began the attack
  hits: events("hit", 4),                     // point and impulse, each applied on its own tick
})
```

**4. A body a model invents.** Five legs and a head on a stalk, with no motion code.

```ts
const spoke = (k: number, r: number) => [r * Math.cos(k * 1.2566), 0, r * Math.sin(k * 1.2566)]
const star = body({
  trunk: { size: [1.2, 0.4, 1.2], height: 0.9 },
  chains: [
    ...[0, 1, 2, 3, 4].map((k) => ({ root: spoke(k, 0.5), lengths: [0.8, 0.9], end: { kind: "foot", home: spoke(k, 1.3) } })),
    { root: [0, 0.2, 0], lengths: [0.7], turn: [3.1, 1.2], end: { kind: "head", rest: [0, 0.6, 0] } },
  ],
})
const walker = move(star, roam(), { go: input("stick", "vec2"), look: input("prey", "vec3") })
```

**5. Replace one law.** This heron starts turning its body at half the neck's range.

```ts
const early = wgsl`
fn turn(c: Creature, b: Body) -> f32 {        // yaw rate the gaze asks of the body
  let neck = bearing(c, b);                   // how far the head is turned from the trunk, radians
  let ease = 0.5 * b.head.turn.x;             // start turning the body at half the neck's range
  return sign(neck) * max(abs(neck) - ease, 0.0) / b.tc;
}`
const bird = move(heron, roam({ lifts: hop, turn: early.turn }), { go: stick, look: fish })
```

**6. Motion from outside in, posed skeletons out.**

```ts
const beats = retarget(tracks("walk.bvh"), human, { LeftFoot: "footL", RightFoot: "footR", Head: "head", Hips: "trunk" })
const walker = move(human, act(), { score: beats, since: time })
export const checks = walker.holds      // what the clip kept, and what gave way to the body
export const out = walker.pose          // joint frames each frame, for a host to read back
```

**7. Replace the whole engine.** A creature that drifts with its limbs hanging, in the manner of Rain World.

```ts
const hover: Engine = {
  state: wgsl`struct Creature { at: vec3f, vel: vec3f }`,
  step: wgsl`
fn step(c: Creature, b: Body, in: Inputs) -> Creature {
  let vel = c.vel + ((in.go - c.vel) * 4.0 + vec3f(0, -9.81, 0)) * dt;
  var at = c.at + vel * dt;
  at.y = max(at.y, b.height);
  return Creature(at, vel);
}`,
  pose: wgsl`fn pose(c: Creature, b: Body, k: u32) -> vec3f { return c.at + b.chain[k].home; }`,   // each tip hangs under its home
}
const drifter = move(crab, hover, { go: input("stick", "vec3") })
```

| Case | Tokens | Lines | The same thing today |
|---|---|---|---|
| 1 Crab scramble: body, motion, scene | 232 (the motion line is 22) | 10 | 2,434 tokens for the path, stepper and scene of the look round, and no player |
| 2 Heron, fish and hop: body, motion | 208 (the motion line is 36) | 8 | 3,957 tokens for the gaze round's two files, on a fixed clock with no fish |
| 3 Boss hit in mid-attack | 139 | 6 | cannot be written |
| 4 Invented body | 235 | 9 | 100 to 700 lines of rig |
| 5 One law replaced | 134 | 7 | edit the scene's expressions |
| 6 Clip in, poses out | 102 | 4 | no import exists |
| 7 Engine replaced | 188 | 12 | not applicable |

What the reader had to learn: `body`, `move`, `roam`, `act`, `events`, `retarget` and `tracks`. Everything else is
TypeScript, WGSL and names Loft already has (`input`, `wgsl`, `scene`). Phase 6 retires most of the 34 exported
functions these replace.

## 2. Architecture

### 2.1 Two kernels and three tables

```text
build:   body plan ──► BODY table (per plan)            score, clip ──► BEATS table
tick:    STATE[n] + BODY + inputs[n] ──step──► STATE[n+1]          a Steps kernel, 120 a second
frame:   STATE[n-1], STATE[n] + BODY ──pose──► POSE table          a Frame kernel
         POSE ──► draws (vertex stage)    POSE ──► a host that reads poses back
```

- **BODY** is the seam between a description and the laws. It has one row per chain and one for the trunk.
- **STATE** is the seam between ticks. It has one row per creature. A row is a record, and a law reads `c.body.vel`,
  not lane 1.
- **POSE** is the seam between motion and everything that draws or exports: the trunk's frame and every joint's
  position.

Each seam is a plain table with a documented layout, so a consumer can stand on either side of one. The pose mixes the
last two states, so the picture is at most one tick (8.3 ms) behind the state. The core already has both kernels
(`When::Steps`, `When::Frame` in `loft-ir`). It lacks six small things, listed in §2.6.

### 2.2 State, and where it lives

State is what cannot be recomputed from the clock: the trunk's place, velocity, attitude and spin, each pin's held
point and swing, each loose chain's offset and rate, and plans. These are the design goal's "plans, momentum, centre
of gravity, contacts, controller phase, targets". All of it is readable in a scene (`scuttle.state.pins[3].load`),
so rendering can use it: dust where a loaded foot lands, a squash from load.

The home follows from what the step reads. `loft show` prints it and the consumer never sets it.

| Home | When | Core mechanism | What it gives |
|---|---|---|---|
| Closed | The part has full authority from a score and no state (a keyed hand) | An expression of time, as today | Proofs by interval; any frame alone |
| Baked | Every input the step reads is scripted (an input's default may be an expression of time) | `Steps` with `Keep::Every` | Scrubbing, any frame alone, training's 16 frames |
| Live | A host drives at least one input | `Steps` with `Keep::Latest`, a per-tick input log and checkpoints | Player control; replay and seek from the log |

One bundle supports baked and live. The same kernel runs in both, so the same inputs give the same states on one
GPU. Phase 1 gates on that equality.

### 2.3 Laws

A law is a pure WGSL function of the previous state: `fn lifts(c: Creature, b: Body, i: u32) -> bool`. An engine's
`step` calls its laws in order, and that order is the priority of the holds. The laws below call the smaller ones a
consumer replaces most often (`strain`, `lifts`, `aim`, `swing`, `turn`, `lean`), which `roam({ ... })` swaps by name.

```wgsl
fn step(c: Creature, b: Body, in: Inputs) -> Creature {
  var n = c;
  let want = intend(c, b, in);          // inputs and score -> what is wanted, with an authority per part
  n.body  = struck(c.body, b, in.hits); // impulses, each on its own tick
  n.pins  = pinned(c, b, want);         // strain, lifts, aim, swing: every foot, head and hand
  n.body  = carried(n, b, want);        // the acceleration wanted, clamped to what the planted pins can give
  n.body  = leashed(n, b);              // no chain is longer than it is: the trunk gives
  n.loose = followed(c, n, b);          // loose chains answer the trunk's acceleration
  n.held  = residuals(n, b, want);      // every hold's margin, kept
  return n;
}
```

Laws are written with `wgsl`, which already lowers a subset of WGSL into the graph. A law inside the subset gets a CPU
twin, so `loft check` can replay a take. A law outside it runs on the GPU only, and its holds are measured there or
reported unknown by name. Decisions are taken from the previous state (Jacobi), so the result does not depend on the
order of invocations. A creature is one invocation that loops over its own pins, so nothing needs colouring, barriers
or atomics.

### 2.4 Holds, givers and verdicts

A hold is a statement with a margin. It is the same `Check` formula Loft has now (`ge`, `equal`, `always`), written
over state. Three things are new.

- **The margin is kept.** `residuals` writes each hold's worst margin for the tick into state. Step 1 found that
  position-based methods compute this number and throw it away.
- **Each hold names its giver.** When a pin and its chain cannot both be satisfied, one of three things yields: the
  trunk (`body`), the pin's own target (`tip`), or the pin (`release`). The giver is body-plan data with a default per
  end. A foot's giver is the trunk: the foot stays and the trunk is held back. A head's giver is its tip: the neck
  keeps its length and the gaze gives.
- **A verdict names its trace.** The verdict type gains one scope, `Trace { ticks, by: Twin | Gpu }`. A live hold is
  "held over 480 ticks of take `scuttle.inputs`", never a bare "holds". Proofs stay where they are possible, on closed
  forms.

The form of a report (the numbers illustrate the form and are not results):

```text
planted    held       slide 0 over 480 ticks x 8 feet (copied, exact)
reach      held       least margin 0.10 m, chain 6 at 2.31 s
gaze       gave way   14.2 deg for 0.22 s from 1.31 s (neck at its range; the body was turning)
balance    held       least load share 0.075; pressure point 5.7 m inside the support
```

`loft check` reads the margins through the twin, or through a GPU readback in a development build. Keel's contract
(inputs in, pixels out) is untouched, because a game never reads them.

### 2.5 The body plan

A body is a trunk and chains. A chain leaves its parent at `root`, has `lengths`, and ends in one of four things.
That end is the sum type every law dispatches on, once.

| End | What it holds | Its re-pin when slack runs out | Default giver |
|---|---|---|---|
| `foot` | A point on the ground, under a `home` | A step | trunk |
| `head` | A gaze, and the skull's place in the world | A thrust of the head, then a turn of the body | tip |
| `hand` | A place a score gives it, at a time | The score's next beat | tip |
| `loose` | Nothing: it follows its root with weight | none | none |

`body()` runs at build. It derives what a model inventing a creature does not know. Masses come from lengths and
girths at the density of water. The trunk's inertia comes from its size. The time constant is Tc = √(height / g).
Each foot's neighbours (the feet that must not swing with it) come from the geometry of the homes. It refuses, by
name and with the fix, a home its chain cannot reach or a head whose rest lies outside its turn range. A wrong mass
shifts the centre of mass a little. It cannot make the motion jitter, because no limb is integrated.

### 2.6 What the core gains

| # | Change | Where | Why |
|---|---|---|---|
| 1 | A live step keeps a per-tick input log and a checkpoint every K ticks | `loft-gpu` (`Keep::Latest`) | Today it rewinds to its init and feeds today's input to yesterday's ticks |
| 2 | An instance owns its live state | `loft-gpu` `instance()` | Today a live kernel is refused, so reactive creatures cannot share a pass |
| 3 | Steps run before the Frame kernels that read them | `loft-core` schedule | Today Frame kernels run first, so the pose would lag a frame |
| 4 | `Scope::Trace` | `loft-core` check | A verdict for stepped state; today it is `Undecided(State)` |
| 5 | Inputs are a sum type: a level, a script (an expression of time), or events (tick-stamped, applied once) | `loft-ir`, recorder | Bake or live from one scene; a hit never applies twice on catch-up |
| 6 | Records in rows | recorder, emit (`Ty::Rec`) | Laws read fields, not lanes |

`recur` keeps its meaning (a step with only scripted reads, baked). Its refusal of host inputs becomes the choice of
home.

### 2.7 How a consumer replaces any part

| Rung | What changes | How | Cost |
|---|---|---|---|
| A number | A length, a strength, a damping | Body data or a law's constant | A table write |
| One law | Stepping, turning, lean, aim | `roam({ lifts: mine })`, case 5 | One module recompiled |
| State | A new field (mood, fatigue) and a law that uses it | Extend `Creature`, pass a `step` that calls the library's laws and yours | A new bundle |
| The schedule | Order, so priority | Your own `step` | The same |
| The engine | Everything | `{ state, step, pose }`, case 7 | The same |
| The seam | Motion computed elsewhere | Any kernel or host that writes POSE | No motion code at all |

Nothing in `roam` or `act` is privileged. They are built from `body`, `move` and `wgsl`, and they are the largest
files in the library on purpose.

### 2.8 Interop

- **In, baked.** `tracks(file)` reads joint tracks. `retarget(tracks, body, map)` turns them into a score: beats for
  the mapped ends in body-relative form (Spore's idea), with contact labels taken from foot speed and height. The
  engine then treats the clip as it treats any direction: a prior. Labelled contacts become pins, so the feet cannot
  slide, and reach and limits hold. `holds` reports where the clip gave way to the body.
- **In, live.** A host writes the same beats into an input table each frame, and the model runs wherever the user
  runs it.
- **Out.** POSE is the posed skeleton. A host that wants it reads the table. glTF export samples it.

A generated clip is never reported as meeting a hold. Only the corrected result is measured.

## 3. The default laws

These are `roam`'s laws, written so that each can be read, predicted and replaced. Units are metres, seconds and
radians. h is the trunk's height, ω = √(g/h), Tc = 1/ω.

### 3.1 The pin

Step 1 observed that a planted foot and a held gaze are one constraint: an end fixed in the world, a chain as slack,
and a re-pin when the slack runs out. The pin makes that a type.

- **Hold.** A planted pin's place is copied from tick to tick, so its slide is exactly 0. Nothing is integrated.
- **Strain** is slack used over slack available. For a foot it is |p − (home + v·Tc)| / R: the foot's distance from
  where the capture point says it should be. For a head it is the neck's turn over its range, or its chord over its
  length.
- **Strain flows to the root; pose flows to the tip.** A pin that nears its limit asks its root to move. A strained
  foot asks for a step. A strained neck asks the trunk to turn, which moves the homes, which strains the feet, which
  step. That is the design goal's sentence "stopping at the neck's limit, then turning the body". It needs the pin
  and one six-line law, `turn`. The default keeps the neck inside a comfortable 0.6 of its range. A fish circling the heron at
  1 rad/s leaves the neck at 0.6 × 1.9 + 1 × 0.44 = 1.58 rad, inside its 1.9, with no gaze error. Past 1.73 rad/s the
  body cannot keep up and `gaze` reports how far it gave.
- **Re-pin.** A foot lifts when it is planted, strained past 1, has been down 0.12 s, and no neighbour is swinging or
  more strained. Gaits are not chosen. The look round's eight feet fell into alternating sets of four from this rule
  alone. `hop` (all feet leave together) is five lines.
- **Swing.** p = from + (aim − from)·m(s) + up·lift·16s²(1−s)², with m the least-jerk profile 10s³ − 15s⁴ + 6s⁵. The
  aim is home + v·(0.25 s + time left). It follows the body until half-way and is committed by s = 0.85. The foot
  lands with zero vertical speed and under 0.1 m/s of horizontal speed. Today's swing lands at π·lift/T, which is
  13 m/s for the look round's fixed step.

### 3.2 The trunk: chosen accelerations, clamped by what the pins can give

The trunk is the only integrated rigid body. Each tick the law chooses an acceleration, the clamp makes it feasible,
and semi-implicit Euler integrates it.

- **Choose.** Horizontally, a = (go − v)/τ with τ = Tc/2. A larger animal answers more slowly because Tc grows as
  √h. Vertically, a plan (§3.3).
- **Clamp.** The pressure point the acceleration needs is p* = c − (h/g)·a, the linear inverted pendulum. It must lie
  in the support of the planted feet. The feet cannot pull (vertical a ≥ −g), cannot slip (horizontal |a| ≤ μg,
  with μ = 0.6 unless the scene says otherwise) and cannot push harder than strength·g. With no foot planted the only
  force is gravity, so a flight is a parabola by construction, not by a hold.
- **Load.** The share each planted foot carries is w_i = 1/n + (p_i − p̄)ᵀ S⁻¹ (p* − p̄), with S the feet's 2×2
  scatter. This is the least-norm answer to "shares sum to 1 and balance about p*", in closed form. A negative share
  means the foot would have to pull, and that is the `balance` hold's margin. From phase 2 a foot lifts only once its
  share is small, so the trunk shifts its weight off a foot before the foot leaves. For a biped that is the whole
  sideways sway. For a crab it is a few percent. One rule gives the right size on every body.
- **Lean.** The attitude is a spring (2 Hz, ζ = 0.7, integrated exactly) toward atan(a/g) about the axis across the
  acceleration.
- **Leash.** If a planted chain would be longer than 0.97 of its lengths, the trunk is moved back and loses the
  outward speed. The leg is never longer than it is and the foot never slides.
- **Hit.** An impulse J at point r on tick n gives Δv = J/m and Δspin = I⁻¹((r − c) × J). Nothing else is special. The
  capture point jumps by Δv·Tc, so feet step if that passes R. The crab absorbs a hit below R/Tc = 2.6 m/s without
  moving a foot and staggers above it. A second hit adds to whatever state the first left.

### 3.3 Plans: anticipation is a boundary condition

A move with a deadline is stated by where it must arrive, how fast, and when: (D, v₁, T). The least-jerk quintic from
rest to that arrival is

  p(s) = D(10s³ − 15s⁴ + 6s⁵) + V(−4s³ + 7s⁴ − 3s⁵),  V = v₁T,  s = t/T.

Its first coefficient is 10D − 4V. **The motion goes backward first exactly when V > 2.5 D**, that is when the
arrival speed is more than 2.5 times the mean speed. No one asks for the wind-up. A strike that must arrive at five
times its mean speed pulls back 0.29 of its travel at 54% of its time. `anticipated(u, 1.7)` in `lively.ts` is the
same shape with the cause removed.

For a jump (D ≈ 0) the lowest acceleration is −1.949 v₁/T. Feet cannot pull, so it must stay above −g. That fixes the
shortest stance, and with it the rest:

- stance T = 1.949 v₁/g; crouch 0.385 v₁²/g at two thirds of the stance; peak push 2.02 g, whatever the jump.

For the heron's hop (v₁ = 3.876 m/s) this gives a 0.77 s stance and a **0.59 m crouch** at 0.51 s. The animator keyed
0.60 m, and the solve near those keys (162 unknowns, 7.6 s) returned 0.60 m. The keys put it at 0.35 s, which no body
standing on the ground can do: free fall alone takes 0.35 s to drop 0.60 m and arrives still moving. With a 2.02 g
push the soonest is 0.43 s. A creature with less strength than 2.02 g takes longer, T = 3.94 v₁/(strength·g), and
crouches deeper. One number per body, `strength`, sets the timing of every
jump it makes. Exaggeration becomes a gain on a physical quantity, as step 1 asked.

A plan is state: its end conditions and start tick. It is evaluated in closed form and replanned only on an event
(a hit, a new aim, a clamp). I tested replanning from the current state every tick. With explicit integration it
loses 5% of the take-off speed and diverges near the deadline, so plans are not built that way.

### 3.4 Loose chains

A loose chain is a damped spring driven by its root's actual acceleration. Its rate comes from length and gravity
(the rule in `pendulum.ts`). The step is the exact solution of the linear equation over one tick, a 2×2 matrix
computed at build, so it is stable at any stiffness. Follow-through and overlap are then caused by what happened to
the parent, including a hit, and are not typed at each call site.

## 4. Two example engines

**`roam`: adaptive locomotion, live.** It is the laws of section 3 with `intend` reading `go` (a wanted velocity) or
`at` (a place the game owns, which the trunk follows on a leash), plus `face`, `look` and `hits`. It has no cycle,
clip or gait table. It adapts to a body because every law reads BODY. It adapts to the player because stepping reads
strain, and strain reads the velocity now.

**`act`: performance.** It is `roam` with `intend` replaced and `shaped` added.

- `intend` reads a **score**: per chain, beats of `[time, place, arrival velocity]`. A pin with a beat arrives on the
  beat's tick exactly, because a swing's arrival is a parameter and not an outcome. "Lands at 1.2 s" is therefore
  true by construction for ends. The trunk gets beats too, as priors: it follows them with authority and inside the
  clamp, and `holds` says where it could not ("trunk behind its keys through the crouch: 0.60 m by 0.35 s needs the
  feet to pull"). A model author learns from that sentence what was wrong with its keys.
- `shaped` adds squash from load and stretch from speed, both from state.
- A hit lowers authority for about Tc (`shock`), so the body recoils, steps and whips its loose parts while the claw
  still arrives on its beat. When authority returns the performance continues from the state the hit left. There is no
  switch to a ragdoll and no get-up clip.

| Disney principle | Where it comes from in `act` |
|---|---|
| Anticipation | The plan's boundary condition (§3.3), and weight leaving a foot before it lifts |
| Slow in and out | Least-jerk swings and bounded acceleration |
| Follow-through, overlap | Loose chains driven by the parent's acceleration |
| Squash and stretch | Load share and speed |
| Arcs | Swings about a fixed root; ballistic flight |
| Timing | Exact at beats for ends; Tc and strength for the trunk |
| Exaggeration | Gains on impulse, strength and gravity, not on poses |
| Staging, appeal, solid drawing | The author's |

**Precompute.** A score with no host input bakes at load and scrubs. Holds that beats cannot state (clear this rail,
land on that deck) are solved at build by **shooting through the same step**. The unknowns are a few beat numbers,
not spline controls. The candidates are rows of one STATE table, so a generation of 256 candidates over 1.8 s is one
bake. Every candidate is physical by construction, and its holds are scored by the same residual lanes.

## 5. Inventions and ambition

What I could not find in the prior work step 1 surveyed. I did not search beyond that survey today.

1. **The pin**: one typed constraint for foot, gaze and hand, with strain, a giver and a re-pin, and with strain
   flowing to the root as the architecture of whole-body reaction.
2. **Chosen acceleration, clamped by pins**, as the whole of the physics: exact contacts, six integrated degrees of
   freedom, no solver, stable at any step.
3. **One step, three homes**, with live and baked bit-equal as a gate, and a baked take and a live run sharing one
   state record so one continues the other.
4. **Verdicts on live motion**: kept margins, givers, and a trace scope.
5. **Direction as a prior with authority, physics as a clamp that reports**, which turns "the solve fought my keys"
   into a sentence that names the keys.
6. **Anticipation from boundary conditions**, with its constants: V > 2.5 D; 1.949 v/g; 0.385 v²/g; 2.02 g; and the
   stagger threshold R/Tc.
7. **The solver as a crowd**: shooting through the live step with candidates as instance rows.
8. **An engine as a WGSL module** between three table seams, with its schedule as an ordinary function.

Where we aim past the state of the art:

- **Speed.** About 0.35 µs of GPU per creature per frame at the margin: a thousand adaptive creatures in under half a
  millisecond, and one creature below the profiler's 0.065 ms tick. Motion compiles to two modules whatever the
  number or kind of bodies. Spore reported 0.2 ms of a 2008 CPU for one creature. Simulated characters step 8 to 20
  times a frame.
- **Developer experience.** A law is one function and replacing it recompiles one module. A new body needs no code. A
  report tells the author, in the author's own terms, what gave way. A session is a file of inputs that replays.
- **Capability.** A choreographed attack and a physical reaction in the same body on the same tick. A head that
  leads and a body that follows. Any leg count. The same scene baked for a film or live for a game.

## 6. Performance theory, to the number

**State.** `roam`'s record is 89 lanes of 16 B, 1,424 B per creature: trunk 4, plan 4, sixteen pins of 4, eight loose
chains of 2, one lane of residuals. A 4 s clip keeps 481 states, 0.68 MB. Trimmed to the crab (eight pins, no loose
chains) it is 41 lanes, 656 B. Trimming is an emit choice.

**Work per crab per tick**, counted from the laws: strain and homes 240 flops, support and loads 250 to 600, lift
decisions 240, swings 400 for up to four in the air, trunk and lean 180, leash 640 for two passes, followers 120,
residuals 60. About **2,500 flops** and 2 × 89 lane transfers. The pose is analytic IK for ten chains and a mix of two
states, about **1,500 flops** and 46 lanes written.

**Per frame** at 60 Hz with 120 ticks a second there are three dispatches, two steps and one pose.

| Creatures | Arithmetic | Memory moved | Predicted GPU time | Basis |
|---|---|---|---|---|
| 1 | 6.5 kflop | 9.3 KB | 0.02 ms, below one tick of the GPU clock | Three dispatches at the measured 6.5 µs a step (480 steps baked in 3.1 ms) |
| 5 | 33 kflop | 46 KB | 0.02 ms | The same: dispatch cost dominates |
| 1,000 | 6.5 Mflop | 9.3 MB | 0.35 ms | 9.3 MB at an assumed 30 GB/s effective, plus 6.5 Mflop at an assumed 150 Gflop/s |

The marginal cost is 0.35 µs per creature per frame, set by memory. Trimming the record halves it. The look round
already measured that reading stepped feet (0.131 ms of pose stages) costs less than evaluating `gait`'s closed form
(0.197 ms).

**Precompute.**

| Work | When | Cost |
|---|---|---|
| `body()`: tables, masses, neighbours, reach refusals, spring matrices | build, CPU | Tens of microseconds; under 2 KB per plan |
| `retarget`: clip to beats | build, CPU | Linear in samples |
| Bake of a scripted take | load, GPU | 480 ticks at no more than 10 µs: 5 ms for 4 s, for 1 to 100 creatures |
| Shooting solve | build, GPU | 216 ticks × (10 µs + 256 × 0.15 µs) ≈ 10 ms a generation; 20 generations ≈ 0.25 s, against 7.6 s (TS) or about 1.2 s (Rust) for the spline solve |
| Checkpoint, live | every 60 ticks | One 1,424 B copy: 2.8 KB/s per creature, and a seek replays at most 59 ticks (0.6 ms) |

**Load.** There are two compute modules for motion. The look round's two kernels took 309 ms and 895 ms of pipeline
setup on the TS host, so the prediction is at most 1.2 s cold for both, once per engine and not per creature. The
305-module, 55 s load for five crabs comes from bodies being code. With POSE, the five differ only in table rows, so
the draws are those of one crab drawn five times: 61 modules and about 11 s at 0.18 s a module (55 s over 305) on
today's emitter. The compact-graph lane takes it from there. That lane owns the number. Motion's part is to make
instances data.

**Training.** The budget is 512² × 16 frames at 50 ms a frame with a 2 s build. A scripted take bakes once (5 ms),
and each frame reads rows. Motion adds no per-frame stepping there.

## 7. The data model and types

```ts
type Body  = { trunk: Trunk; chains: readonly Chain[] }
type Trunk = { size: Vec3; height: number; mass?: number; strength?: number }       // strength in g; default 2.02
type Chain = { name?: string; on?: string; root: Vec3; lengths: readonly number[]; girth?: number;
               bend?: Vec3; turn?: readonly [yaw: number, pitch: number]; gives?: Giver; end: End }
type End   = { kind: "foot"; home: Vec3 } | { kind: "head"; rest: Vec3 } | { kind: "hand"; rest: Vec3 }
           | { kind: "loose"; damping?: number }
type Giver = "body" | "tip" | "release"

type Engine = { state: WgslModule; step: WgslFn; pose: WgslFn; holds?: readonly Check[] }
type Beat   = readonly [time: number, place?: Vec3, velocity?: Vec3]                 // time alone: let go
type Score  = Readonly<Record<string, readonly Beat[]>>                             // chain name, or "trunk"
type Home   = { kind: "closed" } | { kind: "baked"; kept: Kept } | { kind: "live"; checkpoint: number }

type Moved = {
  trunk: Frame; tip(k: number | string): P3; joints(k: number | string): readonly P3[]; spine(k: number | string): (m: F) => P3
  state: Creature        // every field, as expressions
  pose: Table            // the seam: drawn, or read back
  holds: readonly Check[]; home: Home
}
declare function body(plan: Body): BodyTable
declare function move(body: BodyTable, engine: Engine, inputs: Record<string, In>, count?: number): Moved
```

```wgsl
struct Carriage { at: vec3f, mass: f32,  vel: vec3f, shock: f32,  turn: vec4f,  spin: vec3f, tick: f32 }     // 4 lanes
struct Pin      { at: vec3f, mode: f32,  from: vec3f, s: f32,  aim: vec3f, seconds: f32,
                  held: f32, lift: f32, load: f32, gave: f32 }                                              // 4 lanes
struct Loose    { off: vec3f, gave: f32,  vel: vec3f, pad: f32 }                                            // 2 lanes
struct Plan     { from: vec3f, tick: f32,  vel: vec3f, seconds: f32,  to: vec3f, a: f32,  speed: vec3f, b: f32 } // 4 lanes
struct Creature { body: Carriage, plan: Plan, pins: array<Pin, 16>, loose: array<Loose, 8>, held: vec4f }
```

```rust
pub enum Keep   { Latest { checkpoint: u32 }, Every(u32) }
pub enum Input  { Level, Script(NodeId), Events { slots: u8 } }
pub enum Scope  { Proved, Sampled { n: u32 }, Trace { ticks: u32, by: Tracer } }
pub enum Tracer { Twin, Gpu }
```

`Pin.mode` is the one runtime tag: planted, swinging or scored. `pinned` dispatches on it once, and each handler
assumes its mode. Which inputs a scene provides is classified in `move`: a law for an absent input (`look`, `hits`) is
left out of the module, not skipped at run time. A body over an engine's capacity is refused with the fix. Persisted
data gains nothing but the two enums' variants. A bundle is still one format.

## 8. Inherited and ours

| Inherited (recalled unless noted; §10) | Used for |
|---|---|
| Linear inverted pendulum and capture point (Kajita; Pratt) | Tc, strain, the pressure point |
| Raibert's hopping: thrust, foot placement and attitude as three small laws | The decomposition in §3.2 |
| Least jerk (Flash and Hogan) | Swings and plans |
| Neighbour inhibition between legs (Cruse's rules for insects) | `lifts` |
| Least-norm force distribution; task priority | Loads; the order of laws |
| Position as primary state (Jakobsen, PBD); exact integration of a linear spring | Pins; loose chains |
| Lockstep with an input log and checkpoints | The live home |
| Shooting and population search for controllers | The build solve |
| Spore: ends by role, motion relative to the body, the grid of bodies × motions | `End`, `retarget`, the phase 4 gate |
| Rain World, Overgrowth, Euphoria: a small proxy leads, lean from acceleration, laws on a driven body | The level of the dynamics |
| The pigeon's hold and thrust (Troje and Frost); a comfortable range for the head | The head pin, `turn` |
| Loft: `recur`, `Steps` kernels, `wgsl`, holds and verdicts, `solve`, rate from length and gravity, typed refusals, the look rounds' stepper | The substrate |

**Ours:** the eight inventions of section 5, the derived constants of §3.3 and the load-gated lift as a rule for
every leg count. Each ingredient above is old. What is new is the choice of what to integrate and what to pin, the
single constraint type, and making truth and replaceability the interface.

## 9. Phased plan from the current code

Constraints from the repository: the TS engine is frozen, and the Rust path builds and loads but draws nothing yet.
So phase 1 has a picture half on the TS engine using only what it has (a raw `compute` steps kernel with the script
as a function of the tick count held in state), and a live half on the Rust host with no window.

### Phase 1: the crab lives

**Build.** `body()` and the BODY table. `move()` assembling the step and pose modules. `roam` with `intend`,
`pinned`, `carried`, `leashed` and `residuals`: no head, no hits, no loose chains. Core changes 1, 3, 4 and 5 of
§2.6. A headless Rust harness that feeds a script through `set_input` each tick and reads STATE back.

**Scene.** The look round's crab and camera, 4 s, 120 ticks a second. The stick is +1.2 m/s from 0.2 to 1.7 s,
−2.1 m/s from 1.7 to 3.2 s, then 0. τ = Tc/2 = 0.252 s, friction 0.6, R = 1.3 (0.18 of the leg's span), swings
shaped by need as in round 4. Shown beside the round's panel (c).

**Predicted** (evaluated from the equations of §3 at f64; an f32 threshold may move a lift by one tick):

| Check | Predicted | Pass |
|---|---|---|
| Live against baked, both on the Rust host, same GPU and script | 0 of 481 states differ in any bit | 0 |
| Planted slide; neighbours swinging together | exactly 0; 0 | 0; 0 |
| Steps | 21, per foot 2, 3, 2, 3, 3, 2, 3, 3 | 20 to 22 |
| First lift | 1.033 s (foot 0); all eight first lifts by 1.625 s | ±2 ticks |
| Reversal | stick flips at 1.700 s; v crosses 0 at 1.900 s; next lift 2.250 s | ±3 ticks |
| Peak acceleration | 5.886 m/s² (the friction clamp), first reached at 1.70 s and held 0.31 s | exact; ±2 ticks |
| Most lean | 32.0° at 2.025 s (target 30.96°, the spring overshoots) | ±0.5°, ±2 ticks |
| Reach margin; leash engaged | at least 0.10 m; never | margin > 0 |
| Least load share; pressure point | 0.075; 5.7 m inside the support's sideways extent | all shares > 0 |
| Feet planted, fewest | 4 | at least 4 |
| Foot's travel in its last tick before landing | at most 0.6 mm horizontal, 0 vertical (the look round's half-sine swings: 54 to 109 mm) | under 1 mm |
| Swing seconds; lift | 0.120 to 0.342; 0.25 to 1.05 | within 0.01 |
| At 4.0 s | z = −1.08 m, v = −0.09 m/s | ±0.02 |
| State; kept clip | 1,424 B per creature; 0.68 MB | exact |
| GPU, one live crab | frame within two clock ticks of the stepper panel's 0.59 ms; 480 batched steps in at most 5 ms | at most 0.72 ms; at most 5 ms |
| GPU, 1,000 crabs, step and pose | 0.35 ms a frame | at most 0.5 ms |
| Motion modules | 2, for 1 or 1,000 crabs | 2 |
| Author's file | 232 tokens | at most 250 |
| The picture | Reads as weight at least as well as panel (c), and the reversal reads as braking on planted feet | The owner, side by side |

A miss fixes the theory first. If the picture loses to panel (c), the phase stops and the laws are re-derived before
anything else is built.

### Later phases

| Phase | Adds | Scored by |
|---|---|---|
| 2 Head, hop, hit | The head pin and the S-neck pose (round 4's closed form); `turn`; plans; `hop`; `struck`; loose chains; the load-gated lift; core changes 2 and 6 | Heron: stance 0.770 s, crouch 0.59 m at 0.513 s, push 2.02 g at 0.583 s, flight residual 0, neck length error at most 0.2 mm; fish at 1 rad/s: neck settles at 1.58 rad, gaze error 0; at 3 rad/s `gaze` gives and says so. Crab: a 2 m/s hit moves no foot, a 3 m/s hit does |
| 3 `act` | Scores, beats, authority, `shaped`; the shooting solve; the Titan's slam, hit in mid-swing | The claw arrives on its tick with and without the hit; the wind-up matches §3.3 within 2%; the solve takes at most 0.3 s |
| 4 Any body | The grid: 12 bodies (one to eight legs, with and without head and tail) × 6 motions, baked and scored by verdict, then by eye | Every truth hold held on every cell; the picture pass rate reported, cell by cell |
| 5 Interop, crowds | `tracks`, `retarget`, a live beats table, POSE read-back, 1,000 mixed bodies in one dispatch | A corrected clip with slide 0; 0.5 ms for 1,000 |
| 6 Retire | `gait`, `footing`, `replants`, `carriage` and the stances, `lookAt`, the weight curves of `lively.ts`, the `pendulum` drives. Their six files export 34 functions and 24 types; most become recipes inside an engine or are deleted, with a codemod | The corpus's pictures by parity |

Bipeds that walk, not hop, are the hard case. A single small support makes the pendulum unstable between steps, and
the trunk must be steered through its divergent part. The laws are known (the capture-point literature). Their look
on Loft's bodies is a conjecture until phase 4 scores it.

## 10. Standing of the numbers, and what would change my mind

- **Measured in the look rounds** (`agent_notes/2026-11-19_motion.md` on `feat/motion`): the 6.5 µs step, 0.131 and
  0.197 ms pose stages, 0.59 ms frame, 309 and 895 ms pipeline setups, the keyed 0.60 m crouch, the 7.6 s solve.
  From `CORE.md`: 305 modules and 55 s for five crabs.
- **Derived here, by hand:** the quintic's coefficients and the 2.5 D threshold; 1.949, 0.385, 2.02 and the 0.29
  pull-back; the 0.35 s and 0.43 s bounds on the keyed crouch; the least-norm load share; R/Tc; the neck's 1.58 rad;
  lane, byte and flop counts.
- **Evaluated here** (the discrete laws at f64 in a scratch script, about 100 lines, not kept in the repo): every
  phase 1 motion number, and the failure of per-tick replanning. They are predictions and nothing ran on a GPU.
- **Assumed:** 30 GB/s effective memory rate and 150 Gflop/s for branchy scalar WGSL on this GPU. The 1,000-crab
  figure rests on both.
- **Recalled, not re-checked today:** every prior-art attribution in section 8.
- **Token counts:** `o200k_base` on the code blocks of section 1 as written, and on the look round's files with their
  readout checks removed.

What would change the design:

- If live and baked differ on one GPU, the three-homes claim is wrong as built and the kernel paths must be unified
  before anything else.
- If WGSL laws fall outside the lowered subset (structs with arrays, bounded loops), twin verdicts are unavailable.
  The design then leans on GPU-measured margins, and `loft check` on a take needs a GPU.
- If the crab's picture is no better than panel (c), the trunk's laws are too tame for a wide body, and the next
  move is the load-gated lift and weight shift, not more parameters.
- If two phases of fixes go into one law without the picture improving, stop and write that law's failure class
  before a third.

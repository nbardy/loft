# Motion, step 1: where engines fall short of the goals, Loft included

19 November 2026 · critique only · judged against `docs/PHILOSOPHY.md` and `docs/design/motion.md`

## The finding

No system, Loft included, meets the motion goal, and they miss it for the same six reasons:

1. **The shared currency is a pose.** Every pipeline passes joint rotations per frame between its stages. A pose
   carries no mass, no contact and no intent, so each later stage has to guess what the earlier one meant.
2. **The parts are divided by technique.** Clip sampler, state machine, IK, ragdoll, look-at and "secondary motion"
   are separate subsystems with separate state and separate clocks. Their only way to combine is to blend poses, and
   a blend of two poses that each hold a constraint holds neither.
3. **A body is an asset or a program.** It is a set of binary editor files, or a few hundred lines of bespoke code.
   Laws are written against that one body, so nothing transfers to the next.
4. **A law is a black box or a tangle.** It is a trained weight blob, a vendor's closed behaviour, or a hand-tuned
   state machine. None can be replaced one law at a time.
5. **Nothing reports the truth about a constraint.** "Foot locked" is a flag, a reach clamp is silent, and a
   penetration is a tolerance. No engine says: held, missed by 3 mm, or unknown.
6. **None of it runs on the GPU in the frame.** Controllers are branchy per-object CPU code. GPU physics exists for
   training farms, on a different device from the one that draws.

Loft today sits at one extreme. It has only closed forms of time: cheap, seekable and partly provable, but with no
mass, no state, no body description and no reaction. The engines sit at the other: reactive, but opaque, CPU-bound and
never truthful. The goal asks for both ends in one system, and nobody has built that.

Sections 1 to 3 give the evidence. Section 4 lists what to throw out and what to keep. Section 5 lists the questions
this leaves for step 2.

**How to read the numbers.** `[L]` was read in the Loft repository today. `[V]` was checked against a source today
(listed at the end). `[R]` is recalled from the literature and not rechecked. `[E]` is my estimate, with the
arithmetic shown.

**One broken reference.** `docs/design/motion.md` names `agent_notes/2026-11-19_motion.md` and "the look rounds
(heron hop, crab walk, crab scramble)" as evidence. That file does not exist. The proposal is at
`docs/proposals/2026-11-19_motion.md`, and I found no note recording the look rounds. This critique therefore rests
on the code, the proposal and the Astra audit, and on no picture comparison.

## 1. The yardstick

The design goal reduces to eight tests. A system passes a test only if it does the thing without a rewrite.

| # | Test | What passing looks like |
|---|---|---|
| T1 | Any body | A new body plan moves well from its description alone |
| T2 | Reactive | Direction, target or a hit changes this frame; no prebaked cycle |
| T3 | Directable | "Lands at 1.2 s", a held pose, an exaggerated wind-up, to the frame |
| T4 | One machinery | Authored and simulated blend per part and per moment without breaking constraints |
| T5 | Truth | Every stated constraint is reported held, missed by a measured amount, or unknown |
| T6 | GPU per frame | Many creatures per frame on the drawing device, at a fixed cost |
| T7 | Replaceable | Any single law, constraint or state swapped by the consumer; the default is not privileged |
| T8 | Tokens | A model writes the creature and its behaviour as short text and reads it back cold |

Scores are my judgement from the sections below: yes, part, or no.

| System | T1 | T2 | T3 | T4 | T5 | T6 | T7 | T8 |
|---|---|---|---|---|---|---|---|---|
| Unreal (AnimBP, Control Rig, FBIK, Pose Search) | no | part | yes | no | no | no | part | no |
| Motion matching | no | yes | part | no | no | no | no | no |
| Learned kinematic generators (MotionBricks class) | no | yes | part | no | no | part | no | part |
| DeepMimic, AMP, ASE, SuperTrack | no | yes | part | no | part | no | no | no |
| Euphoria | no | yes | no | part | no | no | no | no |
| Spore | yes | part | yes | no | no | no | no | no |
| Overgrowth | no | yes | part | part | no | no | part | part |
| Rain World | part | yes | no | no | no | no | part | no |
| MuJoCo | yes | n/a | n/a | n/a | part | no | part | part |
| PBD, XPBD, Jakobsen | yes | yes | no | no | no | part | yes | yes |
| Spacetime constraints | yes | no | yes | no | part | no | yes | part |
| Loft today | no | no | yes | no | part | part | part | part |

No row has more than four passes. The columns that fail hardest are T4 (one machinery) and T5 (truth). No system
passes either, and they are the two the design goal treats as the centre.

## 2. Prior art, one family at a time

### 2.1 Clip engines: Unreal and Unity

**What they do.** A character is an Animation Blueprint: state machines and blend spaces that sample clips, then IK,
a Control Rig and sometimes a physics asset. Unreal 5 added full-body IK, an IK Retargeter and Pose Search (motion
matching).

**What is wrong, against the goals.**

- **Six subsystems, six languages.** State machine, blend space, Control Rig graph, IK Rig, Physics Asset and Pose
  Search each have their own asset type, editor and notion of state. The philosophy asks for "state, laws and
  constraints, evaluated somewhere". Unreal has six somewheres.
- **Blending is the only composition, and it breaks constraints.** A transition between two clips interpolates joint
  rotations. Feet planted in both clips slide during the blend. Foot IK then pulls them back, and the knee pops.
  Each stage repairs the damage of the one before.
- **Transitions grow as the square of the states.** A state machine with n states has up to n(n − 1) transitions,
  each authored. This is the cost that pushed studios to motion matching.
- **Gaze is a clamped aim plus a threshold.** An aim offset blends authored poses within about ±90° of yaw. Past a
  threshold a state machine plays an authored turn-in-place clip `[R]`. The neck's reach, the eyes' share and the
  body's decision to turn are three unrelated mechanisms. The owner's example (track to the right while turning left,
  stop at the neck's limit, then turn the body) is exactly the case this handles by a hand-tuned threshold.
- **Bodies do not transfer.** Unity's humanoid retargeting requires a mapped human skeleton of at least 15 bones
  `[R]`. Generic rigs do not retarget. Unreal retargets between chains a person maps by hand. A crab is new work.
- **It is CPU work per object.** The graph is evaluated per character on worker threads. A full-quality character
  costs on the order of 0.2 to 1 ms of CPU `[E, from published budgets, not measured here]`, so a 4 ms animation
  budget buys roughly 4 to 20. Engines hide this with update-rate reduction and budget allocators. Crowds use
  vertex-animation textures: thousands of characters, but baked, with no IK and no reaction.
- **A model cannot write it.** An Animation Blueprint is a binary asset edited in a GUI. T8 is not low, it is
  undefined.

**Why it happened.** The clip (keyframed or captured) was the unit of value, so everything else became a correction
applied after it. Each correction shipped as a new subsystem because the old ones could not change under shipped
games.

**Throw out:** the pose as the interchange; blend-of-poses as composition; the state machine as the unit of
behaviour; per-skeleton assets; threshold-triggered turn clips.
**Keep:** the animator's right to direct to the frame; chain-level naming of limbs (spine, leg, arm) as a retarget
vocabulary.

### 2.2 Motion matching and learned kinematic generators

**What they do.** Motion matching (Büttner 2015; Clavet 2016, *For Honor*) keeps a database of captured motion.
Every few frames it searches for the frame whose features best match the current pose and the desired future path,
then jumps there with a short blend. A common feature vector has 27 numbers: foot positions and velocities, hip
velocity, and future trajectory points `[R]`. Learned Motion Matching (Holden et al. 2020) replaces the database with
three networks to cut memory. Learned generators go further. NVIDIA's MotionBricks (SIGGRAPH 2026) trains one model
on over 350,000 clips and reports about 2 ms latency. Its output is kinematic, and its released weights cover a human
skeleton and one humanoid robot `[V]`.

**What is wrong.**

- **The body is fixed by the data.** HumanML3D, the standard text-to-motion set, is 14,616 human motions, 28.59
  hours `[V]`. There is no such set for a crab, a heron or a creature a model invents an hour ago. T1 fails by
  construction, and it fails for the most valuable case.
- **The output is a pose, so physics is missing again.** Generators float, penetrate and skate. PhysDiff's fix, a
  physics projection inside the diffusion loop, cut its physical-error measure by more than 86% against the base
  model `[V]`. That is the size of the error the base carries.
- **Contact is hidden, not held.** Motion matching's jumps are smoothed by inertialisation and repaired by foot IK.
  No number says how far a planted foot moved.
- **The law is the database or the weights.** To change how the creature turns you capture new turns or retrain.
  There is no "replace the stepping law".
- **Coverage is combinatorial.** Unreal's own sample project ships over 500 animations for one unarmed humanoid
  `[R]`. Adding a weapon or an injury multiplies it.

**Why it happened.** Capture became cheaper than animators, and search cheaper than authoring transitions.

**Cost.** Capability on a non-human body is zero. A production database runs to hundreds of megabytes `[R]`, searched
on the CPU. The content is hours of data, not tokens.

**Throw out:** the idea that quality comes from stored performance of the target body.
**Keep:** two ideas. First, the query: "current state plus desired future path" is the right interface between
intent and motion. Second, generators as a **source** the consumer runs elsewhere (the design's interop line): their
kinematic output is a target to be corrected by constraints, never a result to be trusted.

### 2.3 Physics-tracking controllers: DeepMimic, AMP, ASE, SuperTrack

**What they do.** A policy network drives joint motors in a rigid-body simulator so the character imitates reference
motion while staying upright. DeepMimic tracks one clip. AMP replaces tracking with a learned style critic. ASE
learns a reusable skill space. SuperTrack learns a world model and trains through it.

**What is wrong.**

- **Training cost is per body.** DeepMimic needs about 60 million samples and two days on an eight-core machine per
  skill `[V]`. ASE used over 10 billion samples (about 10 simulated years, 10 days on one GPU) from 187 clips
  totalling 30 minutes `[V]`. A changed limb length means training again. "Adapts instantly" is missed by five to
  six orders of magnitude in time.
- **It still needs reference motion of that body.** The style comes from clips. No clips exist for an invented
  creature.
- **The law is a weight blob.** A consumer cannot open the policy and replace the balance law. T7 fails completely.
- **It cannot be directed to the frame.** A reward shapes tendencies. "Lands at 1.2 s on that plank" is not a thing
  a policy promises.
- **The simulator is not in the frame.** The policy runs at about 30 Hz over a simulator at 120 Hz or more
  `[V for ASE]`. The fast simulators are CUDA or JAX batch engines for training, not the renderer's device.

**Why it happened.** Hand-written balance controllers (SIMBICON, 2007) were brittle and biped-only, and reinforcement
learning removed the hand-tuning by spending compute.

**What it gets right.** It is the only family where weight, recovery and contact are real, because the motion solves
dynamics with contact. The missing ingredient elsewhere is dynamics, not data.

**Throw out:** learned monolithic policies as the engine's law; per-body training as the way to adapt.
**Keep:** the structure underneath: a low-rate intent, a high-rate tracking law with bounded actuation, and contact
that is solved, not painted.

### 2.4 Euphoria

**What it does.** An active ragdoll with hand-engineered behaviours (stagger, brace, reach for the wound), blended
with keyframed animation. It shipped in *GTA IV* and *Red Dead Redemption*.

**What is wrong.** It is the clearest case of a baked engine. Behaviours were closed modules tuned by the vendor's
engineers. It was humanoid first, on the CPU, for a handful of characters. It handled reactions only: locomotion
stayed on clips, so the physical and authored halves were two systems with a hand-off. No studio could rewrite one
behaviour, which is why the technique left with the vendor.

**Why it happened.** Balance and reaction controllers took specialist years to tune, so they were sold as a service.

**Throw out:** behaviour as a vendor module; the ragdoll as a separate mode entered on a hit.
**Keep:** the proof that bounded muscle drives over a simulated body read as alive in a way no clip does, and that
the valuable moment is the unplanned one (the second hit before recovery, as the Astra audit also concludes).

### 2.5 Spore

**What it does.** Hecker et al. (2008) author each animation once in a body-independent form. Channels are tagged by
meaning (move this grasper relative to the ground, to the body, to a target). At run time the motion is specialised
to the player's creature and solved by a custom IK solver. Gaits are procedural.

**What is wrong.**

- **It is kinematic.** There is no mass, momentum or balance. A top-heavy creature and a squat one walk with the
  same weight. This is why Spore creatures read as puppets.
- **Generality was paid for by animators.** The paper's own discussion says it takes weeks to build an intuition for
  which motions generalise across bodies and which do not `[V, quoted second-hand]`. The transfer lives in a
  person's head, not in the system.
- **The solver is one ad hoc monolith.** Its priorities and failure cases are fixed inside it. A consumer cannot
  state a new constraint.
- **Nothing is reported.** When a creature cannot do the motion, it does something else, silently.

**Why it happened.** It had to run for any player creature on 2008 CPUs, so dynamics were dropped and semantics were
attached to hand-authored curves.

**Throw out:** kinematic retargeting as the core; the monolithic solver; animator intuition as the transfer function.
**Keep:** the best idea in the field for T1: **describe motion in terms of what body parts are for** (foot, grasper,
mouth, eye, spine) and in which frame a movement is meant (ground, body, target). A body description that names
roles is what lets one law drive a crab and a heron.

### 2.6 Overgrowth and Rain World

**What they do.** Overgrowth (Rosen, GDC 2014) drives a rabbit fighter from very few key poses (reported as 13
`[V, second-hand]`), interpolated by springs, with stride matched to distance travelled, lean from acceleration, and
an active ragdoll. Rain World (Jakobsson, GDC 2016) simulates each creature as a few point masses joined by distance
constraints in 2D, moved by its AI, with the drawn body painted over them.

**What is wrong.** Both are closest in spirit and both stop at one game.

- **The laws are right and the packaging is a codebase.** "Lean with acceleration" and "step when the foot is too far
  behind" live in Overgrowth's code for one biped. A second body plan means a second controller.
- **Rain World's creatures are bespoke programs,** each its own file of tuned rules. The motion emerges and cannot
  be directed to a beat.
- **Neither states or checks a constraint,** and neither runs on the GPU.

**Why it happened.** Small teams wrote what one game needed.

**Throw out:** the controller as game code tied to one body.
**Keep:** the evidence that a handful of key poses plus a few physical laws beats hundreds of clips for
responsiveness; and Rain World's separation of a **small simulated proxy** (a few masses) from the **drawn body**
laid over it. That separation is the cheapest known route to weight.

### 2.7 MuJoCo, position based dynamics, Jakobsen

**What they do.** MuJoCo integrates articulated bodies in joint coordinates with a soft convex contact model, from an
XML body description. PBD (Müller 2006), XPBD (Macklin 2016) and Jakobsen's Verlet scheme (2001, *Hitman*) move
positions directly and project constraints one at a time.

**What is wrong, for this goal.**

- **A simulator is not a creature.** Put a heron in MuJoCo and it falls over. All of the animation problem (intent,
  balance, stepping, gaze) is left to the user. MuJoCo passes T1 for the body and offers nothing for the behaviour.
- **MuJoCo asks for what an author cannot supply.** Its body file needs inertias, actuator gains, joint damping and
  contact softness. A wrong set fails by oscillation or collapse, not by a message.
- **Its contact is soft by design.** "The foot is planted" is true only to a softness parameter, and that slack is
  not reported as a verdict.
- **Its cost varies with the data.** The contact solver iterates to a tolerance, so a frame with many contacts costs
  more. The GPU versions are batch tools for training.
- **PBD converges slowly on what creatures are made of.** Gauss-Seidel projection passes information one constraint
  per sweep, so a chain of n links needs on the order of n sweeps to become stiff. Plain PBD's stiffness depends on
  iteration count and time step. XPBD fixes the dependence and recommends many small substeps for rigid bodies
  `[R]`. On a GPU the sequential sweep needs graph colouring: the Astra audit counts 64 colour barriers per frame
  for one 24-link boss `[L]`.
- **PBD has no muscles and no report.** The creature is passive. A constraint is "satisfied" to whatever residual
  the last sweep left, and nobody reads the residual.

**Why it happened.** MuJoCo was built for control research, where the controller is the research. PBD was built for
cloth and ragdolls that must never explode.

**Throw out:** the general rigid-body engine as the product (the design already says so); iterate-to-tolerance
solvers in the frame; passive ragdolls as "physics".
**Keep:** XPBD's compliance (a constraint with a physical softness and a multiplier you can read as a force);
Jakobsen's insight that positions-plus-projection cannot blow up; MuJoCo's proof that a short text file is enough to
describe any body.

### 2.8 Spacetime constraints

**What they do.** Witkin and Kass (1988) pose a whole motion as one optimisation: unknown curves, physical and
authored constraints, least effort. Loft's `solve` is in this line, with least jerk as the objective.

**What is wrong.**

- **It is offline.** The unknowns are degrees of freedom times time samples. Contact-rich versions took minutes to
  hours per motion `[R]`.
- **It is local.** The answer depends on the starting guess. Loft's heron needs a hand-written arch to start on the
  right side of the rail `[L]`.
- **Least effort is not appeal.** The optimum is the laziest valid motion, and the literature has no good general
  objective for style.
- **It never met the reactive end.** Nothing says what to do when the plan is interrupted at 0.7 s.

**Why it happened.** 1988 hardware; and the idea was framed as a tool for animators, not a controller.

**Throw out:** the whole-clip, solve-once framing as the only use.
**Keep:** the formulation. It is the one piece of prior art where authored direction and physics are the same
machinery (T4): a keyed pose and Newton's law are both constraints.

### 2.9 The Disney principles as easing

**What engines do.** The twelve principles (Thomas and Johnston, 1981) are implemented as shaping functions on a 0
to 1 progress: ease in, ease out, back, overshoot, bounce. This is Penner's 2001 easing set.

**What is wrong.** The principles are observations of what weighted bodies do, written for people drawing by hand.
As curves they are effect without cause:

| Principle | What causes it in a body | What an easing curve does |
|---|---|---|
| Anticipation | Force is bounded, so momentum must be gathered first | A dip of fixed share, unrelated to the action's size |
| Follow-through, overlap | Loose parts have inertia and lag the part that drives them | A delayed copy or a ringing term with a chosen rate |
| Slow in and out | Bounded force means bounded acceleration | A smoothstep |
| Arcs | Limbs rotate about joints | A lifted lerp |
| Squash and stretch | Compliant tissue keeps its volume under load | A scale tied to speed |
| Secondary action | Coupled parts respond to the primary | A second hand-phased channel |

Six of the twelve follow from bounded force, inertia and compliance. The Astra audit's heron shows it: given only
takeoff and landing times, contact dynamics produce a 0.37 m crouch nobody keyed `[L]`. The other six (staging,
timing, exaggeration, appeal, solid drawing, straight-ahead versus pose-to-pose) are direction and belong to the
author.

**Why it happened.** A scalar curve is the cheapest thing to ship, and it needs no body model.

**Throw out:** principles as a vocabulary of curve shapes.
**Keep:** the principles as **the test of the picture**, and exaggeration as an author's operator on physical
quantities (more gravity, a softer limb, a weaker muscle, a later release), which exaggerates every consequence
consistently.

### 2.10 Gaze, and what bird heads show

**The biology.**

- A walking pigeon holds its head still in the world while the body moves under it, then thrusts the head forward
  to a new hold. In the hold the head slips at about 3 mm/s against 500 to 800 mm/s in the thrust `[V]`.
- The bob is driven by vision, not by the legs: on a treadmill with a stationary scene it stops `[V]`.
- A hovering kestrel holds its head to within 5 mm `[V]`.
- Birds' eyes move little, so gaze is carried by the head, on a neck with many more joints than the task needs:
  commonly cited as 11 to 25 neck vertebrae against a mammal's 7 `[R]`.

**What engines do.** A look-at node rotates the head toward a target and clamps the angle. It is an orientation
constraint relative to the parent bone.

**What is wrong.** Four things, each of which the bird contradicts.

1. **A hold is a world position, not an aim.** Head bobbing fixes the head's *position* in the world while the base
   translates. An aim constraint cannot express that at all.
2. **It is a hybrid of hold and re-plant.** The head holds until the neck runs out of reach, then moves to a new
   hold. That is the same structure as a foot in stance and swing. Engines implement feet (foot locking), hands
   (hand IK) and heads (look-at) as three unrelated features. They are one thing: **an anchor with a reach region
   and a law for when and where to re-plant.**
3. **The neck is redundant.** Six numbers fix the head and the neck has many more. The spare freedom is where
   posture (the S-curve) and joint limits live. Engines spend it by fixed percentages down the chain (for example
   25/25/50 `[R]`) or leave it to whatever the IK iteration produces.
4. **Escalation is driven by margin, not by a threshold.** Eyes, then head, then neck, then body, then feet: each
   joins as the one before nears its limit. Engines encode this as a state machine with a magic angle.

The treadmill result adds a fifth point. A system that keys head or body bob to the gait clock is wrong in kind: it
would bob a treadmill pigeon's head.

**Why it happened.** Look-at was added for humanoids, whose eyes do most of the work and whose necks are short.

**Throw out:** look-at as a clamped aim; separate foot, hand and head constraint features; fixed spine percentages.
**Keep:** the hold-and-thrust pattern as the general model of an anchored end, and the slip figures above as a
target for what "held" should mean.

## 3. Loft as it is today

Loft's motion is about 5,800 lines `[L]` in `src/geom` and `src/spacetime`, exported through `src/motion.ts` as 61
values and 46 types `[L, counted]`. It is all kinematic. Every motion is a closed form of the clock or of a host
input, evaluated in the pose and vertex stages. `solve` is the one exception in kind, and it runs at build.

Loft's strengths are real and are listed in section 4. The failures come first.

### 3.1 There is no body

- **A creature is a function.** `heron(body, foot, ahead)` hard-codes its points `[L, examples/second/lib/heron.ts]`.
  The Titan's rig is a 712-line file `[L]`. Nothing describes segments, joints, limits, masses, contacts or eyes.
- **The body is written twice.** The heron's five neck points appear in `lib/heron.ts` to draw it and again in
  `heron_leap.ts` to state clearance `[L]`. Change one and the proof is about a different bird.
- **The heron has no neck to move.** Its neck is one rigid strand through five fixed points in the body frame
  `[L]`. The design's own judged scene, "a heron that tracks a fish while hopping", cannot be written today.
- **Stance is general in name only.** `src/motion.ts` says of it: "one user so far: general or not is unproven
  until a second subject uses it" `[L]`. The heron walk, its second user, had to invent an idle schedule it never
  runs until the module was opened `[L]`.

Cost: T1 fails. A new body plan is new code, which is the opposite of the stated adaptability test.

### 3.2 There is no mass

- **Weight is seven hand-set sizes.** A `brace` stance takes `sink, coil, reach, settle, lift, pitch, roll` as
  author numbers and is "linear in its weights" `[L]`. The walk's body motion is `bob cos(2u)`, `sway sin(u)`,
  `yaw sin(u/2)` scaled by four more `[L]`.
- **Bob is keyed to the gait clock.** By section 2.10 this is the treadmill error.
- **A foot goes where the clock says.** A plant is the course frame at `stride (k + duty/2 − phase)` `[L]`. It does
  not depend on velocity. For the heron (hip at 1.9 m) the capture point at 1.4 m/s is
  1.4 × √(1.9/9.81) = 0.62 m ahead of the hips `[E, textbook formula]`. A shove that adds 1.4 m/s should move the
  next footfall by that much. In Loft it moves it by nothing.
- **A step lands at speed and stops in one frame.** Lift is a half sine, so with lift 0.12 m, period 0.8 s and duty
  0.66 the foot is descending at 1.39 m/s the instant before contact `[L, audit]`.
- **Secondary motion is one degree of freedom, small angle, one way.** `pendulum` answers two drives: a sway that
  "has always been swaying" and an instantaneous jolt `[L]`. It cannot start or stop, cannot swing in two planes,
  cannot chain, and never pushes back on its parent.

Cost: no anticipation, lean or recovery that follows from the action. Every one is tuned by eye, per creature, per
action.

### 3.3 There is no live state

- **`recur` refuses host inputs** `[L, audit]`. It is a bake.
- **Live stepped kernels are single-instance.** `instance()` refuses a bundle with a live `Keep::Latest` kernel
  `[L, docs/keel.md]`. Five bosses cannot each hold their own state.
- **The schedule is one frame late.** Frame kernels are dispatched before the latest state advances `[L, audit]`.
- **Nothing comes back.** The Keel contract is inputs in, pixels out `[L]`. A controller whose foot placement the
  game needs has no path.

Cost: T2 fails. "Reacts to the player" today means a host input feeding a closed form.

### 3.4 Constraints are silent or build-only

- **`chain` clamps reach without a word.** Out of reach, the chain "lies straight toward the target" `[L]`. The tip
  leaves the target and `planted` still reads 1. This is the silent fallback the house rules forbid.
- **There are no joint limits.** "A knee bends as far as the target asks, backward included" `[L]`.
- **Chains stop at three segments,** one bend plane `[L]`. A neck, spine or tail cannot be one.
- **`lookAt` is the engine look-at.** It clamps yaw and pitch in a box, follows a jumping target in one frame, and
  "as the target crosses straight behind ... the held yaw jumps from one limit to the other" `[L, its own doc]`.
- **Blending slides feet, and says so.** `footing` mixes stand and walk positions linearly; "mid-blend feet slide,
  and planted switches at on = 0.5 in one frame" `[L, its own doc]`.
- **Truth is opt-in and build-time.** `solve` reports Holds, WithinPrecision, Unproved or Undecided, and
  `src/check.ts` has checks an author may add (`limbsReach`, `clearance`). The motion functions themselves return no
  verdict, no check measures how far a planted foot's drawn tip moves, and nothing can check motion driven by a
  live input.

Cost: T5 holds for closed-form motion the author remembered to check, and for nothing reactive.

### 3.5 `solve` cannot reach the reactive end

- **It takes 117 ms for 75 unknowns** (309 steps, base heron) and 221 ms for the moved deck `[L]`. The audit's
  per-boss motion budget is 0.25 ms. A per-frame re-solve misses by 117 / 0.25 ≈ 470 times `[E]`.
- **The example is a side view.** Five scalar curves: body x, y, pitch, and one foot curve shared by both feet
  `[L]`. A 3D bird with a six-freedom body and two independent feet is 12 channels, 180 unknowns. That is 2.4 times
  the work if the banded structure is used and about 14 times if it is dense: 0.28 to 1.6 s against a 2 s build cap
  `[E]`.
- **It is local and needs a seeded side.** The heron's start is a hand-written arch `[L]`.
- **It has no mass.** The crouch appears only because the author timed a ballistic flight. Least jerk prefers
  nothing about effort.
- **One variant is a named ill-conditioned exception** `[L, CORE.md]`.

### 3.6 The vocabulary is a small baked engine

- **Seventeen easing names.** `slowIn, slowOut, slowInOut, anticipated, overshot, swing, impact, echo, dragged,
  exaggerated, uneven, alive, arc, squash, stretchOf, beats, looping` `[L]`. `anticipated` and `overshot` are
  Penner's back ease with his constant rounded to 1.7, giving a fixed 10% dip `[L]`. By section 2.9 these are the principles
  as curve shapes.
- **Attacks are a closed taxonomy.** `Motion = Lunge | Sweep | Rear | Remote | Grip` in a 927-line file, with a
  571-line dance routine beside it `[L]`. Each new behaviour arrived as a new closed-form kind with its own spec
  type. That is a framework with privileged cases, in miniature.
- **Each function lists "what it cannot do, because of the law".** Read together, those lists are this section.
  They show one pattern: every limit was met by adding a sibling function, never by a more general state.

Cost: tokens and learning. A model must know 61 names to animate, and most buy no capability a spring with a mass
would not.

### 3.7 Bodies as code hurt load time

CORE records 305 shader modules and 55 s to load five crabs `[L]`. I did not measure motion's share. The cause is
structural: a creature's pose is expression text, so each distinct creature is distinct shader code. "Shaders per
kind, instances as data" cannot hold for motion until a body is data.

### 3.8 The current proposal serves one end

`docs/proposals/2026-11-19_motion.md` §3 offers an analytic `spring` and `prefer: { effort }` `[L]`. The spring is
closed form only for a piecewise cubic driver on an affine clock; otherwise it falls back to `recur`, which cannot
follow a game. Effort needs masses and inertias no rig declares. Both improve authored clips and neither touches T1,
T2 or T4. The Astra audit's first choice, constructing motion between changes of support, is better grounded, but as
written it is three scene functions under one name.

## 4. Throw out, and keep

**Throw out.**

| What | Where it lives | Why |
|---|---|---|
| The pose as the interchange between stages | Every engine | Loses mass, contact and intent (reasons 1, 2) |
| Blend-of-poses as the way to combine motions | Every engine; Loft `footing`, `blendFrames` | Breaks every constraint it crosses |
| State machines and attack taxonomies as the unit of behaviour | Unreal; Loft `attack.ts`, `Stance` kinds | Quadratic transitions; privileged cases |
| Per-end-effector features: foot lock, hand IK, look-at | Every engine; Loft `gait`, `footing`, `lookAt` | One mechanism implemented three times |
| Motion keyed to a gait clock | Clip engines; Loft `carriage` walk, `gait` phases | The treadmill error; cannot react |
| Principles as easing curves | Every engine; Loft `lively.ts` | Effect without cause; 17 names |
| The body as code or as binary assets | Unreal; Loft creature functions and rigs | Nothing transfers; load grows with content |
| Monolithic learned policies and vendor behaviours as the law | ASE class; Euphoria | Not replaceable, not directable, days to adapt |
| Stored performance of the target body as the source of quality | Motion matching; generators | No data exists for an invented body |
| Iterate-to-tolerance solvers in the frame | MuJoCo; naive PBD | Variable cost; residual unread |
| Silent clamps and flags | Loft `chain`, `planted`; every IK | Violates truth |
| Whole-clip solve as the only use of optimisation | Spacetime; Loft `solve` | Cannot survive an interruption |

**Keep.**

| What | From | Why |
|---|---|---|
| Body parts named by role, movements named by frame | Spore | The only proven route to any-body transfer |
| A small simulated proxy under a drawn body | Rain World | Cheapest weight; fits a fixed GPU cost |
| A few key poses plus physical laws | Overgrowth | Beats clip libraries on response and tokens |
| Intent as "state now plus desired future path" | Motion matching | The right query between control and motion |
| Low-rate intent over a high-rate bounded tracking law | DeepMimic class, legged robots | The structure that produces weight |
| Constraints and physics as one formulation | Spacetime constraints | The only prior art that passes the spirit of T4 |
| Compliance with a readable multiplier | XPBD | A constraint that reports its own force |
| Hold and thrust: an anchor, a reach, a re-plant law | Bird heads | Unifies feet, hands, head and gaze |
| Closed forms of time where the motion allows | Loft | Seek, smear, telegraph and any frame alone, at no state |
| World-space plants with no stored state | Loft `gait` | Exact holds when the future path is known |
| Verdicts that name what is unproved or undecided | Loft `solve`, `check` | The only truthful constraint report anywhere |
| Derive, do not key | Loft `pendulum` | Rate from length and gravity; the right instinct |
| Laws that state what they cannot do | Loft doc convention | Makes each default honestly replaceable |
| Pose stages on the GPU, a CPU twin, interval checks | Loft | The substrate T5 and T6 need already exists |

## 5. What this leaves for step 2

The critique reduces to six questions. Step 2 should answer each with a number before designing an API.

1. **What replaces the pose as the shared state?** Something that carries momentum, contacts and targets, small
   enough to be a row in a table per creature.
2. **What replaces blending as composition?** If authored direction and physical law are both constraints, the
   composition is a solve with priorities. What is its fixed per-frame cost on a GPU, per creature?
3. **What is the smallest body description** that lets one stepping law, one gaze law and one balance law drive a
   crab, a heron and an invented creature, and that is data to the shader, not code?
4. **How do closed forms and live state coexist?** Loft has only the first; the engines have only the second. The
   same motion should be a closed form while the future is known and become stepped state when it is interrupted,
   with the hand-over continuous in position and velocity.
5. **What does "held" mean in numbers**, and how is it measured on the drawn joint each frame without a readback?
   The pigeon's 3 mm/s is one candidate scale.
6. **Which names survive?** The test from section 3.6: a name stays only if a body with mass, anchors and bounded
   actuation cannot produce its effect.

## Sources checked today

[MotionBricks](https://nvlabs.github.io/motionbricks/) ([paper](https://arxiv.org/pdf/2604.24833)) ·
[DeepMimic](https://arxiv.org/pdf/1804.02717) · [ASE](https://arxiv.org/pdf/2205.01906) ·
[HumanML3D](https://github.com/yc2768/humanml3d) · [PhysDiff](https://arxiv.org/pdf/2212.02500) ·
[Spore paper](https://chrishecker.com/Real-time_Motion_Retargeting_to_Highly_Varied_User-Created_Morphologies), the
"weeks" quotation via a [review](https://paperdecoder.blogspot.com/2008/07/real-time-motion-retargeting-to-highly.html) ·
[Overgrowth talk](https://gdcvault.com/play/1020049/Animation-Bootcamp-An-Indie-Approach), the 13 key poses via a
[forum post](https://discussions.unity.com/t/an-indie-approach-to-procedural-animation-gdc-video-talk/538228) ·
pigeon head bobbing: [Necker's review](https://www.reinhold-necker.de/Head%20bobbing%20print.pdf) and a
[summary of Frost 1978](https://www.livescience.com/why-pigeons-bob-heads.html) ·
[kestrel head stability](https://www.bristol.ac.uk/cabot/news/2024/kestrels.html).

Not rechecked, marked `[R]`: Unreal and Unity specifics, motion matching's feature vector and memory, XPBD substep
advice, spacetime solve times, bird vertebra counts.

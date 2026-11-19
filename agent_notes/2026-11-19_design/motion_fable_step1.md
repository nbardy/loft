# Motion, step 1: where engines, libraries and Loft fall short of the motion goal

19 November 2026 · Fable · critique only, nothing here is a proposal

Judged against `docs/design/motion.md` and `docs/PHILOSOPHY.md`, ignoring the constraints each system was built
under. Evidence for Loft is the code on `main` and the look rounds on `feat/motion`
(`~/git/_wt/loft-motion/agent_notes/2026-11-19_motion.md`, with its videos and sheets). Section 7 says which prior-art
numbers were checked against a source today and which are recalled.

## The verdict

No system meets the goal, and they miss it in the same few ways.

- Every shipping engine stores motion as poses over time and keeps physics as a second program behind a switch. The
  goal asks for one machinery, blended per body part and per moment.
- No engine can say whether a constraint held. A constraint is a procedure at some place in an evaluation order, and
  its residual is thrown away.
- The body plan is welded into the clips, graphs or network weights. Spore is the exception, and after about a
  thousand authored animations it reached about 90% of animation and creature pairs passing by eye.
- The systems with the most convincing weight are simulated characters driven by neural policies. Their law is a set
  of weights that costs between six minutes and ten GPU-days to change, per body.
- Loft today is the opposite of a physics engine and misses as widely. Every motion is a closed form of the clock,
  nothing has mass, and nothing can react to a player. The two parts of its look rounds that read as alive (feet that
  step from strain, a head that holds still) were written outside the motion library, on its only stateful
  primitive, which refuses player input.

## 1. The yardstick

Seven tests, taken from the design goal. Each critique below names the tests it fails.

1. **One machinery.** Authored and simulated motion are the same state, laws and constraints, mixed per part and per
   moment.
2. **Stated constraints, true verdicts.** A planted foot, a neck length, a held gaze or "lands at 1.2 s" is a
   statement, and what the engine reports about it is proved, measured or named unknown.
3. **Open laws.** A spring, a contact model, a balance rule or a stepping rule is a function the consumer can read
   and replace.
4. **Owned state.** Momentum, centre of mass, contacts and controller phase are visible, and where each lives (a
   closed form, a baked table, a per-frame GPU buffer) is a choice.
5. **Any body plan, described once.** A new creature moves well from its description alone.
6. **GPU cost per creature, many creatures.** Reactive work runs per frame on the GPU, and precompute is used where
   it pays.
7. **Tokens.** A crab that scrambles, a heron that tracks a fish while hopping and a boss reacting to a hit are each
   short and read cold.

## 2. Scorecard

| System | A new body plan costs | Player reverses mid-stride | Can it say a constraint held? | Replacing one law costs |
|---|---|---|---|---|
| Clip graphs, motion matching | New capture and database per skeleton; a crab cannot be captured | Plays a captured plant-and-turn if one exists; otherwise blends and slides | No. Foot locking is a later IK pass | Recapture: the law is the data |
| Spore | Nothing, after ~1,000 animations keyed in body-independent form | Named in its paper as the gait system's biggest open problem | No. ~90% of pairs pass by eye | A C++ change in the solver |
| Control Rig, full-body IK | A rig graph per skeleton | Not its job: it poses, it does not move | No. Strengths and an iteration cap | A C++ node |
| Euphoria | Humanoid only, tuned per title by the vendor | Only after animation has given up control | No | Closed middleware |
| Overgrowth, Rain World | New code per creature | Handled well; this is their strength | No | Edit the game's source |
| PBD and XPBD; MuJoCo | A model with masses, joints and gains, and no controller, so it falls over | Whatever the controller does | Residuals discarded (PBD); soft by design (MuJoCo) | Forces are yours; integrator and contact are fixed |
| DeepMimic, AMP, ASE, SuperTrack | Reference motion for that body plus 6 minutes to 10 GPU-days | Handled if training covered it | No. A success rate | Retrain |
| Learned generators | A corpus that does not exist for a crab | Regenerates in ~2 ms; feet not guaranteed | No | Retrain |
| Spacetime constraints | A new problem statement | Too late: it solves offline | At sample times, for a local optimum | Nothing: objective and constraints are yours |
| Loft today | 100 to 700 lines of scene code | `gait` replays its steps backward; the stepper copes only as a film baked at load | At build, for closed forms; never live; some flags are wrong | Nothing: edit an expression |

## 3. System by system

### 3.1 Clip graphs and motion matching

State machines and blend trees pick and mix recorded clips. Motion matching (Clavet and Büttner; For Honor) replaces
the graph with a nearest-neighbour search, a few times a second, over a feature vector of a few dozen numbers: the
wanted future path, foot positions and velocities.

**Wrong.** It fails tests 1, 2 and 5, and it contradicts "no prebaked cycles" by construction: every frame shown was
recorded. A clip stores the outcome of an intention, a body and physics with all three erased: which foot bore weight,
what the eyes held, how heavy the animal was. Retargeting, foot locking, warping and blend smoothing all exist to
guess those back.

**Why.** Capturing humans was cheap for large studios, hand-built graphs had grown to thousands of clips, and search
removed the graph authoring.

**Cost.** Memory grows linearly with data (Learned Motion Matching was written to remove that). Coverage is a product
of speed, turn, slope, action and body. Responsiveness is traded against continuity by a hand-set weight. A crab or a
titan cannot wear a capture suit, so the method has nothing for them. None of it is text a model can write.

**Throw out.** The clip as the unit of motion and the pose as its state. **Keep.** Recorded motion as a prior that
constraints correct, which is how the goal already frames import, and querying by the wanted future path.

### 3.2 Spore

The closest prior art, and its paper is candid about where it stops. Animators key curves in a body-independent form:
parts are selected by role ("grasper in front") and moved relative to rest, to the ground or to a target. At run time
the curves are specialised to the creature and posed by a Particle IK solver (points joined by length constraints,
five outer iterations, spine first and then limbs). Locomotion comes from a separate gait system: a duty factor and a
step trigger per leg group, authored for one to six feet and generated for seven or more. Parts that no animation
selects get "Jiggles", a damped pseudo-physical pass that never feeds back. A 25-body creature cost 0.2 ms a frame on
a 1.7 GHz Pentium-M, 35% of it in IK.

**Wrong.**
- Kinematic throughout (tests 1, 4): no mass, momentum or balance. The paper's last sentence on gait says its "biggest
  challenge is how to provide believable locomotion with anticipation in the face of discontinuous player and code
  inputs". That is the second bullet of our design goal, named as open in 2008.
- Truth by inspection (test 2): several hundred creatures and a thousand animations, judged by testers on a
  spreadsheet grid, with about 90% passing. Inside the solver lengths are soft (a simplified limb may compress to 10%
  of its rest length), reconstruction then restores them at the expense of the goals, and nothing reports which gave
  way.
- Sealed laws (test 3): the solver is built for "ad hoc tunability" by its own programmers. A creature's collision
  with itself is "completely ignore[d]".
- Authoring (test 7): an animation takes a few hours, but "it takes weeks to build up an intuition" for what
  generalises, and equivalent ways of keying one motion specialise differently.
- Gait and animation are two systems "for historical reasons".

**Why.** A 2008 CPU budget, with the solver run every frame on every creature. A small team of animators was the
source of quality, so the keyframe workflow was preserved. The solver was made path-independent (no state between
frames) for predictability. Loft made the same stateless choice for the same reason and inherits the same ceiling.

**Cost.** About a thousand animations and a standing test team to reach nine pairs in ten, with no weight in the
result.

**Throw out.** Keyed curves as the carrier of meaning; the stateless solver; secondary motion as a one-way
afterthought; a separate gait system. **Keep.** Parts chosen by role; motion stated relative to ground and targets;
the grid of many bodies against many motions as the acceptance instrument for test 5.

### 3.3 Unreal Control Rig and full-body IK

Control Rig is a node graph run per character on the CPU after animation. Its full-body IK is a position-based solver:
effectors pull bones toward goals for a fixed number of iterations, with a stiffness per bone and a strength per
effector.

**Wrong.** A constraint here is a node at a place in an evaluation order (test 2). A later node overwrites it, the
iteration cap ends it, and strengths make "held" a matter of degree. Nothing returns a residual. There is no time in
the model: no velocity and no mass, and a spring node is an interpolator with private state (tests 1, 4). Look-at
rotates a head bone after the body is posed, so the head's position inherits every bob of the body. The graph is a
binary asset authored per skeleton (tests 5, 7).

**Why.** Rigging came into engines from film tools as an artist's graph, evaluated root to leaf on CPU threads.

**Cost.** No weight at any price. A rig graph per skeleton, made in an editor and not in text. Per-character graph
interpretation on the CPU, so crowds switch to baked vertex animation and lose all adaptivity (test 6).

**Throw out.** Evaluation order as the meaning of a constraint; strengths with no report; the hierarchy as the solve
order; graph assets. **Keep.** Analytic two-bone IK as a fast exact case, and every level being reachable.

### 3.4 Euphoria

Hand-written behaviours (balance, stagger, brace, reach for a wound) drive muscles on a simulated ragdoll. It proved
the picture: the falls in GTA IV read as weighted and never repeat.

**Wrong.** It is a mode (test 1). Animation owns the character until a hit, Euphoria owns it until it lies still, and
a get-up clip takes it back: two hand-offs where the goal asks for a blend per part and per moment. The behaviours
were proprietary, humanoid and tuned per title by the vendor's engineers (tests 3, 5).

**Why.** Physics and animation were separate runtimes, and direction mattered more than physics whenever the player
was in control. Physics was given the moments when direction was already lost.

**Cost.** Capability limited to reactions, and no consumer could change a law. Licensing ended in 2017.

**Throw out.** The hand-off; behaviours as closed modules. **Keep.** The evidence that a muscle-driven body under a few
simple behaviour laws reads as alive.

### 3.5 Overgrowth and Rain World

Two tiny teams that got closer to the picture than large ones did. Overgrowth runs on very few key poses (thirteen, by
community report) joined by interpolation laws: lean taken from acceleration, springy transitions, ragdoll blended
in. In Rain World each creature is a few point masses joined by distance constraints and pushed by its
AI, with limbs and sprites drawn to follow the masses. They show two things the large systems hide: a low-dimensional
physical proxy with cosmetic layers that follow it is enough to read as alive, and few poses suffice when laws carry
the timing.

**Wrong.** Every creature is code in the game's source (tests 5, 7). Nothing is stated or checked (test 2). Rain
World's clumsiness is its look, so "lands at 1.2 s" is out of reach (test 1). Overgrowth is bipeds only.

**Why.** One programmer each and no animation team.

**Cost.** A new creature is new bespoke code, and the authored end of the goal is absent.

**Throw out.** A creature as hand-written code. **Keep.** The body leads and the drawing follows; few poses plus laws;
lean derived from acceleration.

### 3.6 Verlet, PBD and XPBD

Jakobsen's Verlet relaxation (Hitman, 2001), Müller's position-based dynamics and XPBD move points, project
constraints one at a time and take velocity from the change in position. They are stable at any step, parallel by
colouring, and already underneath Spore's IK and Unreal's full-body IK.

**Wrong.**
- Test 2: a constraint is satisfied to wherever the loop got. The residual is computed during projection and
  discarded. In PBD the stiffness itself depends on the iteration count and the step; XPBD removes that dependence and
  still reports nothing.
- Test 4: there are positions and no forces. You cannot ask how hard a leg pushes, so effort, balance and friction
  limits are outside the model.
- Test 1: it simulates passive matter. A creature needs a controller, which these methods do not supply.

**Why.** Built for robustness on 2001 to 2007 hardware, for ragdolls and cloth that were never directed.

**Cost.** Heavy bodies on thin chains sag or need many substeps, and "did it hold" is never answered.

**Throw out.** Iteration count as the knob for both stiffness and accuracy; discarded residuals. **Keep.** Position as
primary state, because a planted foot is a fact about position; projection as a kernel that suits a GPU.

### 3.7 MuJoCo

A rigorous simulator in joint coordinates with a convex soft-contact model, built for control research.

**Wrong.**
- Contact is soft on purpose (test 2). Each constraint has a time constant and an impedance, so a planted foot sinks
  and creeps a little, and a separate no-slip pass exists because of it. That choice helps optimisers and breaks
  "planted feet do not slide".
- It supplies no motion (test 1). An unactuated model falls over. Behaviour is torques the user computes, and writing
  that controller is the whole problem.
- The usual step is 2 ms: eight steps per 60 Hz frame spent on stiffness no viewer sees (test 6). The GPU ports are
  built for thousands of training environments, not for one creature inside a render frame.
- The model file wants inertias, armatures, gains and friction (tests 5, 7): numbers a model inventing a creature does
  not have. A bad guess gives jitter, not a refusal that names the fix.

**Why.** It serves robotics and learning, where faithful articulated dynamics matter and the picture does not. The
goal already lists a general rigid-body engine as a non-goal.

**Cost.** A long model file of physical numbers, and still no motion at the end of it.

**Throw out.** Torque as the authoring interface; soft contact for feet; step rates set by stiffness. **Keep.**
Constraints as statements given to a solver; centre of mass, momentum and contact force as readable state; care about
units.

### 3.8 DeepMimic, AMP, ASE and SuperTrack

A neural policy sets joint targets for a simulated body and is trained to imitate reference motion. The results are
the best physical character motion published: weighted, recovering, never repeating.

**Wrong.**
- Test 3: the law is network weights. Nobody can read it, stiffen an ankle in it or lengthen an anticipation in it. A
  change is a retraining: about 60 million samples and a day on 16 workers per DeepMimic skill; 39 million samples for
  AMP (six minutes on 4,096 GPU environments, about 30 hours on 16 CPU cores); over 10 billion samples, ten simulated
  years and ten days on one GPU for ASE's skill set.
- Test 5: one policy per skeleton, because the observation and action sizes are the skeleton. Each body also needs
  reference motion. DeepMimic's dragon and T-rex were keyframed by hand, which is the authored end entering by the
  back door.
- Test 2: success is a rate. "Lands at 1.2 s" can be asked through a reward term and hoped for.
- Test 6: a network per creature on top of a simulation stepped many times a frame (DeepMimic simulates at 1.2 kHz
  under a 30 Hz policy).
- SuperTrack replaces reinforcement learning with supervised learning through a learned world model whose predictions
  drift after about a second. The law is as sealed as before.

**Why.** Contact makes the dynamics discontinuous, so sampling was what worked. Human capture was the supervision
available. The aim was physical validity, with direction a distant second.

**Cost.** The edit loop is a training run, and a new body plan gets nothing without one.

**Throw out.** The sealed policy as the law; training as the edit loop; a policy per skeleton. **Keep.** Imitation as a
preference ("stay near this reference"), and the lesson in what the policies learn, which is small: where to step and
when to push.

### 3.9 Learned kinematic generators

NVIDIA's MotionBricks (2026) is one model over more than 350,000 clips, generating at about 2 ms latency by its
authors' figures, with weights for one human skeleton and one humanoid robot. Its model card says the output is not
validated for physical control.

**Wrong.** The output is poses with no physics, so nothing is guaranteed and feet slide unless a later pass pins them
(test 2). The skeleton is fixed by the corpus, and no corpus exists for a crab (test 5). The law is sealed (test 3).

**Why.** Large human capture sets and sequence models existed.

**Cost.** Nothing for an invented creature, and no statement about the motion that can be trusted.

**Throw out.** Any thought of a generator as the engine. **Keep.** The role the design goal already gives it: motion
that comes in from outside, is retargeted, is projected onto the stated constraints, and is never reported as meeting a
hold it was not checked against.

### 3.10 Spacetime constraints

Witkin and Kass (1988) pose a motion as the minimiser of an objective (muscle power) subject to physics and stated
constraints over the whole span. It is the one formulation in which the author states the goal and the manner in a
single language, and anticipation and follow-through appear unasked: the Luxo lamp crouches because the jump requires
it. It is the ancestor of Loft's `solve`.

**Wrong.**
- Offline (tests 1, 6): a player's reversal arrives after the answer was computed.
- The contact schedule is an input: someone decides when each foot is down. Later work that frees the schedule took
  minutes per motion.
- Local: the answer depends on the starting guess, and any freedom left unconstrained is spent in ways no one wanted.
- Constraints hold at sample times (test 2).

**Why.** 1988 compute and a non-convex problem.

**Cost.** Seconds to minutes per clip. Section 4.5 shows each fault in Loft's own heron.

**Throw out.** The whole-span solve as the only home of constraints; least-anything as a sufficient account of
character. **Keep.** Holds plus a preference as the authoring language; physics holds as the source of anticipation.

### 3.11 Disney's principles as engines implement them

The twelve principles describe what weighted, intended motion looks like. Engines, and Loft's `lively.ts`, implement
the look itself: an ease, a back-overshoot cubic, a ring-down after a stop.

**Wrong.** A principle encoded as a curve shape is a symptom without its cause (tests 1, 3).
- Anticipation is the visible part of gathering impulse. In the heron round one physical hold (leave the ground at
  3.94 m/s) produced a 0.60 m crouch and a 2.27 g push with no key asking for either; v²/2d gives about 13 m/s² on
  average. `anticipated(u, 1.7)` is the easing cubic every tween library ships, and it does not deepen when the jump
  lengthens.
- Follow-through is inertia in parts that nothing drives. `swing(after, amount, 1.4, 0.25)` rings at a typed rate by a
  typed amount, whatever struck it.
- `impact` squashes by "`depth` whatever the speed", in its own words.

**Why.** Hand-drawn craft had no simulator, and curve editors have no mass.

**Cost.** Weight is declared at every call site and derived nowhere, so the wiring grows as parts times events.
Physical systems make the mirror error: correct weight, and no place to state staging, timing or exaggeration.

**Throw out.** Curve shapes as the primitives of weight. **Keep.** The principles as the judge of the picture, and as
preferences applied to physical quantities: exaggerate the impulse and the timing, not the pose.

### 3.12 Gaze and head stabilisation

A walking pigeon holds its head still in the world while the body advances, then thrusts it to a new hold. In the hold
the head is locked in space apart from a small slip (Troje and Frost 2000). The controlled quantity is the head's place
and attitude in the world. The neck is slack, and the hold ends when the neck runs out.

**Wrong.** Engines aim a leaf bone at a target after the body is posed. The head's position is not controlled, limits
clamp (Loft's `lookAt` "stops dead, with a corner"), and nothing tells the body to turn when the neck reaches its
limit. The goal's own example (the head tracks right while the body turns left, stops at the neck's limit, then the
body turns) needs the head solved before the body and a signal from the neck's limit back to the body. No engine has
that path, because the hierarchy is the solve order (tests 1, 2).

**Why.** Look-at was added as a late node on a root-to-leaf chain, and game cameras seldom showed the difference.

**Cost.** Heads that bob with the body and eyes that slide off their target.

**An observation of mine.** A held gaze and a planted foot are one constraint: an end pinned in the world, a chain as
slack, and a re-pin when the slack runs out. The re-pin is a step for a foot, a thrust for a pigeon's head and a turn
of the body for a tracking head. Engines ship these as unrelated features (foot IK, look-at, hand IK). Loft has them
as three unrelated pieces of code: `footing`, `lookAt` and the heron's hand-built follower.

## 4. Loft as it is today

Loft's motion is about 90 exported functions in 16 files (`src/geom`, `src/curve.ts`, `src/map/recur.ts`), nearly all
closed forms of the clock, plus `solve` at build and `recur` for baked state. Its strengths are real: motion is text,
every level can be rewritten, kinds are sum types with typed refusals, each function documents what it cannot do, and
predictions are scored to the number. Of the seven tests it passes one outright (open laws).

### 4.1 Nothing has mass (tests 1, 4)

The motion proposal says it plainly: "Nothing has mass or momentum unless a hold states it." What that costs, measured:

- **Crab walk.** The hand version's body motion is two keyed sines on the gait clock (bob 0.09 cos(4πt/P), roll
  0.04 sin(2πt/P)), and `walkOf` in `stance.ts` hard-wires the same shapes. The solved version's carriage is the course
  to within 3.7 mm, "smoother and, because of that, deader": with no load, least jerk has no reason to move the body.
- **Footfall.** A `gait` foot rises as a half sine, so it arrives at π × lift / swing time. For the round's crab (lift
  1.4, swing 0.76 s) that is 5.8 units/s into the ground, stopped in one frame, on a creature 2.5 units tall.
- **Scramble.** Weight enters as three separate hand-written expressions: lean = atan(a/g), a body lag of 0.08 a, and a
  capture time √(H/g) = 0.505 s. Each is right. None follows from a mass, and a new creature writes them again.
- **Secondary motion.** `pendulum` is the exception that shows the rule: its rate follows from length and gravity
  ("Nothing here is chosen but the damping"). But it is one linearised degree of freedom with two kinds of drive, it
  cannot start or stop, and a history of hits is a sum with one term per remembered event.

### 4.2 Nothing can react (tests 1, 4, 6)

Loft made "a pure function of time" the invariant of motion, for reasons that suit a renderer: any frame draws alone
(training draws 16 frames at 512²), time scrubs, results are deterministic, and a CPU twin can check them. Against
the goal's second end ("adapts instantly ... to a player's control and to sudden changes of direction, with no
prebaked cycles") the invariant rules the feature out:

- `gait` picks a plant by `floor(cycles + phase)`. Its own comment lists what follows: no change of rhythm, no terrain,
  a count that leaps if the period changes. Paced by distance, a reversal runs every foot's step backward: the
  moonwalk in panel (a) of the scramble round.
- `replants` schedules idle steps once, on the CPU, from the inputs' defaults sampled at 20 Hz. Its comment lists
  "answer the game at run time" among what it cannot do.
- `lookAt` follows a target that jumps "in one frame".
- `recur`, the only stateful primitive, refuses a host input ("a baked state cannot follow an input a game changes").
  So the stepper that "reads as weight" is a four-second film: 480 steps baked at load (3.1 ms GPU, 309 ms pipeline
  setup) over a body path that was keyed by hand.
- That stepper is one-way. Feet follow the body, and the body never answers the feet. Balance is assumed, not kept,
  and the support-polygon trigger was "not built".
- Live stepped kernels exist in the Rust host (`Keep::Latest`), but `instance()` refuses a bundle that has one, so
  creatures with live state cannot share a pass.

The adaptive end of the design goal has no implementation in Loft. Its one demonstration is a recording.

### 4.3 Reports that are not true (test 2)

Truth is priority four in `CORE.md`, and the motion library breaks it in small ways today:

- `footing` mixes foot positions linearly into the walk, so "mid-blend feet slide", while `planted` flips at
  `on = 0.5`. `planted(i)` is a function of the clock, never of the drawn foot.
- `chain` clamps: out of reach "the tip stops short on the line to the target". The target is planted, the drawn foot
  is somewhere else, and nothing says so. It has no joint limits: "a knee bends as far as the target asks".
- Reach is checked by `limbsReach`, a 20 Hz scan on the CPU. To pass it, the dance's sway was cut by hand from 0.9 to
  0.5. The author did the constraint solving at design time, for one body.
- `solve` holds are met at samples and re-checked on a finer grid. "Between the fine grid points nothing is proven",
  builds report `Unproved`, and one scene (the moved heron) is a named exception.
- For anything live there is no verdict of any kind.

### 4.4 There is no rig (tests 5, 7)

`src/geom/rig.ts` is 38 lines: one function that carries coordinates on a frame. A body plan is whatever a scene's
code builds. The round's small crab is 101 lines that interleave topology, IK calls, geometry and paint. The Titan's
rig is 712 lines inside a creature of 36 files and 6,246 lines. Masses, joint limits, contacts and eyes do not exist
as things.
`chain` handles two or three segments in one bend plane, so the heron's neck is a hand-rolled S-curve with four Newton
steps written in the scene. The stepper hard-codes eight feet in two rows of four (`mod(i, 4)`, the opposite foot at
`i + 4`). "A new body plan moves well with no new code beyond its description" fails at the first step, because there
is no description.

### 4.5 `solve` is spacetime constraints with the physics left out (tests 1, 2, 6)

The first solved heron (least jerk, 75 unknowns, 117 ms in Rust) pitched 67° nose-down at 0.925 s, crouched 0.77 m,
pushed at 46.8 m/s² (4.8 g) and ended with a snap of −18.4 m/s². Each fault has a cause in the formulation:

- No mass, so the only cost is jerk, and jerk is indifferent to which way up a bird is.
- A free unknown (pitch) was spent to fold the legs over the rail. The head was rigid to the body and went with it.
- The curves are C2 everywhere, so acceleration must already equal −g at takeoff while the feet are still planted.
  Real contact changes are jumps in acceleration.
- Contact times (0.6 s and 1.2 s) are the author's and must fall on knots.
- The flight hold is on the hip's acceleration, not the centre of mass, and angular momentum is absent: the torso
  turned 67° in a third of a second of flight with nothing accounting for it.

The repair was to give the solver the animator's keys as its prior, bound the pitch and add a head unknown: 162
unknowns, 1,110 steps and 7.6 s in the TypeScript solver, for 1.8 s of one bird in one plane. The build cap is 2 s; at
the Rust port's measured 6.5× this solve would take about 1.2 s. Read plainly, the animator wrote the motion and
`solve` projected it onto the holds. That is a useful tool for the authored end. It is not a source of natural motion,
and it runs only at build.

### 4.6 Tokens and speed (tests 6, 7)

The goal names three scenes. The crab that scrambles is 146 lines above a 101-line rig. The heron with a stabilised
head is 192 lines, with the hold-then-thrust on a fixed 0.2 s clock (not triggered by the neck's reach), and it took
two values of a feed-forward gain and a hand-smoothed hand-off to bring head jerk down to 428 m/s³. The boss reacting
to a live hit cannot be written. An author also needs the right dozen of about 90 names.

Per-frame motion is cheap (pose stages 0.13 to 0.20 ms, 4 to 12% of a frame), and the measurements invert the premise
behind the closed forms: reading stepped state costs 0.131 ms where evaluating the `gait` formula costs 0.197 ms. The
speed failure is elsewhere. Bodies are code, the code is inlined per use (the Titan records 308k nodes), and five
crabs compile 305 shader modules and take 55 s to load. "Many creatures at once" fails at load, before any frame.

Interop is one-way: a baked glTF goes out, and no clip import or retarget exists.

## 5. What the failures share

Six causes account for nearly every failure above. The grouping is mine; the ingredients are in the literature.

1. **Motion is stored after its reasons are discarded.** A clip, a keyed curve and `0.09 cos(4πt/P)` are the same
   thing: an outcome with the contact, the intent and the mass erased. Everything downstream guesses them back.
2. **Kinematics and dynamics are two programs.** Engines join them with a switch. Simulators have no authored end.
   Loft has no dynamic end.
3. **A constraint is a procedure, so it has no verdict.** Evaluation order, iteration caps, strengths and clamps decide
   what happens, and the residual is never kept.
4. **The body plan is welded in**: into bone names, feature vectors, network layer sizes, rig graphs or scene code.
5. **Dynamics are resolved at the wrong level.** Clips, IK and Loft resolve none. Simulators and policies resolve every
   joint at 500 to 1,200 Hz. The weight a viewer reads lives in a handful of numbers per creature: centre of mass,
   momentum, the support under it and where it must step (the capture point, inherited from Raibert and Pratt). The
   scramble decided every step from one such number with a 0.505 s time constant, at 120 Hz, in 0.13 ms. A planted
   foot is an exact fact (the stepper copies it, and its slide is exactly 0), where simulators spend most of their
   steps integrating it as a stiff spring that still creeps.
6. **Reacting and replaying are treated as exclusive.** Engines step and cannot scrub. Loft scrubs and cannot react.
   `recur` already keeps checkpoints and replays at most 120 steps, so refusing inputs is a policy, not a necessity.

## 6. What to throw out, and what survives

**Throw out, everywhere.**
- The pose or clip as the state of motion, with state machines and blend trees as its law.
- The switch between animation and physics.
- Constraints as nodes or iteration loops with no residual, and strengths in place of verdicts.
- Sealed laws: solver nodes, vendor behaviours, policy weights.
- Anything tied to one skeleton: bone names, a rig graph, a policy.
- Torque as an authoring interface, step rates set by stiffness, and soft contact for feet.
- Curve shapes as the primitives of weight.
- Per-character graph evaluation on the CPU.

**Throw out in Loft.**
- "Motion is a pure function of time" as the model. Keep it as one of three places a result may live.
- `gait`'s clock-indexed plants, `carriage`'s summed sines, `replants`' build-time schedule and `footing`'s linear mix
  as primitives. At most they are recipes inside an example engine.
- `planted` as a clock flag, and `chain`'s silent clamp.
- The `lively.ts` vocabulary as the way weight is made. Its names remain a checklist for judging the picture.
- Least jerk as the default meaning of natural, and build time as the only place a constraint is solved.
- `recur`'s refusal of inputs, and `instance()`'s refusal of live state.
- Rigs as scene code, and stepping rules with a leg count written into them.

**What survives.**
- From Loft: feet fixed in the world with the body moving over them; stepping triggered by strain; rates derived from
  length and gravity (`pendulum`); holds plus a preference, with keys as the prior; verdicts that name what is
  unknown; sum types and typed refusals; predicting a run to the number before making it.
- From prior art: parts chosen by role (Spore); a small physical proxy that the drawing follows (Rain World); few
  poses joined by laws (Overgrowth); behaviour laws on a driven body (Euphoria); position as primary state (Jakobsen);
  readable momentum and contact state (MuJoCo); imitation as a preference (DeepMimic); a bird's hold and thrust as the
  model of gaze.

## 7. Standing of the numbers

- **Read today from a source.** Every Loft number (the code, `CORE.md`, `docs/stance.md`, `docs/keel.md`, the motion
  note on `feat/motion`). The Spore figures and quotations (Hecker et al. 2008, the paper's text). DeepMimic's sample
  count (its README). AMP's and ASE's counts (their papers, through search summaries). SuperTrack's one-second drift
  (Ubisoft La Forge's post). MotionBricks' figures (the authors' own, not reproduced independently). Euphoria's end of
  licensing. The pigeon's hold phase (Troje and Frost 2000, abstract).
- **Recalled, not re-checked today.** Unreal's full-body IK being position-based. MuJoCo's 2 ms step, soft-contact
  parameters and no-slip pass. DeepMimic's 1.2 kHz simulation and its keyframed dragon and T-rex. The size of a motion
  matching feature vector. Overgrowth's thirteen poses (a community report; the talk's description says "very few key
  frames").
- **Mine.** The seven tests; the six shared causes as a grouping; the pinned-end reading of foot and gaze; the
  footfall speed and the v²/2d check, computed from the scenes' numbers; the 1.2 s Rust estimate, extrapolated from
  the measured 6.5×.

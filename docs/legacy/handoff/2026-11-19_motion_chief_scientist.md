# Handoff: the motion meta engine (for the chief scientist, 19 Nov 2026)

You have seen our measurements and written the language draft (`agent_notes/dump_manual_nov_19th_alpha_v1.md`). We
have adopted it for render (`docs/proposals/2026-11-20_meta_engine_plan.md`). Motion is the part we most want your
mind on before we build it. This note is self-contained: the goal, what we measured, what we designed, where we are
unsure, and what we would like back.

## The goal

One motion system that covers both ends, per body part and per moment:

- **Authored performance.** Springy, appealing, Disney-principled motion (anticipation, squash and stretch,
  follow-through, overlap, arcs, timing) for film and choreographed boss attacks. The animator directs; the system
  adds weight without fighting the direction.
- **Physical, adaptive motion, Spore-like.** Any body plan, described once, moves well with no new code: legs, necks,
  tails, claws. Real-time player control and sudden changes of direction, organic, with no prebaked cycles: feet plant
  and step from wherever they are, the body leans into acceleration, balance is kept or recovered, contact holds.
- **Constraints a consumer can state** (neck length and rotation range, gaze holding a point with the skull aligned,
  as birds do, within what the neck can reach; joint limits; reach; posture; timing) and **laws a consumer can write
  or replace** (springs, muscle-like actuation, gravity and momentum, contact and friction, balance, stepping, target
  tracking: turn the head toward something on the right while the body turns left, stop at the neck's limit, then turn
  the body).
- **Explicit state** (plans, momentum, centre of mass, contacts, controller phase, targets) and an explicit choice of
  where each part runs: closed form of time, baked table, or live per-frame GPU state.
- **Interop**: motion in from outside (clips, motion capture, learned generators such as NVIDIA MotionBricks that
  are trained per rig), retargeted to the body plan and corrected by the constraints; posed skeletons out. Loft does
  not run the models; it works well with people who do.
- **Meta engine stance**: the hard parts are exposed, rewritable code in the language. `roam` (adaptive locomotion)
  and `act` (performance) are example engines, not the system.

Priorities: the picture (does it read as alive and weighted), then GPU cost per creature, then tokens for the author,
then truth (a constraint reported as held is held, measured on the drawn result after IK).

## What we measured (look rounds and Phase 1, M4)

Comparison page with the videos: the owner's artifact "Gait vs Solve". Notes: `agent_notes/2026-11-19_motion.md` on
branch `feat/motion`, `agent_notes/2026-11-19_design/motion_step3_impl.md`.

Heron hop (2.4 s, a rail to clear, a block to land on):

| Take | Crouch | Max body pitch | Head | Read |
|---|---|---|---|---|
| (a) Hand keys | shallow | upright | rigid on body | smooth, a little too linear |
| (b) Solve, least jerk (cubic B-spline unknowns, holds on position, velocity, acceleration, flight acceleration = g) | 0.77 m, 4.8 g push | 67° nose-down | rigid on body, 0.39 m below the hips | far too springy, recoils, head jolts at the ground. Cause: no posture hold, and least jerk penalises change of acceleration, not magnitude |
| (c) Solve near the hand keys (`prefer near`, jerk 1e-7), pitch ≤ 20°, head level | 0.60 m (predicted 0.55 to 0.70) | 12° | 0.17° tilt | the hand look with a real arc and a clean landing |
| (d) (c) plus world-space gaze lock and neck IK (fixed-length neck curve between shoulder and head; level gaze, glance at the landing spot capped at 15° in the run-up only) | as (c) | −11.9° to +8.6° | head jerk 428 m/s³ (from 670) | owner liked the neck motion; the most heron-like |

Crab scramble (hard right then hard left):

| Take | Result |
|---|---|
| Gait paced by distance | moonwalks on the reversal |
| Weight-driven stepping: a recur over 8 feet at 120 steps/s; a foot steps when strain or capture-point error demands it, toward where the body is going, from where it is; lean ≈ atan(a/g); outside legs compress | 24 steps (predicted 20 to 36), lean 30.2° (predicted 30.2°), zero slide while planted, no neighbours swinging together; first lift after reversal 0.36 s (predicted ≤ 0.2 s, slow) |
| Swings by need (duration and lift from step length and urgency) | 0.12 to 0.35 s swings, 23 steps, less mechanical |
| Phase 1 on Rust (`roam`, all crabs fused into one dispatch) | 22 steps as predicted; live equals bake to the bit at 30, 60 and 120 Hz with a six-tick stall; planted-toe drift 0.0008 mm after IK; a planted 20 mm fault caught on the right toe; motion p95 0.070 / 0.101 / 0.086 ms for 1 / 256 / 1,000 crabs; bake 5.3 ms; 216 tokens of author code. Owner verdict on weight pending |

Also derived (Fable, step 2): anticipation from boundary conditions — stance 1.949 v/g, crouch 0.385 v²/g, push
2.02 g — gives the heron 0.59 m against the animator's 0.60 m.

## What we designed (to be judged, not inherited)

`docs/proposals/2026-11-19_motion_architecture.md` (consolidated from three independent designs):

- Core learns nothing about bodies. Three tables: BODY (plan: segments, joints, limits, masses, ends), STATE
  (per-instance live state with checkpoints), POSE (32-byte joint frames read by render).
- Four end kinds: `foot`, `head`, `hand`, `loose`. The **pin** is one constraint for foot, gaze and hand, with strain
  flowing to the root. Holds are measured on the drawn end after IK (the "fake foot" test).
- Physics as chosen acceleration clamped by planted pins; only the trunk is integrated.
- One step function, three homes: closed form, baked table, live.
- Directed build-time motion uses solve near hand keys (the version the owner liked).
- Fused ticks: all motion for all creatures in one dispatch. Your correction stands: bucket by (dependency wave,
  law kind) so every SIMD group runs one law.
- Your draft adds `law`, `solver` (colored Gauss-Seidel or Jacobi, shared memory per strand), `constraint -> Verdict`,
  `Attach[...]` vs `Target[...]`, `@checkpoint(every: 60)`, and invariant gates on solved motion.

## Where we are unsure (please push hardest here)

1. **One formulation or layers?** Is per-tick motion best posed as a small constrained optimisation per creature
   (targets from performance, laws as costs, pins and limits as constraints: a QP, an XPBD system, or an augmented
   Lagrangian like our build-time solve), or as ordered layers (performance → balance → stepping → IK → secondary)?
   What gives Disney appeal and physical plausibility without one fighting the other?
2. **Any body plan, Spore-like.** What is the minimal body description from which stepping, balance, gaze and reach
   work with no per-creature code? How do gait patterns (tripod, wave, gallop) emerge from load and support rather
   than being authored? How does capture-point stepping generalise to N legs, uneven terrain and moving support (a
   foot on a moving ferry deck)?
3. **Solvers on the GPU, deterministic.** Which solver family for contacts, pins and limits gives bit-identical live
   and baked runs per device, scales to 1,000 creatures under 0.5 ms, and converges well enough that verdicts are
   honest? Where must iterations be fixed, and how are iteration caps reported (Unknown)?
4. **Authored and physical together.** How do hand keys, imported clips or generated motion act as the prior
   (`near` targets) while laws add weight, follow-through and recovery? How is anticipation derived rather than
   keyed, and when should the author's timing win?
5. **Gaze and necks.** A formal model of gaze stabilisation with limited neck range and a body-turn handoff (track a
   fish to the right while the body turns left; stop at the limit; then turn). Head-bob as hold-then-thrust.
6. **Secondary motion.** Analytic springs on piecewise-polynomial drivers (closed form per segment, scrubbable) vs
   stepped springs; volume-preserving squash and stretch; overlap on chains (antennae, tails, feathers).
7. **Reaction.** A boss hit mid-attack: impulse response, recovery that changes when the first support slips, a second
   hit before recovery. What state must exist for that, and how is it checkpointed?
8. **Truth.** What a motion verdict should state (held with margin, missed by how much, unknown and why), measured
   where (on the drawn end after IK, in the frame of the surface it grips), and which invariants every creature should
   carry by default (head above hips, no foot slide while planted, joint limits).
9. **Interop.** The smallest retargeting contract (joint map, rest-pose alignment, scale, non-human maps) and how a
   learned generator plugs in as a prior without the engine running it.

## What we would like back

1. The motion part of the language: `body`, `law`, `solver`, `constraint`, state and checkpoints, written so a model
   author can describe a new creature and its behaviour in a few hundred tokens.
2. The formulation you would choose for §1 to §4, with the reasoning and what it gives up.
3. The three cases written in it, read cold: the crab scramble, the heron tracking a fish while hopping (neck limit,
   then the body turns), a boss reacting to a hit mid-attack and to a second hit before it recovers.
4. Their lowering: dispatches per frame (bucketed by wave and law), state bytes per creature, GPU cost for 1 and 1,000
   creatures, bake cost, and what is checkpointed.
5. The default invariant gates and verdicts.
6. The single choice you think matters most, given our numbers.

Be ambitious. Nothing here has to survive except the measured lessons.

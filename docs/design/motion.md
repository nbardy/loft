# Design goal: motion

Read `docs/PHILOSOPHY.md` first. This page states what motion must be able to do and how it is judged, not how to
build it. Evidence so far: `agent_notes/2026-11-19_motion.md` and the look rounds (heron hop, crab walk, crab
scramble), `agent_notes/2026-11-19_audits/motion_audit_astra.md`.

## Two ends, one system

- **Authored performance**: springy, appealing, Disney-principled motion (anticipation, squash and stretch,
  follow-through, overlap, arcs, timing, exaggeration) for video, film and choreographed boss attacks. The author
  directs; the system adds weight and secondary motion without fighting the direction.
- **Physical, adaptive motion**: Spore-like locomotion that adapts instantly to any body plan, to a player's control
  and to sudden changes of direction, with no prebaked cycles. Feet plant and step from wherever they are; the body
  leans into acceleration; balance is kept or recovered; contact holds.

Both ends are the same machinery: state, laws and constraints. A motion can be fully authored, fully simulated or
any blend, per body part and per moment.

## Capabilities

- **Constraints a consumer can state**: a foot stays planted; a neck has a length and a range of rotation; the eyes
  hold a point and the skull stays aligned (gaze stabilisation, as birds do, within what the neck can reach); joint
  limits; reach; posture bounds; clearance; timing ("lands at 1.2 s").
- **Laws a consumer can write or replace**: springs and dampers, muscle-like actuation, gravity and momentum,
  contact and friction, balance (centre of mass over support, capture point), stepping and gait controllers,
  target tracking (turn the head toward something on the right while the body turns left, stopping at the neck's
  limit, then turning the body).
- **State a consumer can see and own**: plans, momentum, centre of gravity, contacts, controller phase, targets.
  Where it lives is explicit: a closed form of time, a baked table, or a per-frame GPU buffer.
- **Rigging**: any body plan, described once (segments, joints, limits, masses, contacts, eyes), so the same laws
  and constraints work on a crab, a heron, a titan or a creature a model invents.
- **Precompute where it pays, live where it must**: solve and bake authored clips and their holds at build;
  run controllers, constraints and secondary motion per frame on the GPU for anything reactive.
- **Interop**: motion in from outside (clips, motion capture, generated motion from models the consumer runs
  elsewhere), retargeted to the body plan and corrected by the constraints; posed skeletons out.

## Judged by

- **The picture**: does it read as alive and weighted, does it match the direction. Judged side by side.
- **Truth**: a constraint reported as held is held (planted feet do not slide, the neck is never longer than it is).
- **Speed**: per-frame GPU cost per creature; build cost for precompute; many creatures at once.
- **Tokens**: the author's code for a crab that scrambles, a heron that tracks a fish while hopping, a boss reacting
  to a hit, read cold.
- **Adaptability**: a new body plan moves well with no new code beyond its description; any law or constraint can
  be replaced by the consumer.

## Not goals

- A general rigid-body physics engine for scenes (motion may use one; it is not the product).
- Back-compat with existing animation formats beyond import.

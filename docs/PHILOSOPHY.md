# Loft: philosophy and direction

The aim is to move 3D engines forward, in rendering and in motion, by building tools that agents and people keep
rewriting. Loft should support a loop of recursive self-improvement: every consumer, human or model, can replace any
part, from a single shading term to the whole engine, and is helped to do so. The best version of each part wins
because replacing a part is cheap, local and safe.

`CORE.md` turns this into priorities and a definition of done. `docs/design/render.md` and `docs/design/motion.md`
are the two design goals. This page is the stance they share.

## 1. No baked engine

An engine here is a default composition of small functions, not a framework that owns the program.

- **The core is tiny.** It loads a set of GPU programs, resources and an ordered list of passes and runs them on a
  device someone else may own. On the order of hundreds of lines. Everything else is library.
- **Every framework is a recipe.** Any "renderer", "animation system", "physics" or "scene" that Loft ships is a
  composition of the same public pieces a consumer has. A consumer can copy it, edit one line, or throw it away.
  Nothing in a default is privileged.
- **The escape hatch is the normal path.** Every level can be written directly: a maths expression, a WGSL function,
  a draw with its own shader, a compute pass, a whole pass list. Higher levels call lower ones and lower ones can be
  called from higher ones. There is no point where the consumer must stop and ask the engine for a feature.

## 2. Modularity that costs nothing

Composition must not cost speed. Parts are small and many; the program that runs is compiled from the composition.

- **Compile the composition, not the components.** Inline, specialise, fuse passes, fold constants and share
  shaders by kind where the frame is faster that way; keep calls, loops and tables where they are faster. The
  author's structure is for people and models; the GPU sees what is fastest.
- **Load time is precompute, frame time is sacred.** We will spend load-time compute (bakes, solves, pipeline
  compilation, acceleration structures) to buy frame time. Load time must still not grow with the amount of
  content: shaders are shared per kind, instances are data.
- **The engine helps, it never owns.** Because parts are explicit functions in one graph, the engine can analyse
  them: a CPU twin, interval checks, solving for curves, scheduling work to the right stage, choosing analytic vs
  marched vs rasterised. Consumers get that help for their own parts too, and may opt out.

## 3. Consumers define the laws

Rendering and motion are both "state, laws, and constraints, evaluated somewhere".

- **State** is explicit and typed: what persists between frames, and where it lives (a closed form of time, a baked
  table, a per-frame GPU buffer).
- **Laws** are functions the consumer can write or replace: a BRDF, a light transport estimator, a spring, a gait
  controller, a contact model.
- **Constraints** are conditions the consumer can state, that the engine can solve for, enforce, or check: a foot
  stays planted, a neck has a length, a gaze holds a point, a material conserves energy.
- **Truth.** Whatever the engine reports about a constraint is true: proved, measured, or named as unknown.

## 4. Few tokens, no hidden code

The consumer is often a model. The shortest correct program wins.

- Write maths as maths (WGSL text with holes), not as nested calls.
- A new name must buy a capability the consumer did not have.
- Every generated program can be printed and read; errors point at the consumer's own line.
- The documentation a model reads is generated from the code, so it cannot drift.

## 5. Ambition

Set new state of the art in three things at once, without inheriting the limits of current engines:

1. **Speed**: frame time and load time, by choosing the fastest exact method per object and compiling compositions.
2. **Developer experience**: adaptability, flexibility and customisation. Any part rewritten in minutes, by a
   person or an agent, without forking an engine.
3. **Capability**: rendering that mixes analytic surfaces, distance fields, triangles, volumes and particles in one
   frame with physically based light transport; motion that mixes authored performance with physical, adaptive,
   constraint-driven movement on any body.

Prior art is a launchpad. Understand why a technique won, keep it if nothing better exists, replace it if we can do
better, and say which ideas are inherited and which are ours.

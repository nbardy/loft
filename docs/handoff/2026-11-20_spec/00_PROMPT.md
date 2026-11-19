# The ask: write Loft's final syntax, API and design spec (render, light, objects, motion)

You are Loft's chief scientist. You design; we implement; you guide. This bundle is everything you need. Please write
the **final specification** of the Loft meta engine: its language (syntax and semantics), its function and type
signatures, and its systems design, for 3D scenes, objects, lighting and motion. We will implement exactly what you
specify, phase by phase, and come back to you with measured numbers at every phase.

## What Loft is now

A **meta engine**: a small core that compiles and runs compositions; sub meta engines for **render**, **light** and
**motion**, each a set of interfaces, orchestration and library functions; and **example engines** written in one
language (`.loft`, your WGSL superset) that any person or model can read, copy, fork and rewrite. The stance is in
`PHILOSOPHY` below. The most important part of it: **highly technical code is cheap now and lock-in is expensive.**
Expose the hard parts (intersectors, BSDFs, light estimators, schedules, solvers, controllers) as ordinary rewritable
code; never seal them behind a framework. Judge a design by what the consumer can change, not by code saved.

Back-compat does not matter. The old TypeScript API and the old Rust engines are left behind (tag
`legacy/2026-11-19`); every example and seed will be regenerated in the new language.

## What we need from you

1. **The language, final.** Grammar and semantics of `.loft`: your draft (`LANGUAGE DRAFT` below) made final, with
   anything you change and why. Include modules and imports, how a library is written and forked, comptime and
   staging, kinds and rows, queries, schedules, laws, solvers, constraints and verdicts, units, spaces, smoothness,
   derived bounds, `raw wgsl`, and the diagnostics format.
2. **The standard library's signatures.** Every function and type an author or engine writer touches, grouped:
   - geometry: analytic primitives with exact intersections and their Lipschitz axioms; SDF primitives and operators;
     domain maps (mirror, repeat grid and polar, folds, kaleidoscope, spiral, warps with and without inverses);
     curves, frames and scans (parallel transport); surfaces (sweep, loft, lathe, extrude, thicken); meshes in and out;
     instancing; noise with tight bounds and smoothness tags;
   - materials and light: the open `Bsdf` algebra (lobes, layering, energy compensation), lights and emitters, sky and
     environment, media (marched and analytic), the six queries (`trace`, `visible`, `material`, `light`, `sky`,
     `medium`) as interfaces, the light estimators (direct, many-light, shadows from every geometry kind, GI,
     reflections, temporal reuse) and the reference path tracer;
   - non-PBR looks: how a toon, ink, woodblock, painterly or neon look is written as a composition of the same pieces,
     replacing any lobe, the shade step, the light fold, or the whole schedule;
   - camera and post: exposure, tone maps, depth of field, motion blur, bloom, grain, grading, outlines;
   - motion: bodies, ends and pins, laws, solvers, constraints, controllers, clips and retargeting, gaze, stepping,
     balance, springs, reactions (see `MOTION HANDOFF` below; that note lists the questions we most want answered);
   - schedules: the pass graph, rates, tiers, culling and indirect draws, what the compiler decides and what the
     author may override.
3. **The systems design.** The four layers (runner, core IR and gates, regenerable compiler passes, library and
   engines), the data model (kinds, rows, tables, state store, checkpoints, visibility record), the lowering rules
   (one module per kind, dependency-wave batching, (wave, law) bucketing, RenderBundle plus one `drawIndirect` per
   kind), Tier 0/Tier 1, and the gates.
4. **Example engines, written out**: `realtime` (vis-buffer deferred plus forward translucency plus traced
   reflections), `reference` (path tracer), one stylised engine (toon or woodblock), `roam` (Spore-like adaptive
   locomotion) and `act` (Disney-style performance). Each as `.loft` source an agent could fork.
5. **Three scenes, written in full**: the hermit crab boss (shell, claw, face, legs, antennae, wet chitin, reacting to
   a hit), a heron hunting (gaze locked on a fish while hopping; neck limit; then the body turns), and a stylised
   scene that breaks PBR entirely.
6. **The escape story, explicitly**: for each of these, what the author writes to fork it, and what they keep for
   free (depth, shadows, analysis, motion verdicts): a single BSDF lobe; the whole material model; the light
   transport; one shader of one kind; the visibility record; the schedule; a motion law; the solver; the whole engine.
7. **Predicted numbers** for the three scenes at 1280×800 on an Apple M4: shader modules, pipelines, GPU commands per
   frame, p50/p95 frame time, cold/warm load, motion cost for 1 and 1,000 creatures. We will measure and report back.
8. **The order to build it in**, as vertical slices with pass/fail checks, so each phase ends in a picture and numbers.

Read in this order: `PROMPT` (this), `STATE AND LESSONS`, `PATTERNS TODAY`, `PHILOSOPHY`, `CORE`, `META ENGINE
PLAN`, `LANGUAGE DRAFT`, `RENDER ARCHITECTURE`, `MOTION HANDOFF`, `FOLDING AND TRACING`, `REFERENCES`, then the
appendices (`LOFT CONTEXT`, `SIGNATURES`) as the record of the old API's names and coverage. Be ambitious; nothing has
to survive except the measured lessons.

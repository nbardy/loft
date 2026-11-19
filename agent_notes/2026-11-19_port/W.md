# W · Emission tape and scene declarations

## Input API for B, S and D

All types below live in `loft_lower::input`. Node IDs index `Input::graph().nodes`, Kernel ordinals index
`graph().kernels`, and the raw-function IDs exposed by the typed API index `graph().raws`. The wire still stores
the shared Fn ordinal space. The Thing payload exposes `FnRef` so draw lanes can name graph and raw functions.
`graph()` and the canonical graph probes retain the watched program roots. Lowering uses the typed requests and
ordered effects for their individual root groups.

```rust
Input::graph(&self) -> &loft_ir::graph::Graph
Input::things(&self) -> &[Thing]
Input::effects(&self) -> &[Effect]
Input::style_emits(&self) -> &[EmitRequest]
Input::lighting(&self) -> Option<&Lighting>
Input::lamp_passes(&self) -> &[LampPass]
Input::posts(&self) -> &[Post]
Input::compute_passes(&self) -> &[ComputePass]
Input::recurrences(&self) -> &[Recurrence]
Input::pass_defs(&self) -> &[PassDef]
Input::scene(&self) -> Option<&SceneDecl>
Input::cameras(&self) -> &[Camera]
Input::authored(&self) -> &[Authored]
Input::cast(&self) -> Option<&Cast>
Input::raw_flags(&self, raw: u32) -> Option<&RawFlags>
```

- B: `Effect { stage, op }` preserves call order. `EffectOp::Register(Box<[u32]>)` carries roots,
  `EffectOp::Declare(u32)` carries a Kernel ordinal, and `EffectOp::Lower(Program)` carries the rate
  program before its register call. Declarations and registrations inside classification are logged too.
  `ComputePass { kernel, reads }` identifies its compute Kernel and each ordered read of a producer output.
  `Recurrence { kernel, name, total, start, after, replay }` adds the authored fields beyond the existing Kernel fill.
  The fill already carries the lanes, cell shape, rate, clock and index, start and step roots, and previous-table
  reference read by `compile/recur.ts`. The companion declaration adds bake and replay scheduling fields.
- S: `EmitRequest { name, args, ret, roots, pick }` carries WGSL argument names and types, a return type string,
  and the selected root index. `Thing { kind, name, payload, tape }` keeps its `Emitted` beside the per-kind payload.
  `Emitted { shaders, draws, posts, notes }` preserves order. A shader has `key, code, depth_entry, mixed_code, libs`.
  `SceneDecl { sun, ground, clear, outline, period, screen_ink }` supplies the default sun direction and scene settings.
  `PassDef` distinguishes given scene cameras from screen passes. `LampPass` adds the rest spread roots.
  `posts()` is the actual scene post order, including the ink post when present. `cast()` holds the recorded text and raws.
- D: `ThingPayload { words, lits, fns, blobs }` holds a lane's own schema, empty at step 1. Blob offsets are in
  `graph().blobs`. The TS writer can encode either a `Role` or a `LibFn` in the Fn column. Tape probes are named
  `tape.<scene ordinal>.<kind>`. They list shader libraries by name and print grid rest roots as a canonical graph.

## Translation

| TypeScript | Rust | Lines |
|---|---|---:|
| `src/record/tape.ts` with observers in `src/compile/{index,bind,tables,scene_wgsl}.ts` | `loft-lower/src/input.rs` and `input/{types,decode}.rs` | 354 TS tape / 875 Rust |
| New layouts in `src/record/{wire,recorder}.ts` | `loft-ir/src/{wire,graph}.rs` | 66 TS / 110 Rust added |
| Canonical declarations in `src/record/print.ts` | `loft-ir/src/print.rs` | 35 TS / 55 Rust added |

The parked path keeps its tag numbers. Port job tags 14–27 use the wire-5 container. Screen uses a new Pass target
variant. Pass and raw metadata can reference forward declarations. A raw dependency still precedes its reader.
`src/authored.ts` exposes the existing registry in insertion order so the tape can capture it before opening any
additional closures. The harness writes the new probes after the artefact, and `--mode ts` compares outcomes,
the compile boundary and the artefact files. Wire and tape probes are scoped to their port gates.

## Prediction and baseline

Pre-edit `--mode lower --run w-base` passed 60/302. The stub first differed at `files.manifest.format` for all
241 built artefacts. `examples__serpent_clip` crashed because its default is a film for `tools/film.ts`. The port
harness now excludes that film fixture. Its underlying scene, `examples__serpent`, remains in the corpus.
The prediction was 241 matching wire prints, 60 module refusals and zero differences in TS artefact bytes.

## Verification and handoff

- `--scope W --run w` passed 301/301. All 241 built scenes wrote jobs and matching `probe.wire` files. The other
  60 entries refused in their modules before compiling. No sections failed.
- `--mode ts --baseline out/port/w-base --run w-ts` passed 301/301 with no differing sections. All 241 TS artefacts
  and their compile boundary sections matched. The 60 module refusals matched as outcomes.
- The scratch screen fixture round-tripped two screen passes with a dependency, an unused compute pass and an
  orthographic camera. A separate typed lamp-shadow compile refusal wrote a job, including its unused compute pass,
  and its wire print matched. It carries the declarations the compiler reached before refusing.
- `rtk proxy pnpm exec tsc --noEmit`, `tools/dev.sh build loft-lower --fast` and `tools/dev.sh lint loft-lower` passed.

B and S take over replay, assembly and refusal reproduction. Per-kind payloads and generated Rust tape probes are
the draw lanes' step 2. Until D4 translates cast generation, the tape carries the emitted cast text and its library
references.

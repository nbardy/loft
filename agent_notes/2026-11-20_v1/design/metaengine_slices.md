# Meta-engine slices: ready-to-run briefs

The owning design is [metaengine.md](metaengine.md). Read §1, the section of your slice, §5 and §6 before you
start. The interface slice S0 is complete on branch `loft/metaengine-core`. It is not on main: it adds 445 counted
lines whose deletions arrive with S1–S3, and the never-grow ratchet (`tools/lines.py`) refuses any intermediate
growth (metaengine.md §5.0). Its frozen types are:
- `crates/loft-check/src/typed.rs`: `Typed`, `Resolved`, `NameRef`, `MemberRef`, `Callee`, `Intrinsic`, `Op`;
- `crates/loft-std/src/linked.rs`: `link`, `Linked`, `LinkedPass`, `Step`, `Stage`, `Slot`, `Work`, `IndexWidth`,
  `LinkError`;
- `loft_ir::execution::Clock::Tick`, wire tag 2.

## Rules for every slice

- **Precondition:** `lane/integrate-meta` has landed on main. It carries probe-door, cut-tape, cut-unused,
  probe-rows and merge-2026-11-23. The lead then creates the integration branch `loft/metaengine` from that main,
  merges `loft/metaengine-core` (S0) into it, and lands S0b (below) on it. Start from that S0b SHA and record it.
- **Worktree:** `git cambium add -b loft/me-<slice> -name me-<slice> -ephemeral ~/git/_wt/loft-me-<slice> <S0b SHA>`.
  Never work in the primary checkout: other sessions have uncommitted files there.
- **Files:** edit only the files your brief owns. If you need another file, stop and report the file, the line and
  the reason (kill K4). Do not edit it. Lean1 failed because four lanes collided at one boundary.
- **Builds:** write the whole change first, then compile:
  - crate checks: `~/.local/bin/agent-build --light tools/dev.sh check <crate>`;
  - workspace check and tests: plain `agent-build`;
  - Picture, Twin and Truth: `agent-build --gpu` around `loft-verify gate`;
  - Speed and Load: `agent-build --gpu --heavy` with the quiet conductor.

  Aim for three builds. Clippy on `loft-abi` already fails on main (`det.rs:33`, excessive float precision). Do
  not fix it in a slice; report it.
- **Evidence:** commit before any run you cite, and put the SHA in your report. Record each prediction in §6 of
  metaengine.md as held or missed in a dated results section at its end. Do not edit §1–§7.
- **Merging:** do not merge your own slice. The lead integrates into `loft/metaengine` in the order S3 → S1 → S2,
  running the full gates after each one. Main receives `loft/metaengine` once, only when the ratchet and every gate
  pass (kill K6). Report your slice's counted delta per crate so the lead can track the running total.
- No paid APIs. Do not touch the `perf-*` branches or `auncel/*`.
- Commit trailer: `Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>`.

---

## S0b: the mechanical prep commit (lead, one commit on `loft/metaengine`, before any slice)

**Outcome.** File ownership becomes disjoint, with no change in behaviour.

**Steps.**
1. Move `emit_kind_module` and its private helpers from `crates/loft-emit/src/emit.rs` into a new
   `crates/loft-emit/src/emit/kind.rs`. Find the code by name, because the post-integration line numbers moved.
   - Make the `Emitter` methods that `kind.rs` calls `pub(crate)`. Expected: `new`, `expr`, `stmt` and the
     function-emitting method. Record their exact signatures in the commit message. S1 must keep them.
   - Re-export from `emit.rs` so that callers are unchanged.
2. Move `Eval::eval_layers` and its helpers from `crates/loft-eval/src/eval.rs` into a new
   `crates/loft-eval/src/layers.rs`, as a second `impl<S: Scalar> Eval<'_, '_, S>` block.
3. Add `crates/loft-check/src/frame.rs` with these hooks:
   ```rust
   pub(crate) fn index_target(ck: &mut Ck, base: &Ty, index: &Expr) -> Res<Ty>;           // `T[px]` on a target
   pub(crate) fn kind_member_call(ck: &mut Ck, kind: &str, query: &Ident, args: &[Arg]) -> Res<Ty>;  // `K.hit(..)`
   pub(crate) fn instantiate(ck: &mut Ck, kernel: &KernelDecl, args: &[Arg]) -> Res<()>;   // `k[K, f]`
   ```
   Each hook returns the diagnostic that `check.rs` produces for that form today. Call each one from the matching
   site in `check.rs` (the `Index` arm on a non-array base, a call whose receiver is a kind name, a kernel entry with
   bracket arguments). S1 owns `check.rs` and S2 owns `frame.rs`.
4. Replace every `types: &mut Vec<Ty>` parameter and argument in the workspace with `typed: &mut Typed`, using
   `typed.types` where the vector was used. Replace the `Ty` writes in `Ck` (`self.types[i] = …` with resize) with
   `typed.set_ty(e, ty)`. This is about 30 sites. No behaviour change.

**Gates.**
- workspace check and tests, with test names and results identical to the parent;
- `tools/lines.py` total within ±10 of the parent;
- `tools/check.sh static`;
- one `loft show` of each stdlib engine, with byte-identical WGSL against the parent.

**Kill (K1).** If any move needs a behaviour change to compile, stop. Run the slices serially (S3, then S1, then S2)
instead of in parallel.

---

## S1: the typed program (`loft/me-typed`)

**Outcome.** Checking records the resolution of every name, member and call once. Evaluation and WGSL emission each
dispatch on `typed.op(e)` with one exhaustive match. Nothing re-resolves `callee.name` or re-tests shadowing.

**Owns.**
- `crates/loft-builtins/**`
- in `crates/loft-check/src/`: `check.rs`, `typed.rs`, `references.rs`, `infer.rs`, `kernel_units.rs`, `world.rs`,
  `bsdf.rs`
- in `crates/loft-eval/src/`: `eval.rs`, `lip.rs`, `refine.rs`, `ad.rs`, `lib.rs`
- in `crates/loft-emit/src/`: `emit.rs`, `emit/column.rs`, `provenance.rs`
- their tests

**Consumes.** `Typed`, `Resolved`, `NameRef`, `MemberRef`, `Callee`, `Intrinsic`, `Op` (S0). You may add variants to
`NameRef`, `MemberRef`, `Intrinsic` and `Op`. You may not rename or remove:
- `Typed::{ty, set_ty, resolve, resolved, op}`
- `NameRef::{Fn, Kind}`, `Callee::Fn`
- `Op::{Call, FnValue, KindRef}`

S2 reads all of these. Keep the `pub(crate)` `Emitter` signatures recorded in the S0b commit, and keep the
`frame.rs` hook call sites in `check.rs`.

**Steps.**
1. `Ck`: at each site that decides a name (`Ck::expr_inner` `Name` arm), a member (`Member` arm, including map
   `.cell`/`.to_parent` and kernel atomics) or a call (`Ck::call`, including the shadowing precedence, world queries
   and intrinsics), call `typed.resolve(e, …)`. Expressions whose syntax is their meaning stay `Syntax`.
2. `Eval<S>::expr_inner`: replace the name chain (time, consts, params, worlds) and `Eval::builtin`'s name-to-id
   mapping with `match typed.op(e)`. The numeric rules per builtin and per `Scalar` domain stay as they are.
3. `Emitter::expression`: replace the guard chain with `match typed.op(e)`:
   - `"workgroup_barrier"`, `"sky_zenith"`, `"held"` and the other string arms;
   - `!self.p.fns.contains_key(..)`;
   - the `WorldCall::resolve` re-run;
   - `is_lobe`.

   Keep the emitter-state guards (`baked_column`, prepared-light `eval_bsdf`) inside their arm.
4. `references::motion_math`: read `Callee::Builtin(id)` and the catalog `MotionClass`, and delete the name list.

**Tests and gates.**
- grammars 330/330 (`tools/check.sh static`);
- diagnostic snapshots identical (no span moves);
- **P1:** byte-identical module text for every module of the 45 gate/corpus scenes and the 11 stdlib engines,
  diffed with `loft show` against the S0b SHA;
- Picture 19/19, Twin 36/36, Truth 34/34;
- workspace tests.

**Predicted lines (P2).** +170 / −420, net −250 ± 100 counted Rust. Report actual numbers per crate.

**Kill (K2).** If any module text changes and you cannot explain it as a pre-existing nondeterminism, or the net is
worse than −100: stop, keep only step 4, and report.

---

## S2: the engine as plan producer (`loft/me-plan`)

**Outcome.** The three programs in metaengine.md §1 compile as written and change what they claim to change. The
stock engines become library code with the same names, arguments and defaults. Rust no longer constructs camera,
visibility, shadow, shading, full-screen or material-fold programs.

**Owns.**
- `crates/loft-parse/**` and `docs/spec/loft.lark`
- in `crates/loft-check/src/`: `defaults.rs`, `source.rs`, `source/scope.rs`, `frame.rs`
- `crates/loft-passes/**`
- in `crates/loft-emit/src/`: `emit/kind.rs`, `emit/shade_tiles.rs`, `emit/paths.rs`, `emit/lighting.rs`,
  `emit/surface.rs`, `material_shape.rs`, `bake_tex.rs`
- `crates/loft-eval/src/layers.rs`
- `stdlib/**` and `engines/**`

**Consumes.**
- `Typed::{ty, op}`, with `NameRef::{Fn, Kind}` and `Op::{Call, FnValue, KindRef}`, to bind function- and
  kind-valued schedule and kernel arguments. Never read `callee.name` to decide meaning.
- The `Emitter` `pub(crate)` API (S0b).
- `loft_std::linked::link` in your tests, to assert the plans you produce: steps, stages, slots and work.
- `Clock::Tick` for `on tick` passes.

**Steps** (design §1 table, §3).
1. Grammar and parser:
   - `for k in <kind set> { … }` in a schedule;
   - bracket compile-time parameters on `kernel` (`kernel f[K, light](…)`) and at use (`entry: f[k, sun_sky]`);
   - `target T: format, extent` at top level;
   - `on tick` beside Probe A's `on load`/`on dirty`.

   `repeat(n) { … }` already parses as `expr block`. Budget 24 grammar lines, with the second grammar path a
   symlink. Both recognizers must accept the new forms.
2. `frame.rs`:
   - image indexing on a `target` (format and extent from its declaration; reading another pixel of a store the same
     pass writes is E505);
   - `K.hit/material/trace/id` on a bound kind;
   - instantiation of bracket parameters, with errors at the user's argument and a note at the use inside the
     kernel.
3. Stage 0 (`pipeline/schedule.rs`, `stage0.rs`): evaluate a schedule into plan steps (`Store`, `Pass{clock, run}`,
   `Repeat`). `run` is one of `Kernel`, `Template`, `Raster`, `Laws` or `Mips`. Lower them in `bundle.rs` to the
   existing records. Resolve the symbolic extents `pixels(T)`, `rows(K)` and `indirect(T, k)` against the compile's
   size. Specialise once per argument tuple and name each instance `f[K]`.
4. Move the stock behaviour into `stdlib/render/`:
   - `camera.loft`, `visibility.loft`, `shadow.loft`, `shade.loft` (`shade_tiles` + `shade_tile[K, light]`),
     `screen.loft`, `display.loft` (`outline_and_paper`) and `frame.loft` (`target` declarations);
   - in `stdlib/materials/bsdf.loft`: the `eval_bsdf` fold and the `ggx_energy` lookup.

   Delete the Rust these replace (§3.2 table). **Keep each stock pass template-backed (Probe A form) until its
   typed-kernel form passes P3. A stock pass may stay template-backed indefinitely.**
5. Add `stdlib/styles/hatch.loft` (1b) and `stdlib/motion/xpbd.loft` (1c), and the fork test for 1a.

**Tests and gates.**
- three fork tests, each showing the user's copy changes the shader or state while the stock engine's module text
  does not change: 1a with draw order and the lamp, 1b with hatching, and 1c with the rope plus a Det replay against
  the Naga twin over 481 ticks;
- grammars;
- Picture 19/19 (deterministic tolerance; template-backed stock passes byte-identical);
- Twin 36/36 and Truth 34/34;
- **P4:** pipeline and module counts per gate scene ≤ the base;
- **P7:** Speed and Load paired, neutral or better;
- **P6:** no scene or engine file edited.

**Host dependency.** An `on tick` pass needs S3's tick loop to *run*. Until S3 integrates, the runner refuses with
`Unsupported("tick-clock passes need the host tick loop (metaengine S3)")`. Prove 1c with `link`, the bundle and the
CPU twin. Its GPU replay runs at integration.

**Predicted lines.** Rust +520 / −880, net −360 ± 150. stdlib +600 ± 150 Loft lines.

**Kill (K3).** If a scene's pipeline or module count rises, or its p95 regresses: keep that pass template-backed and
report it. Two failed fixes in one subsystem means stop and write up the failure class (K5).

---

## S3: the host plan reader (`loft/me-host`)

**Outcome.** Native, the browser and inspection all read a bundle through `loft_std::linked::link`. The structural
validation in `loft-wgpu::Schedule::new` and the duplicates in `loft-std::bundle` are deleted. The runner executes
`Stage::Tick` passes once per due tick before the frame.

**Owns.**
- `crates/loft-std/**`, except the frozen signatures in `linked.rs`. You may add `LinkError` variants, and fields
  that nothing else reads.
- `crates/loft-wgpu/**`, `crates/loft-runner/**`, `crates/loft-native/**`, `crates/loft-cli/**`
- `web/**` and `tools/loft-verify/**`

**Consumes.** `Bundle` (the frozen LOFTB001 wire) and `link`, `Linked`, `LinkedPass`, `Step`, `Stage`, `Slot`,
`Work`, `IndexWidth` and `LinkError` (S0). Do not change `link`'s signature or the shape of
`Step`/`Work`/`Stage`.

**Steps.**
1. `Schedule::new(b, features)` becomes `Schedule::new(&Linked, features)`. Keep naga reflection, outbox discovery and
   `groups` (device-dependent). Delete the structural checks that `link` now performs.
2. The runner (`Instance::new`, `ComputeCache::prepare`, `Initializers::prepare`) reads `Linked` once: descriptor,
   views, stages and additive. Delete the second decode and the range bookkeeping. Map each `LinkError` to its
   existing `RunnerError`, so the public results are unchanged, and keep the `LinkError` text in the message.
3. Tick loop: `Stage::Tick` passes run once per due 120 Hz tick before the frame passes. Use the same catch-up limit
   and `AdvanceArgs` clock as fused laws, including checkpoint and restore. Replace the `Unsupported` refusal that S0
   placed in `initialize.rs`.
4. Browser: in `web.rs`, project `Linked` against `HostCaps::WEB_BASELINE` before creating anything. Keep the host
   shader callback.
5. `loft show --plan`: print `Linked` steps, stages, slots and work with no device.

**Tests and gates.**
- **P10:** a test or `loft-verify` check that links every bundle the corpus build produces. Every one must link,
  with no new refusals.
- the existing runner, wgpu, native and std tests;
- one browser projection test;
- the wasm32 check (`tools/check.sh full` includes it);
- Picture 19/19, Twin 36/36, Truth 34/34;
- Load paired, neutral or better.

**Predicted lines.** +160 / −280, net −120 ± 60 counted Rust. Runner ≤ 2,200 and CLI native ≤ 450 stay within their
ceilings. `loft-std` keeps zero wgpu dependencies.

**Kill (K5 / P10).** If any corpus bundle is refused, fix `link` (you own its body). If the fix needs a signature
change, stop and report.

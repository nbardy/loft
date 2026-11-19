## 1. Architecture Thesis

Loft is a **meta engine**: a tiny, slow-changing GPU runner and typed SSA IR (`crates/loft-{runner,ir}`), modular regenerable compiler passes (`crates/loft-passes`), and **separate, table-coupled sub-engines** for **render**, **light**, and **motion** written in one lean language (`.loft` v1.0).

* **Hard code is cheap; lock-in is expensive.** Every intersector, BSDF, light estimator, schedule, motion law, and solver is ordinary forkable `.loft` source. Nothing in a shipped engine is privileged.
* **Separate, compatible engines.** Motion engines (`roam`, `act`, custom solvers) and render engines (`realtime`, `woodblock`, `reference`, custom schedules) never import each other's internals. They communicate strictly through plain GPU storage tables (`World` surfaces in; `POSE` or custom geometry/control-point tables out). Motion runs completely headless for RL, baking, and server simulation; render runs static with zero motion passes.
* **Supersedes Syntax A (Phase 1) cleanly.** Phase 0 and Phase 1 proved the Layer 0–2 spine on Syntax A (`8` modules, `13` pipelines, `17` commands for 1 and 20 crabs **measured**). Swapping only `crates/loft-parse` (~1,390 LOC **predicted**) and porting the `282`-line (**measured**) Phase 1 `lib/` to v1.0 E+ cuts scene tokens by ~35–42% (**predicted**), enables clean `llguidance` constrained decoding via `loft.lark`, and closes the top cross-cutting bakeoff bugs before Phase 2–5.

## 2. Frozen Design Decisions

1. **Lean E+ surface (`.loft` v1.0) with `wgsl(...) -> T { ... }` escape blocks; zero TypeScript.** Beats WGSL superset A by −26% to −42% tokens (`4,709` vs `6,365` **measured** for E vs A) without host-tracing blowups (`707k` nodes **measured**) or Python/TS false-friend leaks (`+5` Haiku violation gap in D **measured**).
2. **Four layers: Runner (≤ 1,000 LOC), Core IR + Gates (≤ 1,500 LOC), Regenerable Passes (~10k LOC), Library + Engines (unbounded).** Zero domain words (`foot|gait|creature|trunk|pbr|shadow|sea`) in `crates/loft-{runner,ir}`.
3. **`kind` is the sole unit of code identity; continuous values live in `row` tables.** Module count is $O(K_{\text{kinds}})$ by construction (1 vs 20 crabs produce byte-identical modules **measured**); continuous `comptime` kind params are `E401`, > 4 variants are `E402`.
4. **Explicit `build` + `emit` staging (`build { ... }`, `build fn`, `build for`).** Preserves A's `0`-gap Haiku staging safety (**measured**) at 1 token while forbidding bare top-level statements (`E404`) and struct-literal row confusion.
5. **Explicit `let` / `var` bindings + `a.f = e` restricted to `law`/`solver`.** Eliminates newline/unary-minus ambiguity so `loft.lark` is strictly context-free for `llguidance` constrained decoding.
6. **`@` reserved exclusively for spaces (`@Body`, `p3@World`, `xf@Leg>World`).** Zero `@attributes`; schedule/grid/checkpoint modifiers use plain keywords (`grid 96x32`, `checkpoint 60`, `on bake_lut`).
7. **Kind-scoped value `map` (`map sym(p) = ...` with `sym.p`, `sym.pt(...)`, `sym.cell`, `sym.out(...)`, `sym.in(...)`).** Eliminates `'sym` tick-spaces, prevents unbound file-scope maps (A-Haiku **measured**), and stops duplicated mirror chains across `trace`/`material` (4/4 D/E outputs **measured**).
8. **Range-refined row columns (`stiff: f in 0.2..2.0`, `k: m > 0m`) + compiler-derived Lipschitz $L$ and $C^k$ smoothness.** Eliminates unclamped row-divisor E211 bugs (`7/10` bakeoff outputs **measured**); authoring a numeric $L$ is `E210`.
9. **Disjoint field-level write ownership (`writes h.{a, b}`) + automatic intra-tick law fusion.** Fixes the #1 bakeoff bug (`5/8` Round-1 outputs had shared writers **measured**) while fusing disjoint row-local laws on the same kind back into one per-tick `advance(from, to)` dispatch (`0.086 ms` / 1,000 crabs **measured**).
10. **Space-polymorphic helpers (`fn chitin(p, n)` generalizes `S` automatically).** Fixes A-Sonnet and C-Sonnet shared-material E104 errors (**measured**) at zero token cost.
11. **64-bit `rgba16uint` 4× MSAA `VisRec` + `depth32float` (12 B/sample) with `kind_id` packed in word0.** Compatible with `wgpu 30.0.1` MSAA rules, reproduces R0 coverage (`6,341` → `3` px **measured**), and remains a replaceable library struct.
12. **Portable $O(K_{\text{kinds}})$ draw path: GPU cull → `RenderBundle` + 1 `drawIndirect` per kind.** Identical on native `wgpu` and browser WebGPU without requiring `multi_draw_indirect`.
13. **Open staged `Bsdf` algebra + 6 `World` queries (`trace`, `visible`, `material`, `light`, `sky`, `medium`) + joint `(L, T)` RGB media.** Zero runtime closures; compiles to per-kind `K_eval`/`K_sample`/`K_emit`/`K_albedo` with Kulla–Conty compensation.
14. **Tier 0 GPU Tape VM is edit-time only, single dirty kind, workgroup-uniform, 32-vec4f register cap, never for motion.** Bounds `+2.0 ms` (**measured** full-screen 63-op tape) to ≤ `0.45 ms` (**predicted**) with < 2 ms hot-reload.
15. **Compliance-coupled motion over slice-indexed `BODY`/`STATE`/`POSE` + custom output tables, bit-identical per device via one `advance(from, to, keep)` entry.** Preserves `0/481` bit-diff live/bake parity (**measured**), supports arbitrary Spore-like morphologies in 1 shader module, and permits non-rigid motion outputs.
16. **Canonical S-expression IR codec (`loft ir --sexp`) over Layer 1 IR.** Gives RSI agents Lisp-style code-as-data rewriting (~250 LOC) without maintaining a second human surface or type checker.
17. **Default `schedule` + prelude `Sun`/`Sky`.** Omitting `schedule Main` runs `engines::realtime::realtime` over all emitted drawable kinds, cutting ~120–150 tokens of scene boilerplate and preventing omitted-kind bugs.

---

## 3. Scope: Flagship Pack #1 on an Unconstrained Substrate

Layers 0 and 1 know nothing about creatures, PBR, fluids, splats or neural nets: they are typed memory arenas, a pass
DAG with GPU-driven dispatch and loops, SSA math, deterministic host and GPU execution, pluggable backends, and gates.
Procedural 3D, light and creature motion (`std::geom`, `std::light`, `std::stylised`, `std::motion`, the five engines)
are **Flagship Pack #1**, built in Phases 1.5–5. Fluids, Gaussian splats, reverse-mode differentiation, neural
shaders, spectral optics and ECS are future packs. They are not built now, and their seams stay open
(`PATCH_V1_2.md` §6).

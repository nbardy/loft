# Lane: plan the full Rust implementation of Loft (design only; worktree ~/git/_wt/loft-rust-core, branch feat/rust-core)

Owner's model: the DSL (scene authoring: the TypeScript API and the `wgsl` text template) stays in TypeScript and only
RECORDS the expression graph; everything after that is Rust: interner/arena, folding, lowering, WGSL emit, tables,
CPU twin, interval/ranged checks, `solve`, artefact writer, and possibly `loft serve` drawing through wgpu (sharing
code with rust/loft-loader). Today almost all of it is TypeScript (~36k lines); the Rust core in this worktree has only
`solve` and shader sharing behind LOFT_CORE=rust. Read: agent_notes/2026-11-18_rust_core.md (what was found: graph
construction runs library work in JS at construction time; expression ids in WGSL block bytecode bundles),
docs/proposals/2026-11-18_compiler_speed.md, docs/proposals/2026-11-18_authoring_api.md, src/expr.ts, src/compile/,
src/host/, src/ranged.ts, src/interval.ts, src/geom/solve.ts, rust/.

Write docs/proposals/2026-11-19_rust_loft.md (3,000–5,000 words):
- The boundary: exactly what the TS recorder does (node records into typed arrays; how callbacks such as styles,
  surfaces, `iterate` steps, map/scan bodies run only to produce graph, never to evaluate it; how `wgsl` text lowers
  into the same records), the handoff (one call per program; Bun FFI vs wasm, decided with reasons), and what the
  model-facing API keeps unchanged.
- Derived cost per phase after the port for Owl, Titan and heron (use the measured profiles; state the per-node
  costs assumed), and peak memory.
- Module order with a byte-identical parity gate per step (the 63 examples that build today; typed refusals equal
  for the rest), each step small enough for one gpt-6.2-sol lane; which steps can run in parallel without editing the
  same files. Make WGSL names independent of creation order an early step (it also unlocks bytecode bundles and warm
  workers).
- What happens to the CPU twin, checks and `solve` (Rust implementations; JS kept only as a reference until parity).
- `loft serve` on wgpu in Rust vs keeping Dawn: decide with reasons (the game loader is wgpu 30).
- Risks and what stays in TS permanently.
Do not change code. Commit only the doc on feat/rust-core (stage by path; message ending with
Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>). Final message: 200-word summary.

# Loft

Loft compiles `.loft` scenes into GPU programs. The Rust workspace at the repository root
contains the compiler, native CLI and embeddable wgpu runner. The runner also builds for WebAssembly.

## Render a scene

Install rustup and the toolchain pinned in `rust-toolchain.toml`. From the checkout, run

```sh
tools/dev.sh build loft-cli --fast
tools/gpu_lock.sh target/lab/loft still engines/capsules.loft out/capsules.png --t 4 --set count=20
```

`loft show engines/capsules.loft` prints the generated WGSL. `loft build` produces a bundle
for the runner. Start authoring with the [language card](AGENT_CARD.md) and the [v1 specification](docs/spec/README.md).

## Workspace

| Directory | Contents |
| --- | --- |
| `crates/` | ABI, IR, parser, passes, plan evaluation, bundle format, standard library, runner and CLI |
| `lib/`, `engines/`, `scenes/` | Authored prelude, rendering engines and runnable scenes |
| `corpus/` | Phase 5 ports, source identities and grammar inputs |
| `tests/` | Shared fixtures, frozen reference frames and gate mutants |
| `examples/` | Read-only Phase 5 source material |

## Verification

`tools/check.sh full` checks the corpus and grammar, builds the workspace, runs native tests
including all six mutants, checks the wasm32 runner, scores nine bead observations and runs the
five GPU gates once. The checks use `uv`, `rg` and a timestamp-capable GPU. Cargo and GPU work
use separate machine locks. For a focused edit, use `tools/dev.sh check CRATE` or
`tools/gpu_lock.sh tools/dev.sh test CRATE --test-threads 1` (cargo-nextest).

The GPU report is `out/meta/gate.json`. The CLI returns a nonzero status for missed gates.
Main's two outstanding Load verdicts and the Pictures owner queue are recorded in
[OPEN.md](agent_notes/2026-11-20_v1/OPEN.md). The [project priorities](CORE.md) define how changes are judged.

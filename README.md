# Loft

LOFT is a 3D meta-engine designed natively for LLM generation. It has four core parts:

1. **[LOFT Lang](WHY_LOFT_LANG.md)**: a thin transpilation layer on top of WGSL and wgpu, with escape hatches for
   pure machine-specific native code.
2. **LOFT Libs (standard)**: common libraries and patterns, so for most use cases you can just draw shapes and objects
   and get a new model, but also composable and forkable for building your own engine, lighting and render styles.
3. **LOFT Motion**: bodies, state, laws, solvers and constraints, written in the same language and run on the GPU
   next to rendering. Motion is deterministic and has a CPU twin, so it can be tested and trained.
4. **LOFT Core**: a tiny Rust compiler and wgpu runner, native and in the browser, that you can embed in your own
   engine. Every scene is checked by gates (pictures, speed, truth, CPU twin), so every change, whether a person's
   or a model's, is verified.

A "meta-engine" is an engine designed to be the opposite of a traditional framework, which locks you into design
decisions and writes the "hard" code for you. Instead it is a collection of tools and patterns that are always
forkable and never lock you in. Now that LLMs can write state-of-the-art code in maths and algorithms, traditional
engines hold you back. Instead, Loft lets your LLM write specific GPU code that is blazing fast for your use case.

---

> **Slop line.** Everything above was written concisely by a human. Everything below is mostly LLM-written, for other
> LLMs to read. Follow the links rather than expecting it all here.

## Start

- **Write a scene:** [agent card](AGENT_CARD.md) (one page), then the [v1 spec](docs/spec/README.md).
- **Why a language:** [WHY_LOFT_LANG.md](WHY_LOFT_LANG.md).
- **The stance:** [philosophy](docs/PHILOSOPHY.md). **How changes are judged:** [CORE.md](CORE.md).
- **Where things stand:** [open targets](agent_notes/2026-11-20_v1/OPEN.md) and
  [notes and TODO](agent_notes/2026-11-20_v1/NOTES_AND_TODO.md).
- **Working in this repo (agents):** [AGENTS.md](AGENTS.md).

## Render a scene

```sh
tools/dev.sh build loft-cli --fast
tools/gpu_lock.sh target/lab/loft still engines/capsules.loft out/capsules.png --t 4 --set count=20
```

`loft show <scene>` prints the generated WGSL. `loft build` produces a `.loftb` bundle for the runner.

## Layout

| Directory | Contents |
| --- | --- |
| `crates/` | Core: ABI, IR, parser, passes, plan evaluation, bundle format, std, runner, CLI |
| `lib/` | Libs, in `.loft`: geometry, SDFs, BSDFs, light, ReSTIR, noise, post, motion |
| `engines/` | Example engines (forkable): `reference`, `realtime`, `woodblock`, `spectral_caustic`, `hermit`, … |
| `scenes/` | Runnable scenes, including the motion scenes (`crab`, `heron`, `boss`) |
| `grammar/`, `docs/spec/` | The normative grammar and spec |
| `corpus/`, `tests/`, `examples/` | Phase 5 corpus, gate fixtures and mutants, read-only old sources |

## Verification

`tools/check.sh full` runs the corpus and grammar checks, the workspace build, native tests with all six mutants, the
wasm32 runner check, and the five GPU gates once. Focused: `tools/dev.sh check CRATE`. The GPU report is
`out/meta/gate.json`; the CLI exits nonzero on a missed gate. Details: [AGENTS.md](AGENTS.md).

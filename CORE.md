# What Loft optimizes

Loft lets a person or model describe a scene in a short `.loft` program, inspect the GPU
program it generates, and run it on a native or host-owned wgpu device.

1. **Picture.** Judge light, material, shape, and motion in rendered frames at the intended camera.
   A deliberate look change needs owner review. A cleanup must hold the declared picture gate against main.
2. **GPU frame time.** Measure p95 and GPU commands at the specified resolution and instance
   count. Track pipeline count, shader modules and cold and warm load alongside frame time.
3. **Authoring cost.** Count the tokens required to express the scene. The author can inspect
   generated WGSL and replace library behavior where the scene needs it.
4. **Truth.** Compare CPU and GPU observations. A verdict needs evidence. Preserve the
   distinction between a held claim, a measured miss and an unknown result. Contended timing
   runs are never treated as silent passes.
5. **Size (Rust core only; `stdlib/` is uncapped).** Keep one implementation of each contract.
   Target **~10k LOC** for the entire Rust core (`crates/`, excluding `tests/`). Never add baked
   renderers, scene-specific names, or WGSL string `.replace()` surgery to Rust — put engines,
   passes, shapes, materials, and motion in clean, composable `stdlib/` code instead.

## How to score a change

Write down the predicted picture, cost and verdict before running the instrument. The manifest
in `engines/gates.json` specifies the scenes, sample times, budgets and approved RGBA hashes.
Run Pictures, Twin and Truth under the GPU lock. Use paired Speed and Load timing under the quiet
GPU guard. The [gate instructions](docs/GATES.md) describe picture modes, evidence and timing
verdicts.

The [specification](docs/spec/README.md) defines the language and host contract. The measured
gaps and pending picture decisions are in [OPEN.md](agent_notes/2026-11-20_v1/OPEN.md).

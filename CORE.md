# What Loft optimizes

Loft lets a person or model describe a scene in a short `.loft` program, inspect the GPU
program it generates, and run it on a native or host-owned wgpu device.

1. **Picture.** Judge light, material, shape, and motion in rendered frames at the intended camera.
   A deliberate look change needs owner review. A cleanup must preserve main’s RGBA pixels exactly.
2. **GPU frame time.** Measure p95 and GPU commands at the specified resolution and instance
   count. Track pipeline count, shader modules and cold and warm load alongside frame time.
3. **Authoring cost.** Count the tokens required to express the scene. The author can inspect
   generated WGSL and replace library behavior where the scene needs it.
4. **Truth.** Compare CPU and GPU observations. A verdict needs evidence. Preserve the
   distinction between a held claim, a measured miss and an unknown result.
5. **Size.** Keep one implementation of each contract. Measure executor work across every crate
   that performs it, including native preparation and the embedded runner.

## How to score a change

Write down the predicted picture, cost and verdict before running the instrument. The manifest
in `engines/gates.json` specifies the scenes, sample times, budgets and approved RGBA hashes.
Score Pictures, Twin, Speed, Load and Truth under the GPU lock. The gate reports both comparison
to approved references and the actual image hashes for exact comparison with a previous build.

The [specification](docs/spec/README.md) defines the language and host contract. The measured
gaps and pending picture decisions are in [OPEN.md](agent_notes/2026-11-20_v1/OPEN.md).

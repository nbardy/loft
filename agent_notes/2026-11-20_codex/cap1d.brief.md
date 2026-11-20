Effort: xhigh

You are a Sol engineer on Loft's native traversal and neural-shading lane. Repo ~/git/loft, from origin/main containing the capability-gaps proposal. You have no other context.

Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md, agent_notes/2026-11-20_v1/OPEN.md, docs/V1_5_GOALS.md G8, docs/proposals/2026-11-20_capability_gaps.md §§3–4 and the native-backend funding criteria in docs/proposals/2026-11-20_backends_and_escape_hatches.md. Check Cargo.lock and the pinned HAL/validator source in ~/.cargo/registry. The M4 measurements establish a triangle hit and plain-ALU inference. Time the two capabilities independently.

Rules

- Use `git cambium add -b loft/cap1d -name loft-cap1d -ephemeral ~/git/_wt/loft-cap1d origin/main`.
- Predict ray build/refit/traversal and scalar/tiled inference before implementing. Iterate compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check CRATE`. Select variants from the granted device and limits. Record required-capability failures and fallback choices.
- Own the native ray query helper, World mesh traversal variant, its AS bindings and the neural material/probe shaders and fixtures. Use the existing pass loader and feature bits. Keep the two acceptance decisions independent. Shipping a direct Metal executor is outside this lane.
- Serialize every GPU run with `tools/gpu_lock.sh`. For speed use quiet paired runs, 95% intervals, triangle p95 <0.15 ms and no concurrent build. Keep outputs from mismatches, preserve the frozen pictures, and run focused checks and the workspace suite once.
- Preserve emitted shaders, rays, weights, readbacks and raw samples under the primary checkout's out/meta/cap1d/. Write agent_notes/2026-11-20_v1/lanes/cap1d.md. If two fixes fail without narrowing the cause, report the evidence for that feature and finish the independent measurement.
- Commit by path and push loft/cap1d for integration. End commit messages with `Co-Authored-By: GPT (codex) <noreply@openai.com>`. Preserve ignored evidence before safe Cambium removal.

Task

1. Repair AS device negotiation through the native entry point. Validate geometry, primitive, instance, per-stage AS binding and combined slot capacities. Run phase5_ray (1 m / miss, row 17, triangle 0) and a zero-binding-capacity negative. Route ordinary closest-hit and occlusion World mesh queries by RAY_QUERY, with software BVH on WebGPU. Check mixed mesh/SDF worlds and a transformed instance. Report one hardware AABB candidate with an authored intersection against inside/grazing/miss rays, or the precise backend limitation.
2. Score 100k triangles and 262,144 coherent/shuffled rays on the same distributions. Time AS build/refit and frame-amortized cost against software. Target ≤100 ms build, ≥2× traversal throughput and ≥0.30 ms net frame saving. Prove material/primitive/t/normal parity on unambiguous hits and record ambiguous ties.
3. Reproduce the 512² 8→16→3 f32 ALU and I/O control, CPU error ≤10⁻⁵. Integrate a network at real shading hits. Compare dense 32/64-wide ALU vs a supported cooperative matrix tuple, with feature packing, queue grouping and scatter-back inside the total. Exercise a partially populated tile and a device with the optional flag masked. Report p50/p95, numerical and picture error. Promote a cooperative variant only if it saves ≥20% total p95 for that workload.

Final message ≤120 words.

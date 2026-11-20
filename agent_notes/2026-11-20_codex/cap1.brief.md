Effort: xhigh

You are Loft's chief engineer. Design pass with probes allowed (one GPU probe at a time under tools/gpu_lock.sh). Repo ~/git/loft (main). Write docs only.

Read docs/PHILOSOPHY.md, docs/spec/README.md (including the 20 Nov ruling that exactness is opt-in), agent_notes/2026-11-20_v1/OPEN.md, docs/proposals/2026-11-20_backends_and_escape_hatches.md, and Cargo.lock (the pinned wgpu version; read its source in ~/.cargo/registry for real feature flags).

Write docs/proposals/2026-11-20_capability_gaps.md, with one section per gap. Each section gives the design, the predicted cost and speed, and the cheapest proving probe:

1. **Modern GPU physics** (the owner wants it):
   - MPM, FLIP and SPH scatter (P2G), particle-to-grid splats, and race-tolerant accumulation.
   - Two tiers:
     - (a) **deterministic fast:** fixed-point i32/u32 atomicAdd, since integer addition is associative, so it is order-independent and exact. Cover range and precision scaling, and overflow handling.
     - (b) **fast non-deterministic:** float atomics where the backend has them (check wgpu's SHADER_FLOAT32_ATOMIC or its equivalent and the Metal support), outside the opt-in determinism contract, with statistical twin checks.
   - Sort-based scatter as the fallback on the web.
   - Measure P2G throughput for each tier on the M4 with a small probe.
   - Spell out how E504 and the CPU twin apply to each tier.
2. **Data-driven materials alongside kinds:**
   - Thousands of unique procedural materials without a pipeline each: material parameter tables plus an uber-shader per material family; bindless texture arrays where available; and a compiler rule that auto-merges kinds into a family uber-shader when the module or pipeline budget is exceeded.
   - Compare with the Tier 0 tape VM (+2–5 ms) and with per-kind compilation, with predicted ms for 10, 100 and 1,000 unique materials.
   - Define when runtime polymorphism is needed: a path tracer hitting arbitrary BSDFs per bounce, runtime node-graph editing, plugin BSDFs and lights loaded from bundles. Say how each is served.
3. **Hardware ray tracing:**
   - What the pinned wgpu exposes today (ray query: which backends, and the Metal status); what a native Metal backend adds (RT pipelines or intersection functions, plus the M3/M4 RT hardware); what WebGPU will never have soon.
   - Which Loft content benefits: triangles and meshes yes; SDFs only via AABB intersection functions.
   - The plan: a software BVH everywhere, with hardware ray queries behind the RAY_QUERY flag, and the cases that would justify the native backend.
4. **Neural shading and matrix cores:** what is possible on plain ALU in WGSL today (small MLPs in registers), measured on a probe; what needs COOP_MATRIX; and the plan.
5. **The heavy-lighting regression** (AB_OLD_VS_NEW.md §B, where the new engine is about 2× slower than the old on the old lighting workload because its four-tap software path costs +27–32 ms): the root cause and the fix design.

End with a ranked list of 4 Sol lane briefs, agent_notes/2026-11-20_codex/cap1a..d.brief.md, each starting with an `Effort:` line and in the shape of w1.brief.md. List them in agent_notes/2026-11-20_codex/cap1.lanes. Do not launch them.

Commit by path to main and push. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.

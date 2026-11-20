You are Loft's chief engineer, answering an architecture question from the owner. Repo ~/git/loft. Write documents only; small probes are allowed (one GPU probe at a time under tools/gpu_lock.sh).

Read docs/PHILOSOPHY.md (§0.5 "lock-in is expensive"), docs/spec/README.md and the spec (01 for `wgsl(...)` and `kernel`, PATCH_V1_2 §6 for ShaderTarget/ExecutionBackend, 07 for naga), and agent_notes/2026-11-20_v1/OPEN.md ("Frame-cost directions").

**The owner's worry, in their words:** they wanted everything in WGPU because layers with a compiler bottleneck can't match straight Metal: "a light transpilation doesn't get in the way of any performance improvements, BUT a heavy compile does." They worry Loft went too far. Is Loft just inferred-type WGSL, or more? And is the "more" a performance ceiling?

Write docs/proposals/2026-11-20_backends_and_escape_hatches.md:
1. **What Loft's compiler actually does to per-pixel and per-thread code**, versus what it does to orchestration. Use real examples from main: compile engines/hermit.loft and show the emitted WGSL next to its source (`loft show` or the equivalent) for one hot function. Measured: is per-pixel maths a light, near-1:1 lowering? Where does the compiler make choices an expert could not override (dispatch shape, inlining, layout, FMA policy, and so on)?
2. **Escape hatches today, verified against the code, not the spec:**
   - Can a user write a raw WGSL function (`wgsl(...) -> T {}`)?
   - Can they write a whole raw WGSL compute pass, or a raw render pass, with their own bindings?
   - Can they replace a schedule?

   List the gaps.
3. **Native shader escape hatches** (MSL, and later SPIR-V or PTX): is it feasible through wgpu's passthrough shader modules or a native backend? Give the syntax (for example `msl(...) {}` gated by HostCaps, with a required WGSL fallback or an explicit refusal), how bindings stay consistent with pass-derived bind groups, and how the gates treat it: no CPU twin, so Truth is Unknown unless a twin is supplied.
4. **A pure Metal backend** (an ExecutionBackend implementation over objc2-metal or metal-rs):
   - scope: device and buffers, argument buffers, pipelines and binary archives, encoding, indirect command buffers, tile shaders and memoryless attachments, simdgroup matrix and bfloat;
   - the shader path: naga's MSL output, your own MSL emitter, or both;
   - predicted lines, and the order of lanes;
   - what it buys, in predicted ms, for hermit-20 and Shrine (tile memory for the vis-buffer, ICBs);
   - what it costs: macOS-only, a second backend to keep in parity.

   Recommend now, later or never, with the measurement that would decide it.
5. **The "never slower than hand-written" gate.** Propose an escape-hatch parity gate: for each shipped engine, one hot pass hand-written in WGSL (and later MSL) as a control, and Loft's output must be within X% of it. Define X, the passes and the cost.
6. **Answer the worry directly:** where Loft is a light transpiler, where it is a heavy compiler, and whether any heavy part sits between the author and frame time. Name every place it does, with the fix.

Commit by path to main and push. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤150 words.

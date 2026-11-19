# d1b · Recover GPU headroom

You are Loft's Sol implementation engineer for d1b. Repo ~/git/loft. Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), agent_notes/2026-11-20_v1/design/d1.md §2, the i4/i5 reviews in agent_notes/2026-11-20_v1/reviews and the primary checkout's out/meta/{i4,i5}-r2/{BLOCKED,gate.json}. The raw programs, telemetry and stamps are in out/meta/i4-fix, i4-r2 and d1.

**Workspace.** Check `git merge-base --is-ancestor d4b7646 main` in ~/git/loft. If false, stop before creating a workspace and report ESCALATE with main's SHA and out/meta/m23/BLOCKED. On success, create the workspace.

```sh
cd ~/git/loft
git cambium add -b loft/d1b -name loft-d1b -ephemeral ~/git/_wt/loft-d1b main
cd ~/git/_wt/loft-d1b
```

First merge loft/i4-scored-fix and loft/i5-scored-fix using d1a's published common import. Fetch loft/d1a and fast-forward to the `Prepare D1 lane base` commit on its first-parent history. Verify both scored tips are ancestors. Wait for that commit if the workspace starts earlier. If d1a escalates the import, report the dependency. That SHA is the base for ownership review.

Run the import from your lane workspace once the handoff has been pushed.

```sh
git fetch origin refs/heads/loft/d1a:refs/remotes/origin/loft/d1a
imports=$(git log -1 --first-parent --format=%H --grep='^Prepare D1 lane base$' origin/loft/d1a)
test -n "$imports" && git merge-base --is-ancestor loft/i4-scored-fix "$imports" &&
  git merge-base --is-ancestor loft/i5-scored-fix "$imports" && git merge --ff-only "$imports"
```

**Own** crates/loft-passes/src/{emit.rs,paths.rs,bake_tex.rs,bsdf.rs,material_shape.rs,telemetry.rs,world_path.wgsl,spectral_path.wgsl}, crates/loft-cli/src/{native/mod.rs,native/bakes.rs,native/mips.rs,native/paths.rs,gates.rs,profile.rs}, crates/loft-cli/examples/phase4_gate.rs, lib/{bsdf,light}.loft, engines/coral_shrine.loft, and implementation files under crates/loft-passes/src/{emit,paths,bsdf,bake_tex}/ and crates/loft-cli/src/profile/. Own the d1b note. Device negotiation/mesh intersection belongs to d1d and the edit command/session to d1c. The runner and artifact lowering belong to d1a. Keep native Loaded's public API and route its frames through the shared executor from d1a. Wire its declared draw index, bake plans and pass timestamps. Reuse the mesh traversal hook from d1d for World rays. Attribute each implementation hunk to one file owner.

**Write these predictions before coding.**

1. Hermit-20. i4 p95 5.989, Vis/Shadow/Shade medians 1.581/0.638/2.513. Shell Naga 621 ALU, 32 transcendentals, 43 storage loads. Keep the exact 192×32 grid and UV seam. Evaluate its 6,369 distinct row-independent vertices once at load, reuse index/winding and per-instance pose for both projections. Use dependency analysis to lift outgoing/view/roughness terms and fixed sky directions out of the existing light loop. Keep all samples in their original order, unchanged BSDFs and shared module counts at 1/20 instances. Predict **5.1 ms** p95. Compare projected positions, depth and approved-shelf controls before timing.
2. Shrine. Pin the close camera from i4-r2/shrine.json. Cull conservative branch clusters before exact leaf queries. Lower the receiver's per-hit visible query to the solid shadow world. Keep the carved visible lantern and all 3,072 instances. A bore-ray pair must distinguish the two worlds. Target **3.7 ms** p95 at 1280×800, 30 warmups + 100 measured. Retain raw Vis/Shadow/Shade stamps even if the shadow map is eliminated from this schedule.
3. Phase 5. All six command counts already hold (capsules/blobs 5, hermits 17). The original triangle controls were 0.028–0.057 ms. The i5 IR capsule repeats the tail at fixed camera on the i4 profiling executor. Compare the same program and exact 300 orbit frames with immediate timestamp reads and with a bounded per-frame query/readback ring, same pass markers, frame inputs, command count and queue. Give each slot a frame serial, prevent reuse before completion and resolve after the relevant submit is complete. Keep first-start to last-end latency for every frame and prompt reporting of budget failures for interactive/single-frame callers. Predict p95 at 1/20 of **0.65/0.95, 0.85/1.9, 4.4/5.1 ms** for capsules, blobs, hermits. Report both halves of the A/B, paired pass times on the slowest frames, shader hashes and triangle controls. The official Speed run must still score 12/12. If the ring only improves reporting throughput, retain the failed frame-time result and escalate the residual. Never use batch elapsed/N, omit spikes, change 0.35 ms, or manufacture GPU load to warm the device.
4. Bake. The 20-dispatch 2048² mask + 128² cube costs 4.358 ms. Hoist column-invariant texel work, specialize exact per-format box filtering, cache disjoint views, honor intermediate quantization and tails. Compare base vs mip stamps and every mip's texels to the unfused implementation. First dirty, clean, unrelated, changed and repeated frames must execute **5/0/0/5/0** bake commands. Keep 20 at load. Watch all components of a dirty parameter through the common plan. Forecast **1.8 ms total**, including the last mip. Carry dimensions, format and clocks into the common execution boundary.
5. Spectral. Replace the separate read-only World descriptor/media binding by checked ranges in the existing word arena. Reflect usage per entry and stage. Keep group 0 and the writable photon bins. Predict **≤8** storage buffers per stage and a rendered 128² four-sample study on the eight-slot device. Count statically, then load the actual pipelines. Check the closed world, energy, media and BVH probes when changing these shaders.

**Delivery.** Write first, compile once at the end with `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --bins --examples`. Run focused commands against that binary under tools/gpu_lock.sh. Use i4-r2/run.sh for the arguments, writing all results into out/meta/d1b. i6 owns one combined full gate and suite. Rerun at most twice after diagnosis. Never kill other processes. After two failures without narrowing the cause, stop and report **ESCALATE** with both observations and the cheapest discriminating check.

Never change or re-approve references, golden vectors, tolerances or manifests. Preserve lossless diff inputs. Carry the 18 layered decisions forward, label any new intended differences OWNER APPROVAL NEEDED with old/new WebP q95 under out/meta/d1b/approval. Commit and push loft/d1b after milestones, staging by path. End every commit message with `Co-Authored-By: GPT (codex) <noreply@openai.com>`. Leave main to i6. Write agent_notes/2026-11-20_v1/lanes/d1b.md with source locations, predictions vs measurements, command lines and residuals. Preserve ignored evidence before safe workspace removal. Final ≤150 words.

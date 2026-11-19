# d1c · Authored edit to first usable frame

You are Loft's Sol implementation engineer for d1c. Repo ~/git/loft. Read CORE.md, docs/PHILOSOPHY.md, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), the Tier 0 lowering contract in 02, 06 §13, agent_notes/2026-11-20_v1/design/d1.md §3 and agent_notes/2026-11-20_v1/reviews/i23.md. Read the primary checkout's out/meta/i23-r2/{BLOCKED,gate.json,edit.log,run.sh} and probes/edit-{old,new}.loft.

**Workspace.** Check `git merge-base --is-ancestor d4b7646 main` from ~/git/loft. Failure means m23 did not merge. Stop before creating a workspace and report ESCALATE with main's SHA and out/meta/m23/BLOCKED. On success, create the workspace.

```sh
cd ~/git/loft
git cambium add -b loft/d1c -name loft-d1c -ephemeral ~/git/_wt/loft-d1c main
cd ~/git/_wt/loft-d1c
```

Use the common caller after the Phase 4/5 imports. Fetch loft/d1a, locate `Prepare D1 lane base` on its first-parent history and fast-forward to that commit before writing. This merges loft/i4-scored-fix and loft/i5-scored-fix using the shared resolution. Verify ancestry of both. Wait for the handoff if needed. If the import escalates, report that dependency.

Run the import from your lane workspace once the handoff has been pushed.

```sh
git fetch origin refs/heads/loft/d1a:refs/remotes/origin/loft/d1a
imports=$(git log -1 --first-parent --format=%H --grep='^Prepare D1 lane base$' origin/loft/d1a)
test -n "$imports" && git merge-base --is-ancestor loft/i4-scored-fix "$imports" &&
  git merge-base --is-ancestor loft/i5-scored-fix "$imports" && git merge --ff-only "$imports"
```

**Own** crates/loft-passes/src/tier0.rs, tier0_author.rs, tier0.wgsl, tier0_tests.rs and tier0/ beneath that src directory, crates/loft-cli/src/{edit_cmd.rs,native/edit.rs}, crates/loft-std/src/promotion.rs, focused new edit fixtures under crates/loft-cli/tests/d1_edit/, and the d1c note. The common handoff routes the CLI edit command into edit_cmd.rs. Keep the Loaded facade contract. d1a owns the runner's data/handle operations, d1b owns the host and shader-emission hubs. Use the existing checked query and frozen Tier0Opcode types in your owned modules.

**Implement and predict.**

- At session load retain source spans, checked query dependencies, constant/row layouts, bounds dependencies and pre-created edit pipelines. At the first edited source bytes start one foreground timer. Recheck the changed query and affected dependents only. Classify constant-pool, structural tape and incompatible-layout edits exhaustively. A radius change must update the conservative bound, including any caster bound. Retain the last valid generation on a rejected edit. Upload the changed ranges as one coherent generation. Stop the timer only after the queue has accepted its final write. Count actual pipeline creations at their factory. Report CPU parse/check, bounds and lowering, upload and total. The old 0.063666 ms measures only that last portion.
- Replace WGSL string slicing in TraceEdit::shader and the filtered 28-op render switch. Lower one typed instruction representation with the frozen 56-op contract and ≤32 live vec4s. Stage frame/row values outside repeated position evaluations. Fold a proven sphere expression to SD_SPHERE. In a primitive residual, dispatch the operation outside the distance loop, use the same operands for march, normal and AO, and retain the authored march tolerance. For composed expressions use the same operations with kind-uniform PC, valid resource slots and bounded loop semantics. Keep the clean kinds specialized. No scene-name switch or special edit-test pipeline. Preserve unsupported/raw and layout errors with spans. Motion bypasses the VM.
- Keep one 400 ms quiet-period promotion state machine and LRU, with native worker and browser async pipeline creation as adapters. Bind a prepared replacement only if its generation still matches. Rapid edits and errors leave a renderable frame. Keep scheduling out of L0.
- Forecast **1.2 ms** source→upload, **0** synchronous GPU compilations, **+0.35 ms** p95 GPU overhead (limit +0.5) from the observed **3.124958 − 0.682833 = 2.442125 ms**. Predict field error ≤1e−5, ≥99.9% shaded pixels within 1/255, zero changed clean-kind pipelines, zero stale installations. Measure both the real elapsed CPU interval and pre-promotion GPU timings.

**Cheapest proof.** Dump the residual, opcode IDs, live registers, generations and dirtied rows for the 0.59→0.64 edit before using a GPU. A valid structural edit (e.g. sphere to a box with a distinct tape) must exercise the interpreter. Check a loop or resource opcode previously filtered out, a rejection and a stale completion. Exercise the public edit command with these unchanged fixtures.

```sh
evidence="$PWD/out/meta/d1c"
probe="$evidence/probes"
mkdir -p "$probe"
cp ~/git/loft/out/meta/i23-r2/probes/edit-{old,new}.loft "$probe/"
tools/gpu_lock.sh target/debug/loft still "$probe/edit-old.loft" "$evidence/dirty.png" --edit "$probe/edit-new.loft" --kind EditSphere
tools/gpu_lock.sh target/debug/loft still "$probe/edit-new.loft" "$evidence/specialized.png"
```

Keep 1280×800, same camera, same phases and samples on both tiers. Compare both p95s using the original gate statistic. Include a paired diagnostic with repeated timestamps. See actual changed pixels and compare to independently compiled Tier 1 for both a literal and a structural edit. Keep manual 140,000-sample data as a separate regression. Exercise a browser completion or record the precise unverified target.

**Delivery.** Write the prediction and all code first. Compile/test once at the end through `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli -p loft-passes -p loft-std --bins --tests --message-format=json`. Use the emitted test executables for the focused opcode, allocation and promotion checks. Run the authored command using the resulting lane binary and serialize GPU work with tools/gpu_lock.sh. i6 runs the full suite. Rerun at most twice with a narrowed cause. Never kill other processes. Two failures without narrowing the cause means **ESCALATE**, reporting revision, source diff, residual, stage timings and both observations.

Never change or re-approve references, golden vectors, tolerances or manifests. Mark intended look differences OWNER APPROVAL NEEDED and save old/new WebP q95 in out/meta/d1c/approval. Keep diffs lossless. Stage by path, commit and push milestones on loft/d1c. End every commit message with `Co-Authored-By: GPT (codex) <noreply@openai.com>`. Leave main to i6. Write agent_notes/2026-11-20_v1/lanes/d1c.md with measurements, touched files and escalations. Preserve ignored evidence before safe workspace removal. Final ≤150 words.

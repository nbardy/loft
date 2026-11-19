# Scientist's fan-out handoff (20 Nov 2026, verbatim summary of the owner's paste)

Five artifacts before fan-out:
1. **Golden vectors.** Create `crates/loft-abi` with `tests/golden.rs`. It asserts std430 sizes and `offset_of!` values for BodyRow (112 B), ChainRow (112 B), SegmentRow (48 B), CreatureHeader (176 B), FootPin (80 B), PoseJoint (32 B), BvhNode (32 B) and VerdictRecord (32 B), plus the VisRec pack/unpack bit patterns. It also fixes exact coefficients and bit-identical golden outputs for det_sin, det_cos, det_atan2, det_acos, det_exp and Philox-4x32-7 (rng, hash1..3, rng_key). Add one negative `.loft` fixture per diagnostic: E104 E105 E108 E210 E211 E307 E401 E402 E404 E501 E502 E504 (plus E505).
2. **`docs/spec/QUIRKS.md`.** A ledger of Metal/naga/wgpu traps:
   - never emit pack2x16unorm or unpack2x16unorm;
   - mangle reserved words on row fields (target, ref, mod, in, out);
   - in the twin, convert i32→u32 by bitcast;
   - Metal has no PIPELINE_CACHE, so keep codegen deterministically ordered (BTreeMap/IndexMap, never HashMap order);
   - one call site per field/BSDF in shade loops.
3. **A contention canary in the speed gate.** Before profiling a scene, draw a 1-triangle full-screen control. If control p50 > 0.35 ms, tag the speed verdict CONTENDED (warn, not fail).
4. **`tests/gate_mutants.rs`.** Deliberately broken fixtures that prove the gates still bite:
   - missing_frame must exit 3;
   - fake_foot_20mm must exit 2;
   - 1_vs_20_module_drift must exit 4;
   - over_budget_51ms must exit 7;
   - core_domain_word_leak must fail the grep gate.
5. **`AGENT_CARD.md`.** A one-page E+ sheet under 1,400 tokens for scene authors. It shows emit Sun/Sky, ranges, `writes h.{...}`, `map sym(p)` and `fn chitin(p, n)`, and is verified against loft.lark and loft-parse.

Topology: **freeze → parallel lanes → serialized GPU integrator.**
- Never split an unverified shared contract across two parallel agents. Freeze shared types first in one serial commit.
- Lanes prove correctness on the CPU twin and with naga validation only.
- An auditor diff-checks the lanes: line counts, grep gates, no weakened tolerances.
- One integrator holds the GPU lock, merges the wave, runs ONE full GPU gate, pushes, and writes `out/meta/p<N>/gate.json`.

| Phase | Freeze | Lanes |
|---|---|---|
| 2+3 (run together) | MotionABI, det_* vectors, Tier0 opcode enum, VerdictRecord | L1 tier0, L2 motion-core, L3 motion-std (.loft), L4 plan |
| 4 | Bvh2 layout, World 6-query lowering hooks, furnace reference tests | L1 bvh-trace + CoralShrine, L2 bsdf-media, L3 engines (reference, woodblock, ReSTIR DI, post) |
| 5 | GltfScene/Mesh kind, `loft clip` exit-7 harness | L1 gltf-retarget, L2 embed, L3–L6 corpus (295 scenes in 15-scene batches via AGENT_CARD.md) |

Guardrails for one 24 GB M4:
- one shared sccache;
- CARGO_BUILD_JOBS=2 per worker;
- at most 3 Rust-compiling lanes at once;
- debuginfo line-tables-only;
- one GPU lockfile (/tmp/loft-gpu.lock), held only by the integrator;
- checkpoint and push after every merged wave;
- per lane, two failed attempts without narrowing the cause means stop and escalate. Never relax tolerances or change frozen shared types to go green.

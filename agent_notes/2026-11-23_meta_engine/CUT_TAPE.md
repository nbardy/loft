# Cut: the Tier 0 tape VM (lane `cut-tape`)

DESIGN.md §7.3. Base `merge-2026-11-23-r2` (a3cfe2c0). Ratchet before: 34,285 (`tools/lines.py`).

## What goes

| Piece | wc lines | Why it exists |
|---|---:|---|
| `crates/loft-tier0` (whole crate: `tier0.rs`, `tier0_author.rs`, `tier0/session.rs`, `tier0.wgsl`, `tier0/compute.wgsl`, `tier0/render.wgsl`, `tests/tier0.rs`) | 2,442 Rust + 266 WGSL | the tape: lowering, VM shader, incremental admission and bounds |
| `tier0/diagnostics.rs` | 328 | not tape at all: the E101–E503 diagnostic snapshots. **Moves** to `crates/loft-passes/tests/diagnostics.rs` (out of the ratchet, still run) |
| `loft-abi::Tier0Opcode` (`src/tier0.rs`) and the `tier0_words` golden test | 52 + ~22 | the tape word encoding. No bundle wire field carries tape words (`loft-format` has none), so no wire layout changes |
| `loft-wgpu/src/edits.rs` (`Edits`, `Pipelines` trait, `upload_words`) | 242 | runner-side tape routes and the debounced promotion |
| `loft-wgpu/src/promotion.rs`, `loft-std/src/promotion.rs` (`Pending`, 400 ms quiet period) | 42 + 150 | promotion policy. `worker` (one thread + channel) is the one survivor; it is used by parallel pipeline creation and moves inline into `loft-runner/src/encode.rs` |
| `loft-runner/src/tier0.rs` (`register_tier0`, `hot_swap_tape`), the `edits` field and its two hooks in `hot_swap_module` and frame recording | 13 + ~8 | |
| `loft-runner/src/encode/edit_gate.rs` (incl. flaky `time_edit_and_compare_native_promotion`) | 210 | |
| `loft-emit`: `kind_module`, `kind_module_edit`, the `edit` parameter of `emit_kind_module` | ~20 | the tape's march hook (`loft_edit_march`) |
| `stdlib/core/ray_geometry_edit.wgsl` | 8 | |
| `loft-passes`: `bundle::Options::edit`, the trailing-tape row layout, `Builder::new`'s edit source; `strip_shader_provenance` | ~15 | |
| `loft-check`: `Graph::edited_module`, `scope::edited` | ~10 | single-file incremental re-scope for the tape session |
| `loft-cli/tests/d1_edit/*` and the `d1_edit` unit test | 51 + ~50 | tape classification fixtures |
| `loft-native/src/edit.rs` (`TraceSession`) | 361 → ~150 | rewritten, see below |
| `loft-cli/src/edit_cmd.rs` | 260 → ~110 | rewritten, see below |

Kept: `Runner::hot_swap_module`, `module_pipelines` / `install_module_pipelines` / `ModuleRecipe` (the Tier 1
path), `bundle::replacement`, `shadow::Reuse`'s every-frame rule for an editable kind, and the
`hardware_ray_query` opt-out for an editable load.

## What stays for edits

`loft still SCENE OUT.png --edit EDITED.loft --kind K` stays, on the recompile path:

1. `loft_passes::compile` the edited scene (the full compiler: parse, check, bounds, emit).
2. Refuse, with "reload the scene", any edit that changes more than kind K: another module's text, kind
   or instance layout, other kinds' rows or bounds, params, LUTs, passes, camera, sun, shadow size, motion.
3. Lower K's new module text with `bundle::replacement` and compile its render and shade pipelines
   (`ModuleRecipe::compile`, validation error scope); install them.
4. Write K's changed rows, changed culling bounds, and the new sun camera when a caster bound moved;
   refresh K's twin source.
5. The CLI then renders, and checks the swapped picture against a fresh load of the edited scene
   (0 pixels over 1/255, else a Truth failure). That replaces the tape-vs-promotion comparison.

`EditSession` replaces `TraceSession`; there is no background worker, debounce or second tier: an edit
is synchronous and either fully live or refused.

## Predictions

- **Ratchet:** the deleted Rust is 3,236 scc code lines (incl. the 325 of diagnostics that leave the
  ratchet by moving to `tests/`), plus ~45 small cuts; ~230 come back (`edit.rs` ~140, `edit_cmd.rs`
  ~90). Net about −3,050: **34,285 → 31,240 ± 150**. DESIGN P5 counts §7.3 as 3,300 × 0.93 ≈ 3,070;
  this lands on it.
- **`git diff --shortstat`:** about 4,300 deletions, about 650 insertions (330 of them the moved
  diagnostics file, unless git pairs it as a rename).
- **Edit latency by recompile** (capsules-1, material edit, M4, edit received → new pipelines
  installed): compile 30–80 ms, WGSL re-lowering ~3 ms, two pipelines 70–100 ms each in sequence =
  140–200 ms, uploads < 1 ms. **About 230 ms (180–290 ms).** The tape answered in < 2 ms and promoted
  after a 400 ms quiet period plus the same two pipelines, so time-to-final-shader improves; time to the
  first changed pixel gets ~100× worse.
- **P7:** all 19 gate pictures hash-equal, Twin and Truth unchanged: no gate scene loads with `edit`
  set, and `emit_kind_module(.., None, ..)` output is byte-identical without the parameter.

## Results (Apple M4, Metal, machine shared with other lanes)

| Prediction | Measured | Verdict |
|---|---|---|
| Ratchet 34,285 → 31,240 ± 150 | **31,163** (−3,122; loft-runner 2,159 → 1,917) | held, 77 below centre |
| shortstat ~4,300 deletions, ~650 insertions | **47 files, 240 insertions, 3,868 deletions** (git paired the diagnostics move as a rename) | fewer of both: the rename, and smaller rewrites |
| Edit latency ~230 ms (180–290) | **193.8 ms** material edit (compile 3.6, pipelines 190.1, upload 0.1); **200.9 ms** trace edit (compile 14.6, pipelines 186.0, upload 0.3, 7 rows rewritten) | held; compile far cheaper than predicted, pipelines at the predicted 2 × 90 ms |
| P7: 19 pictures equal, Twin and Truth unchanged | `pictures.py deterministic` against r2's capture: **19/19 held, 0 changed pixels, depth and id structure equal** (coral_shrine included). `loft-verify gate`: Pictures 19/19, Twin 36/36, Truth 34/34, Speed 12/12; 18 deterministic gate.json hashes byte-equal to scored main | held |

Both edits matched a fresh load of the edited scene (0 of 1,024,000 pixels over 1/255).
Load 29/33: cold load missed on capsules-1 (1.070 s), capsules-20, blobs-1, blobs-20, on a shared
machine with a fresh worktree; the cut does not touch load (r2's own run missed 7 Load checks).
coral_shrine's `gate.json` hash differs between every run, main's and r2's included (over-2 counts 94, 69, 87);
the capture instrument shows 0 changed pixels against r2.

Kept for ABI: nothing needed. The `.loftb` bundle never carried tape words. Removed `Tier0Opcode` and its golden
test as part of the tape (spec 06 §13 superseded; see `docs/spec/CUTS_2026-11-23.md`).

Not exercised: the refusal path (an edit that changes another kind or the passes); `tools/check.sh lint`.

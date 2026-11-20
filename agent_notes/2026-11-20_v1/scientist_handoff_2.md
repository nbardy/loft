# Chief scientist's handoff to the worker manager, 20 Nov 2026 (verbatim)

# Handoff from Scientist Review (Cursor) → Loft Worker Manager (Claude Code)

**Repo:** `~/git/loft` · **HEAD observed:** `1194779` (after `d4f31cc` merged `pf1a`: Hermit-20 down to **3.752 ms p95**, 17→13 cmds)  
**Full Scientist design note on disk:** `agent_notes/2026-11-20_v1/design/scientist_review.md`  
**Orchestrator status:** The temporary `sci_chain.sh` loop I started has been **stopped** — nothing outside your swarm (`cook.sh` + `x2*`) is spawning jobs anymore.

---

## 1. Immediate Heads-Up: 7 Active `run.sh` Sessions Right Now
`cook.sh` (`PID 47763`) launched `cycle.sh 2` → `p2` (`PID 71387`) at the same time `Wave x2` launched `x2a`–`x2f` (`PIDs 88849..96062`), so **7 `codex exec` sessions are live simultaneously** on the 24 GB M4 (above the 3-compiling-lane guardrail). Watch for swap / `/tmp/loft-gpu.lock` queueing, and consider stopping `cook.sh` (`kill 47763`) once you want `x2` (or a Buddies Expedition) to be the single conductor instead of `cook.sh` auto-launching Cycle 3+.

---

## 2. Two Completed, Pushed Branches Ready for Integration / Review

1. **`loft/s1-lip` at `871ea94` (Big win — ready for owner picture queue + merge)**
   - Fixed the 22.9× Lipschitz inflation on rotated IFS folds in `lip.rs` / `eval.rs`.
   - **`FoldedLantern` derived `L`:** `22.871492` → **`1.000000238`**
   - **Mean / p95 SDF steps:** `129.7 / 182` → **`7.5 / 15`**
   - **Coral Shrine Vis speedup:** **`-0.935 ms p50` / `-0.592 ms p95`**
   - **Why Shrine pixels changed:** Raising only `main`'s 192-step cap recovered **100% of the 10,172 new `t=0` lantern hits** — proving the old Shrine picture was a truncated-march artifact.
   - **Checks:** Hermit-20 + all 18 gate digests unchanged; **156/156** native tests (including all 6 mutants) passed.
   - **Evidence:** `out/meta/s1_lip/lane.md` and `out/meta/s1_lip/shrine-review.webp` (queue `shrine-review.webp` in the owner picture approval list).

2. **`loft/s1-controls` at `aa1087e` (Control-pass gate prototype — needs small parity polish)**
   - Built the first 2 whole-pass hand-written WGSL controls (Hermit Shade + Shrine Vis), Naga check, Metal FMA-leak guard, ABBA ring-timestamp bootstrap, and a synthetic 2× slowdown mutant.
   - **Measured:** Hermit Shade **`0.986×` p95** (95% CI `0.978–0.992`; Depth & ID exact, HDR >1 ULP); Shrine Vis **`1.056×`** (95% CI `0.967–1.120`; 20 ID / 76,908 depth diffs to tighten in the hand-written WGSL); slowdown mutant caught at exit `7` (`1.748×`).
   - **Evidence:** `out/meta/s1_controls/`.

---

## 3. Code-vs-Spec Gaps Not Yet Covered by `x2a..x2f`
Your Wave `x2` already overlaps `s1_exec` (`x2a`), `s1_motion_fma` (`x2b`), sky IBL (`x2c`), A/B (`x2d`), Three.js (`x2e`), and lean-down (`x2f`). Please ensure these verified findings from our audit are folded into `x2` or the next wave:

1. **Make `loft-std` 100% `wgpu`-free (`x2a` / `x2f`):** `loft-runner` only looks small (1,630 LOC) because `loft-std` (4,321 LOC) depends directly on `wgpu` + `wgpu-naga-bridge` and holds ~2,429 lines of device/encoder code, while `loft-cli/src/native/` holds a duplicate 1,938-line renderer. Also fix `loft-runner/src/encode.rs:228–235` & `outbox.rs:66–70` allocating new `create_buffer_init` buffers every frame, `lib.rs:47` slapping `U::VERTEX` on LUTs, and `bundle.rs:325` refusing `.loftb` export for bakes/paths.
2. **Keep FMA strictly ON in shading (`x2b`):** `pf1`/`bk1b` verified shading uses `MTLMathModeFast` and disabling contraction in shading costs `+0.184 ms` p50 / `+0.862 ms` p95 (global Safe mode `+4.680 ms`). Prefer a scoped motion-only contraction barrier over a 900-LOC `u32` soft-float emulator if it hits `0/481` tick diffs, and enforce an automated MSL check confirming zero leakage into shading.
3. **Close the 4 front-end v1.2 stubs (`agent_notes/2026-11-20_codex/s1_escapes.brief.md` is ready to run):**
   - `bake_tex` has **no shader sampling binding** (`StudyPaper.material` in `scenes/spectral_caustic_study.loft:51` ignores `StudyInk` and re-runs procedural `study_ink` per hit).
   - `kernel ... allow(...)` fails with `E405` (`check.rs:395`, `lib.rs:717`) for every name except hardcoded `"deposit_caustic"`.
   - Whole-pass `pass ... reads/writes { ... }` unconditionally fails `E405` (`lib.rs:739–745`).
   - `unproved(step_scale)` fails with `E405`/`unreachable!` (`eval.rs:509`, `ad.rs:97`, `emit.rs:510`).
4. **Browser `web.rs` (`x2e` / `s1_web.brief.md`):** `crates/loft-std/src/web.rs` is currently a 93-line shader/buffer allocator with zero pipelines, bind groups, or `encode_into`.
5. **Explicitly skip / defer:** Native `MetalBackend` (predicted `0.09–0.19 ms` vs `2.3–3.8k` LOC), HDR+Present fusion, `f16` shading, per-kind tile lists (already active), ground fBM bake (`pf1` measured `0 ms` available), and consumer Wasm compiler passes (`behaviour {}`) until `check.rs` (86.5 KB) is split and built-in `kernel`/whole-pass lowering works.

---

## 4. Unused Briefs on Disk (Reuse or Delete as Needed)
- Ready to run when a wave opens: `agent_notes/2026-11-20_codex/s1_escapes.brief.md` (not covered by `x2`).
- Superseded by `x2a`/`x2b`/`x2e` (safe to archive/ignore): `s1_exec.brief.md`, `s1_motion_fma.brief.md`, `s1_web.brief.md`, `is1.brief.md`, `sci_chain.sh`.

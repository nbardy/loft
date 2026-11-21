# Step v15_motion_mood — Meta-Engine-Editable Disney Springs & Emotion/Moods (`stdlib/motion/`)

Read `AGENTS.md`, `stdlib/motion/solve.loft`, `stdlib/motion/body.loft`, `stdlib/motion/gait.loft`, `scenes/lineup/score.loft`, and `stdlib/REFERENCE.md` (lines on `expression.ts`, `restless.ts`, `struck.ts`, `ring.ts`).

## Goal

Complete Loft's Spore + Disney character-animation standard library with **pure, meta-engine-editable `.loft` modules** for secondary springs (Disney 12 principles) and composable emotion/moods — built cleanly on top of `stdlib/motion/solve.loft`'s `step_spring_exact` and `solve_prior_xpbd`, with deterministic CPU/GPU twin parity.

## Task

Work in `~/git/_wt/loft-v15-motion-mood` on branch `loft/v15-motion-mood` from `origin/main` (`git cambium add -b loft/v15-motion-mood -name v15-motion-mood -ephemeral ~/git/_wt/loft-v15-motion-mood origin/main`).

1. **Create `stdlib/motion/springs.loft` (pure `.loft`, zero Rust changes required unless a genuine parser bug blocks it):**
   - Reusable, composable secondary-motion primitives built on `step_spring_exact`:
     - `spring_follow(pos, vel, target, hz, zeta, dt)` (scalar & `v3` follow-through / overlapping action with exact stability at arbitrary `dt`).
     - `spring_chain_step(...)` (3–4 link tail/crest/stalk/antennae overlapping-action wave with per-link lag & stiffness taper).
     - `squash_stretch(vel_y, impact, amount)` (volume-preserving `v3` scale `[sx, sy, sz]` with `sx * sy * sz == 1` clamped cleanly).
     - `anticipation_overshoot(u, anticipate, overshoot)` (Disney C¹/C² action envelope over normalized progress `u in 0..1` for wind-up → strike → settle).
     - `restless_pulse(t, rate, tremor, hold)` & `strike_envelope(t, start, dur, hold)` (uneven micro-motion, darts, and strike-and-hold beats).
   - Keep every function genuinely non-trivial (follows Loft's std rule: ≥ 3× call tokens, no thin wrappers) with concise `#` contract comments where units/ranges matter.

2. **Create `stdlib/motion/mood.loft` (pure `.loft`, meta-engine-editable affect modulation):**
   - Define a clean `Mood` struct (or typed row contract) carrying expressive motion parameters:
     - `cadence` (stride tempo multiplier), `stride_lift` (swing arc height scale), `lean_bias` (posture pitch), `spring_hz` & `spring_zeta` (bounciness vs. heaviness/damping), `squash_gain`, `gaze_dart` (head/eye restlessness), `composure` (XPBD prior stiffness), `claw_open` / `crest_flare` (accent posture).
   - Provide 6 distinct, composable named presets (`mood_jaunty()`, `mood_timid()`, `mood_proud()`, `mood_alert()`, `mood_exhausted()`, `mood_ferocious()`) plus `mix_mood(a, b, t)` for smooth real-time emotional transitions and a helper applying `Mood` cleanly to walker/spring dynamics.
   - Authors and agents can import `stdlib::motion::mood`, blend presets, construct custom `Mood(...)` values, or `loft fork` the file to invent new creature emotions.

3. **Create a multi-mood showcase (`scenes/showcase/mood_springs.loft`):**
   - Demonstrates multiple actors or a timed emotional arc (`timid → alert → jaunty → ferocious → exhausted`) driving a creature with overlapping spring tail/stalks, squash-and-stretch bounces, and capture-point/2-bone-IK footing.
   - Verify it passes:
     - BOTH Earley and llguidance grammar recognizers (`tools/check_loft_grammar.py` — 100% pass across all tracked `.loft` files),
     - `loft check` + `loft lint`,
     - Deterministic CPU/GPU twin parity (0 diverging ticks) and clean still/clip render (`loft still` / `loft clip`).

## Rules (Superintelligence Compile Discipline)
- **Write ALL `.loft` modules and the showcase completely BEFORE running your first check.** Do NOT touch `crates/loft-cli/src/{context,fork,main}.rs` (owned by `v15_context`). Use the existing workspace binary or ONE `CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast` if needed (≤ 2 builds total).
- Write `agent_notes/2026-11-20_v1/lanes/v15_motion_mood.md` (≤ 30 lines) with grammar check counts, `loft check`/`lint` result, twin parity, and render confirmation. Commit by path and push `origin loft/v15-motion-mood`.

Final message ≤150 words.

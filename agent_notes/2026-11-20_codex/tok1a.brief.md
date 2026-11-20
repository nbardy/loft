Effort: high

You are a Loft engineer working in `~/git/loft`. Implement the sampled motion and daylight functions measured on main at `66011c1`. Read AGENTS.md, AGENT_CARD.md, docs/spec/README.md (07 > PATCH_V1_2 > 06 > 00–05), docs/PHILOSOPHY.md §4, CORE.md, agent_notes/2026-11-20_v1/OPEN.md and design/tok1.md beside OPEN.md. The companion tok1.py supplies the exact consumer substitutions.

Rules:

- Use `git cambium add -b loft/tok1a -name loft-tok1a -ephemeral ~/git/_wt/loft-tok1a origin/main`. Record the base SHA. Work against the requested flat-library layout.
- Own only `lib/motion.loft`, `lib/light.loft`, the 75 tracked `corpus/l5e/*.loft` files, the 95 tracked `corpus/seeds/**/*.loft` files, `scenes/spectral_caustic_study.loft` and `agent_notes/2026-11-20_v1/lanes/tok1a.md`. Store ignored evidence in the primary checkout’s `out/meta/tok1a/`.
- NEVER add renamed wrappers for standard patterns or one-line aliases. Use the typed E+ functions and literal tables in the study. Require bit-identical decoded pixels. No compiler changes or reference approvals. Preserve the literal sweep emits, ordering, grids and columns byte for byte.
- Iterate on compile errors with `CARGO_BUILD_JOBS=2 tools/dev.sh check loft-cli`. Run the focused grammar, CLI and motion checks, then the workspace suite. Serialize GPU commands with `tools/gpu_lock.sh`. At most two full test or GPU reruns. If the same failure persists twice without narrowing, escalate with the source, diagnostic and paired evidence.
- Commit by owned path, push `loft/tok1a`, and leave main integration to the integrator. Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>".

Task:

1. Add `ramp_weights` and the two tail-window constants to the motion library. Fold 420 pairs of three scalar windows in the 158 animated ports. Read each component in its original position in the six-term sums. Preserve the stored divisors and the order of f32 operations. Keep `pow` at the surface where AD can prove its exponent constant.
2. Add `daylight` and the five measured tidal constants to the light library. Replace 170 Sun/Sky pairs using the existing view's direction. Pass Spectral Study's four original literals. Each call emits Sun before Sky. Keep the remaining lighting decisions and camera parameters with the scene.
3. Predict 8,805,227 → 8,777,097 l5e tokens and 10,030,367 → 9,995,942 in the remaining seeds. Spectral Study saves 18 whole-file tokens. Score each file with `uv run --with tiktoken==0.14.0` and `o200k_base.encode_ordinary`. Charge the library definitions separately.
4. Compile every changed port. Compare pre/post row counts, row order, bounds and grids. For each changed corpus scene render paired base/branch RGBA at the four corpus sample times with the same camera, overrides, resolution, adapter and binary. Cover both directions and every grid variant. Also compare between-key times 0.125, 0.84, 1.1 and 2.1 s on the train-16/2 probe specified in the study. Compare Spectral Study on its authored path at 0 and 1.37 s. Hash decoded RGBA, record any differing pixel coordinates and require zero. Keep frozen references intact. Verify the two helpers can be copied into scene source under fresh names.
5. Record measured source counts, library cost, compiler/fork checks, matched frame counts and evidence paths in the lane note. Preserve ignored artifacts before Cambium cleanup.

Final message ≤120 words.

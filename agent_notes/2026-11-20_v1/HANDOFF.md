# Loft handoff for 20 November 2026

## Published on main

IX4 merged std1b geometry, noise and frames (`bb1f16e`) and std1e deck-local gait (`3e46079`). The scored merge was `3c24a11` against `2b3c0b6`. The integration audit landed at `2f809a4`. Later orchestration and documentation commits use the same renderer and gate inputs. [Merged lanes](../../out/meta/ix4/MERGED) · [Gate report](../../out/meta/ix4/gate.json)

| Earlier merges on 20 November | Result |
|---|---|
| std1a (`f8fc612`) | Moved the authored library into the nested `stdlib/` tree. |
| build1 (`41c1f4d`) | Warm workspace check fell from 175.74 s to 15.17 s. Third-party rebuilds fell from 107 to zero. |
| pf1b (`968b8c6`) and parity controls (`097dfae`) | Grouped the visibility march tail and resolved the Shrine control’s 20 mismatched IDs. Hermit HDR parity remains open. |
| Earlier waves | The runner cut, shadow-map reuse, Shrine Lipschitz fix and approved pins, old/new A/Bs and the Three.js shared-device demo are on main. [Results by wave](OPEN.md) |

## Measured state

On Apple M4 Metal at 1280 × 800, Pictures held 19/19, Twin 36/36 and Truth 34/34. All 18 layered frames matched the baseline exactly. Shrine had 88 of 1,024,000 pixels above 2/255 and 11 isolated foreground ID differences. The silhouette and depth matched. Picture approvals belong to the lead. [IX4 audit](OPEN.md#wave-x4-integration) · [Approvals](APPROVALS.md)

Speed held 6/12. All six command counts passed. Triangle p95 before and after scoring was 7.961 and 7.386 ms against a quiet limit of 0.150 ms. Load held 23/33 and returned exit 4. All six cold loads missed budget. Hermit also missed both warm-load and pipeline budgets at both counts.

IX4 recorded 159 native passes and two failures reproduced on baseline. CST replay opens the retired `lib/` path. Tier 0 promotion took 0.607 ms p95 against a 0.500 ms budget. Doctests, wasm32, 6/6 mutants and 9/9 bead references passed. Each grammar recognizer classified 302/302 inputs as expected. IX4 counted 34,186 Rust code lines against the first 18,000-line ceiling.

## Held work and next owners

| Lane | Open condition | Next owner |
|---|---|---|
| std1d / std1c | Add the one-argument physical material query and shared counter sampler. Make Harbor woodblock import directly and produce finite high-sample depth. | v15_stdlib_finish |
| port1 | Fix the lineup, Hermit and cast rejected by both recognizers. Five look pairs need lead review. Crab Live measured 15.62 ms p95 versus 9.05 ms legacy. | v15_stdlib_finish and lead |
| v15a / v15b / v15c | Hardware secondary-ray traversal and a quiet B-20 pair, StudyInk bindings and logical table sizes, and both picture modes in the production Rust gate. All three stayed on their branches. | v15_exec, alongside the single executor and wgpu-free loft-std |
| x2c / x2b | Bind the sky bake in Shade, score Shrine cluster culling and verify the motion-only contraction barrier. | v15_perf |
| Other v1.5 targets | Material families, the native bridge, web buffer packing, P2G and corpus regeneration. std1f awaits the library ports. The lead deferred stdaot1. | cap1c, v15_native, cap1b and v15_bloat |

Wave 15 and IX4 have finished. The next conductor waits for `final.finished`, `wave15.finished` and `ix4.done`, runs v15_exec alone, then runs the six follow-up lanes and iv15_2. It defaults to one step and waits for load below 20 and a cleared STALL marker. The earlier cook-cycle planner p2 was abandoned. The Room Runners merge into Keel remains an owner decision. [Blocked IX4 lanes](../../out/meta/ix4/BLOCKED) · [Conductor](../2026-11-20_codex/wave15_2.sh)

## Companion repositories

freeze_sampler master includes fs1 and fs2 at `7821823`. Cambium release/v0.10 includes cam1 and cam2 at `ee292e8`. The installed Cambium was identified as `d2a41cf`. Owner installation and privileged capture verification are pending. cleanup_tools passed 117 tests at the guard revision, with the runbook update at `f9132e5`. Its primary checkout has overlapping edits and it has no configured remote.

## Project references

| Doc | What it is |
|---|---|
| [docs/V1_5_GOALS.md](../../docs/V1_5_GOALS.md) | **The current plan:** checkable targets P0–P7, the pending-branch table, and decisions R1–R9 |
| [OPEN.md](OPEN.md) | Every open target with measured values; routing; in-flight claims; std anti-patterns |
| [NOTES_AND_TODO.md](NOTES_AND_TODO.md) | The reasoning behind decisions (syntax, macros vs passes, commands, escape hatches), the TODO |
| [INTEGRATION_TODO.md](INTEGRATION_TODO.md) | Three.js, Unity, glTF export, material export, motion-twin library |
| [APPROVALS.md](APPROVALS.md) | Look changes approved or rejected |
| [AB_OLD_VS_NEW.md](AB_OLD_VS_NEW.md) | Old vs new engine: unmatched, then matched (equal on light lighting, 2× slower on heavy lighting, measured under contention) |
| [docs/spec/README.md](../../docs/spec/README.md) | Spec precedence (07 > PATCH_V1_2 > 06 > 00–05) and lead rulings, including **exactness is opt-in** |
| [docs/PHILOSOPHY.md](../../docs/PHILOSOPHY.md), [CORE.md](../../CORE.md), [WHY_LOFT_LANG.md](../../WHY_LOFT_LANG.md), [README.md](../../README.md) | Stance, priorities, why a language, the four parts |
| docs/proposals/2026-11-20_*.md | meta_engine_plan, backends_and_escape_hatches (bk1), capability_gaps (cap1), consumer_passes (+_astra) |
| design/*.md | d1, d2 (proved causes), pf1 (perf diagnosis), lean1 (size audit), tok1 (token study), std1 (stdlib audit plus tree), c1, scientist_review |
| scientist_*.md, dump_nov_19th_*.md, owner_notes_platform_power.md | Chief-scientist and owner inputs, verbatim |
| [docs/handoff/2026-11-20_critic/00_CRITIC.md](../../docs/handoff/2026-11-20_critic/00_CRITIC.md) | The lead's critique (round 1); the answers are in scientist_round1_answers.md |

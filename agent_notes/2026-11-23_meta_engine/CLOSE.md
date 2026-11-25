# Meta-engine thread: what landed, and what the next thread starts from

25 Nov 2026. Written by the thread that took the redesign over after the 23 Nov 16:48 reset
(`agent_notes/2026-11-23_restart/meta_engine_handoff.md`, untracked in the primary checkout). The owner closed
it with "merge all complete work and close this out". Measurements are in [INTEGRATE.md](INTEGRATE.md) §5–§8 and
`agent_notes/2026-11-23_auncel/FIXES.md`; this note does not repeat them.

## What that instruction was taken to decide

The owner did not answer the four open questions one by one. Landing the stack settles two of them, and this
records how:

| Question | Taken as | Reversible by |
|---|---|---|
| The cut list, `docs/spec/CUTS_2026-11-23.md` | yes: the cuts land | reverting merges `3c62c2ef` (cut-tape) and `b4d05743` (cut-unused) |
| `motion-stride`, which rides in with probe-rows | lands, **with `crossing` visibly broken** (below) | fixing the gait, or reverting merge `7127e39f` (probe-rows) |
| Warm load 1–2 ms slower than lean2 | accepted unattributed | — |
| The redesign order in `metaengine.md` §8 | not decided; nothing of it is built | — |

## On main

`lane/fix-auncel`, fast-forwarded: probe-door (authored frame passes; cull, classify and tonemap in
`stdlib/core.loft`), cut-tape, cut-unused, probe-rows with motion-stride, the perf lanes (`merge-2026-11-23-r2`),
lean2, `tools-agent-build`, the Auncel compiler fixes, the trainer's four small fixes, the `loft context` and
`timing.py` fixes, and the design review (`agent_notes/2026-11-20_v1/design/metaengine.md` §8).

The last full gate, on `ecdcab8a`: Pictures 19/19 with 0 changed pixels against lean2, Twin 36/36, Truth 34/34,
Speed 12/12, Load 33/33, 51 native test executables. After it came hint text in the checker (its tests pass) and
notes. Rust core: 29,353 counted lines at that gate.

## Not on main, on purpose

| Branch | What | Why it waits |
|---|---|---|
| `loft/metaengine` (`15607b79`) | S0, the slice interface (`Typed`, `link`/`Linked`, `Clock::Tick`), merged on an earlier tip of this stack. Compiles; its three link tests pass | 452 lines that nothing calls until a slice lands. It needs a merge with main before use |
| `loft/metaengine-core` (`db423572`) | S0 alone on `origin/main`, with the review | superseded by the row above; the review text is on main |
| `auncel/a`–`auncel/d`, `loft/div1-anatomy` | scene and creature work | not this thread's |

## Pending, in the order I would take it

1. **`crossing` is broken by motion-stride.** Before, all four feet stay on the ferry deck. After, one leg is
   stretched off the deck into the water at t = 2 s, and at t = 3 s the body sits at the deck edge with two legs
   over the side. `tidal` looks sound. `tidal_fork` was rendered and not looked at. Sheets:
   `~/git/loft/out/meta/motion_stride/`. No gate picture covers the four-site walk, so nothing fails.
2. **Push.** `origin/main` is at `90fc479a`. Nothing from this thread, or from the sessions that committed to local
   main beside it, has been pushed.
3. **Reset the line ratchet.** `tools/lines.py` still allows 33,895; main is near 29,350. Until it is reset, growth
   does not fail. If S0 is to land before its slices, the reset has to leave room for it or come after.
4. **The redesign itself.** Read `metaengine.md` §8 before the slice briefs: the review changes the user-facing
   slice (a per-element pass is a law over a target, not a kernel; 1c's cost; the split into images, kinds, ticks)
   and leaves S1 and S3 as written. `cea5fe7e` on main rewrote `material_shape.rs` and the Bsdf path in `emit.rs`
   after the briefs were written, so S0b's "move `eval_layers`" and S2's material-fold row need re-reading against
   the code.
5. **Binary and stdlib are one versioned unit now** (INTEGRATE §5). A binary from before this landing cannot parse
   this `stdlib/core.loft`, and this binary refuses an older stdlib. Nothing checks for it; the failure is E001 or
   E520 on every scene. A version stamp checked at load, with its own exit code, would turn that into one clear
   refusal. Every stock pass the redesign moves into `stdlib` widens this.
6. **Warm load:** +9–12% (1.0–1.5 ms) on the one-kind scenes and +4–5% (2 ms) on hermit against lean2, in three
   paired runs. Candidates: stage 0 parsing and inlining the schedule functions of `stdlib/core.loft`, the
   per-compile `--set` resolver, `cea5fe7e`.
7. **Paired timing has never had a clean verdict** on five of the six scenes: the harness's triangle control flags
   them contended even on an idle machine at 04:30.
8. **`f1_heading`:** a rotation by a time-dependent angle inside a trace is refused (E212), with or without the
   Auncel fixes. This is Auncel finding F3. The remaining Auncel items (F3, F5, F6, F7, F10, F12, F13) are listed
   in `FIXES.md` and `agent_notes/2026-11-23_auncel/NOTES.md` §6; the trainer's data raises F6 (a law that writes a
   point).
9. **The `loft-runner` edit-gate timing test** failed twice on pre-integration main under load. cut-tape deletes
   it, so it is gone with this landing.
10. From the integrator's list, unchanged: the packed-leg gait port and the deletion it allows, kind parameters
    after that, `outline_and_paper` as an argument of `tonemap`, Probe C (a law over an image: TAA).

## Working with the trainer

The Creative trainer is Loft's first user (`metaengine.md` §8.4). Since 24 Nov each of its runs copies the binary
and `stdlib/` at launch, so moving main no longer changes a live run. Its session sends compiler fixes as branch
names; four were carried this way. `loft context` is its prompt: 18,549 characters before this landing, about
23,300 after.

## Second close, 25 Nov evening

The owner reopened the thread with "complete all work", then closed it again after the first wave. This section
replaces item 4 of the pending list above and adds to the rest. Details: `metaengine.md` §9 and §10.

**Decided.**
- **The typed program (S1) and the host plan reader (S3) are not merged, and S0 with them.** S1's line saving was
  the deleted tape VM's: about −5 on today's tree against a predicted −250. S3 saves 30 lines against 120 and needs a
  260-line reader. Neither gives a user anything to write.
- **The first user-facing piece, a law over a frame image, is parked**: written, never compiled, and nobody has
  asked for it.
- **What users have actually hit**, in the order I would take it: a law that writes a point or vector (the trainer's
  model and the Auncel authors); a rotation by a time-dependent angle inside a trace (E212); a version stamp so a
  binary and a stdlib from different commits refuse once and clearly; `clip` recompiling on every law-written frame.

**Branches left, none on main.**

| Branch | State |
|---|---|
| `fix/crossing-deck` (`cd853a99`) | Clamps every landing into the walker's footing; `.loft` only. A probe over 0–4 s finds no planted foot off the deck. At t = 3 s the walker crouches at the deck edge where it used to stand mid-deck, so **the owner has to look**: `~/git/loft/out/meta/motion_stride/crossing_before_after_fixed.webp`. `tidal` and `tidal_fork` stills are byte-identical. No twin replay; `crab_live` and `motion_bodies` compile and were not rendered; trace hashes will change because `Walker` gains a field |
| `loft/wave1` (`9bcc02f4`) | One cleanup: the motion-math class comes from the builtin catalog (−10 lines). Compiles. Its tests never ran |
| `loft/image-laws` | The parked piece, uncompiled |
| `loft/me-host` (`1afd3cd6`) | S3: compiles for native and wasm32, never tested. Has `loft show --plan` |
| `loft/me-typed`, `loft/me-int` | S0 on main; `me-typed` adds the cleanup above |

**The machine.** `tools/dev.sh` takes the machine-wide light compile lock itself and nothing enforces a time limit
on it. On 25 Nov a `dev.sh test` of mine stalled mid-compile and held that lock for 17 minutes with other projects
queued 80–90 minutes behind it, until another session reported it. Later `agent-build --status` took over two
minutes to return, and three other projects' jobs were holding slots while idle. I stopped all builds at that point;
the cleanup's tests and the image-law compile were never run for that reason. Run a `dev.sh` job where it can be
watched and stopped.

**Instruments left behind.** `~/git/loft/out/meta/me_baseline/`: `loft show` of every `.loft` source on main
`3628005a` (267 shown of 272), hashed, with `show_sweep.py` and `show_diff.py` to remake and compare it. Gate
evidence: `~/git/loft/out/meta/integrate_2026-11-23/`.

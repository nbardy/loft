# Loft: agent guide

Start with [CORE.md](CORE.md) for the project's priorities and [docs/spec/](docs/spec/) for the v1.0 specification.
The Rust workspace is at the repository root. The compiler, runner and CLI are in `crates/`, with `.loft` libraries
in `lib/` and render schedules in `engines/`.

- Use `tools/dev.sh check|lint|test|build|run CRATE` for the edit loop and `tools/check.sh static|quick|full` for
  workspace checks. Cargo uses the root toolchain pin and a compile lock shared with Keel. Each workspace builds
  in its own `target/`.
- Run the five Phase 1 gates with `tools/dev.sh run loft-cli -- gate`. Read the budgets and approved hashes in
  `engines/gates.json`. The gate reads the lossless owner-approved frames from `out/meta/refs/` and writes its
  report to `out/meta/gate.json`.
- For parallel work, use a disposable workspace with `git cambium add -b BRANCH -name NAME -ephemeral`.
  Cambium uses `~/git/_wt` and skips `out/`. Pass the base commit explicitly if it differs from this checkout's HEAD.
  After merging and preserving the deliverables, run `cambium remove --safe NAME` from the primary checkout.
- Generated pictures and reports go under `out/`. The [legacy examples](examples/) and Creative seed references
  are Phase 5 corpus inputs. See [DEPRECATED.md](DEPRECATED.md) for the source tag and archive.
- Reference pictures (concepts, model sheets, storyboards, paint-overs of our renders) come from GPT image through the
  Codex CLI's `$imagegen` skill. It runs on the server, costs nothing extra and takes a few minutes for ten pictures:
  1. Make a scratch directory under `out/` with the brief in `brief.md` and any pictures to look at or paint over in
     `in/` as PNG (`dwebp x.webp -o in/x.png`).
  2. Run it in the background:
     `command codex exec -s workspace-write -C <dir> --skip-git-repo-check -o <dir>/final.txt "$(cat <dir>/brief.md)" < /dev/null`.
     `< /dev/null` is required or it waits on stdin forever. `command` skips the `--yolo` alias.
  3. The brief opens with: use the `$imagegen` skill in its built-in tool mode, never the CLI fallback or an API key;
     save PNGs in the current directory under exact file names; touch nothing else; stop and say so if the tool is
     unavailable. Then the character description and the style clause (both repeated in every prompt, so a set stays
     on model), then the numbered list of pictures, then what the final message should list.
     The tool takes at most five reference images per call: put no more than three or four in `in/`, and tell it to
     report a failed picture and go on, not stop (a thirteen-picture job stopped at the seventh on this).
  4. Ask for sheets a modeller can use: a turnaround (front, three-quarter, side, from above at the game camera's
     angle), an expression sheet, key poses of an action, a storyboard with single frames, close-ups of a surface, and
     paint-overs of our own renders at the same camera. Keep names, project details and anything personal out of the
     brief.
  5. Look at every picture, keep the good ones as WebP (`cwebp -q 90`) beside the brief under
     `docs/art_direction/<round>/`, and show them to the owner before anything is built from them.
  Worked briefs: `docs/art_direction/crab_round4/prompt.md` and `crab_round4/others/prompt.md`. The local Flux script
  (`tools/crab_concepts.py`) is the fallback; it is slower, rougher, and another session may be using that repo.

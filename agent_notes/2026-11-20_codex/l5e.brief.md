You are a Loft v1.0 engineer on ONE lane of a parallel wave. Repo ~/git/loft. You have no other context.

Read these first:
- docs/PHILOSOPHY.md
- CORE.md
- docs/spec/ (06_ADDENDUM.md overrides; README.md holds the rulings)
- docs/spec/QUIRKS.md and AGENT_CARD.md, if they exist
- agent_notes/2026-11-20_v1/fanout_handoff.md
- crates/loft-abi, the frozen shared types and golden vectors

PHASE: 5. YOUR LANE: L5 corpus-c.
Scope: regenerate the third quarter of the corpus batches
Proof before you finish, with NO GPU: same

Hard rules:
- **Your workspace.** Work only in your cambium workspace: `cd ~/git/loft && git cambium add -b loft/l5e -name loft-l5e -ephemeral ~/git/_wt/loft-l5e` (continue in it if it exists). Touch only your lane's files.
- **Frozen types.** NEVER change frozen shared types (crates/loft-abi, the spec's ABI) or golden vectors. If you need a change, stop and write it in your final message.
- **No GPU.** Do not run GPU renders, `loft profile` or Metal pipeline creation; the integrator does those. Prove correctness with the CPU twin, unit tests and `naga::valid::Validator`.
- **Builds: write first, compile once.** Write your whole lane before you build anything. Do not compile after every edit. Then run `CARGO_BUILD_JOBS=2 tools/dev.sh check <crate>` and your focused tests ONCE at the end; fix what that run reports, and rerun at most twice more. tools/dev.sh queues on the machine-wide compile lock, so wait for it and never kill other processes.
- **Stop rule.** If your own acceptance test fails twice without you narrowing the cause, stop and begin your final message with "ESCALATE:" and the evidence. Never relax a tolerance to go green.
- **Commits.** Commit and push your branch after each milestone, staging by path. End each message with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Do NOT merge into main.

Write agent_notes/2026-11-20_v1/lanes/l5e.md covering: what you built, the proof results, the files you touched, and the integration notes.

File ownership: change only files your lane owns, together with the new files it creates. Step 0 pre-declared the hub entries in Cargo.toml, lib.rs and the CLI; fill in the stubs and do not add new ones. The integrator runs tools/lane_check.sh and rejects a lane that touches unowned files, adds crates or `#[ignore]`, or edits crates/loft-abi or tests/golden*.

Corpus rules:
- **Binary.** Use the prebuilt `target/release/loft` binary from main. Never `cargo build` or `cargo run`.
- **Quarantine.** A scene that fails twice goes to corpus/quarantine/<name>.loft, with a one-line .err file giving the code (E211, E307, E602, or R0g) and an entry in out/meta/p5/quarantine.md. Then move on. Never change the compiler to make a scene pass.

Final message ≤120 words.

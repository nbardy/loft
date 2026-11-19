You are Loft's engineer. Repo ~/git/loft. You have no other context.

Read:
- docs/spec/ (06_ADDENDUM.md overrides; README.md holds the rulings);
- the reviewer's findings for 3. Motion Sub-Engine in agent_notes/2026-11-20_v1/reviews/p3.md.

Apply every MUST-FIX and SHOULD-FIX. For a SHOULD-FIX you decline, write the reason into the review file.

Process:
- **Where to work.** Use the phase's cambium workspace if it still exists. Otherwise create one: `cd ~/git/loft && git cambium add -b loft/phase3-review -name loft-phase3-review -ephemeral ~/git/_wt/loft-phase3-review`.
- **Builds.** Use tools/dev.sh, which respects the compile lock. Wait for the lock; never kill other processes.
- **Before merging.** Rerun the phase's gates and focused tests, and run one full workspace test.
- **Commits.** Commit and push after each milestone. Stage by path. End commit messages with "Co-Authored-By: GPT (codex) <noreply@openai.com>".
- **Merging.** When the gates pass, merge into main and push. Append an "Outcome" section to the review file: each finding fixed or declined, and the merged sha.

Builds: write all of your code first, and do not run rustc or cargo after each edit. Compile and run the tests once at the end, through tools/dev.sh (the shared compile lock). Fix what that run reports, and rerun at most twice.

Final message (≤120 words): what you fixed, the gate results, and the merged sha.

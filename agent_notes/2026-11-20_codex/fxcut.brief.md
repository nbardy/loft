You are finishing the runner cut. Repo ~/git/loft. You have no other context. Read out/meta/cut/report.md and the branch loft/cut (59993b5, local; its cambium workspace was retained, so find it with `cambium status`).

The cut brought the runner from 3,758 to 983 lines, and all five gates matched v1.0-alpha. Finish it:
1. **The build.** It fails on missing test imports. Fix them.
2. **The three failing TypeScript tests.** Fix them, or show they are stale tests for removed code and delete them.
3. **capsules-20.** It differs by 1 pixel at t=4 and at t=9. Find the cause. A moved computation that changed its float evaluation order is the likely one. Restore bit-identity, since no picture changes are allowed in this lane.
4. **The native `Loaded` path** still prepares passes separately. Route it through the one dispatcher if that takes ≤150 lines; otherwise record it in OPEN.md.
5. **The D1 accounting** says L0 is ≥1,025 lines including displaced GPU setup. Get the true L0 total to ≤1,000, or record the exact overage in OPEN.md.

Then: build once through tools/dev.sh (at most 2 reruns), and run one locked GPU gate. It needs bit-identical pixels on all 18 scenes against v1.0-alpha, with the five gates matching. If green, merge into main and push. Write out/meta/fxcut/MERGED or BLOCKED. Also fix or record the 38 native test failures listed in OPEN.md; do not hide them.

Commit messages end with "Co-Authored-By: GPT (codex) <noreply@openai.com>". Final message ≤120 words.

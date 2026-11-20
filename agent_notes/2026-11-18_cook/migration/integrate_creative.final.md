Merged `loft/vendor-submodule`, `loft/reconstruct-migrate`, and `loft/migrate` in order. `loft/integrate` is clean at `09d27924d53f06b4aeb68ddae3b02bd62d93692a`. The single `vendor/loft` pin matches Loft `origin/main` at `c8502c2be2848b55b75ca3d49299c1f131c8d114`. No push was made.

**Merge resolutions**

- The vendor merge applied cleanly. Reconstruction conflicted only on the gitlink, which I resolved to the shared pin.
- The training merge conflicted on the gitlink, curriculum, config loader, host docs, import policy, generated allowlist, WGSL fixture, and renderer. I kept the Edit-first schedule and current Improve rubric, switched the migrated configs to v2 seeds and live Loft context, and combined identity receipts and confinement with the TypeScript build and per-frame draw budgets. I lowered the curriculum answer limit to 50,000 tokens to fit the live prompt. Its offline ceiling is $90.82 against the existing $93 cap.

**Verification**

- Regenerated the allowlist and reviewed all eight documented refusals. The drift check, host typecheck, four import tests, 11 focused Python tests, and serve exit-7 smoke passed.
- Batch migration found zero changes or unresolved items across 170 seed programs and 37 reconstruction sources. The existing manifests and hashes still describe their exports. Both seed sets verified, and task regeneration reproduced the committed lists.
- Two seeds per set rendered with identical saved subject-frame hashes. Actions `v1/train-11/1` and Improve `train-1/2` exceeded the 50 ms draw budget at 111.5 ms and 77.1 ms.
- The reconstruction exporter produced two conversations with five states. All three native hand states rebuilt to identical GLB hashes.
- All six configs planned offline. `crab-actions` and `crab-smoke` remain above their paid caps. The full Python suite stopped during collection with seven errors in the VQGAN, legacy GRPO, and training tests.

**Main checkout**

`main` is an ancestor of `loft/integrate`. Its 3 unstaged and 10 staged paths had zero overlap with the 743 integration paths. The read-only checkout dry run passed, so `git -C ~/git/creative merge --ff-only loft/integrate` should preserve the dirty files. I left the main checkout untouched.
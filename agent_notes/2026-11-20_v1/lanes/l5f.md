# Phase 5 L6 corpus-d

## Prediction before validation

Batches 16–20 contain 70 pinned seeds. Each archived R0 build has the same
fixed perspective camera. With its target held at `[0, 0.7, 0]`, the
prebuilt compiler's orbit always puts the time-zero eye at x = 0 m. The
reference eye is at x = 4.5 m. An orbit also changes azimuth with frame time.
Expect the fixed camera declaration to fail at the implicit `orbit`'s
required `at` argument and explicit `use_camera` to fail as an unknown
pass. The orbit control should reach Naga and build. Expect zero accepted
ports until the exact reference view can be represented.

## Results

Verified both pinned JSONL hashes at creative revision `b7f79a82`, all
70 source identities and all 350 archived R0 beauty frame hashes. The R0
build manifests give a fixed eye of `[4.5, 4.5, 8]` m, target of
`[0, 0.7, 0]` m, and a vertical FOV of `0.62rad` for every seed.

| CPU check | Result |
| --- | --- |
| Fixed declaration via the default schedule | 70/70 exit 1, E520 `orbit needs at` |
| Fixed declaration via explicit `use_camera(Fixed)` | 70/70 exit 1, E520 `unknown pass use_camera` |
| Prebuilt `loft build engines/blobs.loft` | Exit 0, four modules passed `naga::valid::Validator` |
| `bun test corpus/manifest.test.ts` | One test passed, 27 assertions |
| Normative grammar on quarantined sources | 70/70 accepted in Earley and llguidance |

The prebuilt executable's SHA-256 was
`6fd1e7d533140b20911bf2cd2a837c16b4f3c4cda0c846be9db1b8c8357179b2`.
Seed builds stopped before shader generation and CPU twin queries.
No corpus scene was accepted. No GPU work, Rust compilation, reference
writes or tolerance changes occurred.

## Files and integration

- `corpus/quarantine/seed__*.loft` and their `.err` sidecars preserve 70
  camera reproducers with seed ID, batch, source SHA-256 and legacy look.
  These files contain the fixed view and pass that fails to lower. The
  original geometry and motion are still in the pinned JSONL files.
- The lane-local ignored `out/meta/p5/quarantine.md` lists all 70
  entries. `out/meta/p5/l5f/builds.json` holds both diagnostics per seed.
  `R0g` is the mandated quarantine classification for the unavailable
  reference view. The compiler diagnostic was E520. No image diff ran.
- This note and the quarantine pairs are the only tracked changes. The
  batches and pinned cohort remain intact.

The compiler currently lowers every default camera declaration to
`orbit`. Its explicit pass dispatcher also lacks the spec's
`use_camera` from 06 §5. The runtime and persisted Program camera field
use `Orbit`, whose time-zero x equals the target x, so using its current
schema with the recorded target loses 4.5 m of lateral offset. The
integrator needs a fixed perspective camera in lowering, playback and
bundle handling, with a CPU regression for its eye, target and FOV at
all four reference times. No frozen contract was edited here.

Resume the strand, motion and look ports from the original seed sources
after the camera works. The local archive contains R0 beauty frames for
the lane. The checked-in R0g set covers the bead, so the integrator
also needs the lane's approved R0g geometry/depth references before
grading Pictures and running the training Speed gate.

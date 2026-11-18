# Loft small items, 18 November 2026

## Solve and checks

`solve` keeps the 128 most recently used verified results per process. A successful hit moves its entry to the
back of the insertion-ordered map. A new result past capacity evicts the oldest. The cache key, hold recheck and
site reconstruction still run on a hit. The test sends 513 distinct solves while touching key 0 every 64 calls.
At the end, the predicted surviving keys are 0 and 386–512. Touching 386 and inserting 385 then evicts 387.
The test uses `report.samples` to observe the difference between a hit (0) and a solve (1).

The bound is on entries. By construction a hit adds one deletion and one insertion to the existing lookup. Keys
can include row content, so retained bytes vary with the inputs. The cap applies to the result map. Loft's shared
expression arena has its own lifetime.

The heron rebuild test now checks zero samples as well as zero Newton steps. Its previous 50 ms wall limit
failed during the suite at 61.68 ms and in a focused run at 69.70 ms. In a separate probe, twelve hits on each
implementation all reported zero samples and zero steps for the same 26 holds. The original cache measured
6.67–121.60 ms and the LRU measured 13.09–73.93 ms, in successive processes on the shared machine.

`examples/ranged.ts` exports two proofs alongside the sampled triangle budget. Scene authors can read the results
with the existing API:

```ts
import { runChecks } from "../src/checks.ts";
import scene, { checks } from "./ranged.ts";

for (const verdict of runChecks(scene, checks)) {
  switch (verdict.kind) {
    case "sampled": console.log(verdict.name, verdict.holds); break;
    case "ranged": console.log(verdict.name, verdict.holds, verdict.bounds, verdict.spent); break;
  }
}
```

`Check.run` still accepts the sampled measurement shape with `pass` for custom checks. `runChecks` tags those
measurements. The CLI, text table and gallery use the same verdict-to-outcome dispatcher. The CLI run returned
exit 5 with 2 passes and 1 intentional failure. It measured 37,570 triangles against 100,000. The claw witness was
0.5083332 s with margin −0.3499924. The landing margin was +0.0155645. The build wrote 16 files. The example still
and serve produced the same raw SHA-256 prefix at the witness, `d16d296289e04786`.

[Claw at 0.500000 s](before.webp) and [claw at 0.508333 s](witness.webp), both 320 × 200 WebP q95.

## Serve

```sh
bun src/cli/main.ts build examples/ranged.ts --out out/small/ranged-files
bun src/cli/main.ts serve --draw-seconds 5
```

```json
{"id":"ranged","artefact":"out/small/ranged-files","camera":"three","times":[0.5,0.508333],"size":"320x200","out":"out/small/ranged"}
```

The deadline defaults to 120 s per occupied draw slot. The supervisor owns the timer and reaps the GPU child before
releasing the expired request's slot and output path. A timer in the draw process could stall in a synchronous Dawn
call, and device destruction also enters Dawn. The next draw starts a fresh child, GPU cache and `--check` instance.
Compile admission and the external JSON lines are the same shape. An unexpected device loss retains the exit-6
shutdown path. The CLI and server share input validation, including component counts, the frame clock and replay slots.

The Metal regression blocks inside the second draw's readback for 15 s. With a 5 s deadline it requires exactly
three answers, exit 7 for the stalled request, no frame from that request, and a matching SHA-256 for the first and
third frames. The timeout and LRU regressions together passed in 8.03 s.
A separate 1 ms run answered two queued requests with 7, including a deadline during the replacement child's startup.

### Cost on Apple M4, macOS 15.5, Metal

Sixteen serial, warm one-frame requests at 320 × 200, after two warmup requests for the same built scene. The
machine was shared with other build lanes. Times are wall-clock medians. Snapshot RSS is per process.

| Measurement | Result |
| --- | ---: |
| End-to-end request | 243.96 ms |
| Artefact GPU load | 132.92 ms |
| Frame, readback, hash and write | 88.87 ms/frame |
| Request wall time less reported load and frame time, per sample | 21.64 ms |
| Supervisor RSS | 30.64 MiB |
| GPU child RSS | 64.86 MiB |

The residual includes bundle reads and validation, IPC, scheduling and stdout. The warmup requests took 24,065.38 ms
and 190.94 ms end to end during the shared suite run. The second request hit all 35 GPU cache entries. By construction
one draw sends one request and one answer across IPC. Its 256,000 RGBA bytes per frame go directly to disk from the GPU
child. Bun's structured cloning preserves signed zero in the request's numbers.

## Verification

- `pnpm typecheck` passed. The generated API matched `docs/api.md` and the loader closure check passed.
- `loft check`, `build` and a CLI still via `render` ran on `examples/ranged.ts`. The check's intentional failure is
  recorded above. The CLI still drew 12,482 triangles in 11 calls, with a single-frame wall measurement of 61.9 ms.
- Building the prior `examples/ranged.ts` source with the same compiler produced byte-identical artefact files.
- The full-suite run (`bun test --timeout 120000`) ended on SIGTERM (143) before aggregate counts. It had reported
  a 180 s example-pipeline timeout, a 120 s crab pose-table timeout, and the superseded 50 ms heron timing failure.
  The revised heron and LRU tests then passed together. [Full-suite log](../../out/small/suite.log).
- The isolated example-pipeline retry also ended on SIGTERM before reporting a result. The isolated crab retry
  exited 75 because another suite held the Mac test lock. The shared-lock change in `tests/setup.ts` arrived
  separately during verification. [Pipeline retry log](../../out/small/shaders-retry.log) and
  [crab retry log](../../out/small/crab-retry.log).

Verification used Metal. Linux/Vulkan and real driver hangs remain unverified. The deadline regression substitutes
a synchronous readback stall. The initial device acquisition still precedes the server's `ready` line.

# A render fails only when the scene is wrong

Two things a reward built on Loft needs: a render that does not fail for no reason, and a failure message that tells
the writer of the scene what to change. This file is the theory, written before the runs, and then the score.

## 1. The intermittent build failure

### What was seen

- `AGENTS.md`: "a shader module has also failed to build intermittently" under a loaded machine.
- `docs/gpu_native.md`: a load of reefback failed in Dawn with *The stage (Fragment) of the entry point "fs" isn't the
  expected one (Vertex)* for `surface.cutterDactyl`; the same load passed four times after.
- `docs/lineup.md`: the same message, load average 30, second load built.

The message says Dawn was handed `"fs"` as the **vertex** entry point. Loft's lowered manifest names `vs` for the vertex
stage (`src/host/lower.ts`); nothing in Loft can produce `"fs"` there. So the string changed between Loft and Dawn.

### Reading bun-webgpu 0.1.7 (`node_modules/bun-webgpu/index.js`)

`defineStruct` packs every string and every nested struct or array into a fresh `Uint8Array`/`ArrayBuffer`, takes its
address with `ptr(...)`, writes the address into the parent struct, and **drops the JS reference** (lines ~1482, 1493,
1522, 1575, 1592, 1610, 1621). Nothing keeps those buffers alive until the FFI call that reads them. Any allocation
later in the same pack (the next string, the next nested struct) can run the collector, free the buffer, and hand its
memory to the next allocation of the same size class. `"vs"` and `"fs"` are both two bytes: the vertex stage's name is
freed, the fragment stage's name lands on the same address, and Dawn reads `"fs"` for both stages.

### Hypotheses and what each predicts

| | Hypothesis | Predicts |
|---|---|---|
| H1 | **Use after free in bun-webgpu's struct packing** (above) | Failures are wrong strings: entry-point stage mismatches, WGSL parse errors in text Loft never wrote, unknown entry points; rate rises with *GC activity*, not with GPU load. With the JSC collector forced to run continuously (`BUN_JSC_collectContinuously=1`) the crab (125 render pipelines, ~100 modules, megabytes of WGSL) fails in **more than half** its loads and the shell (4 pipelines) in **at least one in ten**; with the packer keeping its buffers alive until the call returns, **0** of the same loop, with or without the forced collector. |
| H2 | A race between overlapping async pipeline creations or error scopes | The host awaits each creation in turn (`scoped`, host.ts), so at most one is in flight: predicts no failure attributable to it. If seen: an error naming one pipeline whose cause is another's descriptor, independent of GC. |
| H3 | Device lost or out of memory under a loaded machine | Messages naming a lost device or OOM; correlated with memory pressure, not with the collector. |
| H4 | A timeout | Loft sets none; Dawn's async callbacks have none. Predicts hangs, not errors. |
| H5 | Bun 1.3.10's own crash (`AGENTS.md` / wave_sim notes: exit 133 with Bun's panic banner) | A process killed by a signal with Bun's banner, at any stage, independent of the scene. Not fixable in Loft: can only be named. |

Baseline prediction, the machine at load ~25 with other workers: the crab fails in 0 to 3 of 48 loads, the shell in 0
of 48 (few allocations between pack and call). A baseline of 0 does not refute H1; the forced-collector run decides it.

### The plan

1. Baseline: `bun tools/render_loop.ts base-crab examples/bosses/reefback_titan.ts:game --procs 4 --runs 12`, and the
   shell the same way.
2. H1's amplifier: the same loops with `--env BUN_JSC_collectContinuously=1`.
3. The fix, if H1 holds: a `pnpm patch` of bun-webgpu that keeps every packed buffer alive until the current
   synchronous job ends (Dawn copies descriptors during the call, so that is long enough), then the same four loops.
4. Whatever remains (H5) is classified as the environment's: retried a bounded number of times, then a distinct exit
   code and "not your scene".

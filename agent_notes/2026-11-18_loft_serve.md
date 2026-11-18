# `loft serve`: creative's warm render server, upstreamed (2026-11-18)

Branch `loft/serve`, from main 92b10c2. The command and protocol are documented in `docs/handoff_training.md`
("loft serve"), and the design as built in `DESIGN.md` §3.2. This note holds the evidence and the decisions.

## What was built

- `src/cli/serve.ts`: the server. Requests are JSON lines. A request is a `scene`, compiled in a `loft build` child
  with `LOFT_NO_GPU=1` and wrapped in `--confine`, or an `artefact` directory. There are up to `--compiles`
  compiles at once and one draw at a time. Each answer carries `exit` and `line` from Loft's taxonomy
  (`verdict.ts` `ending`, which `exitWith` now uses too). After each request the server checks, from Bun's module
  registry, that it has loaded no compiler or scene module.
- `src/host/cache.ts`: modules, bind group layouts, pipeline layouts and pipelines, cached by content. The design is
  creative's; the key is now the whole descriptor.
- `src/host/headless.ts`: `LOFT_NO_GPU=1` refuses a device (exit 6), so a compile child cannot reach the GPU even when
  no sandbox is used. A scene with `recur` therefore has to be built outside serve.
- `src/host/host.ts`: headless frames plan their level of detail with an empty memory (determinism, below).
- `tools/serve_bench.ts`: cold against warm, interleaved. It is also an example of a `--confine` wrapper: it logs
  each compile's CPU.
- `tests/serve.test.ts`: A, B, A and a bad request against a fresh `loft frames`. It fails if the cache key ignores
  the WGSL (checked by mutating the key).

**General Loft against creative's policy.** Loft provides the split, the protocol, the cache, the no-GPU compile and
two hooks. `--confine` is a command prefix: creative's `confine.py` profile goes there, and its allowed imports and
AST screening stay in its own compile step. `--check` is a trusted module that refuses a `Bundle` before it reaches
the GPU: creative's bundle.ts limits go there. Loft does not adopt creative's bundle format. Its per-frame `cpu`
uploads are not needed: the 10 training programs below build with 0 `cpu` tables on main
(`tables 70 map, 33 scan, 0 cpu`). Loft also does not adopt creative's limits. Its 16 MB WGSL cap would refuse Loft's
own largest scene (22 MB of shader).

## Determinism proof

Main's hazard, measured: `render examples/strike.ts --views front,side,top,three` drew `side` as
`8e1460aaab0070ac` (63313 triangles), while `--views side` alone drew `5bb4fc4139ce2025` (62321). `top` and
`three` moved as well. Cause: `posedPlan` planned each headless frame with the hysteresis memory of the frame before
it. Fix: plan with `new Map()`. Only the interactive `draw` keeps the memory.

In one server, in this order: crab-r42 (A, `fixed`, t 0.91, 512²), strike `front`, strike `side`, then crab-r42 again.

| | serve | fresh `loft render` (this branch) |
|---|---|---|
| A, first | `3a4c069fc2c25577` | `3a4c069fc2c25577` |
| strike front | `18520c70331ba426` | `18520c70331ba426` (`--views front,side`) |
| strike side, after front | `5bb4fc4139ce2025` | `5bb4fc4139ce2025` both after front and alone (62321 triangles) |
| A, again | `3a4c069fc2c25577`, with 90 more cache hits | |

In the bench, each server drew 14 scenes and then all 14 again. All 28 repeats are equal to their first draw, and all
112 warm frames are equal to the cold process's frames. Against main, the 40 frames of the 10 training programs (4
times each, camera `fixed`) are unchanged. Only frames after a large camera change move, as in strike's views above.

## The 64-lane cap

Loft main has no lane cap. Creative's cap was in its own hostile-data reader (`training/loft/host/bundle.ts`, `lanes:
int(…, 0, 64)`, fixed on `loft-curriculum-order2` b7d4cab1 to `LIMITS.bufferBytes / 4`). Loft's `readBundle`
validates format, version and blob sizes, not field ranges. Loft's own artefacts exceed 64 lanes: `examples/strike.ts`
has a table of 673 lanes, and train-0-group-0 has 53. A `--check` module ported from bundle.ts must not reintroduce the
cap: a table's memory is already bounded by `offset + vec4s` against its buffer.

## Measurements

`bun tools/serve_bench.ts --rounds 2 <14 scenes>`, 2026-11-18, Apple M4, shared with other workers' Bun, Rust and
Chrome processes. The 1-minute load was 7.5–13.7 in round 1 and 12.5–17.1 in round 2. Every row is in
`out/serve/bench/rows.jsonl` (ignored by git). Each round started a fresh server. Per scene, the cold and warm runs
were interleaved: cold first in round 1, warm first in round 2. Then every scene was requested again. Each run drew
four frames (t = 0, 0.37, 0.91, 1.53) at 512 x 512. The programs were creative's 10 server-bench programs
(`agent_notes/2026-11-16-render-fast/server/programs.txt`). Each was wrapped as a scene the way creative's
`compile.ts` wraps it (sea with no swell, its look, its `fixed` camera; the import path rewritten to this checkout).
The examples were shell, lantern, pearls and strike.

**Measured**, per run:
- wall time;
- Bun CPU: the cold process's rusage, which includes its children (`cwebp`). For warm runs, the compile child's
  rusage (logged by the `--confine` wrapper) plus the server's own CPU from `ps` around the request;
- MTLCompilerService CPU, sampled every 100 ms. This is approximate: it misses the last 100 ms of a dying service,
  and another GPU client compiling at the same time would be counted.

Medians over the 10 training programs:

| | cold wall | cold Bun CPU | cold MTL | warm wall | warm Bun CPU (compile + server) | warm MTL | again wall | again Bun CPU |
|---|---|---|---|---|---|---|---|---|
| round 1 | 5.02 | 5.68 | 6.78 | 4.10 | 4.97 | 6.03 | 0.78 | 1.53 |
| round 2 | 1.33 | 2.13 | 0.00 | 0.96 | 1.86 (~1.1 + ~0.6) | 0.00 | 0.55 | 1.16 |

Paired warm against cold, median of the per-program differences: −0.44 Bun CPU s in both rounds, and −0.44 s (round
1) and −0.33 s (round 2) of wall. Over all 14 scenes in round 2: cold 20.9 s wall and 32.3 CPU s in total, warm
16.9 s and 26.2 CPU s. Starting the server took 0.08 to 0.15 s to the ready line.

**Derived** (not measured directly):
- Round 1 is the first time this machine's Metal saw the training shaders. MTLCompilerService spent about 6 CPU s per
  program on either side, which agrees with creative's 10.5 s at its older shader count. In round 2 the Metal disk
  cache had them, so MTL was about 0 on both sides. A training run's programs are each new, but much of their WGSL is
  library code shared between them, so a real run falls between the two rounds.
- The warm saving, about 0.44 CPU s per program, is Bun start, the library import and device creation. Creative's
  estimate was about 1.1 CPU s, but 0.63 s of that was TypeScript loading in its sandbox, which this path does not do.
  Its remaining parts agree.
- What remains per program warm is the compile child, about 1 to 1.5 CPU s and typically 0.45 to 0.85 s of wall (Bun start
  plus `compile` plus `lower`). Load and frames in the server take 0.2 to 0.6 s once Metal has the shaders. The
  in-process cache matters most when the same program, or the same shaders, come back. Then load falls to about
  5 ms (the "again" column).
- So the next lever for throughput is not the server. It is the compile, which still has a cold Bun per program and
  could be made warm too: a warm confined compile worker, if the sandbox policy allows reusing one. The other lever is
  Metal misses, which only shared WGSL can cut.

## What creative must change to use it (for the owner-launched expedition)

1. **compile.ts.** Keep the screening, the transform and the wrapper. Replace the custom bundle with
   `writeBundle(dir, lower(a))` from Loft's `src/host/files.ts`. Drop the per-frame `cpu` uploads: `writeBundle`
   refuses a `cpu` table by name, and none of the bench programs has one on main.
2. **render.py.** Start `bun <loft>/src/cli/main.ts serve --check <creative>/host/bundle.ts`. `--compiles` does not
   matter here, because `artefact` requests compile nothing. Send `{"id", "artefact": dir, "camera": <camera.json object>, "times", "size":
   "512x512", "out"}`, read frames from `<out>/<k>.rgba` (512 x 512 x 4 bytes), and take the sha256 from the answer.
   The LFRQ/LFRS framing goes. The pool (`render.SERVERS`) is unchanged: N servers, N devices. Confine the server as
   today (no network, reads of the Loft checkout and the job directories, writes only to `out` directories). Times
   within [0, 60] and at most 16 frames stay render.py's checks.
3. **bundle.ts becomes a `--check` module.** Port `manifest()` to format 3: optional `fragment` and `color` (depth
   passes), texture `size`, `frame.replay`, `frame.copies`, and `instanceBase` optional. Keep LIMITS as creative's
   policy, with `lanes` uncapped. The default export takes a `Bundle`; throwing refuses it, and the answer is `load`
   (3). The known-shaders mode fits the same hook: hash `mod.chunks` joined from `bundle.chunks`.
4. **Verdicts.** `exit`/`line` replace `fault: "bundle" | "draw"`. A `--check` refusal or an unreadable directory is
   3; a lost device is 6, and the server exits; `internal` (1) is "unknown". Classify a dead server (EOF) as a host
   failure, never as the program's fault.
5. **Pixels.** On main, the training frames are unchanged from creative's server, apart from the level-of-detail
   change. That change moved none of the 40 bench frames at the fixed camera. Re-baseline anyway with the new
   library commit, because creative's frozen library predates main's format 3 and shared shaders.

## Left undone

- No structural validation of a manifest in Loft itself. A malformed or hostile directory is refused by WebGPU
  validation or by a JavaScript error inside the request (an answer of exit 1 or 3; the server goes on), unless the
  host passes `--check`. A port of creative's checker to format 3 belongs in Loft eventually (`readBundle`, typed
  `LoadError`), with limits that admit Loft's largest scenes.
- `tools/check_loader_closure.ts` cannot check serve.ts structurally, for two reasons. `--check` is a dynamic import
  of a path that is not a literal. And `verdict.ts` imports the library lazily to classify refusals, although serve
  never calls that path. The runtime witness after each request stands in for the check.
- No per-request timeout or CPU limit in the server: those belong to the confinement. A draw that hangs the GPU
  hangs that server, so the caller must kill and restart it.
- A transient pipeline failure is retried on a fresh device, at most 3 times, as `render` does. This path was not
  exercised: no such failure happened.
- Requests take no `params` or `inputs`, and no still mode. Add them when a caller needs them.
- Linux was not tried.

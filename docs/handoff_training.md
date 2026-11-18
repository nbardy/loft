# Handoff to the training team

Written 2026-11-13, for whoever runs the first training of a model that writes Loft scenes. Everything below was
checked on `main` on the day it was written; the commit is at the end.

## What Loft is

A TypeScript library. A scene is a module that default-exports `scene({ ground, things, style, sun, cameras })`; shapes,
motion and paint are closed-form expressions, compiled to WGSL and drawn by WebGPU. Time is an argument, so any frame
draws alone and nothing is simulated.

## Setup

- macOS with a GPU is the only platform this has run on. See "Linux" below.
- `bun` 1.3.10, `pnpm` 10. `pnpm install` (it applies a patch to `bun-webgpu` 0.1.7: see "False failures").
- `cwebp` for stills; `ffmpeg` only for clips.
- All commands below run from the repository root. `loft` means `bun src/cli/main.ts`.

## What to give the model

- **`bun src/cli/main.ts context`**: the whole of it, as the system or task context. About 2,200 words (18,000
  characters). It prints in under a second with no GPU, is deterministic for a commit, and its API half is generated
  from the source, so regenerate it whenever the commit changes. It holds: what Loft is and its conventions, one
  complete smallest scene, a craft part of about 450 words led by "change the construction, not the number", the API
  as `name(args)` for everything a scene author calls, and four mistakes that draw a wrong picture with no error.
- **Optional, if the model can call a tool**: `bun src/cli/main.ts api <name>` prints one function whole, with its
  law, what it cannot do, and relatives to try. `bun src/cli/main.ts api` prints the full reference (also `docs/api.md`).
- **Not recommended in context**: the example scenes. They are the strongest pull toward our own shapes. The context
  says so, and `examples/README.md` indexes them by idea for people.
- Longer reading for people: `docs/guide.md` (how to use it), `docs/craft.md` (the long craft guide), `docs/styles.md`.

## Where a sample scene goes

Write each sample as `train/<id>.ts`, importing from `"../src/index.ts"` exactly as the context's example does. `train/`
is ignored by git and is included by `tsconfig.json`, so the type checker sees it.

## The two gates

1. **Does it type-check**: `pnpm exec tsc --noEmit` (exit 0 or not; errors name `train/<id>.ts(line,col)`). It checks
   the whole project in about 2 seconds, so check one sample at a time or a batch at once and attribute errors by file
   name. On the authoring machine a shell hook rewrites a bare `tsc`: use `rtk proxy pnpm exec tsc --noEmit` there.
2. **Does it render**: `bun src/cli/main.ts render train/<id>.ts --views <camera> --size 320x200 --out <file.webp>`.
   The smallest scene takes about 0.9 seconds end to end. Without `--views` it draws four default views; a scene's own
   camera names (the context asks for two) are what `--views` takes.

For motion: `bun src/cli/main.ts frames train/<id>.ts --view <camera> --times 0,0.5,1,1.5 --out <strip.webp>` gives one
strip; `clip` gives an mp4. For a scene that exports `checks`: `bun src/cli/main.ts check train/<id>.ts`.

### Exit codes (`render`, `frames`, `clip`, `check`)

The last line on stderr matches `^loft: (internal|usage|load|scene|check|environment)`.

| Exit | Last line starts | Meaning | Whose fault |
|---|---|---|---|
| 0 | | drew, or every check passed | |
| 1 | `loft: internal` | an untyped error: a Loft bug, or a mistake nothing catches yet | unknown: treat as neither reward nor penalty, and please send us the scene |
| 2 | `loft: usage:` | the command line asked for something absent (a camera, a style, an input) | the harness, or the scene's camera names |
| 3 | `loft: load:` | the module did not load: syntax, an unknown import, a throw at top level, a wrong default export | the scene |
| 4 | `loft: scene:` | a typed refusal: `given …; expected … Fix: … (at file:line)` | the scene |
| 5 | `loft: check:` | the named checks failed or a ranged check could not decide | use the check's result and reason |
| 6 | `loft: environment (not the scene's fault):` | the GPU, the WebGPU binding, or a missing tool | the machine: retry once, else discard the sample |

A refusal's last line is written to be fed back to the model as it stands: it says what was given, what was expected
and the smallest fix. `docs/errors.md` has the table of the 23 mistakes we test (`bun tests/fixtures/mistakes/run.ts`).

`runChecks(scene, checks)` returns tagged results. `kind: "sampled"` has a boolean `holds`. `kind: "ranged"` also
has `bounds`, `witness`, `work` and `spent`, and its `holds` can be `"undecided"`. Read `spent` for the reason a
proof stopped. For example, `examples/ranged.ts` pairs two ranged motion checks with a sampled triangle budget.
The CLI counts undecided results separately. The CLI and gallery both label them `undecided`.

### What each kind of failure prints

Checked on the day of writing with one sample of each. No stack trace is printed for any of these.

| The sample | Type-check gate (`tsc`) | Render gate |
|---|---|---|
| does not parse (a missing brace) | `train/x.ts(2,129): error TS1005: ',' expected.` | exit 3: `loft: load: train/x.ts failed to load: 2 syntax errors: train/x.ts:2:129: Expected "}" but found ")" near \`…\`; …` (file, line, column, what was expected, the text round it; at most three) |
| wrong type (a string where a number goes) | `train/x.ts(2,69): error TS2322: Type 'string' is not assignable to type 'number \| Fn1'.` | may still draw, or be refused with exit 4: the type checker is the gate for this |
| imports a name Loft does not export | `error TS2305` naming it | exit 3 with the nearest real name as the fix, if the name is used; an unused import is dropped by the runtime and draws |
| throws while loading (its own code) | passes | exit 3: `loft: load: … failed to load: <the error's message> (at train/x.ts:3)` |
| is refused by the library | passes | exit 4: `loft: scene: <who>: given …; expected … Fix: … (at train/x.ts:3)` |

So a parse error is reported by both gates with its line and column. The type checker reports every error in the
file; the render gate stops at the first failure. Feed the model the type checker's lines when that gate fails, and
the render gate's last line when that one does.

### False failures

A render used to fail now and then for no fault of the scene: a use-after-free in `bun-webgpu` 0.1.7 let the garbage
collector free a descriptor's strings before the GPU layer read them. It is patched (`patches/`, applied by
`pnpm install`); with the collector forced to run constantly, 36 of 36 load attempts failed before and 0 after. As a
second line, a shader or pipeline that fails to build is retried up to three times on a fresh device inside the
command; if it still fails the exit is 6. Do not upgrade `bun-webgpu` without redoing the patch.

### Determinism

A fresh process gives byte-identical pixels for the same scene, camera, time and size (each frame line prints an
`rgba <hash>`). Since 2026-11-18 a headless frame (`render`, `frames`, `clip`, `serve`) also does not depend on what
the process drew before it: the host's memory of each surface's level of detail is kept only by the interactive
`draw` (`loft dev`, a game), where it stops a slow dolly from flickering. Before that change, `render
examples/strike.ts --views front,side` drew `side` with 63313 triangles and `--views side` alone with 62321, so
"one scene per process" was the rule. Many scenes per process are now fine; `loft serve` (below) relies on it.

### Time and throughput

- A small scene: about 0.9 s (compile 0.1, load 0.5, frame 0.02). Load time grows with content: a large creature is
  seconds, and our largest scene (five creatures, 22 MB of generated shader) takes one to three minutes. Set a timeout
  and decide whether running out of it is a penalty.
- One GPU is shared. Seven heavy renders at once made a 5-second render take 283 seconds here. Measure your own
  throughput before choosing a concurrency.

### `loft serve`: many programs, one warm GPU process

`bun src/cli/main.ts serve [--compiles 2] [--confine "cmd args"] [--check check.ts] [--pending 64] [--compile-seconds 120]
[--draw-seconds 120] [--limits manifest=BYTES,files=N,bytes=BYTES]` supervises one warm GPU child that draws requests
one at a time. It is upstreamed from creative's training host (`training/loft/host/`, 16 Nov). Each scene compiles
in a separate GPU-less child running `loft build` with `LOFT_NO_GPU=1` inside `--confine`. The GPU child reads the
built directory (`docs/artefact_format.md`) as `render --artefact` does. Both the supervisor and GPU child check that
the compiler and scene code stay in the build child.

Protocol: JSON lines. Each line on stdin is a request; each line on stdout is an answer tagged with the request's `id`,
in the order the requests finish. The first line out, unasked, is `{"ready": true, "adapter": ...}`.

```
{"id": "s1", "scene": "train/s1.ts", "camera": "fixed", "times": [0, 0.37, 0.91, 1.53], "size": "512x512", "out": "out/s1"}
{"id": "s2", "artefact": "out/files/s2", "camera": {"kind": "perspective", "eye": [4.5, 4.5, 8], "target": [0, 0.7, 0], "fovY": 0.62}, "times": [1], "size": "512x512", "out": "out/s2", "params": {"stormwing.span": 1.4}, "inputs": {"strike": 0.5}}

{"id": "s1", "ok": true, "tier": "C", "frames": [{"time": 0, "file": "/abs/out/s1/0.rgba", "sha256": "…"}, …], "ms": {"compile": 540, "load": 308, "frames": 48}, "cache": {"hits": …, "misses": …, "entries": …}}
{"id": "s2", "ok": false, "tier": "C", "exit": 4, "line": "loft: scene: …", "detail": ["…"]}
```

- `scene` (compiled here) or `artefact` (a directory you built, for example in your own sandbox); `camera` is a name
  the scene declares (or `front`, `side`, `top`, `three`), or a camera object. A camera object is how a host keeps
  its own camera without trusting the artefact's. Frames are raw RGBA, `width x height x 4` bytes, at
  `<out>/<k>.rgba`.
- `params` (`{"name": number}`) and `inputs` (`{"name": number}`, or a list for a vector input) are optional and
  work as `--param` and `--input` do. A name the scene does not declare, or the wrong number of components, is
  refused as `usage` (2) by name, with the names the scene has.
- Each request owns what it writes. A `scene` builds in a private directory under `$TMPDIR`, removed after its
  answer (the build is not kept). Two requests in flight may not name overlapping `out` directories (the same one,
  or one inside the other): the later is refused as `usage` (2) and names the request holding it. Once a request is
  answered its `out` is free again.
- An artefact is checked before any WGSL or bin file is read: the manifest's size, its format and version, that every
  file it names is one plain name (no `/`, `..`) and a regular file (no links), each bin file's declared size, and
  the totals against `--limits` (defaults: manifest 64 MiB, 65536 files, 1 GiB in all). Anything wrong is `load` (3).
  `--check` then sees the bundle before it reaches the GPU.
- Every answer carries `tier`. Only `"C"` (the compiled scene) exists; the field is there before an interpreter is.
  A frame's identity is (scene, camera, time, size, params, inputs, tier, library commit, adapter, driver). In
  training the tier is chosen once per sample and never promoted within a run, so a frame never depends on what the
  server saw before.
- A failed request is answered and the server goes on. `exit` and `line` are what `loft render` would have ended
  with, from the table above. A refusal by `--check`, or a directory that cannot be read, is `load` (3). The
  server's own limits answer `budget` (7, `loft: budget: …`), naming the limit: a compile still running after
  `--compile-seconds` (killed), a compile killed by `SIGXCPU` (a CPU limit your confinement set, also reported as
  exit 152 by some wrappers), a draw that exceeds `--draw-seconds`, or a full `--pending` queue (refused at once).
  The draw deadline starts at the front of the draw queue and covers artefact loading and all frames. On expiry
  the supervisor kills the GPU child, answers that request with 7, and starts a fresh child and cache for the
  next draw. Discard any files a failed request already wrote to its `out`. An unexpected device loss answers
  every admitted request with 6 and ends the server with exit 6. Start a new server in that case. A compile killed
  by another signal is `internal` (1), because its cause is unknown.
- Why JSON lines: any language reads them with no framing code, a request can be typed by hand, and pixels go through
  files, which the caller already uses for scenes and builds. The server prints nothing else on stdout; logs go to
  stderr.
- Concurrency: compiles run at once, up to `--compiles` (CPU only). Draws run one at a time on the one device. One
  server stops renders from fighting over the GPU. For more throughput, run several servers, each with its own device,
  and measure.
- Shader modules, layouts and pipelines are cached by content across requests (`src/host/cache.ts`). Equal WGSL is
  one module, and a pipeline is a pure function of its WGSL and descriptor, so the cache cannot change pixels. Past
  8192 objects it is emptied between requests. A draw deadline also resets the cache and any state in `--check`.
- What stays the host's policy: the sandbox (`--confine` is a command prefix, such as creative's `confine.py`, split
  on spaces; it should `exec` the build so the deadline's kill reaches it), the screening of programs and allowed
  imports, CPU and memory limits (set them in the confinement), and limits on what an artefact may ask for beyond
  `--limits` (`--check`, a module whose default
  export gets each `Bundle` before it reaches the GPU and throws to refuse it). A scene with `recur` bakes on the GPU, so its
  compile fails with exit 6. Build it outside serve and send the directory.
- A built directory has no loop period yet, so a looping scene's time is not wrapped, the same as `render --artefact`.

Measured on 2026-11-18 (`bun tools/serve_bench.ts`, Apple M4, shared machine, 1-minute load 7.5 to 17). The runs
were interleaved, the order alternated by round, and there were two rounds with a fresh server each. 10 programs
from creative's training runs (wrapped as scenes the way its host wraps them) and 4 examples, four frames at 512 x 512
each:

| median per training program | cold `loft frames` | warm `serve` | warm, same program again |
|---|---|---|---|
| wall s, Metal cache cold (round 1) | 5.02 | 4.10 | 0.78 |
| wall s, Metal cache warm (round 2) | 1.33 | 0.96 | 0.55 |
| Bun CPU s, round 2 (warm: compile child + server) | 2.13 | 1.86 (1.1 + 0.6) | 1.16 |

- Pixel identity: all 112 warm frames (28 requests of 4) equal the cold process's frames, and every repeat equals
  its first draw.
- The warm server saves about 0.44 CPU s and 0.35 to 0.45 s of wall per program (paired median, both rounds). That
  is Bun start, the library import and device creation. Creative's 1.1 s estimate also counted 0.63 s of TypeScript
  loading in its sandbox, which Loft's own compile does not do.
- The big costs are elsewhere. One is the compile (about 1 CPU s, in the child). The other is Metal compiling new
  shaders (about 6 CPU s per new program, in MTLCompilerService, outside Bun). The in-process cache removes the second
  only for shaders the server has already seen. Here those were the shared sea and sky, and a program's own shaders
  when the same program came again.

## What a scene can be

- **Ground**: `plane` (a floor at y = 0), `nothing`, or `sea({ swell: [...] })`. Things that need water refuse other
  grounds by name.
- **Styles**: `toon`, `woodblock`, `biolume`, `papercraft`, `sketch`, `neon`, or a `Style` value the scene composes
  from the exported parts. `--style <name>` redraws any scene in another style without editing it, and `--light <name>`
  relights a style that has named lightings: useful for a judge that scores form apart from look.
- **Since `train-1`** (main after 2026-11-15; the first run used the tag `train-1`, without these):
  - lights as functions of a world point, `lamp`, up to seven besides the sun;
  - paint words (gloss, metal, emits, or the author's own), `scatter` and `shade`, and `glossed` (bodies mirror the sky);
  - `draw.mesh` (triangles from code), `draw.traced` and `march` (the author's own tracer), `castRay`;
  - `recur` (a step baked on the GPU into the files);
  - `pass` (another camera's picture: shadow maps, mirrors);
  - still mode (`--samples`, `--shutter`, `--aperture`).
  - `lamp(..., { shadow: true })`: a lamp's shadow from a depth picture taken from it (about +1.3 ms soft);
  - `loft bake --animated` (a skinned, animated glb); `recur` resumes playback from the last state;
  - creative's library speed work (shared draw shaders, concurrent pipelines, compile interning): pixel-identical,
    and a built file whose draws share rows is format 3.
  Regenerate `loft context` (now about 2,400 words) for any run on main.
- **Pixels that moved on purpose after `train-1`** (main `a46e697`; a host comparing stored renders will see them):
  a sea with no swell no longer prints noise artefacts (sketch's grey labyrinth, woodblock's dark disc, papercraft's
  white blobs; woodblock keeps its pale blue, about 1.4% of pixels move by more than 8/255), and a style with
  outline 0 draws no ink at all (it drew 1.5 px rims; the training host's "smooth", `toon({ outline: 0 })`, changes).
- **Refusals that were exit 1 and are now exit 4** (typed, with the fix): `stations` below 4, an unknown topology
  word, a style `outline` that is not a number or expression, bad `bound` arguments, and a binary helper (`sub`,
  `div`, ...) given other than two operands. `add`, `mul`, `min` and `max` take any number.
- **Not there**: bounced light and reflection of one body in another inside a normal frame (it is not a path tracer: a
  mirror is a `pass`, a ray is `castRay` against stand-in capsules), imported meshes (meshes come out, through
  `loft bake`), live state that a game's input changes.

## What we know is weak

- **Unproven modules.** Planted stance, struck poses, expression casts, emotes, group routines, placing, the ring wave
  and several-subjects-in-one-scene were written for one creature. Each now has a second, unrelated example
  (`examples/second/`), and several were loosened where that example had to fight them, but two users is not many.
  They are marked `~` in the context's map.
- **Messages.** Three of the 23 tested refusals are still vague (a field that is not a distance, a non-finite
  constant, an option missing from a slot), and anything no constructor catches exits 1 with a stack trace.
- **Opaque names.** A few exported names say nothing about what they do (`retarded`, `kifs`, `whorl`, `crown`); the
  context's map does not gloss them. `loft api <name>` does.
- **Soft things** (clouds, glows) keep their own colours in every style, so they look wrong in `sketch` and `neon`.
- **`sketch`** takes its tone only from the light: a body painted in two mid-value colours becomes one white shape.
- **The pull of our examples has not been measured.** The planned test (a fresh model, only `loft context`, three
  unrelated prompts, no references: do the results share our silhouettes, proportions or palette?) has not been run.
  Your first batch is that test: if samples converge on swept tubes on a calm sea in the toon style, tell us.
- **Faults found while documenting, not yet fixed** (each is stated in its function's comment): `rayOf` ignores one
  argument; `rain`'s streak fade is random where it should follow distance; `replants` throws when every foot is on
  one side; `poseFrame` and `course` measure heading in opposite senses; `hit.arc` does not check its range.

## Linux

Not built or run; this is from the code and the binding's documentation. `bun-webgpu` ships an x64 Linux build of
Dawn (no arm64). Loft asks for an adapter with no window, so no display is needed. With a GPU it needs a Vulkan driver.
Without one it needs a software Vulkan driver (Mesa lavapipe, made visible through `VK_ICD_FILENAMES`), which should
work and will be much slower. With no adapter the exit is 6 with `no WebGPU adapter found`. If your training runs on
Linux, tell us before you start: this is the one item that could take days.

## Deferred, by decision

- A differentiable renderer: `docs/differentiable.md`. Not built; the first run does not use it.
- Judging packets, reference pictures, any reward beyond the two gates: the training team has its own.

## State of main when this was written

The whole test suite passes: 215 of 215 tests in 54 files (`pnpm test`, about five minutes; use that script and not a
bare `bun test`, which gives some GPU tests a 5-second limit they miss when everything runs at once). Type-check clean;
24 of 24 mistake fixtures as expected; every snippet in the docs and in `loft context` runs; the recipe and style
import checkers pass.

## Checks to run after pulling a new commit

```
pnpm install
pnpm exec tsc --noEmit
bun tests/fixtures/mistakes/run.ts        # the refusals still say what they said
bun tools/check_snippets.ts               # every snippet in the docs and in `loft context` runs
bun src/cli/main.ts context | wc -w       # the size of what the model is given
```

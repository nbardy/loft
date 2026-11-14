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
| 5 | `loft: check:` | it drew, and the named checks failed | the scene |
| 6 | `loft: environment (not the scene's fault):` | the GPU, the WebGPU binding, or a missing tool | the machine: retry once, else discard the sample |

A refusal's last line is written to be fed back to the model as it stands: it says what was given, what was expected
and the smallest fix. `docs/errors.md` has the table of the 23 mistakes we test (`bun tests/fixtures/mistakes/run.ts`).

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
`rgba <hash>`). Within one process a frame can depend on what was drawn before it, because the host remembers each
surface's level of detail from the previous frame. So: one scene per process, times passed explicitly.

### Time and throughput

- A small scene: about 0.9 s (compile 0.1, load 0.5, frame 0.02). Load time grows with content: a large creature is
  seconds, and our largest scene (five creatures, 22 MB of generated shader) takes one to three minutes. Set a timeout
  and decide whether running out of it is a penalty.
- One GPU is shared. Seven heavy renders at once made a 5-second render take 283 seconds here. Measure your own
  throughput before choosing a concurrency.

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
  Regenerate `loft context` (now about 2,400 words) for any run on main.
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

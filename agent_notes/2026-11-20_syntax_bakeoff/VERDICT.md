# Syntax bakeoff verdict (2026-11-20)

The judge read all eight outputs in full. Syntaxes: **A** WGSL + `kind { row {} }` declarations; **B** WGSL + modules,
lambdas and `add(...)` (a kind is implied by lambda capture); **C** Slang/HLSL `[Kind] struct`; **D** Python-like
`@kind class`.

Token counts come from tiktoken `o200k_base`, a stand-in for Claude's tokenizer, over the code blocks only.
Each cell has one sample and one judge. Treat a difference of one or two violations as noise.

## Per-syntax tables

Semantic checks, in order: crab legs are 12 rows of one kind / reeds are 200 rows of one kind / gaze goes through
world space / the neck limit hands off to the body turning.

### A: WGSL + declarations

| | Sonnet | Haiku |
|---|---|---|
| Tokens T1 / T2 / T3 (total) | 2662 / 1092 / 1997 (5751) | 3062 / 1279 / 2549 (6890) |
| Violations | 4: `chitin(Pt[Leg])` called with Claw and Body points (E104); raw WGSL hash used only for comptime placement (raw escape not needed); `/ self.stiff` not clamped (E211); two laws `writes Heron` | 4: `map sym = mirror_x(q)` at file scope with no query point; `let sun_sky = SunSky{..}` struct literal instead of `emit` (both tasks); invented `clamp_ldr` pass; two laws `writes Heron` |
| Semantics | pass / pass / pass / pass | pass / pass / partial (conversion written correctly, but the map is unbound) / pass |
| Readability, cold | 4: one place per idea (`row`, `query`, `comptime for`, `pass`); only `'sym` is cryptic | |
| Haiku gap | **0** (4 vs 4); no staging errors, because `comptime for` marks the stage on the loop itself | |

### B: WGSL + lambdas / `add`

| | Sonnet | Haiku |
|---|---|---|
| Tokens (total) | 2776 / 1545 / 2571 (6892) | 2628 / 1330 / 2652 (6610) |
| Violations | 3: `/ stiffness` and `/ height` not clamped (E211); `ink` looks passed to `add` as materials, though looks go to `deferred`; `map h = translate(..)` has no domain argument | 6: `leg_socket` and `m_per_s_dt` used but never written; eye `map` at comptime with no `p`, which yields **one** eye; the gaze target `time()` is read at build so it never drifts; two laws return `Heron`; `Pt * k`; legs given the sand material |
| Semantics | pass / pass / pass / pass | pass / pass / **FAIL** (static, single eye) / pass |
| Readability, cold | 3: a kind is implicit (12 `add` calls with one lambda equal one kind with 12 rows). A stranger cannot see the kind boundary. State kind and render `add` are parallel books; schedule threads `s, w` everywhere | |
| Haiku gap | +3 plus the only semantic failure, on the hardest feature (mirrored gaze) | |

### C: Slang/HLSL

| | Sonnet | Haiku |
|---|---|---|
| Tokens (total) | 2958 / 1307 / 2587 (6852) | 2744 / 1050 / 2199 (5993) |
| Violations | 4: four socket helpers never written; `chitin(Pt<Body>)` called with Claw and Leg points; `__wgsl` hash only for comptime placement; CrabBody, Eye and Shell left out of the draw lists | 7: `Pt<Face> eye_c = sym.pt(..)` and stalk mixes Face with `sym.Space` (E104); `Radians` is not a card type; `0.8m * size` gives m^2 (several places); `emit` inside a `technique` pass; two laws write `Heron`; `wrap_angle` never written; no head geometry |
| Semantics | pass / pass / pass / pass | pass / pass / pass (path right, types wrong) / pass |
| Readability, cold | 3: familiar to shader programmers, but heavy (`.field =`, `ISpace` structs, `Pt<sym.Space>`); C `for` with float stepping at comptime | |
| Haiku gap | +3; unit and space typing breaks first | |

### D: Python-like

| | Sonnet | Haiku |
|---|---|---|
| Tokens (total) | 3242 / 1408 / 2748 (7398) | 2619 / 1006 / 2031 (5656) |
| Violations | 1: `sun_sky = SunSky` aliases a class, where a row was intended | 6: **unstaged** top-level calls `crab(..)`, `bed(200)`, `emit(..)`, `spawn()`; `random.Random` in comptime (Python runtime); `rows(Kind, ..)` instead of a build function; two laws write `Heron`; `sky_color` undefined; no material on legs or claws |
| Semantics | pass / pass / pass / pass (clean; split Trunk/Head kinds, E211 clamped) | pass / pass / pass / pass |
| Readability, cold | 4: the easiest to read line by line; `Pt["World"]` strings and `*m` units are noise; staging is invisible | |
| Haiku gap | **+5**, the largest. Python invites Python: script-level code and stdlib | |

## Cross-cutting findings

- **No output wrote** a continuous kind parameter or an authored Lipschitz value. The E401/E402 rules and derived
  bounds were not a problem in any syntax.
- **Five of the eight outputs had two laws write the same kind**: A×2, B-Haiku, C-Haiku and D-Haiku. It is the
  most common error, and none of the four syntaxes prevents it. Only the outputs that split the heron into Trunk and
  Head kinds avoided it.
- **Shared material helpers typed in one space** (`chitin(Pt[Leg])`) broke in A-Sonnet and C-Sonnet.
- **Helpers shown in card examples but never defined** (`leg_socket`, sockets) went missing in B-Haiku, C-Sonnet and
  C-Haiku. This is a gap in the cards, not in the syntaxes.
- Token totals agree within about 7% averaged over both models (A 6320, C 6422, D 6527, B 6751). Token count does
  not decide between them.

## Ranking

1. **A**: no Haiku gap, all semantics pass, cheapest, and staging is visible in the syntax.
2. **D**: best ceiling (Sonnet nearly clean), but the weaker model fails it in a predictable way.
3. **B**: fewest Sonnet errors after D, but kinds are implicit, and Haiku broke the gaze.
4. **C**: most verbose, the most unit and space typing errors, and the largest violation count for Haiku.

## Best syntax: A

A is the one to build. It is the only syntax where the weaker model made no more errors than the stronger one. Its
failures were local slips, not structural ones. The rule named in the rubric ("unstaged loops") is enforced by the
keyword on the loop. D shows that without that keyword, a model falls back to writing a script.

Ideas to graft onto A:

- **From C/D: the map as a value, `sym.p` / `sym.pt(..)`**, in place of the `'sym` tick-space and `sym - eye_c`.
  A-Haiku's map at file scope came from the declaration-like `map sym = ...`. Binding the map to the query point
  as a value makes that mistake hard to write.
- **From B/C/D: a typed verdict, `Verdict[Meters]` / `Verdict[Rad]`.** The A card shows a bare `Verdict`.
- **From D: keyword arguments when emitting a row** are already close to A's struct literal. Keep A's form.
- **New, for the most common error: field-level write ownership,** `law neck(h: Heron) writes Heron.{neck,
  gaze_err}`. Two laws on one kind become legal when their fields are disjoint, and the compiler can check the
  overlap. This is my own proposal; no syntax in the test had it.
- **New: generic material helpers,** `fn chitin[S](p: Pt[S])`, shown once on the card. This fixes the E104 slip
  that both Sonnets made.
- **For every card:** define `leg_socket` and show how `sun_sky` and `sky_color` are declared. Every output
  invented its own light and sky.

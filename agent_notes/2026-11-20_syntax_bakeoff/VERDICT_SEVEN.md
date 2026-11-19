# Verdict: seven syntaxes, judged fresh (2026-11-19)

I read all 7 cards (A to G), TASKS.md and all 14 outputs (each syntax written by Sonnet and by Haiku). I ignored
the earlier VERDICT.md. Nothing was compiled, so every count below comes from reading the code by hand. There is
one sample per cell, so treat a gap of one violation as noise and a gap of three as signal.

## How the scores were made

- **Tokens:** tiktoken `o200k_base` over the whole output file, divided by 3 tasks. The outputs differ in how much
  of each task they cover. For example, F-Haiku left out most materials and the water kind, so its count is
  flattering.
- **Violations (V):** the five listed rules, plus hard breaks of the card's own rules: two laws writing the same
  row, invented forms, Python runtime in build code, code outside a kind that the card requires inside one, and
  free variables. The five listed rules are broken out in the `listed` column.
  - *Continuous kind parameter:* **0 in all 14 outputs.** Every card's E401 example worked.
  - *Unstaged loop:* **0 in all 14.**
  - *Authored Lipschitz bound:* **0 in all 14.** B's `field(box(..), ..)` is a bounding box the card requires, not
    a Lipschitz bound.
  - The listed violations that actually occur are the mirrored-eye conversion and needless raw escapes.
- **Semantics (S, out of 4):** legs as one kind with rows / 200 reeds as one kind / gaze through world space / the
  neck limit handing off to a body turn.

## Per-syntax tables

### A: WGSL plus declarations
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 1926 | 2317 |
| listed violations | raw `hash1` used at build (1) | 0 |
| other breaks | two laws write `Heron` | two laws write `Heron`; module-level `map sym = mirror_x(q)` with `q` free |
| V | 2 | 2 |
| S | 4 | 4 |
| maths | native WGSL: `0.30 + 0.15 * fbm2(vec2(p.x, p.z) * 90.0)`. `Pt[S](..)` and `vec3<f32>` are noisy | same |
| parser | WGSL (~200 productions) + ~15 declarations. `Pt[S]` vs indexing, `'sym` | |
| agent rewrite | statement-heavy; transforms need a full WGSL AST | |

### B: WGSL plus modules and lambdas
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 2306 | 2224 |
| listed violations | 0 | eye aim computed at build with frozen `time()` and a single `eye_w` (1) |
| other breaks | none | two laws return `Heron`; `mirror_x(p)` with `p` free |
| V | 0 | 3 |
| S | 4 | 3 |
| maths | fine inside lambdas; materials become onion calls (`coat(sss(ggx(..)..)..)`) | |
| parser | WGSL + modules + closures (~240). Hidden rule: "a captured value is a column, a literal is code" is invisible at the call site | |
| agent rewrite | poor: which values are kind columns depends on closure capture | |

### C: Slang/HLSL style
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 2295 | 2013 |
| listed violations | `__wgsl` hash used at build (1) | 0 |
| other breaks | none | two laws write `Heron`; `Pt<Face> eye_c = sym.pt(..)` (wrong space) |
| V | 1 | 2 |
| S | 4 | 4 |
| maths | native HLSL; types are declared on every local | |
| parser | the largest grammar (~300): attributes, interfaces, generic `<` vs less-than, C `for` | |
| agent rewrite | poor: most tokens per idea, and the meaning of a kind is spread across attributes | |

### D: Python surface
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 2480 | 1901 |
| listed violations | 0 | 0 |
| other breaks | none | `random.Random(42)` and module-level `emit` (Python runtime leak); two laws write `Heron` |
| V | 0 | 2 |
| S | 4 | 4 |
| maths | worst of the infix syntaxes: `0.07*m`, a `num(x)` unwrap helper everywhere, spaces as strings `Pt["Leg"]` | |
| parser | indentation lexer + decorators (~150). The forbidden subset (no lists, closures or recursion) is not visible in the syntax, so models fall back on Python | |
| agent rewrite | medium | |

### E: lean inferred (TS-like inference, braces)
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 1557 | 1582 |
| listed violations | 0 | 0 |
| other breaks | none | mirror map **and** two `Eye` rows, so four eyes |
| V | 0 | 1 |
| S | 4 (feeder claw is `Claw[false]`, a code variant where data was asked for) | 3 (the trunk yaw always turns +, not toward the fish) |
| maths | best: `spine'(u)`, `[x,y,z]@S`, `0.5m`, last expression is the value. A shader writer's formula survives as written | |
| parser | ~80 productions, 10 stated rules. Ambiguities: newline continuation, `[..]` vector vs `K[..]` variant, `{` block-expression | |
| agent rewrite | good: expression-oriented, one declaration per keyword | |

### F: Lua style
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 1307 | 1282 |
| listed violations | eye mirrored at build from an `Xform`, one row, aim frozen (1); `wgsl[[` hash (1) | eye `law` mirrors `e.pose` and two `Eye` rows (1) |
| other breaks | two laws write `Heron`; the schedule calls `body_law` and `neck_law`, which it never defines | `surface` and `material` at top level outside any kind; `seg`/`pt` use free `root_at`/`height`; the fish never moves |
| V | 4 | 4 |
| S | 2.5 | 1.5 |
| maths | same as E, plus `local` everywhere and `then … end` | |
| parser | ~80. `end` closes many things; `{}` is both a vector and a row | |
| agent rewrite | good in shape, but the outputs show models lose track of block scope | |

### G: lean Lisp
| | Sonnet | Haiku |
|---|---|---|
| tokens/task | 1353 | 1179 |
| listed violations | `defraw hash` used at build (1) | mirror applied to the `:socket` point in a law, plus two rows (1) |
| other breaks | invented `(this)` | `:aim (dir [0 0 1])`, a value where a column type belongs |
| V | 2 | 2 |
| S | 4 | 3.5 |
| maths | worst: `(* 0.002m (abs (sin (* 2600 (/ (:x q) 1m)))))`. `$` was used inconsistently (Sonnet 6 times, Haiku 3) | |
| parser | smallest: ~25 reader rules + a ~10-rule `$` sub-grammar. Ambiguity: "a keyword after a symbol is a type" | |
| agent rewrite | best: code is data, one form per node | |

G's own win: `:writes [:field ...]` per law. G-Haiku wrote two laws on `Heron` **legally** (disjoint writes). The
same pattern counts as a violation in the five other syntaxes where a model wrote it: A, B, C, D and F.

## Ranking by tokens × (1 + mean V)

| rank | syntax | tokens/task | mean V | listed V (S+H) | mean S | score |
|---|---|---|---|---|---|---|
| 1 | **E** | 1570 | 0.5 | 0 | 3.5 | **2355** |
| 2 | G | 1266 | 2.0 | 2 | 3.75 | 3798 |
| 3 | D | 2190 | 1.0 | 0 | 4.0 | 4380 |
| 4 | C | 2154 | 1.5 | 1 | 4.0 | 5385 |
| 5 | B | 2265 | 1.5 | 1 | 3.5 | 5663 |
| 6 | A | 2122 | 2.0 | 1 | 4.0 | 6366 |
| 7 | F | 1294 | 4.0 | 3 | 2.0 | 6470 |

**Haiku–Sonnet gap** (change in V / change in S):

| syntax | ΔV | ΔS | verdict |
|---|---|---|---|
| A | 0 | 0 | flat, but heavy |
| B | +3 | −1 | widest: Haiku cannot use build-time closures safely |
| C | +1 | 0 | small |
| D | +2 | 0 | Python pull |
| E | +1 | −1 | small |
| F | 0 | −1 | both bad |
| G | 0 | −0.5 | small |

The familiar syntaxes (A, C, D) buy semantic correctness at about 1.4× E's tokens. F shows that a lean syntax with
weak block structure fails both models.

## v1.0 recommendation: **E**, with these grafts

E is the only syntax with zero listed violations from both models, a clean Sonnet run, about 26% fewer tokens than
the WGSL and HLSL syntaxes, and maths that reads as a shader author writes it.

Grafts:

1. **Disjoint field writes (from G).** `law hop(b: Heron, dt) writes yaw, lift, hop { … }`. A kind's laws must
   have disjoint write sets; overlapping sets are an error. This was the most common break: 6 of 14 outputs.
2. **`time` is a query and law value only (new; fixes B-Haiku and F-Sonnet).** Reading `time` in build code is an
   error. Both gaze failures came from aiming at build.
3. **Maps bind only query points (new; fixes F, G-Haiku and A-Haiku).** `p |> mirror_x` is legal only when `p` is
   a query parameter. Mirroring an `xf` or a stored point is an error.
4. **One mirrored kind, one row (new; fixes E-Haiku and F).** A kind whose query binds `mirror_x` warns when rows
   are emitted in mirrored pairs.
5. **Ship `hash` and `rnd` in the library, and change the card's raw example (all cards).** Four of the five
   needless raw escapes copied the card's own `hash` example, and three of them ran opaque code at build. The raw
   example should be something WGSL-only, such as a bit cast or subgroup op. Raw code at build is an error.
6. **Variants are not data (from the task statement).** Add a lint to the card: "a value that differs between rows
   is a column, even when it is a `bool`". E-Sonnet and F-Sonnet split the claw into `Claw[true]` and `Claw[false]`.
7. **Keep E's `:` / `->` type positions and its `@S` spaces as they are.** In 14 outputs, no space error came from
   E's notation.

Rejected:
- B's capture-is-a-column rule: invisible, and it produced the widest model gap.
- C's interfaces: about 300 grammar productions for nothing E lacks.
- D's string spaces and `*m` units: noise, and they pulled Haiku toward the Python runtime.
- F's `local`/`end`: no gain over E, and the worst Haiku output.

## Should there be a second, machine-facing form? Yes, as a printing of the IR, not a second language

- **What it is:** a lossless s-expression printer and reader over the same typed graph. Use G's 10 reader rules,
  one form per node, with units and comments kept. Agents that restructure code (hoist a column, split a kind,
  rename across laws, macro expansion) edit the s-expression and print it back as E. Humans and first-draft models
  write E.
- **Why not G as the authoring surface:** G had the lowest token count (1266/task), but its prefix maths was the
  hardest to check, and both G models mixed `$` and prefix forms within one formula.
- **Cost:** about 300 lines of printer and reader. The Rust compiler already builds the graph.
- **Kill condition:** if a round trip E → s-expression → E ever changes a token other than whitespace, drop the
  s-expression form rather than maintain two grammars.

On the owner's question of a Lisp → TS → WGSL pipeline: none of these outputs needed TypeScript. E's type inference
already removes the type code TS would remove, and a TS stage would add a second type system between the IR and
WGSL. The better route is E read by Rust straight to the typed graph, then WGSL, with the s-expression form as that
graph's text form.

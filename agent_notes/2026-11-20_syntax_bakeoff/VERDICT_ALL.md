# Loft v1.0 syntax bake-off: verdict on all five cards (A–E)

Judged fresh on 2026-11-19. `VERDICT.md` was not read. Cards F and G and their outputs are out of scope.
Tokens are counted with tiktoken `o200k_base`, run through uv. Each output's body is split at its `Task N` headings.

| Card | Surface |
|---|---|
| A | WGSL plus declarations (`kind`/`row`, `comptime for`, `map 'sym`, `law … writes`, `schedule { pass … }`) |
| B | WGSL plus modules and lambdas: `add(w, field(box, \|p\| …), mat, attachment)`, where captured values become row columns |
| C | Slang/HLSL: `[Kind] struct`, interfaces, `technique`/`pass` |
| D | Python surface: `@kind class`, `Pt["Leg"]`, `0.3*m` |
| E | Terse custom syntax: `kind K @S (row) { q(p) = expr }`, `[x,y,z]@S`, `f'(u)`, module code runs at build, 10 parse rules |

## How each output was scored

- **Violations** are breaches of the output's own card: a needless raw escape, a missing or mistyped space conversion
  through the mirror, unstaged build code, a divisor on row data that can reach zero (E211), two laws writing one row
  where the card forbids it, a helper used but never written, unit or space mismatches, and Python runtime use.
- **Not found anywhere:** continuous kind parameters (E401) and authored Lipschitz bounds. Every variant parameter
  was a `bool`.
- **Semantics** scores four checks, each 0, ½ or 1:
  - **Legs:** 12 legs are one kind with rows, and a second crab adds rows without new code.
  - **Reeds:** 200 reeds are one kind.
  - **Gaze:** each eye's world position is computed through the mirror.
  - **Heron:** the neck limit records the gaze as missed and hands off to a body turn.

## Per-output scores

| Output | T1 tok | T2 tok | T3 tok | Total | Violations | Semantics /4 |
|---|---|---|---|---|---|---|
| A sonnet | 2667 | 1097 | 2002 | 5778 | 3 | 3.5 |
| A haiku | 3074 | 1292 | 2561 | 6952 | 2 | 4 |
| B sonnet | 2781 | 1550 | 2576 | 6919 | 1 | 3.5 |
| B haiku | 2639 | 1342 | 2663 | 6671 | 3 | 2 |
| C sonnet | 2965 | 1314 | 2594 | 6885 | 4 | 3.5 |
| C haiku | 2755 | 1062 | 2210 | 6039 | 4 | 3.5 |
| D sonnet | 3252 | 1419 | 2758 | 7441 | 0 | 4 |
| D haiku | 2630 | 1018 | 2042 | 5702 | 2 | 3.5 |
| E sonnet | 1876 | 998 | 1777 | 4670 | 1 | 4 |
| E haiku | 2309 | 823 | 1603 | 4747 | 2 | 3 |

## Per-syntax tables

### A: WGSL superset (card 1695 tokens)

| Measure | Result |
|---|---|
| Tokens per task | 2122 on average (both models) |
| Violations | Sonnet 3: raw `hash1` that plain WGSL could write; `chitin(Pt[Leg])` called with `Pt[Claw]` and `Pt[Body]`; `/self.stiff` and `/self.height` unclamped. Haiku 2: module-level `map sym = mirror_x(q)` with `q` unbound; `/self.size` in the claw. |
| Semantics | Legs, reeds and gaze correct in both. In Sonnet's heron the head geometry never follows `neck_yaw`, and the gaze is measured from the body origin, not the eye. |
| Cold readability | 4. Anyone who knows WGSL reads it at once. `Pt[Leg](…)` on every literal and named `pass x:` labels are noise. |
| Haiku–Sonnet gap | About zero. Haiku is slightly better on semantics but uses 20% more tokens. The syntax is forgiving. |
| Parser | WGSL (~250 productions) plus ~15 declaration forms, ~60 extra rules. Ambiguities: `Pt[S](…)` is a type constructor in WGSL's index position; the `'sym` space lexeme; unit suffixes on numbers. |

### B: WGSL + lambdas/`add` (card 1695)

| Measure | Result |
|---|---|
| Tokens per task | 2265 on average, the highest |
| Violations | Sonnet 1: E211 divisors. Haiku 3: two laws return the `Heron` row, which the card forbids; `m_per_s_dt` and `leg_socket` are never written; the eye map is evaluated at comptime with `p` unbound and `time()` frozen, so there is one eye position and no mirror conversion. |
| Semantics | Sonnet 3.5: the legs are captured per `add`, and a second crab means copying the scene code. Haiku 2: the gaze fails; the legs use the sand material; the reed bounding `box` is too small for the sway. |
| Cold readability | 2. Whether a value is a row column or code depends on what a lambda captures. That is invisible on the page, and `s, w` are threaded through every schedule call. |
| Haiku–Sonnet gap | The largest: violations 3 vs 1, semantics 2 vs 3.5. The least forgiving syntax. |
| Parser | WGSL plus lambdas plus modules is moderate. The cost is in the middle end: capture analysis decides which values become row columns and which kinds merge (the same functions share a kind), and authored `box` bounds are required. |

### C: Slang/HLSL (card 1733)

| Measure | Result |
|---|---|
| Tokens per task | 2154 on average |
| Violations | Sonnet 4: `__wgsl` hash that plain code could write; `chitin(Pt<Body>)` space mismatch; `leg_socket`, `claw_socket` and `eye_socket` never written; E211. Haiku 4: `0.8m * size` gives m²; `Pt<Face> eye_c = sym.pt(…)` mislabels the mirrored space; `sin(0.3 * time)` on a `Sec`; E211. |
| Semantics | Sonnet's crab never draws the body, shell or eye (they are missing from `build_indirect`), and its heron head ignores the yaw. Haiku's heron parts all have identity poses. |
| Cold readability | 3. Familiar to shader writers, but heavy: designated initialisers, `Pt<sym.Space>`, attributes. |
| Haiku–Sonnet gap | About zero, but both models are bad: the most violations in total. |
| Parser | The largest grammar: C declarators, `<…>` generics (the `a < b > c` type-table ambiguity), attributes, interfaces, `technique`. It still has to lower to WGSL. |

### D: Python surface (card 1680)

| Measure | Result |
|---|---|
| Tokens per task | 2190 on average |
| Violations | Sonnet 0: it clamps every divisor (`max(self.stiff, 0.0)`, `max(self.height, 0.2*m)`). Haiku 2: build code at module level (`bed(200)`, `crab(...)`, a bare `emit(Water())`), which is unstaged; and `random.Random(42)`, Python runtime that the card forbids. |
| Semantics | Sonnet 4/4, the best single output. Haiku's reed placement uses the Python runtime. |
| Cold readability | 4. Python reads easily. `Pt["Leg"]` strings, `0.07*m` and `num()` unit stripping are noise. |
| Haiku–Sonnet gap | Moderate: violations 2 vs 0 and 23% fewer tokens. Haiku's mistakes are false friends from Python, and the surface invites host-language habits. |
| Parser | Parsing is cheap (Python's `ast`). Defining and checking the subset is expensive: reject lists, dicts, closures, recursion and module RNG; units arrive as multiplication, so unit inference must run over dataflow. Spaces are strings. |

### E: terse custom syntax (card 1527, the smallest)

| Measure | Result |
|---|---|
| Tokens per task | **1570** on average: −26% vs A, −31% vs B. Both models land within 2% of each other. |
| Violations | Sonnet 1: E211 `p.y / height`. Haiku 2: `fish: Fish` used where the card's table type is `[Fish]`; E211 `q.x / size`. |
| Semantics | Sonnet 4/4. Haiku 3: it emits two `Eye` rows *and* mirrors each one, giving four eyes, and the trunk always turns +yaw whichever side the fish is on. |
| Cold readability | 4. `kind CrabLeg @Leg (parent: CrabBody, …) { trace(p) = … }` reads like the spec. A stranger asks what type `[x,y,z]` has, and what `spine'` means. |
| Haiku–Sonnet gap | Small to moderate: one violation and one semantic point apart; tokens equal. |
| Parser | The smallest grammar (~80 productions) with 10 explicit disambiguation rules (newline handling, `>` only after `:`/`->`, `'` only for derivatives, `wgsl {` raw to its balanced `}`). The cost moves to bidirectional type inference for context-typed `[..]` literals, spaces and units. |

## Ranking by tokens × violations

Tokens are the total over both outputs. Violations are the total over both. A (v+1) column is shown so that zero does not
dominate.

| Rank | Card | Tokens | Violations | T×V | T×(V+1) | Semantics /8 |
|---|---|---|---|---|---|---|
| 1 | **E** | 9417 | 3 | 28.3k | **37.7k** | 7 |
| 2 | D | 13143 | 2 | 26.3k | 39.4k | 7.5 |
| 3 | B | 13590 | 4 | 54.4k | 67.9k | 5.5 |
| 4 | A | 12730 | 5 | 63.7k | 76.4k | 7.5 |
| 5 | C | 12924 | 8 | 103.4k | 116.3k | 7 |

D and E are statistically tied on T×V. E wins on tokens by a margin a resampling would not erase (−28%); D wins on
violations by one.

## Recommendation for v1.0: **E**, with these grafts

E is the only card that cuts the token bill substantially: about 26% below the WGSL superset (A), for both the output
and the card itself. It does this without raising violations or the Haiku–Sonnet gap. On this evidence WGSL is the
verbose option. Make it the lowering target and the `wgsl {}` escape, not the surface.

Grafts:

1. **A kind-scoped `map`** (from A's named `map sym = …`). Declare a map once per kind, and every query sees the same
   `sym`. This removes the mirror chain recomputed in `trace` and in `material`, seen in D sonnet, D haiku, E sonnet
   and E haiku. Add a lint: two rows that differ only by a mirrored socket are an error. That catches E haiku's four eyes.
2. **A typed error for a bare kind used as a table** (A/C/D all spell `Table[...]`). In a law or check parameter, `x: K`
   with no `[K]` is an error, unless it is the first, written row. This catches E haiku's `fish: Fish`.
3. **Field-level write ownership** (from A/C/D, "only it writes those fields"), replacing E's whole-row rule. The heron
   naturally splits into body fields and neck fields. Field ownership keeps the clean A haiku and D haiku layouts legal,
   and B haiku's whole-row breach shows the row rule is the one models break.
4. **E211 with a remedy built in** (from D sonnet's guarded divisors). Let a row column carry a range,
   `stiff: f in 0.2..1.4`, so the compiler can prove the divisor nonzero. A bare row divisor is then an error with the
   fix named. Unclamped row divisors were the most common violation (7 of 10 outputs).
5. **Typed `Verdict` units** (from B/C/D's `Verdict[Meters]`/`Verdict[Rad]`). Write `check gaze(h: Head) -> rad`, so
   that a held margin and a missed angle cannot mix units.
6. **Keep E's staging rule: module code runs at build.** Keep it over D's decorators and A's `comptime fn` plus
   `pass rows:`. No E output staged a loop wrongly. D haiku did, and both A outputs spent a line on it.
7. **Drop:** B's capture-to-column inference (the widest gap, invisible on the page), C's declarator grammar, and A's
   and C's named passes.

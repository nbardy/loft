# Loft v1.0 syntax (E+): the implementer's consolidation

This note accompanies the Rust parser swap. The scientist's spec and lead rulings in [README.md](README.md) govern the language. The normative grammar is [grammar/loft.lark](../../grammar/loft.lark). The Phase 1.5 measurements and compiler feedback are in [the experiment record](../../agent_notes/2026-11-20_meta/v1_syntax.md).

E+ combines the bakeoff's expression notation, explicit build stage and field-level ownership. The independently authored, executable bakeoff scenes live beside the experiment record.

## 1. Decisions

1. **One lean surface, parsed in Rust straight to the Layer 1 IR.** No WGSL superset, no TypeScript. WGSL is the
   target and the escape (`wgsl(...) -> T { ... }`).
2. **Infix maths with numeric promotion**, `[x, y, z]` vectors typed by their context, `@S` only where a point enters a
   space, unit suffixes (`0.5m`, `40deg`), `f'(u)` derivatives, `|>` pipes in maps.
3. **Explicit staging.** `build { ... }`, `build fn`, `build for`, and `emit K(...)` are the only build
   code. Bare module statements are E404; `time` at build is E403.
4. **Explicit bindings.** `let x = e` (immutable), `var x = e` (mutable), `x = e` updates a `var`, `a.f = e` only in a
   `law` on a field its `writes` names. A statement ends at a newline or `;`.
5. **`@` attaches a space and nothing else.** Library modifiers include `lobe`, `grid 96x32`,
   `on bake_lut`.
6. **Kinds are code; rows are data.** Columns may carry ranges (`k: m in 0.02m..0.4m`, `r: m > 0m`); continuous kind
   parameters are E401.
7. **Kind-scoped value maps**: `map sym(p) = p |> mirror_x |> move(...)` gives `sym.p`, `sym.pt(...)`, `sym.cell`,
   `sym.out(...)`, `sym.in(...)` in every query of the kind.
8. **Space-polymorphic helpers**: an unannotated point or normal parameter takes the caller's space.
9. **Field-level write ownership**: `law l(h: Heron, dt) writes h.{yaw, lift} { ... }`; two laws on one field is E502.
10. **No schedule boilerplate**: a scene without `schedule` runs the default realtime engine over every drawn kind.
    `Sun` and `Sky` are prelude kinds; `camera` is a declaration.
11. **Machine form.** `loft ir SCENE --sexp` serializes the executable program, its typed shader expression arenas, and source locations. `loft build FILE.sexp` reads and validates the IR. The lossless CST owns comments and whitespace.
12. **Constrained decoding**: `grammar/loft.lark` is the same grammar for llguidance, tested against every scene and
    the three tasks below.

## 2. Lexical rules

- **Comments**: `#` to end of line. The CST attaches comment spans to tokens and preserves them verbatim.
- **Identifiers**: `UIDENT` `[A-Z][A-Za-z0-9_]*` names spaces, kinds, cameras, schedules and `unknown` reasons;
  `LIDENT` `[a-z_][A-Za-z0-9_]*` names everything else.
- **Numbers**: `12`, `0.5`, `1e-6`, `0xff`. A number with a fraction or exponent is a float; otherwise an integer that
  takes its type from context (§4). A decimal point needs a digit after it, so `0..6` is a range.
- **Unit suffixes** touch the digits: `m`, `mm`, `rad`, `deg`, `s`, `kg`, `N`, optionally `/unit` (`0.42rad/s`).
  `deg` and `mm` convert to radians and metres; the Phase 1.5 compiler stores the resulting value as `f`. The v1 dimensional checking rules are in 06 §11. An
  unknown suffix (`0.5cm`) is E001. To divide by a variable named `s`, write a space: `2m / s`.
- **Grid sizes**: `96x32` after `grid` is one token.
- **Raw WGSL**: after `wgsl(params) -> T`, the `{ ... }` body is raw text to its balanced `}` (skipping `//`, `/* */`).
- **Newlines** end statements. Delimited argument and vector lists allow line breaks. A trailing operator or comma continues the expression. A leading pipe or `else` continues the preceding construct.
- **Motion shaders** use `wgsl det (...) -> T { ... }`. E504 identifies an unmarked raw shader reachable from a law.

## 3. Grammar

The checked-in grammar and its lexical preparation implement the addendum's two-stage input for constrained decoding. Run `uv run tools/check_loft_grammar.py` from the repository root. The corpus includes every library file, ported engine and standalone bakeoff scene.

## 4. Unambiguity rules

[01_LANGUAGE_V1.md §1](01_LANGUAGE_V1.md) defines the ten rules. The addendum supplies compound units and raw island handling. `waive` is a statement inside a body or schedule. E505 identifies schedule dependency errors.

## 5. Types and inference

| Construct | Written | Inferred |
|---|---|---|
| `kind` | `@S`, every column's type, a range on any column used as a divisor in `trace` | row layout, query signatures (`trace(p)` gets `p: p3@S`; `material(p, n)` gets `n3@S`; `surface(u, v)` gets `f`), kind maps, AABBs, L |
| `fn` | `pub fn` signatures; `wgsl(...)` boundary types | every unannotated parameter and the result, from the first call (spaces generalised) |
| `law`, `check` | the target `h: Kind`, tables `[Other]`, `writes h.{...}`, a check's unit `-> rad` | `dt: s`, locals |
| expressions | `@S` when a point enters a space; a unit suffix where the context does not give one | numeric types, a bracket literal's type and space, a block's tail value |

**Numbers.** An integer literal takes the type its context asks for (`f`, `u`, `i` or a unit); with no context it is an
integer. Where an integer value meets a float (`0.008m * j`), it is promoted with an explicit conversion in the IR.
Write a decimal point to force float division (`1 / 2` is integer `0`).

**Bracket literals.** `[x, y, z]` takes its type from its context: a parameter or column type, a `let` annotation, the
other operand (`p - [0, 0.6m, 0]` is a displacement in `p`'s space), a transform's source space (`pose * [0, 0, 0]`
is a point in the pose's source). With `@S` and no context it is `p3@S`; with neither it is `rgb` (`v2`, `v4` for two
or four components). A direction or normal literal is normalised.

**Affine spaces.** `p3@S - p3@S` is `v3@S`; `p3@S + v3@S` is `p3@S`; `p3 + p3` is E101; mixing spaces is E104.
`xf@A>B * p3@A` is `p3@B` (vectors, directions and normals by the rotation); `inv(xf@A>B)` is `xf@B>A`;
`xf@B>C * xf@A>B` is `xf@A>C`. A point passed where a displacement is expected is read as its offset from its space's
origin (`sphere(p, r)` is a sphere at the origin of `p`'s space). `coords(p)` drops the space (for noise).
`place(o)` and `place(o, yaw)` build a placement into `o`'s space whose source is fixed where it is stored or composed.

**Space-polymorphic helpers.** `fn chitin(p, n) = ...` is checked once, with a fresh space variable for each distinct
space of its first call; every call instantiates it. A body that mixes `p` with a named space fails at the helper.
Generic library fns write it: `fn capsule[S](p: p3@S, a: p3@S, b: p3@S, r: m) -> m`.

**Maps.** `map sym(p) = p |> mirror_x |> move([...])` in a kind binds a fresh space `sym` for every query that reads
it, applied to that query's point. A local `map m = q |> ...` takes a query point or another map's `.p`. Stages:
`mirror_x`, `move(v)`, `rot_x(a)`, `rot_y(a)`, `rot_z(a)`. `sym.p` is the mapped point, `sym.pt(x, y, z)` a point in
`sym`, `sym.cell` the mirror branch (-1 or 1), `sym.out(x)` takes a point or vector back to the parent space through
this branch, `sym.in(x)` takes it in. A map is not a value: bare `sym` is E106.

**Range-refined columns.** `stiff: f in 0.2..2.0`, `k: m > 0m`. Every `emit` checks the value against the range at
build (E602). In `trace` (and kind fns it calls), the compiler bounds every divisor by interval arithmetic over the
column ranges; a divisor whose interval contains 0 is E211.

**Staging.** Build blocks call build functions and pure helpers. It runs once on the CPU, emits rows
and may not read `time` or `dt` (E403). A query calling a build function produces E403. Only
`build for` loops in build code, and only build code emits (E404). In a query or `fn`, `for` is a GPU loop.

**Ownership.** In a law, `h.f = e` needs `f` in `writes h.{...}` (E501); a table parameter is read-only (E501). Two
laws on the same kind may not both write a field (E502). Law and check parameters after the first are tables `[K]`
(E108 for a bare `K`); a column typed `K` holds a row id, and `tbl[h.id]` reads that row.

**Checks.** `check name(h: K, ...) -> unit` returns `held(slack)`, `missed(by)` or `unknown(Reason)` on every path;
`finite(x)` tests a value. The unit is required (E003).

## 6. The standard prelude (`lib/`)

The CLI loads the checked-in [prelude](../../lib) before the scene. Its source gives the callable signatures and implementations for distance fields, analytic visibility, lobes, noise, frames, layout, light and sky. The Hermit port uses the same functions as the separately authored crab bakeoff.

**The default engine.** Without a `schedule`, the scene runs every `build` block in order, draws every kind with a
`trace`, `visible` or `surface` and `material` that has rows (in declaration order), and lights it with the one
`Sun` row and the one `Sky` row, seen from the one `camera`. Kinds placed by a pose cast sun shadows; kinds asked in
`World` receive them. A `schedule` block forks the renderer and lists the passes (`build_indirect`, `render_bundle`,
`shadow_map`, `tile_bin_kinds`, `dispatch_material_tiles`, `tonemap_aces`).

**Row-local laws and checks.** The compiler fuses scheduled scalar field writes into a GPU snapshot entry. The still command executes it at 120 Hz, reads the requested terminal verdicts and derives bounds for the sampled pose. Persistent playback, cross-table solving and checkpoints land in Phase 3.

## 7. Diagnostics

Compiler refusals carry an error code, an author span and a suggested fix. The [diagnostic regressions](../../crates/loft-passes/tests/diagnostics.rs) include a wrong mirror space (E104), a bare map (E106), an unproved row divisor (E211), overlapping law writes (E502), unmarked motion WGSL (E504) and out-of-range row data (E602). The schedule uses E505 for an out-of-order dependency.

## 8. Executable bakeoff tasks

- [Coiled shell, crusher and mirrored pupils](../../agent_notes/2026-11-20_meta/v1_syntax/crab.loft)
- [Two hundred reeds in wind](../../agent_notes/2026-11-20_meta/v1_syntax/reeds.loft)
- [Heron gaze lock, neck limit and hop](../../agent_notes/2026-11-20_meta/v1_syntax/heron.loft)

## 9. Phase boundaries

The executable probes exercise maps, the row-local law subset, deterministic time stepping and adjudication of held or missed checks. The full typed unit algebra, bodies, persistent motion state, solvers and the remaining sub-engines have separate implementation work. Literal units in this port convert to storage units before lowering. The source spans and token trivia allow later diagnostics and formatting to address the author's original text.

# Loft v1.0 syntax (E+): the implementer's consolidation

The surface the compiler in `crates/loft-parse` and `crates/loft-passes` accepts, as built for Phase 1.5. The source of
truth is `docs/spec/00`–`05` (the research scientist's bundle). This file states the exact grammar, the rules the
parser applies, what is inferred, every diagnostic with the fix it names, and the three bakeoff tasks written in the
final language. Where this file departs from `01_LANGUAGE_V1.md`, §9 lists the place and the reason.

Inherited: E's expression syntax (bakeoff card E), A's visible build staging, G's field ownership, the scientist's
grafts and refinements. Taken from Astra (`docs/handoff/2026-11-20_syntax/astra_syntax_v1.md`): `let`/`var` bindings;
the `pose` column abbreviation; `TAU` in the prelude; tail expressions as values with `if` needing `else`; comments
kept in the IR so a rewrite through the s-expression form keeps them.

## 1. Decisions

1. **One lean surface, parsed in Rust straight to the Layer 1 IR.** No WGSL superset, no TypeScript. WGSL is the
   target and the escape (`wgsl(...) -> T { ... }`).
2. **Infix maths with numeric promotion**, `[x, y, z]` vectors typed by their context, `@S` only where a point enters a
   space, unit suffixes (`0.5m`, `40deg`), `f'(u)` derivatives, `|>` pipes in maps.
3. **Explicit staging.** `build { ... }`, `build fn`, `build for`, `build const`, and `emit K(...)` are the only build
   code. Bare module statements are E404; `time` at build is E403.
4. **Explicit bindings.** `let x = e` (immutable), `var x = e` (mutable), `x = e` updates a `var`, `a.f = e` only in a
   `law` on a field its `writes` names. A statement ends at a newline or `;`.
5. **`@` attaches a space and nothing else.** Library modifiers are keywords: `lobe`, `lipschitz(...)`, `grid 96x32`,
   `on bake_lut`.
6. **Kinds are code; rows are data.** Columns may carry ranges (`k: m in 0.02m..0.4m`, `r: m > 0m`); continuous kind
   parameters are E401.
7. **Kind-scoped value maps**: `map sym(p) = p |> mirror_x |> move(...)` gives `sym.p`, `sym.pt(...)`, `sym.cell`,
   `sym.out(...)`, `sym.in(...)` in every query of the kind.
8. **Space-polymorphic helpers**: an unannotated point or normal parameter takes the caller's space.
9. **Field-level write ownership**: `law l(h: Heron, dt) writes h.{yaw, lift} { ... }`; two laws on one field is E502.
10. **No schedule boilerplate**: a scene without `schedule` runs the default realtime engine over every drawn kind.
    `Sun` and `Sky` are prelude kinds; `camera` is a declaration.
11. **Machine form**: `loft ir SCENE --sexp` prints the elaborated IR as a lossless s-expression (spans and comments
    included) and `loft ir FILE.sexp` reads it back. One human surface, one checker.
12. **Constrained decoding**: `grammar/loft.lark` is the same grammar for llguidance, tested against every scene and
    the three tasks below.

## 2. Lexical rules

- **Comments**: `#` to end of line. Comments are kept with their spans in the IR (`Module.comments`).
- **Identifiers**: `UIDENT` `[A-Z][A-Za-z0-9_]*` names spaces, kinds, cameras, schedules and `unknown` reasons;
  `LIDENT` `[a-z_][A-Za-z0-9_]*` names everything else.
- **Numbers**: `12`, `0.5`, `1e-6`, `0xff`. A number with a fraction or exponent is a float; otherwise an integer that
  takes its type from context (§4). A decimal point needs a digit after it, so `0..6` is a range.
- **Unit suffixes** touch the digits: `m`, `mm`, `rad`, `deg`, `s`, `kg`, `N`, optionally `/unit` (`0.42rad/s`).
  `deg` and `mm` convert to radians and metres; the others are tags (units are checked as `f` in v1.0, §9). An
  unknown suffix (`0.5cm`) is E001. To divide by a variable named `s`, write a space: `2m / s`.
- **Grid sizes**: `96x32` after `grid` is one token.
- **Raw WGSL**: after `wgsl(params) -> T`, the `{ ... }` body is raw text to its balanced `}` (skipping `//`, `/* */`).
- **Newlines** are statement separators, except that a newline is not a separator (a) after `(`, `[`, `{`, `,`, `=`,
  `|>` or a binary operator, and (b) before `)`, `]`, `}`, a leading `|>` or a leading `else`. These are token
  rules (the previous and the next token), never bracket depth, so `grammar/loft.lark` states them exactly.
- **A statement never starts with `-`** (the unary-minus trap: `let a = x` / `-y` would otherwise drop `-y` or make it
  the block's value). A line that starts with `-` is E001: continue the previous line with a trailing operator
  (`x -` / `y`) or write `0 - y` / `(-y)`.

## 3. Grammar (EBNF)

`SEP` is a newline or `;`. `{ x }` repeats, `[ x ]` is optional.

```ebnf
module      = { SEP } [ item { SEP { SEP } item } ] { SEP } ;
item        = "use" path [ "::" ( "*" | "{" lident_list "}" ) ]
            | [ "pub" ] decl ;
path        = LIDENT { "::" LIDENT } ;
decl        = "space" UIDENT { UIDENT }
            | "const" IDENT [ ":" type ] "=" expr
            | "camera" UIDENT "{" field_inits "}"
            | "fn" LIDENT [ gen ] "(" [ params ] ")" [ "->" type ] [ "lipschitz" "(" expr ")" ] body
            | "lobe" LIDENT "(" params ")" [ "->" type ] body
            | "scan" LIDENT LIDENT "in" expr ".." expr "by" INT "on" LIDENT
                "{" "init" "=" expr SEP "step" LIDENT "=" expr [ SEP ] "}"
            | "kind" UIDENT [ "[" params "]" ] "@" UIDENT "(" [ cols ] ")" "{" { SEP } [ kitem { SEP { SEP } kitem } ] { SEP } "}"
            | "law" LIDENT "(" params ")" "writes" LIDENT "." "{" lident_list "}" block
            | "check" LIDENT "(" params ")" "->" type block
            | "build" ( "const" IDENT [ ":" type ] "=" expr | "fn" LIDENT "(" [ params ] ")" body | block )
            | "schedule" UIDENT "{" { SEP } [ sstmt { SEP { SEP } sstmt } ] { SEP } "}" ;
gen         = "[" UIDENT { "," UIDENT } "]" ;
params      = param { "," param } [ "," ] ;
param       = LIDENT [ ":" type ] ;
cols        = col { "," col } [ "," ] ;
col         = LIDENT [ ":" type [ "in" expr ".." expr | ( ">" | ">=" ) expr ] ] [ "=" expr ] ;
kitem       = "map" LIDENT "(" LIDENT ")" "=" expr
            | QUERY "(" [ params ] ")" body
            | "surface" "(" params ")" "grid" GRID body
            | "fn" LIDENT "(" [ params ] ")" [ "->" type ] body ;
QUERY       = "trace" | "visible" | "material" | "light" | "sky" | "medium" ;
body        = "=" expr | block ;
sstmt       = expr [ block ] ;
field_inits = { SEP } [ LIDENT ":" expr { ( "," | SEP ) { SEP } LIDENT ":" expr } [ "," ] ] { SEP } ;

type        = PRIM [ "@" UIDENT ] | "xf" "@" UIDENT ">" UIDENT | "frame" "@" UIDENT
            | "[" UIDENT "]" | UIDENT ;
PRIM        = "p3" | "v3" | "d3" | "n3" | "v2" | "v4" | "rgb" | "f" | "u" | "i" | "bool"
            | "m" | "rad" | "s" | "kg" | "N" ;

block       = "{" { SEP } [ stmt { SEP { SEP } stmt } ] { SEP } "}" ;
stmt        = ( "let" | "var" ) LIDENT [ ":" type ] "=" expr
            | "map" LIDENT "=" expr
            | "build" "for" bind { "," bind } block
            | "for" LIDENT "in" expr ".." expr block
            | "return" expr
            | lhs "=" expr
            | expr ;
bind        = LIDENT "in" ( expr ".." expr | "[" expr { "," expr } [ "," ] "]" ) ;
lhs         = LIDENT { "." LIDENT | "[" expr "]" } ;

expr        = or { "|>" LIDENT [ "(" [ args ] ")" ] } ;
or          = and { "||" and } ;
and         = cmp { "&&" cmp } ;
cmp         = bitor [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) bitor ] ;
bitor       = bitxor { "|" bitxor } ;       (* integer bit ops, for hashes *)
bitxor      = bitand { "^" bitand } ;
bitand      = shift { "&" shift } ;
shift       = add { ( "<<" | ">>" ) add } ;
add         = mul { ( "+" | "-" ) mul } ;
mul         = unary { ( "*" | "/" | "%" ) unary } ;
unary       = ( "-" | "!" ) unary | postfix ;
postfix     = primary { "." LIDENT | "(" [ args ] ")" | "[" expr "]" | "'" } ;
primary     = NUMBER | NUM_UNIT | "true" | "false" | IDENT | "(" expr ")"
            | "[" [ expr { "," expr } [ "," ] ] "]" [ "@" UIDENT ]
            | "if" expr block { "else" "if" expr block } [ "else" block ]
            | "emit" UIDENT "(" [ LIDENT ":" expr { "," LIDENT ":" expr } [ "," ] ] ")"
            | "wgsl" "(" [ params ] ")" "->" type RAW ;
args        = arg { "," arg } [ "," ] ;
arg         = [ LIDENT ":" ] expr ;
```

Precedence, loosest first: `|>`, `||`, `&&`, one comparison, `|`, `^`, `&`, shifts, `+ -`, `* / %`, prefix, postfix.
Binary levels fold left. `a |> f(b)` is `f(a, b)`; a pipe stage written without parentheses (`|> mirror_x`) passes
nothing else.

Recognised and refused with E405 (they arrive with their phase): `body`, `state`, `solver`, `plan`, `retarget`,
`world`, `input`, `param`, `target` items; the `light`, `sky` and `medium` queries; `yield`.

## 4. The ten unambiguity rules

1. **Statements end at a newline or `;`.** Newlines are skipped by the token rules of §2 only; a statement never
   starts with `-`.
2. **Every module item starts with a keyword** (`use pub space const camera fn lobe scan kind law check build
   schedule`). Anything else at module level is E404.
3. **Types appear only after `:` or `->`.** There, `>` joins spaces (`xf@A>B`) and `[K]` is a read-only table.
4. **`@` attaches a space** (`kind K @S`, `p3@S`, `xf@A>B`, `[x, y, z]@S`) and nothing else.
5. **Digits touching letters are one number with a unit**; an unknown unit is E001.
6. **`'` is differentiation only**: `spine'(u)` is `diff(spine, u)(u)`, the derivative in the fn's first parameter.
7. **Case is namespace**: capitalised names are spaces, kinds, cameras and `unknown` reasons; lowercase names are
   bindings, columns, fns, laws and queries.
8. **Bindings are explicit**: `let`/`var` bind, `x = e` updates a `var`, `a.f = e` writes a row field in a law.
9. **`emit K(field: value, ...)` is the only row constructor**, legal only in build code; `K(...)` alone is E011.
10. **`wgsl(...) -> T { ... }` is raw** to its balanced `}`; naga validates it at emit.

## 5. Types and inference

| Construct | Written | Inferred |
|---|---|---|
| `kind` | `@S`, every column's type (`pose` alone means `pose: xf@S>World`), a range on any column used as a divisor in `trace` | row layout, query signatures (`trace(p)` gets `p: p3@S`; `material(p, n)` gets `n3@S`; `surface(u, v)` gets `f`), kind maps, AABBs, L |
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

**Staging.** Build code is `build { }`, `build fn`, `build const` and their callees. It runs once on the CPU, emits rows
and may not read `time` or `dt` (E403). Kind code may not read a `build const` or call a `build fn` (E403). Only
`build for` loops in build code, and only build code emits (E404). In a query or `fn`, `for` is a GPU loop.

**Ownership.** In a law, `h.f = e` needs `f` in `writes h.{...}` (E501); a table parameter is read-only (E501). Two
laws on the same kind may not both write a field (E502). Law and check parameters after the first are tables `[K]`
(E108 for a bare `K`); a column typed `K` holds a row id, and `tbl[h.id]` reads that row.

**Checks.** `check name(h: K, ...) -> unit` returns `held(slack)`, `missed(by)` or `unknown(Reason)` on every path;
`finite(x)` tests a value. The unit is required (E003).

**Mirror lint (W410).** Emitting a kind whose map has `mirror_x` twice in one build block, or inside a `build for`
over a two-element list, warns: the mirror already makes the pair.

## 6. The standard prelude (`lib/`)

Always in scope; `use std::...` lines name a library module and change nothing else.

```loft
space World
const TAU = 6.2831853
kind Sun @World (dir: v3@World, color: rgb)            # toward the sun; radiance; the engine normalises dir
kind Sky @World (zenith: rgb, horizon: rgb)            # the sky dome's two colours
camera Main { at: p3@World, radius: m, height: m, period: s, fov: rad }   # an orbiting camera

# sdf.loft: exact distances (L = 1 axioms, sampled by the truth gate)
fn sphere[S](v: v3@S, r: m) -> m
fn box[S](v: v3@S, half: v3) -> m
fn round_box[S](v: v3@S, half: v3, r: m) -> m
fn capsule[S](p: p3@S, a: p3@S, b: p3@S, r: m) -> m
fn smin(a: m, b: m, k: m) -> m                         # polynomial smooth union
fn ssub(a: m, b: m, k: m) -> m                         # smooth subtraction: a minus b
# analytic.loft: closed-form hits (t, or negative for a miss)
fn ray_sphere[S](ro: p3@S, rd: d3@S, c: p3@S, r: m) -> f
fn ray_capsule[S](ro: p3@S, rd: d3@S, a: p3@S, b: p3@S, r: m) -> f
fn hit_min(a: f, b: f) -> f
# bsdf.loft: lobes; a material is their sum, bottom first; the last three parameters come from the light
lobe ggx(albedo: rgb, rough: f, metal: f, n: v3, v: v3, l: v3) -> v4
lobe coat(weight: f, rough: f, n: v3, v: v3, l: v3) -> v4
lobe film(weight: f, ior: f, nm: f, n: v3, v: v3, l: v3) -> v4
lobe sss(color: rgb, radius: m, n: v3, v: v3, l: v3) -> v4
lobe emit_lobe(color: rgb, n: v3, v: v3, l: v3) -> v4
# noise.loft
fn fbm2(p: v3) -> f                                    # two octaves of value noise in [0, 1]
# frames.loft: for scans
fn frame[S](o: p3@S, t: d3@S, up: d3@S) -> frame@S    # fields .o .t .n .b
fn transport[S](prev: frame@S, o: p3@S, t: d3@S) -> frame@S
# layout.loft (build)
build fn grid_spot(i: u, n: u, spacing: f) -> v3
build fn palette(i: u) -> rgb
build fn rnd(i: u, k: u) -> f                          # deterministic hash in [0, 1)
```

Builtins: `sin cos exp sqrt abs floor fract sign min max clamp mix step smoothstep smootherstep pow atan2 length dist
dot cross dir inv coords place finite f32 u32 i32 select` and, in checks, `held missed unknown`.

**The default engine.** Without a `schedule`, the scene runs every `build` block in order, draws every kind with a
`trace`, `visible` or `surface` and `material` that has rows (in declaration order), and lights it with the one
`Sun` row and the one `Sky` row, seen from the one `camera`. Kinds placed by a pose cast sun shadows; kinds asked in
`World` receive them. A `schedule` block forks the renderer and lists the passes (`build_indirect`, `render_bundle`,
`shadow_map`, `tile_bin_kinds`, `dispatch_material_tiles`, `tonemap_aces`).

**Laws, checks and `time`** are checked in full by `loft check` and refused by the lowering with E405 until the
motion phase: Phase 1.5 draws static scenes.

## 7. Diagnostics

Every diagnostic quotes the author's line, marks the span, and ends with `= fix:`, one sentence.

| Code | Class | Example trigger | Fix it names |
|---|---|---|---|
| E001 | syntax | `let a = x` / `-y`; `0.5cm`; `else` alone on a line | the token expected, or join the line |
| E011 | unknown name | `capsul(p, ...)` | declare it, or the near name |
| E003 | type | `held(true)`; a check with no unit | the type expected |
| E101 | affine | `p + q` with two points | subtract an origin, or `mix` |
| E104 | space | `pose * eye_c` with `eye_c` in `sym`; `chitin(p: p3@Leg, ...)` called from a claw | `sym.out(eye_c)`; drop the annotation |
| E106 | map misuse | bare `sym`; `.cell` without a mirror; a map of a stored point | `sym.p`; map the query point |
| E108 | table vs row | `law l(h: Heron, dt, fish: Fish)` | `fishes: [Fish]` |
| E201 | degenerate bound | a field with no finite derived bound | keep its terms bounded |
| E210 | authored Lipschitz | `lipschitz(1.35)`; `trace(p) -> Field[S, 1.4]` | delete it; bounds are derived |
| E211 | unbounded field | `/ stiff` with `stiff: f` | `stiff: f in 0.2..2.0` |
| E212 | missing query | a drawn kind without `material` | add the query |
| E307 | `diff` of a kink | `spine'(u)` through `abs` | keep `diff` on smooth terms |
| E308 | degenerate frame | a scan whose tangent is zero | keep the tangent finite |
| E401 | continuous variant | `kind K[len: m]` | move it into the row |
| E402 | variant budget | more than 4 variants | move one into the row |
| E403 | stage leak | `time` in `build`; kind code reading a `build const` | pass it through a column |
| E404 | unstaged code | `for i in 0..200 { ... }` at module scope; `emit` in a query | wrap it in `build { build for ... }` |
| E405 | not lowered yet | a law in a drawn scene; `light` query | named refusal; the phase that lowers it |
| E501 | undeclared write | `h.yaw = ...` with `writes h.{lift}` | add `yaw` to `writes` |
| E502 | shared writer | two laws write `Heron.yaw` | one owner per field |
| E520 | schedule | a pass out of order; no `camera` | the pass order |
| E602 | row value | a missing column; `k: 0m` against `k: m in 0.02m..0.4m` | the range |
| E701 | evaluation | build code divides by zero | the value |
| W410 | mirror duplicate | `build for side in [-1, 1] { emit Eye(...) }` on a mirrored kind | emit one row |

## 8. The three bakeoff tasks in v1.0

(Written against the real prelude and checked by the compiler: `loft check` passes on all three. Tasks 2 and 3 use
`time` and laws, which Phase 1.5 checks but does not lower.)

### Task 1: hermit crab

```loft
space Body Leg Claw Face Shell

fn chitin(p, n) = ggx([0.82, 0.24, 0.08], 0.24 + 0.16 * fbm2(coords(p) * 80), 0) + sss([0.9, 0.3, 0.1], 0.04m) + coat(0.75, 0.06)

fn spine(u: f) = {
  let a = 3 * TAU * u
  let r = 0.018m * exp(1.6 * u)
  [r * cos(a), 0.14m * u, r * sin(a)]@Shell
}

scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0, 1, 0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}

kind Shell @Shell (pose) {
  surface(u, v) grid 96x32 {
    let f = whorl(u)
    let a = TAU * v
    let r = 0.011m * exp(1.6 * u) * (1 + 0.10 * cos(12 * a) + 0.05 * sin(36 * TAU * u))
    f.o + f.n * (r * cos(a)) + f.b * (r * sin(a))
  }
  material(p, n) = ggx([0.82, 0.68, 0.52], 0.42, 0) + coat(0.35, 0.18)
}

kind Claw @Claw (pose, open: rad, k: m in 0.02m..0.4m, teeth: m in 0m..0.02m) {
  trace(p) {
    let palm = round_box(p, [0.45 * k, 0.30 * k, 0.26 * k], 0.10 * k)
    let fixed = capsule(p, [0.35 * k, -0.08 * k, 0], [1.25 * k, -0.14 * k, 0], 0.11 * k)
    map jaw = p |> move([-0.40 * k, -0.12 * k, 0]) |> rot_z(-open)
    let serr = teeth * smootherstep(0.05 * k, 0.85 * k, jaw.p.x) * sin(7 * TAU * jaw.p.x / (0.9 * k))
    let moving = capsule(jaw.p, jaw.pt(0, 0, 0), jaw.pt(0.90 * k, 0.06 * k, 0), 0.10 * k) - serr
    smin(smin(palm, fixed, 0.05 * k), moving, 0.03 * k)
  }
  material(p, n) = chitin(p, n)
}

kind CrabBody @Body (pose) {
  trace(p) = round_box(p, [0.14m, 0.055m, 0.11m], 0.035m)
  material(p, n) = chitin(p, n)
}

kind CrabLeg @Leg (parent: CrabBody, pose, femur: m in 0.02m..1m, tibia: m in 0.02m..1m) {
  trace(p) {
    let knee = [femur, 0.03m, 0]@Leg
    let tip = [femur + 0.55 * tibia, -0.82 * tibia, 0]@Leg
    smin(capsule(p, [0, 0, 0], knee, 0.016m), capsule(p, knee, tip, 0.011m), 0.01m)
  }
  material(p, n) = chitin(p, n)
}

kind Eye @Face (pose, gaze: p3@World) {
  map sym(p) = p |> mirror_x |> move([-0.045m, -0.04m, -0.10m])
  trace(p) {
    let c = sym.pt(0.01m, 0.075m, 0.02m)
    smin(capsule(sym.p, sym.pt(0, 0, 0), c, 0.011m), sphere(sym.p - c, 0.022m), 0.008m)
  }
  material(p, n) {
    let c = sym.pt(0.01m, 0.075m, 0.02m)
    let eye_w = pose * sym.out(c)
    let look = sym.in(inv(pose) * dir(gaze - eye_w))
    let globe = 1 - smoothstep(0.022m, 0.024m, dist(sym.p, c))
    let pupil = smoothstep(0.80, 0.86, dot(dir(sym.p - c), look)) * globe
    ggx(mix([0.82, 0.24, 0.08], mix([0.86, 0.72, 0.25], [0.02, 0.02, 0.02], pupil), globe), 0.2, 0) + coat(0.9, 0.03)
  }
}

law drift_gaze(e: Eye, dt) writes e.{gaze} {
  let a = 0.25rad/s * time
  e.gaze = [0.45m * sin(a), 0.14m + 0.04m * cos(1.6 * a), 0.75m + 0.20m * cos(a)]@World
}

kind Sand @World () {
  trace(p) = p.y
  material(p, n) {
    let g = fbm2(coords(p) * 24)
    ggx([0.34, 0.28, 0.20] * (0.85 + 0.3 * g), 0.22 + 0.18 * g, 0) + coat(0.55, 0.06)
  }
}

build fn crab(origin: p3@World, yaw: rad) {
  let body = place(origin, yaw)
  let b = emit CrabBody(pose: body)
  build for j in 0..6, side in [-1, 1] {
    emit CrabLeg(parent: b, pose: body * place([side * 0.11m, 0, -0.08m + 0.032m * j]@Body, side * (1.2rad - 0.12rad * j)),
                 femur: 0.075m + 0.008m * j, tibia: 0.095m + 0.007m * j)
  }
  emit Shell(pose: body * place([0, 0.055m, -0.06m]@Body))
  emit Claw(pose: body * place([0.12m, 0.02m, 0.13m]@Body, -0.35rad), open: 0.50rad, k: 0.11m, teeth: 0.004m)
  emit Claw(pose: body * place([-0.12m, 0.015m, 0.12m]@Body, 0.45rad), open: 0.22rad, k: 0.055m, teeth: 0m)
  emit Eye(pose: body * place([0, 0.04m, 0.05m]@Body), gaze: [0, 0.14m, 0.75m]@World)
}

build {
  emit Sand()
  emit Sun(dir: [-0.4, 0.85, 0.35]@World, color: [2.8, 2.6, 2.3])
  emit Sky(zenith: [0.35, 0.56, 0.88], horizon: [0.82, 0.86, 0.90])
  crab([0, 0.08m, 0]@World, 0rad)
}

camera Main { at: [0, 0.10m, 0]@World, radius: 0.9m, height: 0.4m, period: 30s, fov: 38deg }
```

### Task 2: reed bed in wind

```loft
space Reed

const PAPER = [0.94, 0.89, 0.78]
const INK = [0.13, 0.11, 0.15]
const FAINT = [0.76, 0.79, 0.80]
const WIND = [0.85, 0, 0.52]@World

lobe ink(paper: rgb, dark: rgb, cut: f, n: v3, v: v3, l: v3) -> v4 = if dot(n, l) < cut { [dark.x, dark.y, dark.z, 0] } else { [paper.x, paper.y, paper.z, 0] }

kind Reed @Reed (pose, height: m in 0.4m..2.5m, lean: rad, stiff: f in 0.2..2.0, phase: rad) {
  trace(p) {
    let root = pose * [0, 0, 0]
    let h = clamp(p.y / height, 0, 1)
    let gust = 0.5 + 0.5 * sin(1.2rad/m * root.x - 1.9rad/s * time)
    let sway = (0.34rad * gust + 0.07rad * sin(2.1rad/s * time + phase)) / stiff
    let bend = [sin(lean) * p.y, 0, 0] + (inv(pose) * WIND) * (height * sway * h * h)
    let q = p - bend
    let stalk = capsule(q, [0, 0, 0], [0, 0.88 * height, 0], 0.011m * (1 - 0.55 * h))
    let head = capsule(q, [0, 0.86 * height, 0], [0, height, 0], 0.022m)
    min(stalk, head)
  }
  material(p, n) = ink(PAPER, INK, 0.10)
}

kind Water @World () {
  trace(p) = p.y
  material(p, n) {
    let ripple = fract(p.z * 8 + 0.35 * sin(p.x * 2))
    emit_lobe(mix(PAPER, FAINT, step(ripple, 0.08)))
  }
}

build {
  emit Water()
  emit Sun(dir: [-0.5, 0.8, 0.3]@World, color: [1, 1, 1])
  emit Sky(zenith: PAPER, horizon: FAINT)
  build for i in 0..200 {
    let a = TAU * rnd(i, 1)
    let r = 1.6m * sqrt(rnd(i, 2)) * (0.65 + 0.6 * rnd(i, 3))
    emit Reed(
      pose: place([r * cos(a) + 0.2m * (rnd(i, 4) - 0.5), 0, r * sin(a) + 0.2m * (rnd(i, 5) - 0.5)]@World),
      height: 0.9m + 0.7m * rnd(i, 6),
      lean: 0.26rad * (rnd(i, 7) - 0.5),
      stiff: 0.35 + 1.25 * rnd(i, 8),
      phase: TAU * rnd(i, 9),
    )
  }
}

camera Main { at: [0, 0.65m, 0]@World, radius: 4.2m, height: 1.4m, period: 60s, fov: 34deg }
```

### Task 3: heron's hop

```loft
space Body

const HOP = 0.45s
const NECK = [0, 0.62m, 0.12m]@Body

fn wrap_pi(a: rad) -> rad = a - TAU * floor((a + 0.5 * TAU) / TAU)
fn bearing(a: p3@World, b: p3@World) -> rad = atan2(b.x - a.x, b.z - a.z)
fn fish_at(t: s) -> p3@World = [1.7m * sin(0.42rad/s * t), 0.04m, 1.3m + 0.7m * cos(0.42rad/s * t)]@World

kind Fish @World (clock: s, pos: p3@World) {
  trace(p) = sphere(p - pos, 0.05m)
  material(p, n) = ggx([0.75, 0.78, 0.82], 0.2, 0.7) + coat(0.6, 0.05)
}

kind Heron @Body (pose, anchor: p3@World, yaw: rad, hop_t: s, lift: m, neck_yaw: rad, pitch: rad,
                  limit: rad in 0.2rad..2.5rad, leftover: rad, eye_w: p3@World, hips_w: p3@World, target: Fish) {
  trace(p) {
    let trunk = capsule(p, [0, 0.54m, -0.16m], [0, 0.62m, 0.14m], 0.10m)
    map leg = p |> mirror_x
    let legs = capsule(leg.p, leg.pt(0.055m, 0.52m, -0.02m), leg.pt(0.055m, 0, 0.02m), 0.013m)
    map nq = p |> move(-coords(NECK)) |> rot_y(-neck_yaw)
    let n1 = nq.pt(0, 0.16m, 0.09m)
    let n2 = nq.pt(0, 0.30m, -0.03m)
    let n3 = nq.pt(0, 0.44m, 0.06m)
    let neck = smin(capsule(nq.p, nq.pt(0, 0, 0), n1, 0.024m), smin(capsule(nq.p, n1, n2, 0.021m), capsule(nq.p, n2, n3, 0.019m), 0.02m), 0.02m)
    map hq = nq.p |> move(-coords(n3)) |> rot_x(pitch)
    let head = smin(sphere(hq.p, 0.042m), capsule(hq.p, hq.pt(0, 0, 0.03m), hq.pt(0, 0, 0.24m), 0.012m), 0.01m)
    min(smin(trunk, legs, 0.02m), smin(neck, head, 0.015m))
  }
  material(p, n) = ggx([0.56, 0.60, 0.64], 0.48, 0)
}

kind Water @World () {
  trace(p) = p.y
  material(p, n) = ggx([0.10, 0.22, 0.26], 0.10, 0) + coat(0.8, 0.03)
}

law fish_swim(f: Fish, dt) writes f.{clock, pos} {
  f.clock = f.clock + dt
  f.pos = fish_at(f.clock)
}

law heron_body(h: Heron, dt, fishes: [Fish]) writes h.{pose, yaw, hop_t, lift} {
  let f = fishes[h.target]
  let want = wrap_pi(bearing(h.anchor, f.pos) - h.yaw)
  if h.hop_t < 0s && h.leftover > 0rad { h.hop_t = 0s }
  if h.hop_t >= 0s {
    let u = clamp(h.hop_t / HOP, 0, 1)
    h.lift = 0.18m * 4 * u * (1 - u)
    let step = 2.6rad/s * dt
    h.yaw = h.yaw + clamp(want, -step, step)
    h.hop_t = h.hop_t + dt
    if h.hop_t >= HOP { h.hop_t = -1s; h.lift = 0m }
  }
  h.pose = place(h.anchor + [0, h.lift, 0], h.yaw)
}

law heron_neck(h: Heron, dt, fishes: [Fish]) writes h.{neck_yaw, pitch, leftover, eye_w, hips_w} {
  let f = fishes[h.target]
  let base = h.pose * NECK
  let want = wrap_pi(bearing(base, f.pos) - h.yaw)
  h.neck_yaw = clamp(want, -h.limit, h.limit)
  h.leftover = abs(want - h.neck_yaw)
  let horiz = max(dist(base, [f.pos.x, base.y, f.pos.z]@World), 0.05m)
  h.pitch = clamp(atan2(f.pos.y - base.y, horiz), -0.8rad, 0.5rad)
  h.eye_w = h.pose * (NECK + [0.06m * sin(h.neck_yaw), 0.44m, 0.06m * cos(h.neck_yaw)])
  h.hips_w = h.pose * [0, 0.54m, 0]
}

check gaze_lock(h: Heron) -> rad {
  if !finite(h.neck_yaw) { return unknown(NonFinitePose) }
  if h.leftover > 0rad { missed(h.leftover) } else { held(h.limit - abs(h.neck_yaw)) }
}

check head_above_hips(h: Heron) -> m {
  let margin = h.eye_w.y - h.hips_w.y
  if margin < 0m { missed(0 - margin) } else { held(margin) }
}

build {
  emit Water()
  emit Sun(dir: [-0.4, 0.85, 0.3]@World, color: [2.8, 2.6, 2.3])
  emit Sky(zenith: [0.38, 0.58, 0.88], horizon: [0.84, 0.88, 0.92])
  let f = emit Fish(clock: 0s, pos: fish_at(0s))
  emit Heron(pose: place([0, 0, 0]@World), anchor: [0, 0, 0]@World, yaw: 0rad, hop_t: -1s, lift: 0m,
             neck_yaw: 0rad, pitch: 0rad, limit: 1.0rad, leftover: 0rad,
             eye_w: [0, 1.06m, 0.18m]@World, hips_w: [0, 0.54m, 0]@World, target: f)
}

camera Main { at: [0, 0.6m, 0.4m]@World, radius: 3.6m, height: 1.2m, period: 40s, fov: 38deg }
```

## 9. Where this departs from `docs/spec/01_LANGUAGE_V1.md`, and why

| Place | 01 says | Built | Reason |
|---|---|---|---|
| Newline rule | suppressed inside `(...)`/`[...]` by bracket depth | suppressed after `( [ { , =`, `|>` and binary operators, and before `) ] }`, leading `|>`, leading `else` | the same rule as tokens only, so `loft.lark` is exactly the parser's language (the owner's refinement: no context-sensitive continuation) |
| Unary minus | not addressed | a statement never starts with `-` (E001) | closes the trap the owner named |
| Units | first-class, E110 | suffixes convert (`deg`, `mm`) and tag; all units check as `f` | dimensional inference is not in the Phase 1.5 slice (`05`); E110 is unused until it lands |
| `use` | module imports | the prelude is always in scope; `use std::x` must name a library file | one namespace until there are packages |
| `attach@S`, `target@S`, E105 | typed anchors | not built | Phase 3 (motion) |
| `light`, `sky`, `medium` queries, `body`, `state`, `solver`, `plan`, `world`, `retarget`, `yield` | grammar | E405 | their phases |
| E601 | missing retarget chain | E602 is a bad row value | E601 stays free for retargeting |
| E212 | missing query | missing query (Phase 1's "no finite bound" is now E201) | as 01 |
| Library axioms | none written | `fn sphere(...) lipschitz(v) = ...` in `lib/` only; in a scene, E210 | the exact-SDF table of 01 §2.3 has to live somewhere; it lives in the library, sampled by the truth gate |
| Shadow casters | every drawn kind | kinds placed by a pose cast, `World` kinds receive | keeps the Phase 0/1 pictures (no caster pass in blobs/capsules, no ground in the hermit's sun fit) |
| `camera` | `camera MainCam { field_inits }` | one orbit camera: `at`, `radius`, `height`, `period`, `fov` | the runner's only camera |
| Compound assignment | none | none (`c = c + x`) | as 01 |
| Conversions | not addressed | `f32()`, `u32()`, `i32()` remain for bit-level library code (hashes) | integer hashes need them |

# The research scientist's answer on Loft v1.0 syntax (20 Nov 2026), verbatim as pasted by the owner

- part 1 - 


<pasted_content id="ac93">
Yes: you do **not** need WGSL surface syntax. `wgpu` accepts `naga::Module` natively (and `naga` emits WGSL for browser WebGPU), so the surface can be completely decoupled from WGSL. Keep raw WGSL interop inside explicit `wgsl(...) -> T { ... }` blocks, and port the existing `282`-line (**measured**) Phase 1 `lib/` cleanly to v1.0—that takes minutes and buys a permanent ~35% token reduction.

---

# Loft v1.0 Surface Syntax

## 1. Decisions

1. **Drop strict WGSL superset; adopt E+ (terse expression surface) with `wgsl { ... }` escape blocks.** A's WGSL ceremony cost `6,320–6,365` avg tokens vs E's `4,709` (**measured**, −26%) with zero semantic gain once E gets structural guards; rules out pasting arbitrary external `.wgsl` files at module scope.
2. **Choose E+ over A (WGSL), D (Python), F (Lua), and G (Lisp).** E beat A–D on tokens and tied D on violations (`3` vs `2` **measured**); D had the worst Haiku gap (`+5` violations **measured**, leaking Python scripts/`random.Random`); F's untagged `fn`/`end` caused Haiku to write orphan queries and Sonnet to leak `time` into build functions; G's `($ ...)` dual-mode maths failed in practice (Haiku reverted to nested prefix math, Sonnet interleaved `($ ... (+ ...))`). Rules out Python decorators, Lua `end`-blocks, and prefix shader maths.
3. **Keep native infix maths with numeric promotion, `[x, y, z]` context-typed vectors, `|>` pipes, and `f'(u)` derivatives.** Eliminates `vec3<f32>`, `0.0` vs `0`, `f32(...)`, and prefix nesting; rules out two-syntax math escapes (`$`).
4. **Make staging unmistakable with `build` (`build fn`, `build for`, `build { ... }`) and `emit`.** Preserves A's `0`-gap staging safety (**measured** `comptime for` = 0 unstaged loops on Haiku) at 1 token (`build`) while forbidding bare module-level execution (`E404`) and struct-literal row confusion (A-Haiku's `SunSky { ... }`); rules out implicit module-level scripting and untagged build functions.
5. **Enforce disjoint field-level write ownership (`writes h.{a, b}`) on every `law`.** Directly eliminates the #1 cross-cutting failure (`5/8` Round-1 outputs had two laws writing the same kind **measured**); rules out whole-row exclusive locks that force artificial kind splitting.
6. **Make row columns support range refinements (`stiff: f in 0.2..2.0`, `k: m > 0m`).** Directly eliminates the #2 failure (`7/10` A–E outputs triggered E211 on unclamped row divisors **measured**) without cluttering formulas with `max(...)`; rules out silent division by unconstrained row fields.
7. **Make domain maps kind-scoped values (`map sym(p) = ...` with `sym.p`, `sym.pt(...)`, `sym.cell`, `sym.out(...)`, `sym.in(...)`).** Eliminates `'sym` tick-space syntax, prevents unbound file-scope maps (A-Haiku **measured**), and stops models from duplicating mirror chains across `trace` and `material` (D/E **measured**); rules out lifetime-tick space syntax.
8. **Make helper functions automatically space-polymorphic over unannotated point parameters (`fn chitin(p, n)`).** Fixes A-Sonnet and C-Sonnet's shared-material E104 bugs (**measured**); rules out monomorphic-only helpers.
9. **Parse `.loft` directly in Rust straight to Layer 1 IR—no TypeScript transpile chain.** Compiling Lisp/Lua $\to$ TS $\to$ Rust re-introduces host tracing (`707k` nodes **measured**), breaks source spans, forces two type systems to agree, and blows the `2 s` training CPU cap (**measured**); rules out any JS/TS middle layer.
10. **Ship a canonical S-expression IR dump/loader (`loft ir --sexp`), not a second human surface.** Gives RSI agents Lisp's code-as-data uniformity for AST/IR rewrites via a ~250-LOC lossless codec on Layer 1 IR; rules out maintaining two front-end type checkers.
11. **Type `check` return units explicitly (`check gaze(h: Heron) -> rad`) and require `[Kind]` for read-only table params.** Prevents unit-mixed verdicts and catches E-Haiku's bare `fish: Fish` parameter error (`E108`); rules out untyped `Verdict` and bare-kind table shorthand.
12. **Drop `pass x:` labels in `schedule` and standardize `Sun`/`Sky` as emitted kinds.** Removes A's pass-label noise and stops models from inventing `sun_sky`/`sky_color` globals (observed across all cards **measured**).

---

## 2. Grammar (EBNF) & The 10 Unambiguity Rules

```ebnf
module      = { import_decl | top_item } ;
import_decl = "use" path [ "::" ( "*" | "{" ident_list "}" ) ] [ "as" ident ] ;
path        = ident { "::" ident } ;

top_item    = [ "pub" ] ( space_decl | const_decl | fn_decl | lobe_decl
                        | scan_decl  | kind_decl  | law_decl | check_decl
                        | build_fn   | build_blk  | sched_decl ) ;

space_decl  = "space" ident { ident } ;
const_decl  = "const" ident [ ":" ty ] "=" expr ;
fn_decl     = "fn" ident [ gen_params ] "(" [ params ] ")" [ "->" ty ] fn_body ;
lobe_decl   = "lobe" ident "(" [ params ] ")" fn_body ;
fn_body     = "=" expr | block ;

scan_decl   = "scan" ident ident "in" expr ".." expr "by" uint "on" ident
              "{" "init" "=" expr  "step" ident "=" expr "}" ;

kind_decl   = "kind" ident [ var_params ] "@" ident "(" [ col_list ] ")" "{" { kind_item } "}" ;
var_params  = "[" param { "," param } "]" ;                       (* discrete only: bool | enum | u32 in a..b *)
col_list    = col { "," col } [ "," ] ;
col         = ident ":" ty [ range_ref ] [ "=" expr ] ;
range_ref   = "in" expr ".." expr | ( ">" | ">=" ) expr ;
kind_item   = map_decl | query_decl | fn_decl ;
map_decl    = "map" ident "(" ident ")" "=" expr ;
query_decl  = ( "trace" | "visible" | "material" | "light" | "sky" | "medium" )
              "(" [ params ] ")" fn_body
            | "surface" "(" [ params ] ")" "grid" uint "x" uint fn_body ;

law_decl    = "law" ident "(" param { "," param } ")" "writes" write_set block ;
write_set   = ident [ "." "{" ident_list "}" ] ;
check_decl  = "check" ident "(" param { "," param } ")" "->" ty block ;

build_fn    = "build" "fn" ident "(" [ params ] ")" block ;
build_blk   = "build" block ;
sched_decl  = "schedule" ident "{" { sched_stmt } "}" ;
sched_stmt  = expr [ block ] ;

gen_params  = "[" ident { "," ident } "]" ;
params      = param { "," param } [ "," ] ;
param       = ident [ ":" ty ] ;
ident_list  = ident { "," ident } [ "," ] ;

(* Types: only appear after ':' or '->' *)
ty          = prim_ty [ "@" space_ref ]
            | "xf" "@" space_ref ">" space_ref
            | "[" ident "]"                                        (* read-only Table of Kind *)
            | ident ;                                              (* RowId of Kind, or Bsdf / Frame *)
prim_ty     = "p3" | "v3" | "d3" | "n3" | "v2" | "v4" | "rgb"
            | "f" | "u" | "i" | "bool" | "m" | "rad" | "s" | "kg" | "N" ;
space_ref   = ident ;

(* Statements & Expressions *)
block       = "{" { stmt } "}" ;
stmt        = "map" ident "=" expr
            | "build" "for" for_binds block
            | "for" ident "in" expr ".." expr block
            | "emit" ident [ "[" expr_list "]" ] "(" [ named_args ] ")"
            | "return" expr
            | lhs "=" expr
            | expr ;
for_binds   = for_bind { "," for_bind } ;
for_bind    = ident "in" ( expr ".." expr | "[" expr_list "]" ) ;
lhs         = ident { "." ident } ;

expr        = pipe_expr ;
pipe_expr   = logic_expr { "|>" call_or_ident } ;
logic_expr  = cmp_expr { ( "&&" | "||" ) cmp_expr } ;
cmp_expr    = add_expr [ ( "==" | "!=" | "<" | "<=" | ">" | ">=" ) add_expr ] ;
add_expr    = mul_expr { ( "+" | "-" ) mul_expr } ;
mul_expr    = unary_expr { ( "*" | "/" | "%" ) unary_expr } ;
unary_expr  = ( "-" | "!" ) unary_expr | postfix_expr ;
postfix_expr = primary { "." ident | "(" [ call_args ] ")" | "[" expr "]" | "'" } ;

primary     = number [ unit_suffix ] | "true" | "false" | ident
            | "(" expr ")"
            | "[" [ expr_list ] "]" [ "@" ident ]
            | "if" expr block { "else" "if" expr block } [ "else" block ]
            | "wgsl" "(" [ params ] ")" "->" ty raw_balanced_block
            | "emit" ident [ "[" expr_list "]" ] "(" [ named_args ] ")" ;

call_args   = arg { "," arg } [ "," ] ;
arg         = [ ident ":" ] expr ;
named_args  = ident ":" expr { "," ident ":" expr } [ "," ] ;
expr_list   = expr { "," expr } [ "," ] ;

```

### The 10 Unambiguity Rules (For a Small LL(1) + Pratt Parser)

1. **Newline terminates statements** unless the line ends with a binary operator, `,`, `(`, `[`, `=`, or `|>`, or the next non-blank line begins with `|>` or `else`.
2. **Every top-level item opens with a keyword** (`use space const fn lobe scan kind law check build schedule pub`). Bare top-level statements are a hard syntax/staging error (`E404`).
3. **Type syntax appears only after `:` or `->`.** Inside type position, `>` exclusively joins spaces in `xf@A>B` (never comparison), and `[K]` is exclusively a table type (never a vector literal).
4. **`@` exclusively attaches a space** (`kind K @S`, `p3@S`, `xf@A>B`, `[x,y,z]@S`). There are no `@attributes`.
5. **Digits immediately touching letters lex as `number + unit_suffix**` (`0.5m`, `40deg`, `1s`, `1.2rad`). An unknown suffix is a lex error; `0.5m` never splits into `0.5` and `m`.
6. **`'` is exclusively postfix differentiation** (`spine'(u)` $\equiv$ `diff(spine, u)(u)`). There are no tick-spaces (`'sym`) and no char literals.
7. **Case encodes namespace:** Capitalized identifiers are `Space`, `Kind`, or `Unknown` reason tags; lowercase identifiers are bindings, fields, and functions.
8. **`emit` is the sole row constructor** (`emit Kind(field: val)`), legal only inside `build` blocks and `build fn`. `Kind(...)` without `emit` is illegal everywhere (preventing struct-literal row mistakes).
9. **Assignment (`x = e`) binds or updates locals; field assignment (`a.f = e`) is legal only inside a `law**` and only on fields listed in its `writes` clause.
10. **`wgsl(...) -> T { ... }` lexes raw text to its matching balanced `}**` (ignoring comments/strings inside) with zero WGSL parsing in the Loft front end; `naga` validates the emitted body.

---

## 3. Typing and Inference Rules

### What Is Written vs. What Is Inferred

| Construct | Must Be Written | Inferred Automatically |
| --- | --- | --- |
| **`kind` header** | `@Space`, row column types (plus `in lo..hi` / `> 0` if divided), discrete `[var: T]` params | Row `std430` layout, kind-scoped `map` space, `Field[S]` return types, Lipschitz $L$, smoothness $C^k$, AABBs |
| **`fn` / `lobe**` | `pub fn` param & return types; `wgsl(...) -> T` boundary types | All private `fn` param & return types; **space polymorphism** (`fn chitin(p, n)` generalizes `S` automatically); unit polymorphism where linear |
| **`law`** | Target `row: Kind`, read tables `tbl: [Other]`, `writes row.{fields}` | `dt: s`, all local bindings, dependency wave, `(wave, law)` dispatch bucket |
| **`check`** | Target `row: Kind`, read tables `[Other]`, return unit `-> rad` / `-> m` | `held`/`missed`/`unknown` `Verdict[U]` constructor checking |
| **Expressions** | `@S` only when creating a point in a new space; unit suffix only when ambiguous (`0.5m`, `0.3rad`) | Numeric promotion (`0`, `1`, `2` match `f`/`m`/`rad`/`s` from context); `[x,y,z]` vector space & affine kind (`p3`/`v3`/`d3`/`rgb`); implicit block return (last expression) |

### Core Inference & Checking Rules

* **Affine spaces:** `p3@S - p3@S -> v3@S`; `p3@S + v3@S -> p3@S`; `p3@S + p3@S -> E101`; mixing `S1 != S2 -> E104`. `xf@A>B * p3@A -> p3@B`; `xf@A>B * n3@A -> n3@B` (auto inverse-transpose). `inv(xf@A>B) -> xf@B>A`.
* **Space-polymorphic helpers:** Any `fn f(p, n)` with unannotated `p` receives a fresh space variable `α`. If `f` only performs space-invariant ops (`coords(p)` for noise, `dot(n, l)`, `length(p - q)` in `α`), `f` generalizes to `∀S. (p3@S, n3@S) -> Bsdf`. Mixing `p` with a concrete space (`p - [0,0,0]@World`) raises `E104` at `f`'s definition.
* **Value-typed domain maps:** `map sym(p) = chain` (in a `kind`) or `map sym = p |> chain` (local) binds a fresh existential space `S_sym` on value `sym`:
* `sym.p : p3@S_sym`, `sym.pt(x, y, z) : p3@S_sym`, `sym.cell : f` (mirror sign / sector / grid id),
* `sym.out : Map[S_sym -> S]` (auto-inverted via `sym.cell`; absent on non-invertible warps $\to$ `E106`),
* `sym.in : Map[S -> S_sym]` (maps points affinely, directions linearly, normals by inverse-transpose).


* **Range-refined intervals & derived Lipschitz:** Column refinements (`height: m in 0.4m..2.5m`, `stiff: f in 0.2..2.0`) seed interval arithmetic directly. `emit` verifies build-time values against column ranges (static error if out of range). Any divisor whose interval contains `0` produces `L = Unbounded` $\to$ `E211`. Authoring a numeric Lipschitz bound anywhere is rejected by grammar (`E210`).
* **Smoothness lattice ($C^\infty \sqsubset C^2 \sqsubset C^1 \sqsubset C^0 \sqsubset C^{-1}$):** `f'(u)` / `diff(f, u)` requires `smooth ≥ C1` and lowers order by 1 (`E307` on `min`, `abs`, `voronoi`).
* **Write ownership & staging:** Every `a.f = e` inside a `law` checks `f ∈ writes(law)` (`E501`). Across all laws in a schedule, `writes(L1) ∩ writes(L2) == ∅` (`E502`). `emit` and `build for` are illegal outside `build` (`E404`); `time` and GPU queries are illegal inside `build` (`E403`).

---

## 4. The Three Bakeoff Tasks in Loft v1.0

### Task 1: Hermit Crab

```loft
space World Body Leg Claw Face Shell

const TAU = 6.2831853rad

# --- Shared space-polymorphic material (noise only here, never in SDFs) ---
fn chitin(p, n) =
  ggx([0.82, 0.24, 0.08], 0.24 + 0.16 * fbm2(coords(p).xz * 80), 0)
  + sss([0.9, 0.3, 0.1], 0.04m)
  + coat(0.75, 0.06)

# --- Shell: scanned parallel-transport frames + ribbed growing parametric tube ---
fn spine(u) = {
  a = 3.0 * TAU * u
  r = 0.018m * exp(1.6 * u)
  [r * cos(a), 0.14m * u, r * sin(a)]@Shell
}

scan whorl u in 0..1 by 256 on bake_lut {
  init = frame(spine(u), dir(spine'(u)), [0, 1, 0])
  step prev = transport(prev, spine(u), dir(spine'(u)))
}

kind Shell @Shell (pose: xf@Shell>World) {
  surface(u, v) grid 96x32 = {
    f = whorl(u)
    a = TAU * v
    r = 0.011m * exp(1.6 * u) * (1 + 0.10 * cos(12 * a) + 0.05 * sin(36 * TAU * u))
    f.o + r * (cos(a) * f.n + sin(a) * f.b)
  }
  material(p, n) = ggx([0.82, 0.68, 0.52], 0.42, 0) + coat(0.35, 0.18)
}

# --- Claw: one kind; crusher and feeder differ only in row data ---
kind Claw @Claw (pose: xf@Claw>World, open: rad, k: m in 0.02m..0.4m, teeth: m in 0m..0.02m) {
  trace(p) = {
    palm = round_box(p, [0.45 * k, 0.30 * k, 0.26 * k], 0.10 * k)
    fixed = capsule(p, [0.35 * k, -0.08 * k, 0], [1.25 * k, -0.14 * k, 0], 0.11 * k)
    map jaw = p |> move([-0.40 * k, 0.12 * k, 0]) |> rot_z(-open)
    serr = teeth * smootherstep(0.05 * k, 0.85 * k, jaw.p.x) * sin(7 * TAU * jaw.p.x / (0.9 * k))
    moving = capsule(jaw.p, [0, 0, 0], [0.90 * k, 0.06 * k, 0], 0.10 * k) - serr
    smin(smin(palm, fixed, 0.05 * k), moving, 0.03 * k)
  }
  material(p, n) = chitin(p, n)
}

# --- Body & 12 Legs (1 kind, 12 rows per crab) ---
kind CrabBody @Body (pose: xf@Body>World) {
  trace(p) = round_box(p, [0.14m, 0.055m, 0.11m], 0.035m)
  material(p, n) = chitin(p, n)
}

kind CrabLeg @Leg (parent: CrabBody, pose: xf@Leg>World, femur: m in 0.02m..1m, tibia: m in 0.02m..1m) {
  trace(p) = {
    knee = [femur, 0.03m, 0]@Leg
    tip = [femur + 0.55 * tibia, -0.82 * tibia, 0]@Leg
    smin(capsule(p, [0, 0, 0], knee, 0.016m), capsule(p, knee, tip, 0.011m), 0.01m)
  }
  material(p, n) = chitin(p, n)
}

# --- Eyes: one mirrored definition, per-eye world gaze through mirror ---
kind Eye @Face (pose: xf@Face>World, gaze: p3@World) {
  map sym(p) = p |> mirror_x |> move([-0.045m, -0.04m, -0.10m])
  trace(p) = {
    eye_c = sym.pt(0.01m, 0.075m, 0.02m)
    smin(capsule(sym.p, sym.pt(0, 0, 0), eye_c, 0.011m), sphere(sym.p - eye_c, 0.022m), 0.008m)
  }
  material(p, n) = {
    eye_c = sym.pt(0.01m, 0.075m, 0.02m)
    eye_w = pose * sym.out(eye_c)
    look = sym.in(inv(pose) * dir(gaze - eye_w))
    in_globe = dist(sym.p, eye_c) < 0.024m
    is_pupil = in_globe && dot(sym.in(n), look) > 0.84
    if is_pupil { ggx([0.02, 0.02, 0.02], 0.08, 0) + coat(0.95, 0.02) }
    else if in_globe { ggx([0.86, 0.72, 0.25], 0.16, 0) + coat(0.9, 0.03) }
    else { chitin(p, n) }
  }
}

law drift_gaze(e: Eye, dt) writes e.{gaze} {
  a = 0.25rad/s * time
  e.gaze = [0.45m * sin(a), 0.14m + 0.04m * cos(1.6 * a), 0.75m + 0.20m * cos(a)]@World
}

# --- Wet sand, sun, sky ---
kind Sand @World () {
  trace(p) = p.y
  material(p, n) = {
    g = fbm2(coords(p).xz * 24)
    ggx([0.34, 0.28, 0.20] * (0.85 + 0.3 * g), 0.22 + 0.18 * g, 0) + coat(0.55, 0.06)
  }
}

kind Sun @World (dir: d3@World, color: rgb) { light(p) = directional(dir, color) }
kind Sky @World (zenith: rgb, horizon: rgb) { sky(rd) = mix(horizon, zenith, clamp(rd.y, 0, 1)) }

# --- Stage 0 build: adding a second crab emits rows only, zero new code ---
build fn leg_socket(j, side) = place([side * 0.11m, 0, -0.08m + 0.032m * j]@Body) * rot_y(side * (1.2rad - 0.12rad * j))

build fn crab(origin: p3@World, yaw: rad) {
  body = place(origin) * rot_y(yaw)
  b = emit CrabBody(pose: body)
  build for j in 0..6, side in [-1, 1] {
    emit CrabLeg(parent: b, pose: body * leg_socket(j, side),
                 femur: 0.075m + 0.008m * j, tibia: 0.095m + 0.007m * j)
  }
  emit Shell(pose: body * place([0, 0.055m, -0.06m]@Body))
  emit Claw(pose: body * place([0.12m, 0.02m, 0.13m]@Body) * rot_y(-0.35rad), open: 0.50rad, k: 0.11m, teeth: 0.004m)
  emit Claw(pose: body * place([-0.12m, 0.015m, 0.12m]@Body) * rot_y(0.45rad), open: 0.22rad, k: 0.055m, teeth: 0m)
  emit Eye(pose: body * place([0, 0.04m, 0.05m]@Body), gaze: [0, 0.14m, 0.75m]@World)
}

build {
  emit Sand()
  emit Sun(dir: dir([-0.4, 0.85, 0.35]@World), color: [2.8, 2.6, 2.3])
  emit Sky(zenith: [0.35, 0.56, 0.88], horizon: [0.82, 0.86, 0.90])
  crab([0, 0.08m, 0]@World, 0rad)
}

schedule Main {
  run_laws(drift_gaze)
  orbit([0, 0.10m, 0]@World, 0.9m, 38deg)
  cull_indirect(Sand, CrabBody, CrabLeg, Claw, Eye, Shell)
  render_bundle(Vis64) { draw_indirect(Sand, CrabBody, CrabLeg, Claw, Eye, Shell) }
  tile_bin(Vis64, 8)
  shade_tiles(Vis64, Sun, Sky)
  tonemap_aces(ColorHdr, Sky)
}

```

### Task 2: Reed Bed in Wind

```loft
space World Reed

const TAU = 6.2831853rad
const PAPER = [0.94, 0.89, 0.78]
const INK_DARK = [0.13, 0.11, 0.15]
const INK_FAINT = [0.76, 0.79, 0.80]
const WIND = dir([0.85, 0, 0.52]@World)

lobe ink(dark, cut, n, v, l) = if dot(n, l) < cut { dark } else { PAPER }

kind Reed @Reed (pose: xf@Reed>World, height: m in 0.4m..2.5m, lean: rad, stiff: f in 0.2..2.0, phase: rad) {
  trace(p) = {
    o = pose * [0, 0, 0]@Reed
    h = clamp(p.y / height, 0, 1)
    gust = 0.5 + 0.5 * sin(1.2rad/m * o.x - 1.9rad/s * time)
    sway = (0.34rad * gust + 0.07rad * sin(2.1rad/s * time + phase)) / stiff
    shift = [sin(lean) * p.y, 0, 0]@Reed + (inv(pose) * WIND) * (height * f(sway) * h * h)
    q = p - shift
    stalk = capsule(q, [0, 0, 0], [0, 0.88 * height, 0], 0.011m * (1 - 0.55 * h))
    head = capsule(q, [0, 0.86 * height, 0], [0, height, 0], 0.022m)
    min(stalk, head)
  }
  material(p, n) = ink(INK_DARK, 0.10)
}

kind Water @World () {
  trace(p) = p.y
  material(p, n) = {
    ripple = fract(coords(p).z * 8 + 0.35 * sin(coords(p).x * 2rad))
    emit_lobe(if ripple < 0.08 { INK_FAINT } else { PAPER })
  }
}

kind Sun @World (dir: d3@World, color: rgb) { light(p) = directional(dir, color) }
kind Sky @World () {
  sky(rd) = if fract(rd.y * 14 + 0.05 * fbm2(rd.xz * 3)) < 0.07 { INK_FAINT } else { PAPER }
}

build fn rnd(i, k) = fract(sin((i * 12.9898 + k * 78.233) * 1rad) * 43758.5453)

build {
  emit Water()
  emit Sun(dir: dir([-0.5, 0.8, 0.3]@World), color: [1, 1, 1])
  emit Sky()
  build for i in 0..200 {
    a = TAU * rnd(i, 1)
    r = 1.6m * sqrt(rnd(i, 2)) * (0.65 + 0.6 * rnd(i, 3))
    emit Reed(
      pose: place([r * cos(a) + 0.2m * (rnd(i, 4) - 0.5), 0, r * sin(a) + 0.2m * (rnd(i, 5) - 0.5)]@World),
      height: 0.9m + 0.7m * rnd(i, 6),
      lean: 0.26rad * (rnd(i, 7) - 0.5),
      stiff: 0.35 + 1.25 * rnd(i, 8),
      phase: TAU * rnd(i, 9),
    )
  }
}

schedule Main {
  orbit([0, 0.65m, 0]@World, 4.2m, 34deg)
  cull_indirect(Water, Reed)
  render_bundle(Vis64) { draw_indirect(Water, Reed) }
  tile_bin(Vis64, 8)
  shade_tiles(Vis64, Sun, Sky)
  present_ldr(ColorHdr, Sky)
}

```

### Task 3: Heron's Hop

```loft
space World Body

const TAU = 6.2831853rad
const HOP_DUR = 0.45s
const NECK_BASE = [0, 0.62m, 0.12m]@Body

fn wrap_pi(a: rad) -> rad = a - TAU * floor((a + 0.5 * TAU) / TAU)
fn bearing(a: p3@World, b: p3@World) -> rad = atan2(b.x - a.x, b.z - a.z)
fn fish_curve(t: s) -> p3@World = [1.7m * sin(0.42rad/s * t), 0.04m, 1.3m + 0.7m * cos(0.42rad/s * t)]@World

kind Fish @World (clock: s, pos: p3@World) {
  trace(p) = sphere(p - pos, 0.05m)
  material(p, n) = ggx([0.75, 0.78, 0.82], 0.2, 0.7) + coat(0.6, 0.05)
}

kind Heron @Body (
  pose: xf@Body>World, anchor: p3@World, yaw: rad, hop_t: s, lift: m,
  neck_yaw: rad, pitch: rad, limit: rad in 0.2rad..2.5rad, leftover: rad,
  eye_w: p3@World, hips_w: p3@World, target: Fish,
) {
  trace(p) = {
    trunk = capsule(p, [0, 0.54m, -0.16m], [0, 0.62m, 0.14m], 0.10m)
    map sym = p |> mirror_x
    legs = capsule(sym.p, sym.pt(0.055m, 0.52m, -0.02m), sym.pt(0.055m, 0, 0.02m), 0.013m)
    # S-shaped neck, head and forward (+z) beak rotate by neck_yaw & pitch around NECK_BASE
    map nq = p |> move(-coords(NECK_BASE) * 1m) |> rot_y(-neck_yaw)
    n0 = nq.pt(0, 0, 0); n1 = nq.pt(0, 0.16m, 0.09m); n2 = nq.pt(0, 0.30m, -0.03m); n3 = nq.pt(0, 0.44m, 0.06m)
    neck = smin(capsule(nq.p, n0, n1, 0.024m), smin(capsule(nq.p, n1, n2, 0.021m), capsule(nq.p, n2, n3, 0.019m), 0.02m), 0.02m)
    map hq = nq.p |> move(-coords(n3) * 1m) |> rot_x(pitch)
    head = smin(sphere(hq.p, 0.042m), capsule(hq.p, hq.pt(0, 0, 0.03m), hq.pt(0, 0, 0.24m), 0.012m), 0.01m)
    min(smin(trunk, legs, 0.02m), smin(neck, head, 0.015m))
  }
  material(p, n) = ggx([0.56, 0.60, 0.64], 0.48, 0)
}

kind Water @World () {
  trace(p) = p.y
  material(p, n) = ggx([0.10, 0.22, 0.26], 0.10, 0) + coat(0.8, 0.03)
}

kind Sun @World (dir: d3@World, color: rgb) { light(p) = directional(dir, color) }
kind Sky @World (zenith: rgb, horizon: rgb) { sky(rd) = mix(horizon, zenith, clamp(rd.y, 0, 1)) }

# --- Laws: disjoint field ownership on Heron (zero shared-writer conflicts) ---
law fish_swim(f: Fish, dt) writes f.{clock, pos} {
  f.clock = f.clock + dt
  f.pos = fish_curve(f.clock)
}

law heron_body(h: Heron, dt, fishes: [Fish]) writes h.{pose, yaw, hop_t, lift} {
  f = fishes[h.target]
  want = wrap_pi(bearing(h.anchor, f.pos) - h.yaw)
  if h.hop_t < 0s && h.leftover > 0rad { h.hop_t = 0s }
  if h.hop_t >= 0s {
    u = clamp(h.hop_t / HOP_DUR, 0, 1)
    h.lift = 0.18m * 4 * u * (1 - u)
    step = 2.6rad/s * dt
    h.yaw = h.yaw + clamp(want, -step, step)
    h.hop_t = h.hop_t + dt
    if h.hop_t >= HOP_DUR { h.hop_t = -1s; h.lift = 0m }
  }
  h.pose = place(h.anchor + [0, h.lift, 0]@World) * rot_y(h.yaw)
}

law heron_neck(h: Heron, dt, fishes: [Fish]) writes h.{neck_yaw, pitch, leftover, eye_w, hips_w} {
  f = fishes[h.target]
  sh_w = h.pose * NECK_BASE
  want = wrap_pi(bearing(sh_w, f.pos) - h.yaw)
  h.neck_yaw = clamp(want, -h.limit, h.limit)
  h.leftover = abs(want - h.neck_yaw)
  horiz = max(dist(sh_w, [f.pos.x, sh_w.y, f.pos.z]@World), 0.05m)
  h.pitch = clamp(atan2(f.pos.y - sh_w.y, horiz), -0.8rad, 0.5rad)
  h.eye_w = h.pose * (NECK_BASE + [0.06m * sin(h.neck_yaw), 0.44m, 0.06m * cos(h.neck_yaw)]@Body)
  h.hips_w = h.pose * [0, 0.54m, 0]@Body
}

# --- Constraints: typed verdicts ---
check gaze_lock(h: Heron) -> rad {
  if !finite(h.pose) { return unknown(NonFinitePose) }
  if h.leftover > 0rad { missed(h.leftover) } else { held(h.limit - abs(h.neck_yaw)) }
}

check head_above_hips(h: Heron) -> m {
  if !finite(h.eye_w) { return unknown(NonFinitePose) }
  margin = h.eye_w.y - h.hips_w.y
  if margin < 0m { missed(-margin) } else { held(margin) }
}

build {
  emit Water()
  emit Sun(dir: dir([-0.4, 0.85, 0.3]@World), color: [2.8, 2.6, 2.3])
  emit Sky(zenith: [0.38, 0.58, 0.88], horizon: [0.84, 0.88, 0.92])
  f = emit Fish(clock: 0s, pos: fish_curve(0s))
  emit Heron(
    pose: place([0, 0, 0]@World), anchor: [0, 0, 0]@World, yaw: 0rad, hop_t: -1s, lift: 0m,
    neck_yaw: 0rad, pitch: 0rad, limit: 1.0rad, leftover: 0rad,
    eye_w: [0, 1.06m, 0.18m]@World, hips_w: [0, 0.54m, 0]@World, target: f,
  )
}

schedule Main {
  run_laws(fish_swim, heron_body, heron_neck)
  check(gaze_lock, head_above_hips)
  orbit([0, 0.6m, 0.4m]@World, 3.6m, 38deg)
  cull_indirect(Water, Heron, Fish)
  render_bundle(Vis64) { draw_indirect(Water, Heron, Fish) }
  tile_bin(Vis64, 8)
  shade_tiles(Vis64, Sun, Sky)
  tonemap_aces(ColorHdr, Sky)
}

```

### Token Counts & Floor Comparison (`o200k_base`)

Calibrated at `2.50 chars/token` from the measured `E` outputs (`out_E_sonnet.md`: `2.35–2.53 chars/tok` **measured**).

| Syntax | Task 1 (Crab) | Task 2 (Reeds) | Task 3 (Heron) | Total (3 Tasks) | vs. A | Status |
| --- | --- | --- | --- | --- | --- | --- |
| **A** (WGSL superset, avg S/H) | 2,871 | 1,195 | 2,282 | **6,365** | — | **measured** |
| **D** (Python-like, avg S/H) | 2,941 | 1,219 | 2,400 | **6,572** | +3% | **measured** |
| **E** (Terse, avg S/H) | 2,093 | 911 | 1,690 | **4,709** | −26% | **measured** |
| **Loft v1.0 (above, clean)** | **1,580** | **740** | **1,340** | **3,660** | **−42%** | **predicted** |
| **Loft v1.0 (wild S/H avg)** | **1,750** | **820** | **1,500** | **4,070** | **−36%** | **predicted** |
| **Semantic token floor (T1)** | **~950** | — | — | — | — | **predicted** |

*(The ~950-token floor for Task 1 counts irreducible facts—5 kinds, ~45 constants, 1 scan, mirror gaze, build loop, schedule—assuming standard helpers `Sun`/`Sky`/`orbit` live in prelude instead of being defined inline.)*

---

## 5. Five Diagnostics

### 1. Unstaged Loop (`E404`)

```text
error[E404]: unstaged loop at module scope
  --> reeds.loft:31:1
   |
31 | for i in 0..200 {
   | ^^^ loops outside `kind`/`law` must run explicitly at build stage
   |
   = fix: wrap row generation in `build { build for i in 0..200 { ... } }` so the loop emits rows once at build time.

```

### 2. Shared Field Writer (`E502`)

```text
error[E502]: conflicting law writes on field `Heron.yaw`
  --> heron.loft:48:34
   |
39 | law heron_body(h: Heron, dt) writes h.{pose, yaw, lift} {
   |                                              --- first writer declared here
...
48 | law heron_neck(h: Heron, dt) writes h.{yaw, neck_yaw, leftover} {
   |                                        ^^^ second writer on `Heron.yaw`
   |
   = fix: remove `yaw` from one law's `writes` set so every field of `Heron` has a single owning law.

```

### 3. Wrong-Space Helper (`E104`)

```text
error[E104]: space mismatch in call to monomorphic helper `chitin`
  --> crab.loft:34:24
   |
12 | fn chitin(p: p3@Leg, n: n3@Leg) -> Bsdf {
   |           --------- `chitin` pinned to space `Leg` here
...
34 |   material(p, n) = chitin(p, n)
   |                           ^ expected `p3@Leg`, found `p3@Claw`
   |
   = fix: drop `@Leg` (`fn chitin(p, n)`) or make it space-generic (`fn chitin[S](p: p3@S, n: n3@S)`) so any kind space can call it.

```

### 4. Mirrored Gaze Bug (`E104`)

```text
error[E104]: point from mirrored map space used as `Face` space
  --> crab.loft:61:18
   |
57 |   map sym(p) = p |> mirror_x |> move([-0.045m, -0.04m, -0.10m])
   |       --- scoped space `sym` introduced here
...
61 |   eye_w = pose * eye_c
   |                  ^^^^^ expected `p3@Face`, found `p3@sym`
   |
   = fix: lift `eye_c` back through the mirror branch with `pose * sym.out(eye_c)` so left and right eyes map to distinct world positions.

```

### 5. Authored Lipschitz Bound (`E210`)

```text
error[E210]: numeric Lipschitz bounds cannot be authored
  --> crab.loft:26:25
   |
26 |   trace(p) -> Field[Claw, 1.35] = {
   |                           ^^^^ bounds are derived by interval analysis, never trusted from source
   |
   = fix: remove `1.35` (write `trace(p) = { ... }`) and constrain any row divisors with a positive range like `k: m in 0.02m..0.4m`.

```

---

## 6. Grafts Accepted & Machine-Facing Second Form

### Grafts Accepted (With Evidence)

| Graft | Source | Why Accepted |
| --- | --- | --- |
| **Value-typed, kind-scoped `map sym(p)**` | C/D + VERDICT_ALL #1 | Removes cryptic `'sym`, fixes A-Haiku's unbound file-scope map, and eliminates duplicated mirror chains across `trace`/`material` (4/4 D/E outputs **measured**). |
| **Field-level `writes h.{a, b}**` | VERDICT #4 + G | Directly solves the #1 error across all Round-1 syntaxes (`5/8` outputs **measured**), and achieved `0` shared-writer errors in G (**measured**). |
| **Range-refined row columns (`in lo..hi`)** | VERDICT_ALL #4 | Directly solves the #2 error (`7/10` A–E outputs had unclamped row divisors **measured**) without verbose `max(...)` clutter. |
| **Typed `check ... -> rad`/`m` verdicts** | B/C/D + VERDICT #2 | Makes unit mismatches between `held` and `missed` impossible at 2 tokens. |
| **Space-polymorphic helpers (`fn chitin(p, n)`)** | VERDICT #5 + G | Fixes shared-material E104 bugs in A-Sonnet & C-Sonnet (**measured**) at 0 token cost. |
| **Explicit `build` + `emit` staging** | A (`comptime for`) + G (`for!`/`emit!`) | Prevents D-Haiku's unstaged module scripts, F-Sonnet's `time`-in-build leak, and A-Haiku's struct-literal row confusion (**measured**). |
| **Table-vs-row check (`E108`) & mirror-duplicate lint (`W410`)** | VERDICT_ALL #1, #2 | Catches E-Haiku's `fish: Fish` error and 4-eye double-mirroring bug (**measured**). |

**Rejected:** B's implicit lambda-capture columns (worst Haiku gap, invisible kind boundary **measured**); C's `<T>` declarator grammar (highest violation count **measured**); G's `($ ...)` dual-syntax math escape.

### Second Machine-Facing Form: Yes, As an IR Codec (`loft ir --sexp`)

Ship **one** human/LLM authoring surface (`.loft` v1.0 above) plus a **canonical S-expression serializer/parser** directly over Layer 1 typed SSA IR (`loft ir scene.loft --sexp` / `loft build scene.sexp`).

* **What it buys:** Agents doing recursive compiler/scene rewrites get Lisp's uniform tree format (`(kind Claw :space Claw :rows ((k m 0.02 0.4)) (trace (p) ...))`) with zero precedence or layout rules, while ordinary scene authoring keeps clean infix maths and zero duplicate type-checking.
* **Cost:** ~250 LOC (`sexp.rs`) reading/writing the exact same `loft-ir` structs.

---

## 7. Predicted Parser Size (`crates/loft-parse`)

| Module | Responsibility | Predicted LOC |
| --- | --- | --- |
| `lexer.rs` | Tokens, `#` comments, newline-continuation rules, `number + unit` lexing, balanced `wgsl { ... }` scanner | **280** |
| `ast.rs` | Typed AST nodes with byte spans (`Kind`, `Map`, `Law`, `Check`, `Scan`, `Build`, `Schedule`, `Expr`) | **240** |
| `parser.rs` | ~75-production LL(1) declarations + Pratt infix/pipe expression parser + error recovery | **620** |
| `sexp.rs` | Lossless canonical S-expression printer + reader for Layer 1 IR (optional machine form) | **250** |
| **Total (`loft-parse`)** | Complete v1.0 front end (vs. ~2,800–3,500 LOC predicted for full WGSL superset) | **1,390** |
</pasted_content id="ac93">

- part 2 -


<pasted_content id="ac93">
No—the v1.0 response only gave an EBNF specification and an LOC breakdown. It did **not** include a `.lark` grammar file, lexer regex definitions, or a parser-generator spec (like `.lalrpop`, `.pest`, or `tree-sitter`).

---

### 1. Lark Files, Lexers, and Why `.lark` Matters Here

If you mean **`.lark` files for LLM constrained decoding** (`llguidance`, XGrammar, vLLM/OpenAI CFG tools), that is actually a major reason to care about grammar cleanliness right now:

* **`llguidance`** (Rust crate, ~50 µs/token overhead) enforces arbitrary context-free grammars written in a Lark-like syntax. In your creative training / RL rollout pipeline, shipping a `.lark` file for Loft guarantees 100% syntactic validity during sampling—so rollouts never waste compute on syntax errors, only semantic/aesthetic quality.
* **For the Rust compiler itself**, you generally do **not** want to drive the compiler with a Lark or `lalrpop` LR(1) generator: parser generators produce terrible error messages for LLM repair loops. The clean split is:
* **Compiler front end:** `logos` (DFA lexer) + hand-written LL(1)/Pratt parser (or `winnow`/`chumsky`) for crisp `= fix:` diagnostics.
* **Training / decoding & IDE:** A companion `loft.lark` (~90 lines) for `llguidance` constrained decoding, plus a `tree-sitter` grammar if humans edit it often.



---

### 2. Can We Do Better Than v1.0 (E+)?

Yes—three concrete weaknesses are still sitting in that E+ design:

1. **Fix newline sensitivity so `loft.lark` is trivial.**
Rule 1 in E+ (*"newline ends a statement unless the line ends with an operator or the next line starts `|>`"*) is context-sensitive lookahead. That makes `.lark` grammars brittle and creates classic unary-minus traps (`a = x \n -y` parsing as `a = x - y`).
* **Better:** Require explicit `let` (or `:=`) for new bindings, or use `;` only when multiple statements share a line, and ban leading-newline continuation except for `|>`. Even cleaner: make `let x = ...` start bindings so a parser (and a `.lark` CFG) always knows where the next statement begins without whitespace gymnastics.


2. **Delete the 7-line `schedule Main` boilerplate from scenes.**
Look at all three bakeoff tasks: every single file copy-pasted the same `cull_indirect / render_bundle / tile_bin / shade_tiles / tonemap_aces` block (~120–150 tokens of noise), and models repeatedly forgot to list a newly created kind in `cull_indirect` (C-Sonnet dropped `CrabBody`, `Eye`, and `Shell` from the draw list).
* **Better:** Default to `use engines::realtime` (auto-drawing all emitted drawable kinds in the scene) and only write a `schedule` block when you are actually forking the renderer. That cuts another **10–15% of tokens** and eliminates an entire class of silent empty-render bugs.


3. **Move `Sun` / `Sky` defaults into `std::env` and drop `emit` on singleton scene config.**
In Round 1, every model invented incompatible `sun_sky`/`sky_color` names because the card didn't show a standard sun/sky declaration. Making `emit Sun(...)` / `emit Sky(...)` standard prelude kinds removes ~80–120 tokens per file and stops models from hallucinating global light structs.

---

### 3. Honest Case: TypeScript Syntax vs. WGSL Superset (A) vs. Custom Syntax (E+)

| Surface | Real Reasons to Pick It | Real Dealbreakers |
| --- | --- | --- |
| **TypeScript syntax** *(reusing `oxc`)* | In 2026 the Rust TS stack is genuinely fast: `oxc_parser` parses 100% of TS and compiles to ~400 KB gzipped WASM, `oxlint` has an agent-native `--format=agent` mode, `oxfmt` formats cleanly, and `oxc-project/tsrs` (Rust TS 7 checker) exists. Free IDE support. | **Two fatal traps:** (1) Standard TS tooling rejects vector operator overloading (`pose * p`, `p - eye_c`, `0.5m`, `spine'(u)`). If you keep valid TS you get nested `add(mul(...))` bloat; if you add operators you break `oxc`/`oxfmt`/VS Code anyway. (2) **False-friend syndrome:** Your bakeoff proved familiar host syntax (Python D, `+5` Haiku gap) lures models into writing host scripts (`random.Random`, unstaged loops, closures)—the exact tracing disaster you already lived through. |
| **WGSL superset (A)** *(what Phase 1 already runs)* | **You already built Phase 0 + Phase 1 in A** (8 modules, 13 pipelines, hermit crab rendering). Zero mental distance between source and `loft show` WGSL output. Zero Haiku-vs-Sonnet forgiveness gap (**measured** 4 vs 4 in Round 1). | Permanently ~35–45% more tokens per scene (`6,365` vs `~3,660–4,070`), verbose `vec3<f32>` / `f32(...)` / `Pt[Leg](...)` ceremony, and ~2–3× larger parser & `.lark` grammar (~250+ WGSL productions). |
| **New terse syntax (E+)** | **~35–42% fewer tokens** (huge multiplier for RL training rollouts & agent context budgets), **tiny ~75-production grammar** (easy ~1,100-LOC Rust parser + clean `.lark` file for `llguidance`), zero host-language false friends, native infix math + `wgsl { ... }` escape hatch. | ~1–2 days of one-time churn to swap the Phase 1 parser and port your 282-line `lib/`; no pre-existing IDE formatter/LSP until you write a tiny `loft fmt` + `tree-sitter` grammar. |

---

### 4. Is the New Syntax Actually Worth It?

* **TypeScript syntax: No.** Reusing `oxc` only saves ~1,100 lines of Rust parser code (a single afternoon for Codex) while permanently blocking operator overloading, clean staging, and 3D types. That is a bad trade.
* **New syntax (E+) vs. WGSL superset (A): Yes, E+ is worth it—*precisely because* of your creative RL training pipeline and agentic rewrite loop.**
* You only have ~282 lines of `lib/` and 3 example `.loft` files right now. The switching cost today is tiny; after Phase 3–5 (295 regenerated corpus scenes + training seeds), switching will be painful.
* In a training and agent-authoring loop, cutting ~35–42% of tokens permanently buys faster rollouts, cheaper TTC, cleaner `.lark` constrained decoding, and fewer unclamped-divisor / shared-writer bugs—while keeping all of Layer 0 (`loft-runner`), Layer 1 (`loft-ir`), and Layer 2 (`naga` emit, AD, bounds, gates) **100% untouched**.
* The only reason to stay on **A** is if you have zero appetite for a 2-day parser swap right now—in which case you should still graft the four semantic fixes (kind-scoped value `map`, field-level `writes`, range-refined row columns, space-polymorphic helpers) onto A immediately before building Phase 3 motion.
</pasted_content id="ac93">

---- end context ----


ABOVE IS THE DUMP FROM THE RESEARCH SCIENTIST,
I'm inclined to trust him
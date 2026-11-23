## 1. The 10 Unambiguity Rules

1. **Statement termination:** Statements end at `NEWLINE` or `;`. The lexer suppresses `NEWLINE` inside `(...)` and `[...]`, immediately after a binary operator / `,` / `=`, and immediately before a leading `|>` or `else`.
2. **Top-level discipline:** Every top-level item starts with a keyword (`use`, `pub`, `space`, `const`, `fn`, `lobe`, `scan`, `kind`, `body`, `state`, `law`, `solver`, `plan`, `check`, `retarget`, `world`, `build`, `schedule`, `input`, `param`, `camera`, `target`). Bare module statements are `E404`.
3. **Type position isolation:** Types appear only after `:` or `->`. Inside type position, `>` exclusively joins spaces (`xf@A>B`) and `[K]` is exclusively a read-only table type (`Table[K]`).
4. **Single-purpose `@`:** `@` attaches a coordinate space (`kind K @S`, `p3@S`, `xf@A>B`, `[x, y, z]@S`) and nothing else.
5. **Unit suffixes:** Digits immediately touching letters lex as `NUM_UNIT` (`0.5m`, `40deg`, `1.2rad`, `0.5s`, `18kg`, `260N`). Unknown suffixes are lex errors.
6. **Single-purpose `'`:** Postfix `'` exclusively denotes differentiation (`spine'(u)` $\equiv$ `diff(spine, u)(u)`).
7. **Case namespace:** `UPPER_CAMEL` identifiers are `Space`, `Kind`, `Body`, `State`, `World`, `Schedule`, or `Unknown` reason tags; `lower_snake` identifiers are locals, columns, functions, laws, and queries.
8. **Explicit bindings vs. writes:** `let x = e` introduces an immutable binding; `var x = e` introduces a mutable local; `x = e` updates a `var`; `a.f = e` is legal only inside a `law`/`solver` on fields owned by its `writes` clause.
9. **Explicit row construction:** `emit Kind(field: val)` is the sole row emitter and is legal only inside `build` blocks / `build fn`. `Kind(...)` without `emit` is illegal.
10. **Raw WGSL island:** `wgsl(a: f, b: v3) -> f { ... }` lexes raw text to its matching balanced `}` (skipping `//`, `/* */`, `"..."`); validated by `naga` at emit time.

## 2. Typing, Inference & Staging Rules

### 2.1 Written vs. Inferred

| Construct | Must Be Written | Inferred Automatically |
| --- | --- | --- |
| `kind` | `@Space`, row column types (with `in lo..hi` or `> 0` if divided), discrete `[var: T]` | `std430` layout, kind-scoped `map` spaces, `Field[S]`, Lipschitz $L$, smoothness $C^k$, AABBs |
| `fn` / `lobe` | `pub fn` signature; `wgsl(...) -> T` boundary types | Private `fn` types; automatic **space polymorphism** on unannotated point/normal args |
| `law` / `solver` | Written target `h: Kind`, read tables `[Other]`, `writes h.{fields}` | `dt: s`, locals, dependency wave, `(wave, kind)` bucket, intra-tick fusion |
| `check` | Target `h: Kind`, read tables `[Other]`, verdict unit `-> rad` / `-> m` | `held` / `missed` / `unknown` typing |
| Expressions | `@S` only on space change; unit suffix only when ambiguous | Numeric promotion (`0`, `1` unify with `f`/`m`/`rad`/`s`); `[x, y, z]` affine type (`p3`/`v3`/`d3`/`rgb`); block tail return |

### 2.2 Spaces, Affine Geometry & Domain Maps

* `p3@S - p3@S -> v3@S`; `p3@S + v3@S -> p3@S`; `p3@S + p3@S -> E101`; `S1 != S2 -> E104`.
* `xf@A>B * p3@A -> p3@B`; `xf@A>B * v3@A -> v3@B`; `xf@A>B * d3@A -> d3@B`; `xf@A>B * n3@A -> n3@B` (inverse-transpose).
* `attach@S` (rigid body anchor) and `target@S` (independent world goal) never coerce implicitly (`E105`).
* `map sym(p) = expr` (kind-scoped) or `map sym = expr` (local) binds an existential space `S_sym` on value `sym`:
* `sym.p : p3@S_sym`, `sym.pt(x, y, z) : p3@S_sym`, `sym.cell : f`, `sym.stretch : f`,
* `sym.out(q) : p3@S` (auto-inverted via `sym.cell`; absent on non-invertible `warp` $\to$ `E106`),
* `sym.in(x)` (maps `p3`/`v3`/`d3`/`n3` from `S` into `S_sym`).


* **Space polymorphism:** `fn chitin(p, n)` assigns a fresh space variable `α` to `p` and `n`. Using `coords(p)` strips space/units cleanly for noise; mixing `p` with a concrete space (`p - [0,0,0]@World`) raises `E104`.

### 2.3 Smoothness ($C^k$) & Compiler-Derived Lipschitz ($L$)

* Smoothness order: $C^\infty \sqsubset C^2 \sqsubset C^1 \sqsubset C^0 \sqsubset C^{-1}$.
* $C^\infty$: arithmetic, `sin`, `cos`, `exp`, `smin_exp`, `sphere`, `plane`.
* $C^2$: `smootherstep`, `smin_cubic`, `whorl(u)` (cubic B-spline / quintic Hermite LUT), `grad_noise2`, `grad_noise3`, `wave_noise3`.
* $C^1$: `smoothstep`, `smin_poly`, `smooth_voronoi`, `capsule`, `torus`, `round_box`.
* $C^0$: `min`, `max`, `abs`, `voronoi_cone`, `box`, `cylinder`, `mirror_x`.
* $C^{-1}$: `step`, `floor`, `fract`, runtime `if`/`select`, `wgsl { ... }`.


* `f'(u)` / `diff(f, x)` requires `smooth ≥ C1` and reduces smoothness by 1 (`E307`). `subgrad(e, x)` opts into $C^0 \to C^{-1}$ shading gradients.
* **Loop AD rule:** `diff` crossing a runtime `for` loop with trip count > 4 lowers GPU normals to 4-tap tetrahedron differences unless `symbolic_ad` is requested (prevents the 27× coral register blowup **measured**).
* **Lipschitz derivation (never authored; `E210` if written):** Computed via outward-rounded interval arithmetic over the kind's local AABB $D$ (seeded by row range refinements `in lo..hi`, factored out of `pose` so distinct poses share 1 derivation):
* Exact SDFs: `1`; `ellipsoid(p, r)`: `max(r)/min(r)`; constants/rows: `0`; isometries/mirrors: `L(f)`.
* Uniform scale `f(s*p)` vs `s*f(p/s)`: `|s|*L(f)` vs `L(f)`; conformal maps multiplied by `m.stretch`: `L(f)`.
* `repeat_grid` / `repeat_polar`: `L(f)` with automatic cell-wall clamp unless wall symmetry is proven.
* `a*f ± b*g`: `|a|L(f) + |b|L(g)`; `f*g`: `‖I(f)‖∞ L(g) + ‖I(g)‖∞ L(f)`; `h(f)`: `sup |h'(I(f))| * L(f)`.
* `min`/`max`/`smin_*`/`ssub_*` (const `k`): `max(L(f), L(g))`; varying `k(p)`: `+ c_blend * L(k)`.
* `grad_noise3` (`L=2.45`), `wave_noise3` (`L=1/√3`), `smooth_voronoi3` (`L=1.0`), `fbm3` (`|s|·L_n·Σ(gain·lac)^i`).
* Division with `0 ∈ I(denom)`, unbounded `exp`/`tan`, `wgsl { ... }`: `Unbounded` $\to$ `E211` in `trace` unless clamped or wrapped in `bound_step(f, max_step: ...)`.



### 2.4 Staging, Ownership & Motion Fusion

* **Stage 0 (`build`):** Runs once on CPU/WASM. `build for` emits rows via `emit Kind(...)` and returns `RowId`. Cannot read `time` or GPU state (`E403`).
* **Stage 1 (`kind`):** `kind K[flag: bool]` or `kind K[n: u in lo..hi]` (upper bound excluded) allows discrete variants only (cardinality ≤ 8; `E401`/`E402`). `emit` supplies a parameter like a column and the row stores it, so CPU evaluation reads it unchanged; every GPU module of the kind folds it to a literal, so a loop over it unrolls exactly. The current compiler builds one variant per kind: rows that disagree are `E402`, a law cannot write a parameter (`E501`), and a host that upserts rows must keep the emitted value.
* **Stage 2 (`runtime`):** `for` inside `kind`/`law`/`solver` stays a GPU loop.
* **Law ownership & fusion:** Every `a.f = e` checks `f ∈ writes` (`E501`). Across a schedule, two laws on the same kind must have disjoint `writes` sets (`E502`). Consecutive row-local laws on the same kind with disjoint `writes` **automatically fuse inside one per-tick `advance(from, to)` loop** in schedule order—preserving 1 dispatch/frame and bit-exact multi-tick catch-up.

## 3. Diagnostic Codes & Worked Messages

| Code | Trigger |
| --- | --- |
| `E001` / `E011` | Syntax error, unknown unit suffix, import cycle, or unbound name |
| `E101` / `E104` | Affine error (`p3 + p3`), space mismatch, or leaked `map` space |
| `E105` / `E106` | `attach` vs `target` mismatch; `.out` called on non-invertible `warp` |
| `E108` | Bare kind `f: Fish` used as read-only table parameter instead of `f: [Fish]` |
| `E110` | Dimensional unit mismatch (`m + s`) or ambiguous unitless literal |
| `E201` / `E210` | Degenerate zero-volume bound; authored numeric Lipschitz constant |
| `E211` / `E212` | `Unbounded` field in `trace` (e.g. unclamped row divisor); missing required query |
| `E307` / `E308` | `diff` applied to `C0`/`Cneg1` expression; degenerate scan tangent frame |
| `E401`–`E404` | Continuous kind variant; > 4 variants; `time` in `build`; unstaged module loop/stmt |
| `E501`–`E503` | Undeclared field write; overlapping `writes` across laws; race in `solver` |
| `E601` / `W410` | Missing `retarget` chain; warning for duplicate mirrored rows on a kind with `mirror_x` |

```text
error[E404]: unstaged loop at module scope
  --> scenes/reeds.loft:24:1
   |
24 | for i in 0..200 {
   | ^^^ loops outside `kind`/`law`/`solver` must run explicitly at build stage
   |
   = fix: wrap row generation in `build { build for i in 0..200 { ... } }`.

error[E502]: conflicting law writes on field `Heron.yaw`
  --> scenes/heron.loft:52:38
   |
41 | law heron_body(h: Heron, dt) writes h.{pose, yaw, lift} {
   |                                              --- first writer declared here
...
52 | law heron_neck(h: Heron, dt) writes h.{yaw, neck_yaw, leftover} {
   |                                        ^^^ second writer on `Heron.yaw`
   |
   = fix: remove `yaw` from one law's `writes` set so every field has one owning law.

error[E104]: point from mirrored map space used as `Face` space
  --> scenes/hermit.loft:61:22
   |
57 |   map sym(p) = p |> mirror_x |> move([-0.045m, -0.04m, -0.10m])
   |       --- scoped space `sym` introduced here
...
61 |   let eye_w = pose * eye_c
   |                      ^^^^^ expected `p3@Face`, found `p3@sym`
   |
   = fix: lift `eye_c` through the mirror branch with `pose * sym.out(eye_c)`.

error[E211]: unclamped divisor can reach zero (`L = Unbounded`)
  --> scenes/reeds.loft:14:44
   |
 9 | kind Reed @Reed (height: m, stiff: f) {
   |                             -------- unconstrained `f` includes 0.0
14 |   let sway = 0.34rad * gust / stiff
   |                             ^^^^^^^ denominator interval includes 0
   |
   = fix: add a positive range refinement `stiff: f in 0.2..2.0` on the column or clamp with `max(stiff, 0.2)`.

```

---

# FILE 3: `docs/spec/loft.lark` (Lark CFG + `logos` Lexer Spec)

Drop `loft.lark` directly into `llguidance` / XGrammar for 100% syntactically valid LLM rollouts.

```lark
// Loft v1.0 Context-Free Grammar (Lark / llguidance compatible)
start: (_NL)* (top_item (_SEP top_item)*)? (_SEP)?

top_item: use_decl
        | "pub"? ( space_decl | const_decl | input_decl | param_decl
                 | camera_decl | target_decl | fn_decl | lobe_decl
                 | scan_decl | kind_decl | body_decl | state_decl
                 | law_decl | solver_decl | plan_decl | check_decl
                 | retarget_decl | world_decl | build_fn | build_blk | sched_decl )

use_decl: "use" mod_path ("::" ( "*" | "{" ident_list "}" ))? ("as" LIDENT)?
mod_path: LIDENT ("::" LIDENT)*

space_decl:  "space" UIDENT+
const_decl:  "const" IDENT (":" type_expr)? "=" expr
input_decl:  "input" LIDENT ":" type_expr ("=" expr)?
param_decl:  "param" LIDENT ":" type_expr "=" expr ("in" expr ".." expr)?
camera_decl: "camera" UIDENT "{" field_inits "}"
target_decl: "target" UIDENT ":" LIDENT (INT "x" INT | "view" ("*" NUMBER)?)? ("msaa" INT)?

fn_decl:     "fn" LIDENT gen_params? "(" params? ")" ("->" type_expr)? fn_body
lobe_decl:   "lobe" LIDENT "(" params? ")" fn_body
fn_body:     "=" expr | block

scan_decl:   "scan" LIDENT LIDENT "in" expr ".." expr "by" INT "on" LIDENT "{" _NL? "init" "=" expr _SEP "step" LIDENT "=" expr _SEP? "}"

kind_decl:   "kind" UIDENT var_params? "@" UIDENT "(" col_list? ")" "{" (_NL? kind_item (_SEP kind_item)* _SEP?)? "}"
var_params:  "[" var_param ("," var_param)* ","? "]"
var_param:   LIDENT ":" type_expr ("in" expr ".." expr)?
col_list:    col ("," col)* ","?
col:         LIDENT ":" type_expr range_ref? ("=" expr)?
range_ref:   "in" expr ".." expr | (">" | ">=") expr
kind_item:   map_decl | query_decl | fn_decl
map_decl:    "map" LIDENT "(" LIDENT ")" "=" expr
query_decl:  QUERY_NAME "(" params? ")" fn_body
           | "surface" "(" params? ")" "grid" GRID_DIM fn_body

body_decl:   "body" UIDENT "{" (_NL? body_item (_SEP body_item)* _SEP?)? "}"
body_item:   "trunk" "{" field_inits "}"
           | "chain" LIDENT ("[" expr "]")? ":" LIDENT "{" field_inits "}"
           | build_for_stmt

state_decl:  "state" UIDENT ("checkpoint" INT)? ("home" LIDENT)? "{" (_NL? col (_SEP col)* _SEP?)? "}"

law_decl:    "law" LIDENT "(" params ")" "writes" write_set block
write_set:   LIDENT ("." "{" ident_list "}")?

solver_decl: "solver" UIDENT "over" path_ref ("writes" write_set)? "config" "{" field_inits "}" block
path_ref:    IDENT ("." LIDENT)*

plan_decl:   "plan" UIDENT "(" params? ")" "->" type_expr "{" _NL? "minimize" expr _SEP "subject_to" block _SEP? "}"
check_decl:  "check" LIDENT "(" params ")" "->" type_expr block
retarget_decl: "retarget" UIDENT "from" UIDENT "to" UIDENT "{" field_inits "}"

world_decl:  "world" UIDENT "{" field_inits "}"
build_fn:    "build" "fn" LIDENT "(" params? ")" fn_body
build_blk:   "build" block
sched_decl:  "schedule" UIDENT "{" (_NL? sched_stmt (_SEP sched_stmt)* _SEP?)? "}"
sched_stmt:  expr block?

// --- Types (only after ':' or '->') ---
type_expr:   PRIM_TY ("@" IDENT)?
           | "xf" "@" IDENT ">" IDENT
           | ("attach" | "target" | "frame") "@" IDENT
           | "[" UIDENT "]"
           | "array" "[" type_expr "," INT "]"
           | UIDENT

gen_params:  "[" UIDENT ("," UIDENT)* ","? "]"
params:      param ("," param)* ","?
param:       LIDENT (":" type_expr)?
ident_list:  LIDENT ("," LIDENT)* ","?
field_inits: (_NL? field_init (("," | _SEP) field_init)* ("," | _SEP)?)?
field_init:  LIDENT ":" expr

// --- Statements & Blocks ---
block:       "{" (_NL? stmt (_SEP stmt)* _SEP?)? "}"
stmt:        "let" LIDENT (":" type_expr)? "=" expr
           | "var" LIDENT (":" type_expr)? "=" expr
           | "map" LIDENT "=" expr
           | build_for_stmt
           | "for" LIDENT "in" expr ".." expr block
           | "constrain" LIDENT "(" LIDENT "," LIDENT ")" "in" expr ".." expr block
           | "yield" expr
           | "return" expr
           | lhs "=" expr
           | expr

build_for_stmt: "build" "for" for_bind ("," for_bind)* block
for_bind:    LIDENT "in" (expr ".." expr | "[" expr_list "]")
lhs:         LIDENT ("." LIDENT | "[" expr "]")*

// --- Expressions (Pratt precedence encoded cleanly) ---
expr:        pipe_expr
pipe_expr:   or_expr ("|>" pipe_rhs)*
pipe_rhs:    LIDENT ("(" call_args? ")")?
or_expr:     and_expr ("||" and_expr)*
and_expr:    cmp_expr ("&&" cmp_expr)*
cmp_expr:    add_expr (("==" | "!=" | "<" | "<=" | ">" | ">=") add_expr)?
add_expr:    mul_expr (("+" | "-") mul_expr)*
mul_expr:    unary_expr (("*" | "/" | "%") unary_expr)*
unary_expr:  ("-" | "!") unary_expr | postfix_expr

postfix_expr: primary ( "." LIDENT | "(" call_args? ")" | "[" expr "]" | "'" )*

primary:     NUM_UNIT | NUMBER | STRING | "true" | "false" | IDENT
           | "(" expr ")"
           | "[" expr_list? "]" ("@" IDENT)?
           | "if" expr block ("else" "if" expr block)* ("else" block)?
           | "emit" UIDENT ("[" expr_list "]")? "(" named_args? ")"
           | "wgsl" "(" params? ")" "->" type_expr RAW_WGSL_BLOCK

call_args:   arg ("," arg)* ","?
arg:         (LIDENT ":")? expr
named_args:  field_init ("," field_init)* ","?
expr_list:   expr ("," expr)* ","?

// --- Terminals ---
QUERY_NAME:  "trace" | "visible" | "material" | "light" | "sky" | "medium" | "hit" | "bound" | "sigma" | "tau"
PRIM_TY:     "p3" | "v3" | "d3" | "n3" | "v2" | "v4" | "rgb" | "quat" | "f" | "u" | "i" | "bool" | "m" | "rad" | "s" | "kg" | "N"
GRID_DIM:    /[0-9]+x[0-9]+/
NUM_UNIT:    /[0-9]+(\.[0-9]+)?(m|mm|rad|deg|s|kg|N)(\/[a-zA-Z]+)?/
NUMBER:      /[0-9]+(\.[0-9]+)?([eE][+-]?[0-9]+)?/
UIDENT:      /[A-Z][a-zA-Z0-9_]*/
LIDENT:      /[a-z_][a-zA-Z0-9_]*/
IDENT:       UIDENT | LIDENT
RAW_WGSL_BLOCK: /\{[^{}]*(\{[^{}]*\}[^{}]*)*\}/

_NL:         /(\r?\n)+/
_SEP:        (_NL | ";")+
COMMENT:     /#[^\n]*/
%ignore /[ \t]+/
%ignore COMMENT

```

### Rust Lexer (`crates/loft-parse/src/lexer.rs`, `logos` + Newline Filter)

Wrap a `logos::Logos` token enum in a 45-line `TokenStream` filter implementing Rule 1:

* Track `paren_depth` (`(` / `[`). Suppress `Newline` whenever `paren_depth > 0`.
* Suppress `Newline` if the previous emitted token is in `{ Plus, Minus, Star, Slash, Percent, Eq, Comma, Pipe, AndAnd, OrOr, EqEq, NotEq, Lt, Le, Gt, Ge }`.
* Peek next non-whitespace token; suppress `Newline` if next token is `Pipe` (`|>`) or `Else`.
* On `Keyword::Wgsl` followed by `(...) -> ty {`, run a 20-line brace-depth scanner (skipping `//`, `/* */`, `"..."`) emitting a single `RawWgsl(Span)` token.

---

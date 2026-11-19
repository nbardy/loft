```text
HANDOFF BUNDLE: Loft Meta Engine v1.0 (Unified E+ Surface, Render, Light & Motion)
Target directory: docs/spec/
Files included:
  1. docs/spec/00_OVERVIEW_AND_DECISIONS.md
  2. docs/spec/01_LANGUAGE_V1.md
  3. docs/spec/loft.lark  (+ Rust `logos` lexer spec)
  4. docs/spec/02_CORE_ABI_AND_LOWERING.md
  5. docs/spec/03_SUB_ENGINES_AND_STDLIB.md
  6. docs/spec/04_ENGINES_AND_SCENES.md
  7. docs/spec/05_NUMBERS_AND_BUILD_ORDER.md

```

---

# FILE 1: `docs/spec/00_OVERVIEW_AND_DECISIONS.md`

## 1. Architecture Thesis

Loft is a **meta engine**: a tiny, slow-changing GPU runner and typed SSA IR (`crates/loft-{runner,ir}`), modular regenerable compiler passes (`crates/loft-passes`), and **separate, table-coupled sub-engines** for **render**, **light**, and **motion** written in one lean language (`.loft` v1.0).

* **Hard code is cheap; lock-in is expensive.** Every intersector, BSDF, light estimator, schedule, motion law, and solver is ordinary forkable `.loft` source. Nothing in a shipped engine is privileged.
* **Separate, compatible engines.** Motion engines (`roam`, `act`, custom solvers) and render engines (`realtime`, `woodblock`, `reference`, custom schedules) never import each other's internals. They communicate strictly through plain GPU storage tables (`World` surfaces in; `POSE` or custom geometry/control-point tables out). Motion runs completely headless for RL, baking, and server simulation; render runs static with zero motion passes.
* **Supersedes Syntax A (Phase 1) cleanly.** Phase 0 and Phase 1 proved the Layer 0–2 spine on Syntax A (`8` modules, `13` pipelines, `17` commands for 1 and 20 crabs **measured**). Swapping only `crates/loft-parse` (~1,390 LOC **predicted**) and porting the `282`-line (**measured**) Phase 1 `lib/` to v1.0 E+ cuts scene tokens by ~35–42% (**predicted**), enables clean `llguidance` constrained decoding via `loft.lark`, and closes the top cross-cutting bakeoff bugs before Phase 2–5.

## 2. Frozen Design Decisions

1. **Lean E+ surface (`.loft` v1.0) with `wgsl(...) -> T { ... }` escape blocks; zero TypeScript.** Beats WGSL superset A by −26% to −42% tokens (`4,709` vs `6,365` **measured** for E vs A) without host-tracing blowups (`707k` nodes **measured**) or Python/TS false-friend leaks (`+5` Haiku violation gap in D **measured**).
2. **Four layers: Runner (≤ 1,000 LOC), Core IR + Gates (≤ 1,500 LOC), Regenerable Passes (~10k LOC), Library + Engines (unbounded).** Zero domain words (`foot|gait|creature|trunk|pbr|shadow|sea`) in `crates/loft-{runner,ir}`.
3. **`kind` is the sole unit of code identity; continuous values live in `row` tables.** Module count is $O(K_{\text{kinds}})$ by construction (1 vs 20 crabs produce byte-identical modules **measured**); continuous `comptime` kind params are `E401`, > 4 variants are `E402`.
4. **Explicit `build` + `emit` staging (`build { ... }`, `build fn`, `build for`).** Preserves A's `0`-gap Haiku staging safety (**measured**) at 1 token while forbidding bare top-level statements (`E404`) and struct-literal row confusion.
5. **Explicit `let` / `var` bindings + `a.f = e` restricted to `law`/`solver`.** Eliminates newline/unary-minus ambiguity so `loft.lark` is strictly context-free for `llguidance` constrained decoding.
6. **`@` reserved exclusively for spaces (`@Body`, `p3@World`, `xf@Leg>World`).** Zero `@attributes`; schedule/grid/checkpoint modifiers use plain keywords (`grid 96x32`, `checkpoint 60`, `on bake_lut`).
7. **Kind-scoped value `map` (`map sym(p) = ...` with `sym.p`, `sym.pt(...)`, `sym.cell`, `sym.out(...)`, `sym.in(...)`).** Eliminates `'sym` tick-spaces, prevents unbound file-scope maps (A-Haiku **measured**), and stops duplicated mirror chains across `trace`/`material` (4/4 D/E outputs **measured**).
8. **Range-refined row columns (`stiff: f in 0.2..2.0`, `k: m > 0m`) + compiler-derived Lipschitz $L$ and $C^k$ smoothness.** Eliminates unclamped row-divisor E211 bugs (`7/10` bakeoff outputs **measured**); authoring a numeric $L$ is `E210`.
9. **Disjoint field-level write ownership (`writes h.{a, b}`) + automatic intra-tick law fusion.** Fixes the #1 bakeoff bug (`5/8` Round-1 outputs had shared writers **measured**) while fusing disjoint row-local laws on the same kind back into one per-tick `advance(from, to)` dispatch (`0.086 ms` / 1,000 crabs **measured**).
10. **Space-polymorphic helpers (`fn chitin(p, n)` generalizes `S` automatically).** Fixes A-Sonnet and C-Sonnet shared-material E104 errors (**measured**) at zero token cost.
11. **64-bit `rgba16uint` 4× MSAA `VisRec` + `depth32float` (12 B/sample) with `kind_id` packed in word0.** Compatible with `wgpu 30.0.1` MSAA rules, reproduces R0 coverage (`6,341` → `3` px **measured**), and remains a replaceable library struct.
12. **Portable $O(K_{\text{kinds}})$ draw path: GPU cull → `RenderBundle` + 1 `drawIndirect` per kind.** Identical on native `wgpu` and browser WebGPU without requiring `multi_draw_indirect`.
13. **Open staged `Bsdf` algebra + 6 `World` queries (`trace`, `visible`, `material`, `light`, `sky`, `medium`) + joint `(L, T)` RGB media.** Zero runtime closures; compiles to per-kind `K_eval`/`K_sample`/`K_emit`/`K_albedo` with Kulla–Conty compensation.
14. **Tier 0 GPU Tape VM is edit-time only, single dirty kind, workgroup-uniform, 32-vec4f register cap, never for motion.** Bounds `+2.0 ms` (**measured** full-screen 63-op tape) to ≤ `0.45 ms` (**predicted**) with < 2 ms hot-reload.
15. **Compliance-coupled motion over slice-indexed `BODY`/`STATE`/`POSE` + custom output tables, bit-identical per device via one `advance(from, to, keep)` entry.** Preserves `0/481` bit-diff live/bake parity (**measured**), supports arbitrary Spore-like morphologies in 1 shader module, and permits non-rigid motion outputs.
16. **Canonical S-expression IR codec (`loft ir --sexp`) over Layer 1 IR.** Gives RSI agents Lisp-style code-as-data rewriting (~250 LOC) without maintaining a second human surface or type checker.
17. **Default `schedule` + prelude `Sun`/`Sky`.** Omitting `schedule Main` runs `engines::realtime::realtime` over all emitted drawable kinds, cutting ~120–150 tokens of scene boilerplate and preventing omitted-kind bugs.

---

# FILE 2: `docs/spec/01_LANGUAGE_V1.md`

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
* **Stage 1 (`kind`):** `kind K[flag: bool]` allows discrete variants only (cardinality ≤ 8, used variants ≤ 4 by default; `E401`/`E402`).
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
var_params:  "[" param ("," param)* ","? "]"
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

primary:     NUM_UNIT | NUMBER | "true" | "false" | IDENT
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

# FILE 4: `docs/spec/02_CORE_ABI_AND_LOWERING.md`

## 1. Layer 0: Runner Interface (`crates/loft-runner`, ≤ 1,000 LOC)

Embeddable on a caller-owned `wgpu::Device` (native Metal/Vulkan/DX12 and `wasm32` WebGPU); zero forced clears, zero synchronous readbacks in the frame loop.

```rust
pub struct HostCaps {
    pub depth_convention: DepthConvention, // StandardZeroToOne | ReverseOneToZero
    pub supports_timestamps: bool,
    pub supports_pipeline_cache: bool,
    pub msaa_samples: u8,                  // 1 or 4
}

pub enum InputValue<'a> {
    Level(&'a [f32]),
    Event { seq: u64, payload: &'a [f32] }, // deduplicated by seq, consumed once on its tick
    PosePacket { tick: u64, data: &'a [f32] },
}

pub struct FrameMetrics {
    pub gpu_pass_ms: Box<[(PassId, f32)]>,
    pub total_gpu_ms: f32,
    pub commands: u32,
    pub modules: u32,
    pub pipelines: u32,
    pub load_ms: f32,
}

impl Runner {
    pub fn attach(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>, caps: HostCaps) -> Self;
    pub fn load_bundle(&mut self, bundle: &Bundle) -> Result<InstanceId, RunnerError>;
    pub fn hot_swap_tape(&mut self, inst: InstanceId, kind: KindId, tape: &[u32]) -> Result<(), RunnerError>;
    pub fn hot_swap_module(&mut self, inst: InstanceId, module: ModuleId, wgsl: &str) -> Result<(), RunnerError>;
    pub fn push_input(&mut self, inst: InstanceId, slot: InputId, tick: u64, val: InputValue<'_>);
    pub fn seek(&mut self, inst: InstanceId, target_tick: u64) -> Result<(), RunnerError>;
    pub fn encode_into(&mut self, inst: InstanceId, encoder: &mut wgpu::CommandEncoder,
                       target: &wgpu::TextureView, depth: Option<&wgpu::TextureView>,
                       time_s: f64, dt_s: f32) -> Result<FrameIdentity, RunnerError>;
    pub fn last_metrics(&self, inst: InstanceId) -> FrameMetrics;
    pub fn read_table_async(&self, inst: InstanceId, table: TableId, cb: Box<dyn FnOnce(&[u8]) + Send>);
    pub fn read_verdicts_async(&self, inst: InstanceId, cb: Box<dyn FnOnce(&[VerdictRecord]) + Send>);
}

```

## 2. Frozen Conventions & Bind Group ABI

* **Coordinates:** Right-handed, **Y-up**, local object forward **`+Z`**, camera looks along **`-Z`**, column-major `mat4x4f`, quaternions `vec4f(x, y, z, w)`, screen `(0, 0)` top-left, parametric normal `normalize(cross(du, dv))`.
* **Bind Groups (immutable contract between `loft-runner` and `loft-passes`):**

| Group | Bindings | Update Rate | Contents |
| --- | --- | --- | --- |
| **`@group(0)`** | `0`: `uniform FrameUniform`<br>

<br>`1`: `storage<read> InputsBuf`<br>

<br>`2`: `storage<read> ParamsBuf` | Per-frame | `time`, `dt`, `tick`, `frac`, `frame_idx`, `sample_idx`, `view_size`, `cam` (`view`, `proj`, `inv_vp`, `eye`, `reverse_z`), per-tick `input_ring` |
| **`@group(1)`** | `0`: `storage<read> RowTables`<br>

<br>`1`: `storage<read> BodyTables`<br>

<br>`2`: `storage<read> BvhNodes`<br>

<br>`3`: `storage<read> LutTables` | Per-load / row edit | std430 kind rows (via `kind_base_offset`), `BODY` rows, `Bvh2` nodes + leaf refs, baked `scan` & Kulla–Conty LUTs |
| **`@group(2)`** | `0`: `storage<read_write> StateBuf`<br>

<br>`1`: `storage<read_write> PoseBuf`<br>

<br>`2`: `storage<read_write> IndirectBuf`<br>

<br>`3`: `storage<read_write> VerdictBuf` | Per-step / per-pass | Live/baked `STATE`, 32-byte `POSE` joints (or custom motion tables), compacted `visible_rows` + `DrawIndirectArgs`, lock-free `VerdictRecord` slots |
| **`@group(3)`** | `0..7`: sampled textures (`VisBuf`, `DepthBuf`, `ShadowMap`, `HdrBuf`, `PrevHdr`, `Assets`)<br>

<br>`8`: linear sampler, `9`: nearest/comparison sampler | Per-pass | Transient schedule targets & imported assets |

## 3. Layer 1: Core Data Structures (`crates/loft-ir`, ≤ 1,500 LOC)

### 3.1 `VisRec` (4× MSAA `rgba16uint` + `depth32float` = 12 B/sample)

```text
word0 (u32 from lanes 0..1): [31..23] kind_id (9 bits, max 512 kinds; 0 = background)
                             [22]     ink_bit (1 bit)
                             [21..0]  row_id  (22 bits, max 4,194,304 rows/kind)
word1 (u32 from lanes 2..3): cover-owned payload (packed by hand; never pack2x16unorm due to Metal naga bug):
  - grid surface:  lane2 = u_u16, lane3 = v_u16
  - triangle mesh: word1 = tri_id (u32)
  - proxy ray/SDF: word1 = ray_t_f32_bits or cell_hash (u32)

```

### 3.2 `Bvh2` Acceleration Nodes (32 B/node) & `VerdictRecord` (32 B/slot)

```rust
#[repr(C)]
pub struct BvhNode { pub lo: [f32; 3], pub left_or_first: u32, pub hi: [f32; 3], pub count: u32 }
#[repr(C)]
pub struct LeafRef { pub kind_id: u32, pub row_id: u32 }

#[repr(C)]
pub struct VerdictRecord {
    pub constraint_id: u32, pub instance_id: u32, pub site_id: u32, pub tick: u32,
    pub tag: u32,         // 0=Skipped, 1=Held, 2=Missed, 3=Unknown
    pub reason_code: u32, // 0=None, 1=IterCap, 2=NonFinite, 3=MissingSurface, 4=MissingFrame, 5=SolveFailed
    pub value: f32,       // margin | error | residual
    pub law_hash_lo: u32, // invalidates cached verdict if law changes
}

```

## 4. Layer 2: Key Lowering Rules (`crates/loft-passes`, ~10k LOC Modular)

1. **Pose-factored bound & $L$ derivation:** Strip `pose: xf@S>World` before hashing shape geometry. Derivations scale with distinct local shapes (`11` at both 1 and 20 crabs **measured**), not instance count. Cache derivations on disk keyed by `(ir_hash, shape_row_bytes)` to cut warm CPU load from `230 ms` (**measured**) to `< 30 ms` (**predicted**).
2. **Staged `Bsdf` monomorphization:** `material` lobe sums (`ggx + sss + coat`) lower at compile time into four static WGSL functions per kind (`K_eval`, `K_sample`, `K_emit`, `K_albedo`) with zero heap closures and one call site per shade loop (keeping Metal cold compile at ~`70 ms`/pipeline serial **measured**).
3. **Field-disjoint law fusion + `(wave, kind)` bucketing:** Topologically sort laws into dependency waves. Any sequence of row-local laws on the same kind `K` with disjoint `writes` fuses sequentially into one `advance(from_tick, to_tick, keep)` compute entry (trip bound 64, zero atomics, explicit `fma()`). Same entry serves `Baked` and `Live` homes for 0-bit per-device parity.
4. **Indirect draw + `RenderBundle`:** Single `cull` dispatch compacts visible `row_id`s per kind and writes 16-byte `DrawIndirectArgs`. Primary visibility (and shadow maps) execute pre-recorded `RenderBundle`s with 1 `setPipeline` + 1 `drawIndirect` per kind.
5. **Single `naga` validation + name mangling:** Build `naga::Module` directly, mangle WGSL reserved words used as row fields (e.g. `target` $\to$ `r_target`, fixing Phase 1's catch), validate once, pass `ShaderSource::Naga` natively and emit WGSL text for WebGPU / `loft show`.
6. **Build-time `plan` solver:** Uniform cubic B-spline (`N = 24` knots, `M = 96` collocation points), analytical sparse Hessian, exact 3-DOF ballistic flight elimination, augmented Lagrangian (`max 40` iters, `tol = 1e-5`); failures emit `Verdict::Unknown(SolveFailed)`.
7. **Tier 0 Tape VM (edit-time, single dirty kind only):** `var reg: array<vec4f, 32>` (512 B/thread, zero spill), 56 3-address 32-bit ops (`[op:6][dst:5][a:5][b:5][imm11:11]`), kind-bucketed dispatch (uniform PC across all 32 SIMD lanes), `400 ms` quiet promotion via `create*PipelineAsync`, tolerance `|T0 - T1| ≤ 1e-5` (fields) and `≤ 1/255` on ≥ `99.9%` shaded pixels. Motion never runs in Tier 0.
8. **S-expression IR codec (`sexp.rs`, ~250 LOC):** Lossless serializer/reader over Layer 1 `Bundle`/SSA IR (`loft ir scene.loft --sexp` / `loft build scene.sexp`) for agent-driven compiler/IR transformations.

## 5. The Five CI Gates

| Gate | Pass Condition |
| --- | --- |
| **Pictures** | All expected frames present (missing frame fails). Geometry/depth pass **R0g**; light passes converged `reference` (95% CI ≤ `0.005`, interior rel bias ≤ `1%`). |
| **Twin** | Motion: `0`-bit diff live vs bake per device (30/60/120 Hz + 6-tick stall). Fields: `≤ 1e-5`; AD normals `≤ 1e-4`; lit materials `≤ 2e-5` (60° off specular peak). |
| **Speed** | Quiet M4 `p95` frame time and `commands = O(kinds + waves)` within scene budget. |
| **Load** | Modules, pipelines, cold/warm load within budget; **module text byte-identical between 1 and 20 instances**. |
| **Truth** | Every default and scene `check` returns `Held` unless explicitly marked `expect Missed` / `expect Unknown`. |

---

# FILE 5: `docs/spec/03_SUB_ENGINES_AND_STDLIB.md`

## 1. Engine Composition Contract (Separate, Compatible Engines)

```text
  [Host Inputs] ──► [Motion Engine: roam | act | clip | custom]
                         │  writes POSE (32B/joint) OR custom row table + VerdictBuf
                         ▼
               [Plain GPU Storage Tables] ◄── (zero coupling; runnable headless)
                         │
                         ▼
  [World + Cam] ──► [Render Engine: realtime | woodblock | reference | custom]
                         │  reads POSE / rows; calls 6 World queries + Light Estimators
                         ▼
                    [Swapchain]

```

## 2. Render & Light Standard Library (`std::geom`, `std::light`, `std::stylised`, `std::post`, `std::sched`)

```loft
# --- std::geom: Analytic Kinds (exact hit + bound + L=1) ---
# Plane(n: n3@World, d: m), Sphere(pose, r: m), Box(pose, half: v3), Capsule(a, b, r: m),
# Cone(a, b, ra: m, rb: m), Washer(pose, r0: m, r1: m), Torus(pose, major: m, minor: m),
# Mesh(pose, mesh_id: u), BrickField(pose, brick_id: u), Specks(count: u, buf: u, size: m)

# --- std::geom: SDF Primitives & Ops (L derived, smoothness tagged) ---
fn sphere(p, r: m) -> m                                       # Cinf, L=1
fn box(p, half) -> m                                          # C0, L=1
fn round_box(p, half, r: m) -> m                              # C1, L=1
fn capsule(p, a, b, r: m) -> m                                # C1, L=1
fn round_cone(p, a, b, r1: m, r2: m) -> m                     # C1, L=1
fn torus(p, major: m, minor: m) -> m                          # C1, L=1
fn cylinder(p, r: m, half_h: m) -> m                          # C0, L=1
fn wedge(p, half) -> m                                        # C0, L=1
fn ellipsoid(p, r) -> m                                       # Cinf, L=max(r)/min(r)
fn smin(a: m, b: m, k: m) -> m                                # C1 quadratic smooth union
fn smin_cubic(a: m, b: m, k: m) -> m                          # C2 cubic smooth union
fn smin_exp(a: m, b: m, k: m) -> m                            # Cinf exponential smooth union
fn ssub(a: m, b: m, k: m) -> m                                # C1 smooth subtraction
fn shell(d: m, thick: m) -> m                                 # C0 hollow shell
fn ray_capsule(ro, rd, a, b, r: m) -> f                       # closed-form hit t (-1 miss)
fn ray_round_cone(ro, rd, a, b, ra: m, rb: m) -> f            # closed-form hit t (-1 miss)
fn hit_min(a: f, b: f) -> f                                   # nearest positive t

# --- std::geom: Domain Maps (introduce scoped value space) ---
# mirror_x(p), mirror_plane(p, n), move(p, delta), rot_x(p, a), rot_y(p, a), rot_z(p, a),
# repeat_grid(p, period, lim), repeat_polar(p, count), fold_kaleidoscope(p, planes, iters),
# fold_screw(p, pitch, count), fold_spiral(p, arms, rate), fold_kifs(p, iters, scale, off, rot),
# warp_twist(p, rate), warp_bend(p, rate), warp_custom(p, fwd, inv_opt)

# --- std::geom: Frames, Surfaces, Noise & Assets ---
fn frame(o, t, up) -> Frame                                   # orthonormal seed frame
fn transport(prev, p, t) -> Frame                             # double-reflection RMF step
fn grad_noise2(p: v2) -> f                                    # C2, L=2.0
fn grad_noise3(p: v3) -> f                                    # C2, L=2.45
fn wave_noise3(p: v3) -> f                                    # Cinf, L=1/sqrt(3) (exact certificate)
fn fbm2(p: v2) -> f                                           # C2 3-octave FBM
fn smooth_voronoi_cone(p: v3, k: f) -> f                      # C1, L=1.0
fn smooth_voronoi3(p: v3) -> f                                # C1, L=1.2
build fn import_gltf(path: str) -> MeshAsset
build fn export_gltf(path: str, pose: TableRef)
build fn bake_brick(field_fn, bounds, res: u) -> BrickAsset

# --- std::light: 6 World Queries, Open Bsdf & Transport ---
fn trace(w: World, r: Ray) -> Hit                             # nearest hit across all kinds in w
fn visible(w: World, a: p3@World, b: p3@World) -> rgb         # any-hit + RGB media transmittance
fn material(w: World, h: Hit) -> Bsdf                         # open surface BSDF
fn light(w: World, idx: u, p: p3@World, rng) -> Lit           # sample emitter idx (0 = sun)
fn pick_light(w: World, p: p3@World, rng) -> Lit              # power/RIS emitter pick
fn sky(w: World, rd: d3@World) -> rgb                         # environment radiance
fn medium(w: World, p: p3@World) -> Sigma                     # summed overlapping media

fn ggx(albedo: rgb, rough: f, metal: f) -> Bsdf               # Kulla-Conty energy-compensated GGX
fn coat(weight: f, rough: f) -> Bsdf                          # dielectric clearcoat (attenuates below)
fn film(weight: f, ior: f, nm: f) -> Bsdf                     # Belcour-Barla thin-film iridescence
fn sheen(color: rgb, rough: f) -> Bsdf                        # Charlie grazing microfiber sheen
fn sss(color: rgb, radius: m) -> Bsdf                         # Burley dipole (realtime) / random walk (ref)
fn transmission(ior: f, rough: f, tint: rgb) -> Bsdf          # dielectric refraction (delta if rough=0)
fn emit_lobe(radiance: rgb) -> Bsdf                           # emissive lobe (registers emitter)
fn direct_mis(w: World, h: Hit, wo: d3@World, rng) -> rgb     # 1-sample MIS direct light
fn trace_refl(w: World, h: Hit, wo: d3@World, rng) -> rgb     # 1-bounce glossy reflection
fn inscatter_joint(w: World, r: Ray, t_max: m, steps: u, rng) -> Vol # unified RGB (L, T)

# --- std::stylised & std::post ---
fn cel_bands(lit: f, cuts: v3, levels: v4, soft: f) -> f
fn bokashi(u: f, foot: f, under: v2, amt: f) -> f
fn cross_hatch(px: v2, lit: f, spacing: f, deep: f) -> f
fn overprint(base: rgb, ink: rgb, cover: f) -> rgb
fn outline_vis(vis, depth, px: v2, ink: rgb, width: f) -> v4
fn tonemap_aces(hdr, sky)                                     # ACES Filmic display pass
fn bloom_mip(hdr, thresh: f, gain: f, radius: f)              # dual-filter mip bloom
fn paper_grain(c: rgb, px: v2, fibre: f, tooth: f) -> rgb

```

## 3. Motion Formulation & Standard Library (`std::motion`)

**Formulation:** Compliance-coupled dynamics ($\alpha = 1/(k\Delta t^2)$) inside a fused 4-stage fixed-order tick (`dt = 1/120 s`), plus optional build-time horizon `plan` (effort + jerk + `prefer near`, fixing the `67°`/`4.8 g` least-jerk heron failure):

1. **Stage A (Arbiter):** Moving-surface pin frames (`SurfacePin`), friction-cone slip ($\|f_t\| > \mu_s f_n$), **feedforward-acceleration capture point** $p_{\text{cp}} = p_{\text{com}} + \tau(v - v_s) + \frac{1}{2}\tau^2(a_{\text{cmd}} - a_s)$ (cuts `0.36 s` crab reversal lag to ≤ `0.08 s` **predicted**), 2D support-polygon margin (`Flight|Point|Segment|Polygon`), continuous nonnegative least-norm load shares, greedy strain-ordered lifts, unwrapped Schmitt-trigger neck handoff (`0.85` latch / `0.25` release).
2. **Stage B (Coupled Trunk + Pin Core):** Fixed symmetric Gauss-Seidel XPBD (`4..8` iters in registers, zero atomics). Authored priors (keys, `plan`, MotionBricks `PosePacket`s) enter at $\alpha_{\text{prior}} / \max(\text{composure}^2, 0.002)$; planted pins enter at $\alpha = 0$. Hit 1 drops `composure` and triggers shear slip; Hit 2 compounds natively on the narrowed support hull.
3. **Stage C (Post-Trunk IK + `present`):** Constant-accel span at frame fraction $\Delta \in [0, dt)$, closed-form 2-bone / 3-bone / constant-length $C^1$ S-neck IK (or clamped FABRIK for arbitrary editor trees) in the gripped surface's frame $\to$ writes `POSE` (or custom output table) + post-IK `Verdict`s.
4. **Stage D (Secondary & Juice):** Exact 2×2 analytic springs or 2-color shared-memory XPBD chains (`checkpoint 60`) + volume-preserving squash ($\det S = 1$).

**Trade-offs given up:** Approximate simultaneous multi-contact load sharing (NNLS + sym-GS instead of full LCP); one-way secondary feedback; anticipation requires a future beat/target (instant player input uses frame-1 lean + feedforward lift).

```loft
# --- std::motion: Stateless Biomechanics Primitives (CPU + GPU) ---
fn body_com(parts) -> MassProps                               # parallel-axis CoM & inv inertia
fn capture_point_ff(t, h: m, a_cmd, v_s, a_s, g_eff) -> p3@World
fn support_hull(feet, g_eff) -> SupportHull                   # Flight | Point | Segment | Polygon (<=16)
fn support_margin(hull, pt: p3@World) -> m
fn support_margin_without(feet, skip: u, pt: p3@World, g_eff) -> m
fn clamp_accel_support(t, hull, a_want, b, g_eff) -> v3@World
fn load_shares_nnls(feet, p_star: p3@World) -> array[f, 16]   # continuous >= 0 load shares
fn quintic_plan(d, v1, dur: s, t: s) -> KinSample             # boundary-condition anticipation (V > 2.5D)
fn solve_reach_leash(t, feet, chains, alpha: f) -> f          # XPBD hard pin reach leash
fn solve_prior_xpbd(t, prior, alpha: f) -> f                  # compliance-coupled prior pull
fn solve_dist_edge(a, b, im_a, im_b, rest: m, alpha: f) -> f  # XPBD chain edge constraint
fn solve_2bone_ik(root, tgt, l1: m, l2: m, pole) -> Pose3
fn solve_3bone_ik(root, tgt, lens: v3, pole) -> Pose4
fn solve_s_neck_ik(sh, head, len: m, segs: u) -> NeckRes      # constant-length 2-arc S-curve
fn solve_fabrik_clamped(pts, n: u, tgt, lim) -> f             # arbitrary N-link editor chains
fn step_spring_exact(x: f, v: f, drive_a: f, hz: f, zeta: f, dt: s) -> v2
fn look_level_clamped(head, tgt, trunk_q, yaw_lim: v2, glance: rad, prev_yaw: rad) -> GazeRes
fn check_pin_drift(drawn: p3@World, pin, surfs, tol: m) -> Verdict
fn check_joint_limits(pose, chains) -> VerdictStream
fn check_posture(pose, trunk, body) -> Verdict

```

**Six Default Creature Invariants (active unless `@waive`d):** `NoPlantedSlide` (≤ `0.5 mm` in surface frame), `JointLimits` (≤ `0.1°`), `NoGroundPenetration` (≤ `2.0 mm`), `PostureSanity` (`head.y > hip.y` while `composure > 0.3`), `SolverConverged` (residual ≤ `0.2 mm`, finite), `LiveBakeParity` (`0`-bit diff across 30/60/120 Hz + 6-tick stall).

## 4. The Escape Table

| Forkable Level | Author Writes | Keeps for Free | Loses |
| --- | --- | --- | --- |
| **1. Single BSDF lobe** | One `lobe` fn | Vis-buffer, all shapes, shadows, MIS, media, post, furnace checks | Multi-scatter LUT until re-integrated on twin (automatic for continuous lobes) |
| **2. Whole material model** | Custom `lobe`s + `shade_fn` | Exact depth, 4× MSAA silhouettes, true shadows, `World` overrides, motion | Furnace energy test (replaced by aesthetic/reference-input checks) |
| **3. Light transport** | New `shade_fn` using 6 `World` queries | All shape kinds, BSDFs, `RenderBundle`, media, post, R0g gate | Default unbiased MIS (scored against `reference`) |
| **4. One query of one kind** | `wgsl(...) -> T { ... }` inside that query | Typed bindings, all other kinds, vis-buffer, motion, speed/load gates | Twin, AD, derived $L$, Tier 0 tape **for that function only** |
| **5. Visibility record** | New record layout + `find_vis` + `shade_tiles` | All shapes, culling, `RenderBundle`, `drawIndirect`, motion, `reference` | Default 12 B MSAA layout & built-in `outline_vis` |
| **6. Schedule** | Custom `schedule Main { ... }` | All kinds, rows, queries, motion, `(wave, law)` bucketing, 5 gates | Default frame-budget constants until profiled |
| **7. One motion law** | One `fn` passed to `roam`/`act` or custom `law` | `BODY`/`STATE`/`POSE`, fused ticks, checkpoints, live/bake parity, 6 verdicts | Prior law's cached verdict hashes (auto re-checked) |
| **8. Motion solver** | New `solver` writing `POSE` or custom rows | Render binding, glTF export, checkpoints, replay identity, post-IK verdicts | Default XPBD compliance & built-in residual hook |
| **9. Whole engine** | Custom `schedule` + `wgsl` passes on `Runner` | Native/WASM `wgpu` runner, arenas, pipeline cache, input log, profiler, gates | Nothing forced; zero framework overhead |

---

# FILE 6: `docs/spec/04_ENGINES_AND_SCENES.md`

## 1. Forkable Example Engines (Unified E+ v1.0)

### 1.1 `engines/realtime.loft`

```loft
use std::light::*
use std::post::*
use std::sched::*

pub fn direct_plus_refl(w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb {
  var rng = rng_key(px, Frame.idx, 0)
  let m = material(w, h)
  emit_of(m, wo) + direct_mis(w, h, wo, rng) + trace_refl(w, h, wo, rng)
}

pub schedule realtime(w: World, cam: Camera, sun_cam: Option, shade_fn = direct_plus_refl, history: bool = false) {
  if_some(sun_cam, find_shadow_map(w, sun_cam, 2048))
  cull_indirect(w, cam)
  find_vis(w, cam, 4)
  tile_classify(VisBuf, 8)
  shade_tiles(VisBuf, shade_fn)
  screen_pass(inscatter_pass(w, cam, DepthBuf, 24), PremulOver)
  if_true(history, temporal_accum(HdrBuf, VisBuf, PrevHdr, PrevVis))
  bloom_mip(HdrBuf, 1.2, 0.18, 3.0)
  tonemap_aces(HdrBuf, Sky)
}

```

### 1.2 `engines/reference.loft`

```loft
use std::light::*
use std::post::*

fn path_sample(w: World, cam: Camera, px: v2, bounces: u) -> v4 {
  var rng = rng_key(px, Frame.sample, 0)
  var ray = camera_ray(cam, px + rand2(rng), rand2(rng), rand1(rng))
  var l_acc = [0, 0, 0]
  var t_acc = [1, 1, 1]
  var allow_emit = true
  for b in 0..bounces {
    let h = trace(w, ray)
    let vol = inscatter_joint(w, ray, if h.t >= 0m { h.t } else { 1e6m }, 32, rng)
    l_acc = l_acc + t_acc * vol.l
    t_acc = t_acc * vol.t
    if h.t < 0m { l_acc = l_acc + t_acc * sky(w, ray.d); break }
    let m = material(w, h)
    if allow_emit { l_acc = l_acc + t_acc * emit_of(m, -ray.d) }
    let lt = pick_light(w, h.p, rng)
    let vis = visible(w, h.p + h.n * 0.0001m, lt.p)
    l_acc = l_acc + t_acc * eval_bsdf(m, h.n, -ray.d, lt.wi) * lt.l * vis / max(lt.pdf, 1e-6)
    let sc = sample_bsdf(m, h.n, -ray.d, rng)
    if sc.pdf <= 0 { break }
    t_acc = t_acc * sc.weight
    allow_emit = sc.delta
    ray = make_ray(h.p + h.n * (if dot(sc.wi, h.n) > 0 { 0.0001m } else { -0.0001m }), sc.wi)
  }
  [l_acc.x, l_acc.y, l_acc.z, 1]
}

pub schedule reference(w: World, cam: Camera, bounces: u = 16) {
  screen_pass(path_sample(w, cam, Frame.px, bounces), Additive)
  resolve_aces(SumF32)
}

```

### 1.3 `engines/woodblock.loft`

```loft
use std::light::*
use std::stylised::*
use std::post::*

pub fn woodblock_shade(w: World, shadow_w: World, h: Hit, wo: d3@World, ink: bool, px: v2) -> rgb {
  let m = material(w, h)
  let sun = light(w, 0, h.p, rng_key(px, 0, 0))
  let vis = visible(shadow_w, h.p + h.n * 0.001m, sun.p).r
  let ndl = max(dot(h.n, sun.wi), 0) * vis
  let band = cel_bands(ndl, [0.08, 0.42, 0.78], [0.22, 0.55, 0.85, 1.05], 0.03)
  let wash = bokashi(h.uv.y, 0.15, [0, 0.65], 0.35)
  let htch = cross_hatch(px, ndl, 5.0, 0.25)
  let col = overprint(bsdf_albedo(m, max(dot(h.n, wo), 0)) * band, [0.08, 0.12, 0.22], wash * 0.45 + htch * 0.30)
  if ink { [0.05, 0.05, 0.07] } else { col }
}

pub schedule woodblock(w: World, shadow_w: World, cam: Camera) {
  cull_indirect(w, cam)
  find_vis(w, cam, 4)
  tile_classify(VisBuf, 8)
  shade_tiles(VisBuf, woodblock_shade(w, shadow_w))
  outline_and_paper(PrintBuf, VisBuf, DepthBuf, [0.05, 0.05, 0.07], 1.5, 0.04, 0.03)
}

```

### 1.4 `engines/roam.loft` & `engines/act.loft` (Motion Engines)

```loft
# engines/roam.loft — Spore-like adaptive locomotion over slice-indexed BODY/STATE/POSE
use std::motion::*

pub fn default_turn(yaw: rad, lim: rad, tc: s, latch: bool) -> { rate: rad/s, latch: bool } {
  let r = abs(yaw) / max(lim, 0.1rad)
  let next_latch = if latch { r > 0.25 } else { r >= 0.85 }
  let rate = if next_latch { sign(yaw) * 3.8rad/s * clamp(r - 0.25, 0, 1) } else { 0rad/s }
  { rate: rate, latch: next_latch }
}

pub law roam_step(c: CreatureState, dt, bodies: [BodyRow], chains: [ChainRow], surfs: [SurfaceRow], inp: RoamInput)
  writes c.{trunk, feet, gaze}
{
  let b = bodies[c.body_id]
  if inp.hit.active { apply_impulse(c.trunk, b, inp.hit) }
  let g_eff = [0, -9.81, 0]@World - mean_pin_accel(c.feet, surfs)
  let hull = support_hull(c.feet, g_eff)
  let a_want = (inp.go - c.trunk.vel) / (b.tc * 0.5)
  let a_clmp = clamp_accel_support(c.trunk, hull, a_want, b, g_eff)
  let p_star = c.trunk.pos - a_clmp * (b.rest_h / 9.81)
  let shares = load_shares_nnls(c.feet, p_star)
  let cp = capture_point_ff(c.trunk, b.rest_h, a_clmp, mean_pin_vel(c.feet, surfs), mean_pin_accel(c.feet, surfs), g_eff)
  step_feet_and_slips(c.feet, chains, b, c.trunk, cp, p_star, shares, g_eff, dt)
  if inp.has_look { step_gaze_and_handoff(c.gaze, c.trunk, b, hull, inp.look, default_turn) }
  integrate_trunk_and_leash(c.trunk, c.feet, chains, a_clmp, dt, 4)
  if support_margin(hull, cp) > 0m { c.trunk.composure = saturate(c.trunk.composure + 0.55 * (dt / 1s)) }
}

# engines/act.loft — Disney-principled performance (composes roam + compliance-coupled priors + squash)
pub law act_step(c: CreatureState, dt, bodies: [BodyRow], chains: [ChainRow], surfs: [SurfaceRow], score: BeatScore, inp: RoamInput)
  writes c.{trunk, feet, gaze, manips}
{
  roam_step(c, dt, bodies, chains, surfs, inp)
  let alpha_t = score.trunk.compliance / max(c.trunk.composure * c.trunk.composure, 0.002)
  for it in 0..6 {
    solve_prior_xpbd(c.trunk, score.trunk, if c.trunk.composure < 0.15 { 1e6 } else { alpha_t })
    solve_reach_leash(c.trunk, c.feet, chains, 0)
  }
  apply_end_beats(c.manips, score.ends)
  step_volume_squash(c.trunk, dt, 3.2, 0.42)
}

```

## 2. Three Reference Scenes (Unified E+ v1.0)

### 2.1 Scene 1: Hermit Crab Boss (Hit 1 + Slip + Chained Hit 2)

```loft
use std::geom::*
use std::light::*
use std::motion::*
use engines::act::act_step
use engines::realtime::realtime

space World Body Shell Claw Face Leg

fn chitin(p, n) =
  ggx([0.68, 0.14, 0.07], 0.20 + 0.22 * grad_noise3(coords(p) * 6.5), 0.05)
  + sss([0.9, 0.25, 0.08], 0.08m) + coat(0.88, 0.06)

fn spine(t: rad) = { let e = exp(0.18 * (t / 1rad)); [e * 0.95 * cos(t), e * 0.95 * sin(t), -e * 0.72]@Shell * 1m }
scan whorl t in 0rad..28rad by 256 on bake_lut {
  init = frame(spine(t), dir(spine'(t)), [0, 0, 1])
  step prev = transport(prev, spine(t), dir(spine'(t)))
}

kind HermitShell @Shell (body: BossBody, pose: xf@Shell>World, aggro: f in 0..1) {
  fn macro_s(t: rad, a: rad) = {
    let f = whorl(t)
    let s = exp(0.18 * (t / 1rad)) * 0.62m
    let rib = 0.08 * pow(0.5 + 0.5 * cos(6 * t), 4) * (0.4 + 0.6 * sin(5 * a))
    f.o + s * (1.15 + rib) * cos(a) * f.n + s * (0.82 + rib) * sin(a) * f.b
  }
  fn barn_h(t: rad, a: rad) = 0.035m * smooth_voronoi_cone([t / 1rad * 2.1, a / 1rad * 3.0, 0], 0.12)
  surface(u, v) grid 96x32 = {
    let t = 28rad * u; let a = 6.2831853rad * v
    let p0 = macro_s(t, a); let n0 = dir(cross(diff(macro_s, t)(t, a), diff(macro_s, a)(t, a)))
    p0 + n0 * barn_h(t, a)
  }
  material(p, n) = {
    let grime = saturate(0.4 + 0.35 * grad_noise3(coords(p) * 4))
    ggx(mix([0.62, 0.48, 0.36], [0.09, 0.21, 0.14], grime), mix(0.28, 0.78, grime), 0)
    + coat(1 - 0.85 * grime, 0.08) + film(0.25 * (1 - grime), 1.42, 380)
  }
}

kind CrusherClaw @Claw (pose: xf@Claw>World, open: rad, scale: f in 0.3..1.5, aggro: f in 0..1) {
  trace(p) = {
    let q = p / scale
    let palm = ssub(round_box(q, [0.65m, 0.35m, 0.95m], 0.28m), box(q - [0.1m, -0.35m, 0.2m], [0.7m, 0.3m, 0.9m]), 0.06m)
    map ring = q |> repeat_polar(7)
    let studs = sphere(ring.p - ring.pt(0.75m, 0, 0), 0.11m)
    map jaw = q |> move([0, 0.22m, -0.85m]) |> rot_x(-open)
    let teeth = 0.045m * sin(26 * (jaw.p.z / 1m)) * smootherstep(0.1m, 0.9m, jaw.p.z)
    let dactyl = capsule(jaw.p, jaw.pt(0, 0, 0), jaw.pt(0, -0.15m, 1.15m), 0.24m) - teeth
    scale * smin(smin(palm, dactyl, 0.08m), studs, 0.04m)
  }
  material(p, n) = {
    let veins = smootherstep(0.62, 0.78, smooth_voronoi3(coords(p) * 5.2)) * aggro
    chitin(p, n) + emit_lobe([1.0, 0.32, 0.04] * veins * (3.5 + 1.5 * sin(8 * (time / 1s))))
  }
}

kind CrabFace @Face (pose: xf@Face>World, aggro: f in 0..1, gaze: target@World) {
  map sym(p) = p |> mirror_x |> move([-0.42m, -0.55m, -1.18m])
  trace(p) = {
    let sway = 0.08m * sin(2.1 * (time / 1s))
    let eye_c = sym.pt(sway, 0.24m, 0.16m)
    map brow = sym.p |> move([0, -0.28m, -0.05m]) |> rot_z(-0.35rad * aggro) |> rot_x(0.28rad * aggro)
    smin(smin(capsule(sym.p, sym.pt(0, -0.2m, 0), eye_c, 0.09m), sphere(sym.p - eye_c, 0.135m), 0.05m),
         wedge(brow.p, [0.32m, 0.10m, 0.38m]), 0.03m)
  }
  material(p, n) = {
    let eye_c = sym.pt(0.08m * sin(2.1 * (time / 1s)), 0.24m, 0.16m)
    let eye_w = pose * sym.out(eye_c)
    let look = sym.in(inv(pose) * dir(gaze.pos - eye_w))
    let is_pupil = dist(sym.p, eye_c) < 0.14m && dot(sym.in(n), look) > 0.90
    if is_pupil { ggx([0.02, 0.02, 0.02], 0.08, 0) + emit_lobe([1.0, 0.75, 0.10] * (1.2 + 3 * aggro)) }
    else { chitin(p, n) }
  }
}

kind CrabLeg @Leg (body: BossBody, chain_idx: u, r0: m, r1: m) {
  visible(ro, rd) = pose_chain_hit(body, chain_idx, ro, rd, r0, r1)
  material(p, n) = chitin(p, n)
}

body BossBody {
  trunk { mass: 420kg, inertia: [180, 240, 190], rest_h: 1.35m, mu: 0.65, strength: 2.2 }
  build for i in 0..8 {
    let s = if i >= 4 { 1 } else { -1 }; let z = (-0.75 + 0.5 * (i % 4)) * 1.4m
    chain leg[i]: support { socket: [s * 0.95m, -0.1m, z]@Body, lens: [0.55m, 1.25m, 1.35m, 0m],
                            rest: [s * 2.3m, -1.35m, z * 1.3]@Body, mu_s: 0.65, mu_k: 0.35 }
  }
  chain clawR: manip { socket: [0.8m, 0.1m, 1.1m]@Body, lens: [0.95m, 1.15m, 0m, 0m] }
}

state BossAnt checkpoint 60 { pos: array[p3@World, 64], prev: array[p3@World, 64] }

solver BossAntennae over BossAnt writes BossAnt.{pos, prev}
  config { strategy: colored_gs(2), substeps: 4, iters: 6, shared_mem: true }
{
  let root_w = pose_joint(body_id, 0).pos + [0.25m, 0.48m, 1.25m]@World
  pos[0] = root_w; prev[0] = root_w
  for i in 1..16 {
    let cur = [sin(time / 1s + 0.4 * i), 0.2, cos(0.7 * (time / 1s))]@World * 1.4
    let v = (pos[i] - prev[i]) / dt; prev[i] = pos[i]; pos[i] = pos[i] + (v * 0.96 + cur * dt) * dt
  }
  constrain edge(i, j) in 0..15 { solve_dist_edge(pos[i], pos[j], if i == 0 { 0 } else { 5 }, 5, 0.18m, 0.002) }
}

check boss_truth(c: CreatureState, surfs: [SurfaceRow]) -> m {
  for i in 0..8 { if c.feet[i].mode == 0 { yield check_pin_drift(pose_end(c.body_id, i), c.feet[i].pin, surfs, 0.5mm) } }
  yield check_posture(c.body_id, c.trunk)
}

build {
  emit Plane(n: [0, 1, 0]@World, d: 0m)
  emit Sun(dir: dir([-0.4, 0.85, 0.35]@World), color: [2.8, 2.6, 2.3])
  emit Sky(zenith: [0.35, 0.56, 0.88], horizon: [0.82, 0.86, 0.90])
  build for c in 0..1 {
    let bid = emit BossBody(origin: [c * 6m, 0m, 0m]@World)
    emit HermitShell(body: bid, pose: xform_id(), aggro: 0.85)
    emit CrusherClaw(pose: place([0.8m, 0.6m, 1.1m]@World), open: 0.45rad, scale: 1.0, aggro: 0.85)
    emit CrusherClaw(pose: place([-0.8m, 0.5m, 1.0m]@World), open: 0.20rad, scale: 0.62, aggro: 0.85)
    emit CrabFace(pose: xform_id(), aggro: 0.85, gaze: [0, 1.8m, 8m]@World)
    build for l in 0..8 { emit CrabLeg(body: bid, chain_idx: l, r0: 0.16m, r1: 0.05m) }
  }
}

schedule Main {
  act_step(hit_events: [hit(0.50s, [0, 1.1m, 0]@World, [-1800, 0, -900]),
                        hit(0.625s, [0.5m, 1.2m, 0]@World, [-1400, -300, -600])])
  run_solver(BossAntennae)
  check(boss_truth)
  realtime(SceneWorld, MainCam, SunCam)
}

```

### 2.2 Scene 2: Heron Hunting (Effort+Jerk Plan, Gaze Lock, Neck Limit → Hop)

```loft
use std::geom::*
use std::motion::*
use engines::act::act_step
use engines::realtime::realtime

space World Body

body HeronBody {
  trunk { mass: 2.2kg, inertia: [0.18, 0.12, 0.22], rest_h: 0.85m, mu: 0.9, strength: 2.02 }
  build for i in 0..2 {
    let s = if i == 1 { 1 } else { -1 }
    chain leg[i]: support { socket: [s * 0.18m, -0.15m, -0.08m]@Body, lens: [0.45m, 0.48m, 0m, 0m],
                            rest: [s * 0.18m, -0.85m, 0m]@Body, mu_s: 0.9, mu_k: 0.6 }
  }
  chain neck: gaze { socket: [0, 0.18m, 0.24m]@Body, lens: [1.55m, 0m, 0m, 0m], yaw_lim: [-1.9rad, 1.9rad] }
}

plan HeronHop(keys: PriorSpline, rail: Box, block: SurfaceRow) -> PriorTable {
  minimize integral(1.0 * sq_norm(acc - [0, -9.81, 0]) + 1e-7 * sq_norm(jerk) + 8.0 * sq_norm(pos - keys.pos))
  subject_to { clear(rail, 0.08m); land_on(block); pitch_in(-15deg, 15deg); flight_ballistic() }
}

fn fish_at(t: s) -> p3@World = [2.4m * cos(1.0rad/s * t), 0.05m, 2.4m * sin(1.0rad/s * t)]@World

check heron_truth(c: CreatureState, surfs: [SurfaceRow]) -> m {
  for i in 0..2 { if c.feet[i].mode == 0 { yield check_pin_drift(pose_end(c.body_id, i), c.feet[i].pin, surfs, 0.5mm) } }
  yield check_s_neck_length(c.body_id, 1.55m, 0.2mm)
  yield check_posture(c.body_id, c.trunk)
}

check heron_gaze(c: CreatureState) -> rad {
  check_gaze_honest(c.body_id, fish_at(time), c.gaze, !any_planted(c.feet))
}

schedule Main {
  act_step(prior: HeronHop, look: fish_at(time))
  check(heron_truth, heron_gaze)
  realtime(HeronWorld, MainCam, SunCam)
}

```

### 2.3 Scene 3: Non-PBR Coral Shrine (Woodblock + Dual Shadow World)

```loft
use std::geom::*
use std::stylised::*
use engines::woodblock::woodblock

space World Shrine

kind FoldedLantern @Shrine (pose: xf@Shrine>World) {
  trace(p) = {
    map k = p |> fold_kifs(4, 1.45, [0.42m, 0.28m, 0.35m], rot_y(0.55rad))
    max(round_box(k.p, [0.32m, 0.45m, 0.32m], 0.04m) * k.stretch, -cylinder(p, 0.28m, 1.2m))
  }
  material(p, n) = emit_lobe(mix([0.86, 0.24, 0.14], [0.95, 0.88, 0.72], smootherstep(0m, 1.8m, p.y)))
}

kind CoralBranch @World (a: p3@World, b: p3@World, ra: m, rb: m, level: f) {
  visible(ro, rd) = ray_round_cone(ro, rd, a, b, ra, rb)
  material(p, n) = emit_lobe(mix([0.14, 0.26, 0.44], [0.88, 0.32, 0.22], level))
}

world PrimaryWorld { things: [Plane, FoldedLantern, CoralBranch] }
world SolidShadow  { things: [Plane, SolidLanternProxy, CoralBranch] }

schedule Main { woodblock(PrimaryWorld, SolidShadow, MainCam) }

```

---

# FILE 7: `docs/spec/05_NUMBERS_AND_BUILD_ORDER.md`

## 1. Predicted Performance (1280×800, Quiet Apple M4)

**Measured calibration anchors:** `d = 15.3 µs`/cmd (**measured** 229-scene fit); serial Metal pipeline compile `~70 ms`/pipeline (**measured** Phase 1); `0.070 / 0.101 / 0.086 ms` p95 for 1 / 256 / 1,000 fused crabs (**measured**); `3.08 / 0.72 ms` close/far for 3,069 analytic coral branches (**measured**); Phase 1 crab render `8` modules, `13` pipelines, `17` commands, `0.99–1.08 s` cold load (**measured**).

| Scene | Modules | Pipelines | Cmds/Frame | Cold / Warm Load | GPU p50 / p95 (Native) | Motion p95 (1 / 1,000) | Assumptions |
| --- | --- | --- | --- | --- | --- | --- | --- |
| **Hermit Boss (1 / 20)** | `12` / `12` | `15` / `15` | `19` / `19` | `1.15 s` / `0.06 s` *(cached deriv)* | `2.5 / 2.9 ms` (`1`), `4.5 / 5.2 ms` (`20`) | `0.14 ms` / `0.42 ms` *(2,368 B state)* | 1 crab: `7.2k` shell + `3.8k` leg tris, `140k` SDF px (`L=2.28`/`1.0`), `256k` fg px. 20 crabs: `140k` vis tris, `350k` SDF px, `665k` fg px. **predicted** |
| **Heron Hunting (1)** | `10` | `12` | `14` | `0.85 s` / `0.05 s` | `1.9 / 2.2 ms` | `0.08 ms` / `0.17 ms` *(352 B state)* | `5.6k` tris + `18k` head SDF px + `120k` fg px; `plan` CPU solve `2.8 ms`, GPU 600-tick bake `0.7 ms`. **predicted** |
| **Coral Shrine (Close / Far)** | `8` / `8` | `9` / `9` | `10` / `10` | `0.62 s` / `0.04 s` | `3.4 / 3.9 ms` / `1.6 / 1.9 ms` | `0.00 ms` *(static)* | `3,069` analytic cones (`Bvh2`, `310k` close px) + `95k` KIFS lantern px + dual shadow world. **predicted** |

*(Browser WebGPU adds `~0.6 ms` Dawn encoder floor and `~0.25 s` cold load; identical module/pipeline/command counts.)*

## 2. Build Order (From Current Phase 1 State on `meta/phase1`)

| Phase | Vertical Slice Deliverable | Pass / Fail Gates (Predicted) | Code Delta (LOC) | Deletes |
| --- | --- | --- | --- | --- |
| **1.5. E+ Parser Swap** *(days 1–2)* | Replace Syntax A parser with v1.0 E+ (`crates/loft-parse`, `loft.lark`, `sexp.rs`); add `let`/`var`, kind-scoped `map`, range refinements, field-level `writes`, space-polymorphic helpers, WGSL keyword mangling, disk derivation cache. Port Phase 1 `lib/` (`282` LOC) + `blobs`/`capsules`/`hermit`. | All Phase 1 gates green (`18/18` Pictures, `36/36` Twin after claw tolerance/fix, `33/33` Load, `34/34` Truth); warm load drops `230 ms` → `≤ 60 ms`; `hermit.loft` tokens drop `−35%`; `loft.lark` validates all scenes. | Parse `1,390` (replaces A), Passes `+450`, Lib `280` | Syntax A parser |
| **2. Interactive Edit** *(days 3–4)* | Tier 0 56-op kind-bucketed Tape VM (`32` vec4f regs), async Tier 1 promotion + LRU cache, `loft show` with E+ spans, full `E101`–`E503` diagnostic snapshots. | Dirty-kind hot-reload `≤ 2 ms` (`0 ms` Metal compile); dirty-kind overhead `≤ 0.5 ms`; Tier 0↔1 diff `≤ 1/255` on `≥ 99.9%` px. | Runner `+150`, Passes `+1,800` | Legacy TS `wgsl` string parser |
| **3. Motion Sub-Engine** *(days 5–8)* | `body`, `state checkpoint`, `law` (with field-disjoint fusion), `solver` (`sym_gs`, `colored_gs(2)`), `plan` B-spline solver, `check` `VerdictRecord` buffer, `std::motion`, `roam`, `act`. Full Crab Scramble, Heron Gaze+Hop, Boss Hit 1 + Slip + Hit 2. | Crab: `0/481` bit diff (30/60/120 Hz + 6-tick stall), reversal lift `≤ 0.10 s`, toe drift `≤ 0.05 mm`, fake-foot `20 mm` caught, `1,000` crabs `≤ 0.15 ms`. Heron: crouch `0.59 m`, pitch `≤ 12°`, neck err `≤ 0.2 mm`, `0` rear flips. Boss: `0.69 m` windup, slip + Hit 2 deterministic. | IR `+300`, Passes `+1,800`, Std+Engines `2,400` | Legacy TS motion (`src/motion/*`, `gait`, `stance`, `lively`, `pendulum`) |
| **4. Light & Reference** *(days 9–11)* | `reference` path tracer, `Bvh2` world accel, joint `(L, T)` `inscatter_joint`, ReSTIR DI, full `Bsdf` (`clearcoat`/`sheen`/`film`/`transmission`/`sss` + Kulla–Conty LUT), viewer temporal reprojection, `woodblock` + post. | Closed furnace (`1.50E`, `1.75E`, `2.00E` `± 1%`); white furnace `0.50 ± 2%`; coincident red/blue media `(0.375, 0, 0.375)` both orders (`1e-4`); `1,000` lights bias `≤ 1%`; Coral Shrine close `p95 ≤ 4.0 ms`. | IR `+100`, Passes `+700`, Std+Engines `2,200` | Legacy `src/soft/*`, `src/style/*` |
| **5. Interop, Corpus & Embed** *(days 12–16)* | `import_gltf` / `export_gltf`, `retarget` + live `PosePacket` (MotionBricks), hardware ray-query backend where exposed, Keel / room-runners embedding, Codex fan-out regenerating 295 corpus scenes on R0g + Speed gates. | `100k`-tri BVH build `≤ 0.1 s`; retargeted walk drift `≤ 0.1 mm`; glTF export err `≤ 1 mm`; `100%` R0g corpus pass; training `512² × 16` `≤ 50 ms`/frame & `≤ 2 s` CPU build; Keel zero caller clears/readbacks. | Passes `+400`, Std `1,800`, Tools `800` | Entire `legacy/2026-11-19` TS tree (`47k` LOC) & abandoned Rust port (`19k` LOC) |

## 3. Open Questions (With Recommended Defaults)

1. **Multi-hit transparency (`glass` / nested refractive surfaces):** Trace secondary rays inside `shade_tiles` (exact, zero sorting) vs. separate forward pass. **Default:** Trace inside `shade_tiles` for `transmission` lobes; optional forward pass in `realtime` for single-layer `SeaSurface`/`Specks`.
2. **Segment tracing vs. global kind Lipschitz:** Teeth raise claw $L$ from `1.0` to `2.28`. **Default:** Global derived $L$ in Phases 1–3; add opt-in `segment_trace` pass in Phase 4 only if SDF trace exceeds `1.5 ms` quiet M4.
3. **Bipedal walking stability on arbitrary editor-built 2-leg creatures:** **Default:** Use `roam`'s dynamic capture-point fallback (`Point`/`Flight` regime) in Phase 3 and score explicitly on Phase 5's 12-body × 6-motion Spore matrix before locking defaults.
4. **Cross-creature physical contacts (20 colliding crabs):** **Default:** Keep `roam`/`act` row-local in Phase 3; add optional Wave-1.5 spatial-hash broadphase + Jacobi impulse pass in `std::motion::crowd` when multi-creature collisions are needed.
5. **WebGPU `f16` unlock:** **Default:** Stay strictly `f32` through Phase 4; add `f16` emitter pass only when a corpus scene proves `≥ 15%` frame-time win within Twin/Tier tolerances.

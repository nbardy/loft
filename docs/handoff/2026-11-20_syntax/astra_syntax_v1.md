# Loft v1.0 surface syntax

## Decisions

Use E's expression syntax, A's visible build boundary and G's field ownership. A reader should be able to point to the loop that creates legs, the field that changes their shape, and the transformation that gives each eye its own origin.

**Reading the numbers.** Tables mark observations from files or named instruments **measured** and design estimates **predicted**. Numeric literals in proposed code, grammar examples and proposed limits are **predicted design inputs**. Version, tier, diagnostic, type and tokenizer names are identifiers. The programs are specifications for a proposed front end.

| Question | Decision and reason | Excludes |
|---|---|---|
| Superset | Keep WGSL as a typed escape and emission target. Native formula spelling survives, while the common path gains contextual literals and tail expressions. | Whole WGSL files need a raw entry or a port. A strict superset would retain WGSL's existing parses and meanings, including numeric overload and conversion rules. |
| Family | Start with E. It gave the best compromise between complete scenes and author tokens across the blind reviews. Keep explicit kinds and named row columns. | Lambda captures deciding code identity, C declarators, Python module execution and Lua block conventions. |
| Maths | Use one infix expression grammar for every stage. A pipe inserts its left operand into an ordinary call. | Prefix arithmetic and G's second arithmetic dialect inside `$`. |
| Inference | State row schemas and exported interfaces. Infer a local expression against its enclosing operation and expected result. | Inferring a schema from the first emitter, or silently picking a space or physical dimension. |
| Stage | Declare a `build` routine, mark each allocating loop `build for`, and spell every append `emit`. Imports have no execution effect. | An ordinary loop emitting code or rows and a frame clock captured during build. |
| Ownership | Each law declares its writable columns on its receiver. Link an exclusive owner for each active field before sorting passes. | Whole-row replacement and schedule order as a way to legalize overlapping writes. |
| Shared helpers | Bind a space parameter on the helper. Every call instantiates the same checked body with its caller's space. | An implicit cast from Claw to Leg to reuse a coat. |
| Compilation | Parse in Rust, resolve a spanned syntax tree, check and elaborate to typed graph, then use the existing emitters. TypeScript can send editor edits or host inputs over the data ABI. | A mandatory Lisp/Lua → TypeScript → Rust pipeline, host tracing, and diagnostics located in generated host files. |
| Machine form | Ship a deterministic, versioned, read-only graph dump and source spans. Apply source edits to the concrete syntax tree and recheck. | A second editable program format with its own authority over source. |
| Token floor | Scope the practical floor to the named prelude and the full crab obligations. Measure a readable witness and its compact spelling below. | Treating the shortest incomplete answer or A's whole-suite average as a crab floor. |

A strict WGSL superset could add shorthand wherever WGSL leaves the spelling free. Its compatibility promise constrains the meaning of every existing valid shader, including its overloads and conversions. A separate surface gives up pasting an arbitrary WGSL function straight into a Loft module. The raw boundary recovers shader reuse and hardware access at the cost of opacity to Loft's bounds, differentiation and CPU evaluation.

The following comparison uses the same fenced-code extraction for every sample. Violation pairs and semantic scores come from the fresh seven-card judge. The first score in each pair is Sonnet's. Scores are human-readable review judgments. The candidates were never compiled.

| Family | Suite tokens, mean (measured) | Violations, S / H (measured) | Semantic checks passed out of four, S / H (measured) | Source detail that affects the decision |
|---|---:|---:|---:|---|
| A · declarations | 6,320.5 | 2 / 2 | 4 / 4 | Both models marked row loops correctly. Haiku put a mirror at module scope with a free query point. |
| B · builders | 6,751 | 0 / 3 | 4 / 3 | Haiku calculated `time()` in the scene builder and constructed the eye from an unbound point. |
| C · HLSL | 6,422.5 | 1 / 2 | 4 / 4 | Sonnet's cull and draw lists omit its shell and eyes. Haiku put a mirror-local centre in a face-space variable. |
| D · Python | 6,527 | 0 / 2 | 4 / 4 | Sonnet separated trunk and head. Haiku wrote module-level calls and imported Python's habits with `random.Random`. |
| E · expressions | 4,670 | 0 / 1 | 4 / 3 | Sonnet made the feeder a different structural variant. Haiku emitted a mirrored pair twice and turned the heron toward positive yaw on either side. |
| F · Lua | 3,849.5 | 4 / 4 | 2.5 / 1.5 | Sonnet folded a transform in the builder. Haiku's shell surface was outside a kind and its wind helper used free row fields. |
| G · Lisp | 3,769.5 | 2 / 2 | 4 / 3.5 | Haiku's heron laws legally split fields. Its reed has a world root, but no placement of the local stalk at that root. Both authors switched between prefix and infix within formulas. |

| Observed defect | Affected outputs / reviewed (measured) | What the judge counted |
|---|---:|---|
| Whole-kind writer conflict | 5 / 8 | The original judge included laws whose actual field assignments were disjoint because their cards claimed the whole kind. |
| Potentially zero row divisor | 7 / 10 | The five-card judge checked whether a row value used as a denominator had a positive lower bound. |
| Literal unstaged loop | 0 / 14 | The seven-card judge accepted the loop inside D-Haiku's decorated function. Its call and row appends still sat at module scope. |
| Authored Lipschitz constant | 0 / 14 | The seven-card judge found no author asserting the spatial bound used for tracing. |

A-Sonnet and C-Sonnet independently passed claw points to a material fixed to another space. The five-card judge scored A-Sonnet's heron down because the drawn head ignores the law. That issue matters more to a scene author than a counted spelling error.

The judges grouped errors differently and counted code fences and headings differently. The five-card and seven-card rankings both selected E. G's compactness is real, but a lower token bill can also buy a missing pupil or a stationary fish. There is one output per model/card cell. The new card needs its own blind trial to estimate an authoring error rate. [Original verdict](../../../agent_notes/2026-11-20_syntax_bakeoff/VERDICT.md) · [Five-card verdict](../../../agent_notes/2026-11-20_syntax_bakeoff/VERDICT_ALL.md) · [Seven-card verdict](../../../agent_notes/2026-11-20_syntax_bakeoff/VERDICT_SEVEN.md)

The preservation target is the built declaration engine. The later meta-engine plan resolves the older CORE note about a TS recorder. Keep CORE's priority order, including picture and frame time ahead of tokens. The built crab shared identical module text at both populations. The new spelling must lower each migrated source to the same row schemas, query functions, schedule and pictures before it replaces A. [CORE](../../../CORE.md) · [Philosophy](../../PHILOSOPHY.md) · [Meta-engine plan](../../proposals/2026-11-20_meta_engine_plan.md) · [Built crab](../../../engines/hermit.loft)

| Built render-only reference (measured) | Single crab | Twenty crabs |
|---|---:|---:|
| Shader modules / pipelines / commands per frame | 8 / 13 / 17 | 8 / 13 / 17 |
| Distinct shape-bound derivations | 11 | 11 |

Those numbers belong to the built reference scene. The programs below have their own geometry and have yet to pass a render gate. [Phase-one gate and differences from the prior spec](../../../agent_notes/2026-11-20_meta/phase1.md)

## Grammar

The EBNF below uses postfix `?`, `*` and `+` for optional, repeated and nonempty repeated productions. Capitals name lexer tokens. `END` is a logical line break. The start symbol is `module`. Literal names in quoted terminals are reserved in their indicated positions.

```ebnf
module: (item (END item)*)?
item: "use" path | "pub"? decl
path: NAME ("." NAME)*
decl: "space" names | "unit" NAME ("=" type)?
    | "const" binding | "type" NAME "=" type
    | "struct" NAME formals? "(" columns? ")"
    | "enum" NAME "{" tag ("," tag)* "}"
    | routine | raw | kind | scan | law | check | solver | schedule
tag: NAME ("(" type ("," type)* ")")?
routine: ("fn" | "build" | "lobe") head body
raw: "raw" "fn" head effects? "wgsl" RAW_BLOCK
head: NAME formals? "(" params? ")" ("->" type)?
formals: "[" names "]"
params: param ("," param)*
param: NAME (":" type)?
names: NAME ("," NAME)*
type: type_atom (("*" | "/") type_atom)*
type_atom: NAME ("[" type_arg ("," type_arg)* "]")? | "(" type ")"
type_arg: type | NUMBER
kind: "kind" NAME ("[" columns "]")? "@" NAME "(" columns? ")" "{" (member (END member)*)? "}"
columns: column ("," column)*
column: NAME (":" type ("in" interval)?)?
interval: signed "..=" signed
signed: "-"? NUMBER
member: "fn" head body | query "(" params? ")" ("grid" "(" expr "," expr ")")? body
query: "trace" | "visible" | "surface" | "material" | "light" | "sky" | "medium"
scan: "scan" NAME "(" NAME "in" interval "," expr ")" "on" NAME "{"
      "init" "=" expr END "step" "(" NAME ")" "=" expr "}"
effects: "writes" "(" names ")"
law: "law" head effects block
check: "check" head block
solver: "solver" head effects "using" NAME "(" args? ")" block
schedule: "schedule" NAME block
body: "=" expr | block
block: "{" (stmt (END stmt)*)? "}"
stmt: ("let" | "var") binding | "return" expr | "break"
    | "build" "for" NAME "in" expr block | "for" NAME "in" expr block
    | expr ("=" expr)?
binding: NAME (":" type)? "=" expr
expr: pipe
pipe: range ("|>" NAME ("(" args? ")")?)*
range: disj ((".." | "..=") disj)?
disj: conj ("||" conj)*
conj: bits_or ("&&" bits_or)*
bits_or: bits_xor ("|" bits_xor)*
bits_xor: bits_and ("^" bits_and)*
bits_and: compare ("&" compare)*
compare: shift (("==" | "!=" | "<" | "<=" | ">" | ">=") shift)?
shift: sum (("<<" | ">>") sum)*
sum: product (("+" | "-") product)*
product: unary (("*" | "/" | "%") unary)*
unary: ("!" | "-" | "+") unary | postfix
postfix: atom (("." NAME) | ("(" args? ")") | ("[" expr "]"))*
atom: literal | NAME ("'" "(" expr ")")? | "(" expr ")"
    | "[" (expr ("," expr)*)? "]" ("@" NAME)?
    | conditional | match_expr | emission
conditional: "if" expr block ("else" (block | conditional))?
match_expr: "match" expr "{" arm (END arm)* "}"
arm: NAME ("(" names ")")? "=>" block
literal: NUMBER | STRING | "true" | "false"
emission: "emit" NAME ("[" expr ("," expr)* "]")? "(" named_args? ")"
args: arg ("," arg)*
arg: NAME ":" expr | expr
named_args: NAME ":" expr ("," NAME ":" expr)*
```

**Ten parsing rules**

| Rule | Deterministic choice |
|---|---|
| Newlines | Discard blank lines and `#` line comments. Emit one END between complete items, members or statements. Inside the innermost parentheses or brackets, after an opener, comma, infix operator or `=`, before a closing delimiter, before `else` or a leading `\|>`, and at file edges, discard the break. Other leading operators start a new statement if legal. Keep a header's opening brace on its line. |
| Tokens | Use longest match. `NAME` is an ASCII identifier with letters, digits and underscores, beginning with a letter or underscore. NUMBER is unsigned decimal digits, optional fractional digits following a point, optional signed exponent, then an optional adjacent unit name. A decimal point requires a digit after it. Thus `0..6` is a range. Unknown unit suffixes are errors after name resolution. Strings use escaped double quotes. |
| Item starts | Declarations and members have fixed heads. At file scope only an item can start a line. Keyword-shaped members such as `light` select the query production in a kind. A dotted import resolves a module without executing it. |
| Delimiters | Braces contain ordered items or statements as dictated by the header. Tuple, vector and fixed-sequence literals all start with the same bracket node and are classified by expected type. A suffix `@S` on a literal constructs a point. It never casts an arbitrary expression. |
| Types | Enter type grammar only in a schema, a type alias, after a colon in a declaration, or after a return arrow. Generic formals follow a declared name. At a use site square brackets after a value index it. Only `emit K[...]` supplies structural arguments. Ordinary generic function arguments are inferred. |
| Calls | At the start of a call argument, `NAME :` selects a named argument. Ordinary calls allow positional arguments followed by named arguments. Row appends require names. Duplicate/unknown arguments fail at the label. A bare callee name also denotes a function value. |
| Precedence | Postfix binds tightest, then prefix, product, sum, shifts, a single comparison, bitwise AND/XOR/OR, logical conjunction, disjunction, a single range and finally pipe. Repeated binary levels fold left. `a \|> f(b)` becomes `f(a,b)` once. A prime on a bare unary function call requests its derivative. General partials use `diff(f,x)`. |
| Bindings | `let` binds an immutable local and `var` a mutable one. After parsing a statement expression, a single trailing `=` makes it an assignment. The checker validates the local, index or owned field on the left. Assignment has no expression value. |
| Blocks | Parse a routine's `= expr` or braced body exactly once. The final expression of a braced body provides its value. Earlier expressions must be consumed or have an allowed effect. A value-producing `if` requires an `else`. A closing brace makes the attachment of `else` unique. |
| Raw text | Only a raw header followed by `wgsl` opens RAW_BLOCK. Scan WGSL tokens to the matching outer brace, skipping line and nested block comments. Keep offsets into the original buffer, then give the body and generated signature to Naga. A WGSL error maps back into that body. |

`formals` bind types/spaces. Kind-level bracketed columns bind finite structural values. Unit expressions in type positions have their own products and quotients. A loop over `a..b` excludes the upper endpoint and `a..=b` includes it. A scan over a closed interval samples the stated number of stations including both ends. Surface operations such as `diff`, `prev`, `rows` and pass composition have ordinary call syntax. A compact schedule can call these same functions as a longer schedule that explicitly names its intermediate images and buffers.

## Typing and inference

The checker carries a value type, physical dimension, coordinate space, rate/effects and analysis facts with the source span. The following judgments explain where information comes from and where inference stops.

| At the boundary | Judgment |
|---|---|
| Module | Resolve declarations before checking bodies. Public callable parameters and results are explicit, apart from query and law roles fixed by the language. Private helpers may omit uniquely determined types. Check a private body's constraints before specializing at a call. Unsolved or competing interpretations require an annotation at the binding. Imports and function references retain their definition spans. |
| Row | `kind K @S(...)` introduces a code family and a named storage schema. Write each column's type. The single abbreviation `pose` expands to `pose: Xf[S,World]`, the placement already required for a local geometry kind. A world-space query needs no placement. The local query receives the inverse-placed point exactly once. `emit K(...)` checks every column exactly once and returns `Id[K]`. The nominal row value `K`, its handle `Id[K]` and a read-only `Table[K]` have distinct types. An auxiliary law input binding other rows must name the table and index it with a matching handle. |
| Numbers | An unadorned scalar literal can adopt the unique expected dimension. A standalone number defaults to dimensionless. Fractional or floating contexts use `f`. An integral loop index stays integral until a context-checked promotion. Applying a float operation to an integer uses its numeric value. A stored scalar never acquires metres or radians by promotion. Multiplication/division add/subtract dimension exponents. In a product with a dimensioned operand a bare scalar factor defaults to dimensionless. Competing dimension assignments require suffixes. Trig consumes angles and `atan2` of like dimensions returns radians. `sqrt` halves exponents. `tau` is one turn in radians. |
| Literals | A point literal takes metre components, with nonliteral components checked as lengths. An untagged bracket literal receives a unique shape, dimension and space from both operands and the result context. In `c - [0,0,0.065]` passed to a capsule endpoint, the expected point result selects a displacement on the right. Without a unique solution, require a type such as `let off: Vec[Leg] = …`. Space labels never convert an existing point. A direction literal must have unit length. Use `dir(v)` for checked normalization. A swizzle such as `p.xyz` explicitly extracts coordinate components for numeric texture maths and preserves their units. |
| Affine maths | Same-space point minus point yields a vector. Point plus/minus displacement yields a point. Adding points, scaling an absolute point or subtracting across spaces fails. `Xf[A,B] * Pt[A]` yields `Pt[B]`. Transform vectors by the linear part, directions by the normalized linear part, and normals by the normalized inverse transpose. Frame composition unifies the intermediate space. The function `inv` reverses a nonsingular frame. Dot products require like frames. Vector equality is componentwise equality reduced to a Boolean. |
| Spaces in helpers | `fn chitin[S](p: Pt[S]) -> Bsdf` checks with a fresh abstract space. The caller may supply a leg point, claw point or witnessed eye point. Two parameters with the same `S` must agree at the call. An implementation that adds a body-space offset fails inside the generic definition. Private pure helpers can generalize a free space constrained only by point/vector operations. Public helpers spell it. Erase phantom spaces after checking. Specialization by space alone cannot mint a shader kind. |
| Measures and ranges | A row refinement is a **validated data contract**, inclusive at both ends. Check emitted values and host patches, and require every reachable law/solver store to preserve it. For an immutable column the compiler may also use the actual row values with their generation as certificate data. Accept division in a certified field when the denominator interval excludes zero. Otherwise diagnose the operator and its range. Reject nonfinite inputs to a finite range. There is no implicit epsilon or silent clamp. |
| Witnessed maps | `let e = mirror_x(p)` binds a fresh space to an actual spatial sample. `e.p` is the child point, `e.cell` the branch, `e.out(c)` its checked inverse on that branch, `e.into(v)` the forward map or differential. Pipelining domain operators composes witnesses in order. For reflection, the positive side owns the midplane. For a general warp, an inverse must be available and applicable. Normal and direction projections use the appropriate Jacobian. A query, or a pure spatial helper called on its coordinate, may create maps. Track the query-point dependency through translations, intermediate variables and helper arguments. A transform, build-time socket or free `p` cannot provide the current hit's witness. A bare child point cannot escape its binder. A scalar field composed with `e.p` retains its dependence and bound in the parent's evaluation domain. |
| Rates and identity | Build evaluates finite row generation on CPU/WASM. Build routines call other build routines or pure helpers on available data. Only `build for` iterates a build body, and only build code may append rows. The specialization stage accepts discrete structural parameters (bool, enum, or finite small integer). The existing default cap is **four used variants (measured from the cards)**. A continuous kind parameter is E401 and excess variants are E402. Query and law `for` loops remain loops in the graph. No closure, row ID, row count, placement or per-instance boolean implicitly specializes a kind. Runtime observers supply `time` at their declared sample. A law also receives `dt`. Effects propagate through helpers. A call to `target(time)` in a builder is E403 even if the callee is pure. A static `target(0s)` is legal. |
| Writes and clocks | The first law parameter names its row kind. `writes(…)` names top-level receiver columns. Subfields, array elements and aliases belong to the same column. Reject stores through read-only tables, escaping writable handles, overlapping active law/solver effects, and undeclared raw writes. Validate over the assembled schedule including imported laws. A single law reads a stable input row and commits its field patch atomically. Use locals for intermediate updates. Disjoint later laws see earlier published fields. `prev(h)` explicitly reads the preceding tick, initialized by the build row. A dependency cycle without such a delay fails. Checks read the final version used by drawing. Duplicate writer syntax can be typed into a file but cannot produce an executable plan. |
| Solvers and scheduling | `solver relax(r: Strand) writes(p) using colored(2) { … }` grants writes within the solver's declared region after proving same-color destinations disjoint. `jacobi(…)` reads a stable snapshot and applies a deterministic gather. The same ownership and hazard checks apply when a schedule calls a custom pass. The compiler assigns dependency waves, buckets work by law/kind, and derives indirect counts. `schedule` orders observations, tick dependencies, passes and resources. A callable library recipe can be replaced by its constituent passes. |
| Grades and bounds | Derive regularity per independent variable. Composition takes the weakest admitted grade. `min` and `abs` are generally continuous at a crease. A discrete branch may be discontinuous. `f'(u)` means `diff(f,formal)(u)` for a unary function. Differentiating consumes one guaranteed order, with infinite smoothness stable. A second derivative needs C². The operator `subgrad` explicitly requests a one-sided/piecewise convention. A baked scan initializes its first station and transports from each predecessor through the final station. A call to `sample_c2` uses a reconstruction with certified knot continuity, implemented in the built slice with a uniform cubic B-spline. An animated spine must schedule a new scan after its dependencies. |
| Geometry and truth | In `trace(p)`, a metre-valued tail defines an implicit in the local frame. Derive its extent and spatial slope from primitive contracts and interval analysis. `Unbounded` carries the first failing operation. Exact consumers need a certified replacement or return a named unknown. A step cap and sampling alone cannot certify a hit. Raw WGSL executes as a validated GPU shader and is opaque to the CPU twin, derivatives and bounds. Give raw entries explicit argument, result and resource-access schemas. An edited eligible render kind alone can enter Tier 0. Laws and solvers stay native. |
| Queries and materials | Keep the six world readings `trace, visible, material, light, sky, medium`. The A/phase-one spelling `trace(p)` supplies the local field and `visible(o,d)` supplies the local analytic hit. A `surface(u,v) grid(…)` member is a raster geometry provider for visibility. It supplies positions in the kind's own space. World closest-hit/transmittance observers use the appropriate providers. The record stays kind/row/reconstruction payload plus depth. The surface syntax never fixes a material layout in that record. Ordered `Bsdf` addition layers bottom first. A source-defined `lobe` contributes its factor and through-layer weight under the library ABI. An engine may replace the estimator or use flat pigments. |
| Verdicts | `check` requires a dimensioned `Verdict[U]`. `held` takes nonnegative slack, `missed` a positive violation in the same units, and `unknown` a reason. Nonfinite operations, unavailable sites and arithmetic too uncertain to decide yield named unknowns. Attach site, time/domain, source and pose generation to evidence. Positive acceptance requires Held. A sampled verdict only covers the named samples. |

The rules above retain the draft's distinctions between rates, geometry and evidence. The proposed surface adds local constraints and a field-level effect to the built engine's named-kind model. They are design contracts for the Rust checker. [Language draft](../../../agent_notes/dump_manual_nov_19th_alpha_v1.md) · [Prior specification](../2026-11-20_spec/astra_spec_v1.md)

## The bakeoff programs

The crab floor assumes the specified prelude, meaningful names, explicit ownership of the scene's geometry and the mirror conversion visible in source. The first verdict's rounded A figure refers to the suite. Comparing crab with crab gives the following reduction.

| Workload | A Sonnet (measured tokens) | A Haiku (measured tokens) | A mean (measured tokens) | Proposed source (measured tokens) | Reduction from A mean (measured) |
|---|---:|---:|---:|---:|---:|
| Crab | 2,662 | 3,062 | 2,862 | **1,500** | **47.6%** |
| Reed bed | 1,092 | 1,279 | 1,185.5 | **678** | 42.8% |
| Heron | 1,997 | 2,549 | 2,273 | **1,352** | 40.5% |
| Suite | 5,751 | 6,890 | **6,320.5** | **3,530** | **44.1%** |

| Crab floor and its basis | Tokens | Status |
|---|---:|---|
| Compact spelling of the complete crab | 1,333 | **Measured** after removing blank lines, indentation and spaces adjacent to punctuation. The sequence of lexical tokens is identical. This gives an achievable upper bound on the minimum. |
| Practical crab floor | **≈1,300** | **Predicted**, allowing modest factoring of the readable source with the same prelude. |
| Shell / claw / body with eyes | 271 / 234 / 320 | **Measured** in the readable spelling. Shortening a pass name cannot eliminate these algorithms. |

A library function that already implements the crab would change the question. There is no lower bound independent of the vocabulary.

**Counting convention.** Run the following from the repository root. Count each complete scene from its first source byte through its final newline with `o200k_base`, retaining comments and indentation in the contestants' programs. Exclude headings and fence delimiters. The measured A results reproduce the original verdict. Compare lengths as authoring costs. The fair semantic comparison still requires an accepted picture and checks. [Task text](../../../agent_notes/2026-11-20_syntax_bakeoff/TASKS.md)

```sh
uv run --with tiktoken python - <<'COUNT'
from pathlib import Path
import re
import tiktoken

enc = tiktoken.get_encoding("o200k_base")
here = Path("docs/handoff/2026-11-20_syntax/astra_syntax_v1.md")
contest = Path("agent_notes/2026-11-20_syntax_bakeoff")
for path in [*sorted(contest.glob("out_*.md")), here]:
    language = "loft" if path == here else r"[^\n]*"
    fence = chr(96) * 3
    pattern = rf"^{fence}{language}\n(.*?)^{fence}[ \t]*$"
    bodies = re.findall(pattern, path.read_text(), re.M | re.S)
    assert len(bodies) == 3, path
    counts = [len(enc.encode(body)) for body in bodies]
    print(path.name, *counts, sum(counts))
    if path == here:
        lines = [line.strip() for line in bodies[0].splitlines() if line.strip()]
        compact = "\n".join(re.sub(r"(?<!\w) +| +(?!\w)", "", line) for line in lines) + "\n"
        print("compact crab", len(enc.encode(compact)))
COUNT
```

**The prelude on the proposed card.** Each block below is a separate complete file importing `std.scene`. This is the proposed public library surface. The wind, claw hinge, sockets, fish path and hop controller are defined in the scenes. These library contracts account for the names that otherwise went missing in the bakeoff.

| Names in scope | Contract used here |
|---|---|
| `rigid(origin,yaw)`, `inv`, `dir`, `angle`, `tau` | A rigid placement maps the local origin to the supplied point and turns local +z toward +x for positive yaw. The result context fixes its source space. Normalize a nonzero vector with `dir`. For unit vectors `angle(a,b)` computes the unsigned angle with `atan2(length(cross(a,b)),dot(a,b))`. |
| `box`, `round_box`, `capsule`, `round_cone`, `ray_capsule`, `hit_min` | Local box around the origin/offset with half-extents, rounded box, segment distance minus radius, and exact tapered capsule distance. The ray functions combine nearest valid hits with the miss case handled explicitly. Obtain the hit normal from the selected analytic surface. Units and primitive bounds travel with the signatures. |
| `mirror_x`, `translate`, `rot_x`, `frame_from_tangent`, `transport_frame`, `sample_c2` | Translating a sample adds the offset in the current chart. Chained mappings retain the original parent and witnessed inverse. Frame members are `origin, tangent, normal, binormal`. Scan initialization projects the up hint off the tangent. Each subsequent station transports the prior basis. |
| `rnd(index,stream)`, `fbm2`, `ggx`, `sss`, `coat`, `emission` | Use the existing deterministic integer noise hash for independent build streams, with `rnd(i,k) = hash3(i,k,0)`. The existing value noise returns a unitless sample. The lobes take linear pigment, dimensionless roughness/weight, and a length for the subsurface radius. |
| `rows`, `run`, `observe` | Select the once-per-build row entry, ordered fixed-tick laws, and final-pose checks respectively. All operate on the same built row tables. |
| `look_at`, `sun`, `render`, `sky_dome`, `daylight`, `aces`, `linear` | Camera arguments are world eye, world target, up and vertical field of view. A sun carries a world direction **toward** the light and its RGB radiance. The library recipe `render(camera,sun,sky,shade,display)` enumerates reachable drawable kinds, culls, records Vis64/depth, evaluates posed sun visibility, classifies tiles, shades and presents. `sky(d)` handles misses. `shade(hit,sun,visibility)` returns linear RGB. `Hit` carries the world normal, the kind tag and the material reading. `daylight` evaluates the layered response with sun and sky, and `emitted(hit)` reads its emitted pigment. `linear` preserves print colors. All of these passes are replaceable in source. |

The render recipe removes draw-list repetition. The measured savings include that library choice as well as the syntax change. A future matched-library trial can separate their contributions. The built sources provide the corresponding frames, analytic hits, SDFs, lobes, noise and light evaluation. [Frames](../../../lib/frames.loft) · [Analytic geometry](../../../lib/analytic.loft) · [SDFs](../../../lib/sdf.loft) · [Lobes](../../../lib/bsdf.loft) · [Noise](../../../lib/noise.loft) · [Lighting](../../../lib/light.loft)

### Hermit crab

```loft
use std.scene
space Body, Shell, Claw, Leg

fn chitin[S](p: Pt[S]) -> Bsdf {
  let rough = 0.23 + 0.16 * fbm2(p.xyz / 0.003m)
  ggx([0.76,0.19,0.05], rough, 0) + sss([0.94,0.34,0.12], 0.008m) + coat(0.8, 0.06)
}

fn spine(u: f) -> Pt[Shell] {
  let r = 0.12m * exp(2.5 * (u - 1))
  let a = 16rad * u
  [r*cos(a), 1.2 * (0.12m-r), r*sin(a)]
}
scan whorl(u in 0..=1, 256) on bake_lut {
  init = frame_from_tangent(spine(u), dir(spine'(u)), [0,1,0])
  step(prev) = transport_frame(prev, spine(u), dir(spine'(u)))
}
kind Shell @Shell(pose) {
  surface(u,v) grid(192,32) {
    let fr = sample_c2(whorl,u)
    let a = tau * v
    let r = 0.065m * exp(2.5 * (u-1)) * (1 + 0.08*cos(28*tau*u))
    fr.origin + fr.normal * (r*cos(a)) + fr.binormal * (r*sin(a))
  }
  material(p,n) = ggx([0.69,0.48,0.31], 0.43, 0) + coat(0.4, 0.13)
}

kind Claw @Claw(pose, size: m in 0.02m..=0.12m, gape: rad) {
  trace(p) {
    let k = size
    let palm = round_box(p, [0.58*k,0.34*k,0.65*k], 0.18*k)
    let fixed = capsule(p, [0,-0.18*k,0.45*k], [0,-0.13*k,1.9*k], 0.13*k)
    let jaw = p |> translate([0,-0.13*k,-0.48*k]) |> rot_x(gape)
    let z = jaw.p.z / k
    let teeth = 0.042*k * sin(32rad*z) * smoothstep(0.1,0.5,z)
    let moving = capsule(jaw.p, [0,0,0], [0,0,1.4*k], 0.13*k) - teeth
    min(palm, min(fixed, moving))
  }
  material(p,n) = chitin(p)
}

kind Leg @Leg(pose, femur: m, tibia: m in 0.10m..=0.18m) {
  fn knee() -> Pt[Leg] = [femur,0,0]
  fn foot() -> Pt[Leg] = [femur + sqrt(tibia*tibia - 0.07m*0.07m),-0.07m,0]
  visible(o,d) = hit_min(ray_capsule(o,d,[0,0,0],knee(),0.011m),
                         ray_capsule(o,d,knee(),foot(),0.006m))
  material(p,n) = chitin(p)
}

fn target(t: s) -> Pt[World] = [0.3m*sin(0.32rad*t/1s),0.18,0.8]
kind Crab @Body(pose) {
  trace(p) {
    let e = mirror_x(p)
    let c = [0.043,0.072,0.088]@e
    let stalk = capsule(e.p, [0.036,0.018,0.064], c, 0.007m)
    let globe = length(e.p-c) - 0.015m
    min(round_box(p,[0.061,0.035,0.078],0.018m), min(stalk,globe))
  }
  material(p,n) {
    let e = mirror_x(p)
    let c = [0.043,0.072,0.088]@e
    if length(e.p-c) <= 0.016m {
      let eye = pose * e.out(c)
      let look = e.into(inv(pose) * dir(target(time)-eye))
      let pupil = dot(e.into(n),look) > 0.88
      let iris: rgb = if pupil { [0.012,0.009,0.006] } else { [0.94,0.75,0.38] }
      ggx(iris,0.18,0) + coat(0.8,0.04)
    } else { chitin(p) }
  }
}

kind Sand @World() {
  trace(p) = box(p-[0,-0.04,0]@World, [2,0.04,2])
  material(p,n) = ggx([0.39,0.30,0.19],0.48,0) + coat(0.65,0.12)
}
build crab(at: Xf[Body,World]) {
  emit Crab(pose: at)
  emit Shell(pose: at * rigid([0,0.045,-0.065]@Body,0rad))
  build for side in [-1,1] {
    emit Claw(pose: at * rigid([side*0.073m,-0.014,0.075]@Body,side*0.22rad),
              size: if side > 0 { 0.074m } else { 0.043m }, gape: if side > 0 { 0.6rad } else { 0.34rad })
    build for j in 0..6 {
      let yaw = (1-side)*90deg + side*(j-2.5)*0.16rad
      emit Leg(pose: at * rigid([side*0.05m,0,0.018m*(j-2.5)]@Body,yaw),
               femur: 0.064m + j*0.003m, tibia: 0.103m + j*0.004m)
    }
  }
}
build scene() {
  emit Sand()
  crab(rigid([0,0.076,0]@World,0rad))
}
schedule Main {
  rows(scene)
  render(look_at([0.46,0.31,0.59],[0,0.11,0],[0,1,0],42deg),
         sun(dir([-2,4,2]),[2.8,2.6,2.2]), sky_dome, daylight, aces)
}
```

### Reed bed

```loft
use std.scene
space Stalk

const paper: rgb = [0.94,0.88,0.73]
const indigo: rgb = [0.14,0.13,0.21]
const blue: rgb = [0.40,0.52,0.59]
fn wash() = mix(paper,blue,0.16)
fn sky(d: Dir[World]) -> rgb = mix(paper,blue,0.07)

kind Reed @Stalk(pose, height: m in 0.6m..=1.3m, lean: rad, stiff: f in 0..=1, phase: rad) {
  trace(p) {
    let root = pose * [0,0,0]@Stalk
    let gust = 0.5 + 0.5*sin(1.9rad*time/1s - 1.3rad*root.x/1m)
    let wind = (0.17 + 0.24*gust + 0.07*sin(phase + 2.8rad*time/1s)) / (1 + 3*stiff)
    let mid = [height*(0.5*sin(lean) + 0.25*wind), height*0.5, 0]@Stalk
    let tip = [height*(sin(lean) + wind), height, 0]@Stalk
    let top = tip + dir(tip-mid)*0.095m
    min(round_cone(p,[0,0,0],mid,0.009m,0.006m),
        min(round_cone(p,mid,tip,0.006m,0.003m), capsule(p,tip,top,0.015m)))
  }
  material(p,n) = emission(indigo)
}
kind Water @World() {
  trace(p) = box(p-[0,-0.05,0]@World, [4,0.05,4])
  material(p,n) = emission(wash())
}
fn ink(h: Hit, l: Sun, visibility: f) -> rgb {
  if h.kind == Water || dot(h.n,l.dir)*visibility < 0.18 { emitted(h) } else { paper }
}
build bed() {
  emit Water()
  build for i in 0..200 {
    let a = tau*rnd(i,0)
    let r = 1.5m*sqrt(rnd(i,1))
    emit Reed(pose: rigid([1.3*r*cos(a),-0.035,r*sin(a)]@World,0rad),
              height: 0.68m + 0.55m*rnd(i,2), lean: (rnd(i,3)-0.5)*0.24rad,
              stiff: rnd(i,4), phase: tau*rnd(i,5))
  }
}
schedule Main {
  rows(bed)
  render(look_at([2.8,1.15,3.4],[0,0.42,0],[0,1,0],50deg),
         sun(dir([-2,3,1]),[1,1,1]), sky, ink, linear)
}
```

### Heron's hop

```loft
use std.scene
space Bird

const limit = 0.78rad
const flight = 0.42s
fn eye() -> Pt[Bird] = [0,1.06,0]
fn hips() -> Pt[Bird] = [0,0.48,0]
fn forward() -> Dir[Bird] = [0,0,1]
fn fish(t: s) -> Pt[World] {
  let a = 1.16rad * smoothstep(0.35s,0.95s,t)
  [2.1m*sin(a),0.019,2.1m*cos(a)]
}
fn sight(pose: Xf[Bird,World], t: s) -> Dir[Bird] = dir(inv(pose)*fish(t) - eye())

kind Fish @World() {
  trace(p) {
    let c = fish(time)
    capsule(p, c-[0,0,0.065], c+[0,0,0.065], 0.024m)
  }
  material(p,n) = ggx([0.48,0.68,0.58],0.23,0.65)
}
kind Heron @Bird(pose, yaw: rad, left: s, spin: rad/s, aim: Dir[Bird], limited: bool) {
  trace(p) {
    let trunk = capsule(p,[0,0.48,-0.17],[0,0.58,0.11],0.11m)
    let side = mirror_x(p)
    let legs = capsule(side.p,[0.062,0.49,0],[0.075,0.012,0.025],0.012m)
    let neck = min(capsule(p,[0,0.57,0.10],[0,0.74,0.18],0.037m),
                   min(capsule(p,[0,0.74,0.18],[0,0.90,-0.06],0.030m),
                       capsule(p,[0,0.90,-0.06],eye(),0.025m)))
    let skull = capsule(p,eye()-aim*0.045m,eye()+aim*0.032m,0.046m)
    let beak = round_cone(p,eye()+aim*0.04m,eye()+aim*0.24m,0.017m,0.002m)
    min(min(trunk,legs),min(neck,min(skull,beak)))
  }
  material(p,n) {
    let along = dot(p-eye(),aim)
    let ochre = along > 0.065m && p.y > 0.84m
    let color: rgb = if ochre { [0.84,0.49,0.10] } else { [0.66,0.70,0.72] }
    ggx(color,0.47,0)
  }
}

law body(h: Heron) writes(pose,yaw,left,spin) {
  let launch = h.left == 0s && prev(h).limited
  let remaining = if launch { flight } else { h.left }
  let want = sight(h.pose,time)
  let spin = if launch { atan2(want.x,want.z)/flight } else { h.spin }
  let step = min(dt,remaining)
  let left = remaining-step
  let u = left/flight
  let yaw = h.yaw + spin*step
  h.left = left
  h.spin = spin
  h.yaw = yaw
  h.pose = rigid([0,0.56m*u*(1-u),0]@World,yaw)
}
law neck(h: Heron) writes(aim,limited) {
  let want = sight(h.pose,time)
  let over = angle(forward(),want) > limit
  h.limited = over
  h.aim = if over {
    let tangent: Dir[Bird] = dir([want.x,want.y,0])
    dir(forward()*cos(limit) + tangent*sin(limit))
  } else { want }
}
check gaze(h: Heron) -> Verdict[rad] {
  let want = sight(h.pose,time)
  let room = limit-angle(forward(),want)
  if !finite(want) || !finite(h.aim) { unknown(NonFiniteGaze) }
  else if h.aim == want && room >= 0rad { held(room) }
  else { missed(angle(h.aim,want)) }
}
check above_hips(h: Heron) -> Verdict[m] {
  let gap = (h.pose*eye()).y - (h.pose*hips()).y
  if !finite(gap) { unknown(NonFinitePose) }
  else if gap < 0m { missed(-gap) } else { held(gap) }
}

kind Bank @World() {
  trace(p) = box(p-[0,-0.08,0]@World, [0.55,0.08,0.6])
  material(p,n) = ggx([0.28,0.23,0.16],0.8,0)
}
kind Water @World() {
  trace(p) = box(p-[0,-0.12,0]@World, [5,0.08,5])
  material(p,n) = ggx([0.13,0.23,0.24],0.24,0) + coat(0.6,0.06)
}
build scene() {
  let at: Xf[Bird,World] = rigid([0,0,0]@World,0rad)
  emit Heron(pose: at,yaw: 0rad,left: 0s,spin: 0rad/1s,aim: sight(at,0s),limited: false)
  emit Fish()
  emit Water()
  emit Bank()
}
schedule Main {
  rows(scene)
  run(body,neck)
  observe(gaze,above_hips)
  render(look_at([3.3,1.75,4.4],[0.55,0.47,0.60],[0,1,0],44deg),
         sun(dir([-2,4,2]),[2.6,2.4,2.1]), sky_dome, daylight, aces)
}
```

**What these programs predict on screen.** The body row itself carries the reflected pair. Duplicating a crab calls the same builder with another placement. Each eye lifts its own centre and projects the world direction back through the same branch. At the reeds' midpoints the wind term is a quarter of the tip displacement. The gust uses world root position. Oscillation phase and stiffness are per row. Visibility from the sun contributes to the hard pigment edge, and water and sky receive different dilutions of the same second ink.

The heron's eye is the head rotation pivot. The neck law clamps the full target direction to a cone about the trunk's forward axis. In the saturated branch, the tangent component lies in the plane through the trunk axis and the fish. The angular miss is the excess over the cone angle. On this trajectory the fish stays below eye height, so the tangent used there is nonzero. The check reads the same axis used to draw the skull and beak. A geometry fork that bypasses that site must supply its own final-site observation or receive `Unknown(UnobservedSite)`. A positive residual is a miss even if smaller than a typical display tolerance. The body reads the preceding tick's saturation and chooses its signed angular velocity on takeoff. It keeps that spin until landing, while the neck re-aims from the new pose every tick.

| Witness | Value | Status and instrument |
|---|---:|---|
| Single crab / doubled crab, leg rows | 12 / 24 | **Predicted**, nested build loops. Both use one Leg kind. |
| Single crab / doubled crab, claw rows | 2 / 4 | **Predicted**, both claws share one definition and the two sides differ only by row data. |
| World eye centres, horizontal separation | 0.086 m | **Predicted**, apply the mirror branch and the emitted pose to the centre. |
| Angle between world eye rays at time zero | 0.12052 rad | **Measured** in an independent scalar evaluation of the two lifted centres and common target. |
| Reed rows / Reed definitions | 200 / 1 | **Predicted**, half-open build range with one append. |
| Heron reference cadence / duration | 120 ticks/s / 2 s | **Measured** validation inputs to an independent binary64 scalar replay. |
| First clipped gaze / takeoff / reacquired gaze / landed | ticks 82 / 83 / 121 / 133 | **Measured** in that replay, evaluating fish, then body, then neck and checks at each tick end. |
| Peak gaze miss / peak lift | 0.13432 rad / 0.13999 m | **Measured** in the scalar replay. |
| Final yaw, written fish curve / horizontally reflected curve | +0.70006 / −0.70006 rad | **Measured** in the scalar replay. Each take landed after one hop. |
| Head–hip vertical gap through flight | 0.58 m | **Predicted** for the whole take. The common yaw preserves height, and the common vertical offset cancels in the difference. |
| Scene files accepted by the printed EBNF | 3 / 3 | **Measured** using an independent LALR recognizer. |

The motion and eye probes exercise the intended equations. Compilation, device arithmetic, render parity and blind authoring accuracy on this particular surface remain unmeasured.

## Five diagnostics

These are deliberately rejected excerpts in the proposed spelling. The conflicting line and its repair fit in one diagnostic. The shared-writer error also retains the first owner's span.

```text
E403  for j in 0..6 {
      Write “build for j in 0..6” because this loop in build crab appends Leg rows.

E502  law neck(h: Heron) writes(aim,limited,yaw) {
      Remove yaw from neck’s write set and read the heading published by body, which already owns Heron.yaw.

E104  material(p,n) = chitin(p)
      Declare “fn chitin[S](p: Pt[S]) -> Bsdf” because this call supplies Pt[Claw] to a helper fixed to Pt[Leg].

E104  let eye = pose * c
      Lift c through the branch opened by mirror_x(p) using “pose * e.out(c)” to recover this eye’s world position.

E211  fn claw(p: Pt[Claw]) -> Field[Claw,1.35] {
      Return the distance in metres so trace can derive the Lipschitz bound that the numeric argument tries to assert.
```

The authored-bound excerpt is a regression fixture from the older language draft. No bakeoff author made that error. A mirrored kind emitted twice can still pass the space checker because both pairs are well typed. A row-count assertion or an image showing the eye sites addresses that separate mistake.

## Grafts and the machine-facing form

| Origin | Adopt, revise or decline | Reason in an author's program |
|---|---|---|
| A and G | Adopt a stage marker on the allocating loop and a distinct append operation. | A's authors used the correct loop in both outputs. No syntax can make an accidental time read safe at build. Check the transitive effect, including inside a helper. |
| E and F | Adopt inferred locals, infix maths, named row data, unit suffixes, contextual brackets, a final expression and first-argument pipes. Prefer E's braces. | A shell spine and a material formula keep their mathematical structure. F-Haiku placed the entire shell surface outside a kind. |
| B, C, D, E and G | Adopt point-bound map values with explicit point, inverse and differential projections. Allow a reusable function of the query point. | A kind-scoped *recipe* can remove repetition. A shared value would capture the branch from a different evaluation. Restrict by provenance so a legal translated query point still works. |
| Five-card and seven-card reviews | Decline a universal ban on emitting two mirrored rows. Offer a build inspection that lists the emitted row and its reflected sites per parent. | A second crab legally contains another pair. Only an authored expectation about the eye count distinguishes the unintended duplication. |
| G | Adopt mandatory field write lists. | G-Haiku already expresses body and head control on one kind. Distinct owners avoid copies and preserve one visible state schema. |
| B, C and D | Adopt typed verdicts and restore the actual drawn site as the evidence source. | A clamp with a positive leftover angle must still report Missed. A correctly updated variable disconnected from head geometry cannot establish gaze. |
| Original and five-card reviews | Adopt explicit space-polymorphic signatures, optional validated row ranges and a table/row distinction. | Sharing chitin needs no coordinate erasure. A denominator known positive at data ingress needs no repeated clamp. A fish row ID cannot silently stand in for an entire table. |
| Seven-card review | Put the integer noise/hash in the card as a usable library operation. Use a hardware-only feature for the raw example. Put even Boolean instance variation in rows when the topology is shared. | Both claw shapes here use the same serrated field. Raw hashing at build and a feeder-specific program buy nothing for the task. |
| Built engine and prior Astra spec | Keep layers in source order and public schedule recipes. Keep raw entries, compute passes, replaceable record codecs and source spans. | The terse scene can open the renderer and replace its lighting or its culling. Cold shader and row identities remain inspectable. |
| G's reader and macros | Ship a deterministic graph printer for structural inspection. Defer a reader or graph-edit protocol until a recurring transformation needs it. | Shared typed nodes already give agents structural equality. New user macros that generate shader syntax could reintroduce expansion proportional to instances. |

For machine output, serialize the existing IR inventory in stable dependency order, with explicit operand references, typed columns, stages, effects and versioned resources. Keep source spans and expansion origins in a side table, outside the canonical code-identity hash. An s-expression printer is sufficient. Use it to tell a row-data edit from a new shader body. For a source refactor such as extracting `chitin`, patch the span-bearing concrete tree and re-run the checker. Shared SSA cannot recover a comment, the choice of pipe versus call, or which of two source occurrences the author meant to edit. A future editable transport would need schema validation, rechecking of all effects, and a typed-graph round trip, plus a separate policy for preserving source. There is no evidence in these scene-writing trials of a rewrite benefit large enough to ship that second input path.

## Parser size

| Rust parsing work | Lines | Basis |
|---|---:|---|
| Existing declaration parser including its lexer | 741 | **Measured** physical lines using `wc -l` on the current file. |
| Lexer, source spans, logical newlines and raw-region boundaries | 250 | **Predicted** physical lines. |
| Pratt expressions, postfix, calls, literals, patterns and types | 390 | **Predicted**. |
| Declarations, schemas, query/law/solver/scan headers and imports | 440 | **Predicted**. |
| Blocks, statements, assignments and stage-marked loops | 230 | **Predicted**. |
| Recovery at enclosing delimiters, diagnostics and parser entry/glue | 240 | **Predicted**. |
| **Replacement parser total** | **1,550** | **Predicted**, plausible range **1,250–1,900** physical lines. |

Count hand-written Rust with blanks and comments on the same basis as the existing parser. This is a replacement total. It excludes AST data definitions shared with lowering, the type/effect checker, interval and AD passes, Naga, library code, tests and the optional IR printer. Rust needs a source tree before the typed graph so every refusal still has an author line. The small parser estimate says nothing about the cost of implementing a sound space or ownership checker. [Existing parser](../../../crates/loft-passes/src/parse.rs)

Before freezing the card, repeat the blind tasks and inject the negative excerpts. The acceptance prediction is that all injected errors are refused at the indicated line, a second crab adds no shader identity, a reflected fish reverses the turn, and rotating the published eye axis away from the reachable fish yields a gaze miss. Score the complete images, the weaker author's first-pass task coverage and the repairs it actually needs.

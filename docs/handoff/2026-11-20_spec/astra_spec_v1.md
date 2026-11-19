# The Loft meta engine specification, v1

## 1. Decisions

The unit an author edits is a function, a kind, or a schedule. The unit an implementer ships is a picture with its executable plan, measurements and evidence. A consumer can replace the function that finds a surface, the function that assigns an impulse to a foot, or the entire plan without changing the runner.

**Reading the numbers.** **Measured** means observed in the supplied Loft runs, on the stated path. **Predicted** covers every new benchmark value, workload assumption, allocation and acceptance threshold in this document, including inherited targets that have yet to pass on v1. Numeric literals in source, equations, data formats, language rules and worked examples are **specified** inputs or exact consequences by definition, never observations. Section, phase, tier, type, error and version numbers are identifiers. Timing tables use milliseconds unless their headings give another unit.

1. **Change · surface.** Write scenes, generators, laws and schedules in one staged WGSL superset with Rust native/WASM tools. This removes the host evaluation that expanded a call into thousands of nodes and retires the TS orchestration bridge.
2. **Change · architecture.** Freeze the runner ABI and semantic IR slowly, regenerate compiler passes behind gates, and ship algorithms in source. A compiler-sized immutable substrate would freeze the very choices an engine writer needs to change.
3. **Identity.** A kind owns executable variation, including generated pass kinds. Instance counts, lengths, LOD selection and target positions are rows. This makes multiplying an existing creature a data edit and excludes per-instance compilation.
4. **Change · metaprogramming.** Build loops emit typed rows, functions retain bodies, runtime loops retain bounds as operands, and structural variants have finite budgets. Post-hoc deduplication cannot recover work already expanded.
5. **Types.** Infer affine frames and physical units from signatures and use sites, generate a fresh space and cell witness at each map, and distinguish attachments from goals. This catches the mirrored eye error while allowing a deliberately rigid skull.
6. **Change · differentiation.** Carry smoothness with respect to the differentiation variable, use forward jets, and transport frames by a scheduled scan with a genuinely smooth reconstruction. An ordinary Hermite lookup and twice differentiating a Voronoi crease cannot supply shell curvature.
7. **Change · tracing.** Derive bounds on the authored field including the original teeth and displacement. Certify root existence separately from safe empty distance. An arbitrary maximum step, sampled gradient or user-provided Lipschitz number cannot certify a hit or a miss.
8. **Analysis.** One operator inventory supplies types, intervals, derivatives, twin and emission, with explicit opaque leaves. This precludes a plausible CPU answer for a function only the GPU executed.
9. **Dispatch.** The schedule declares observations, resources and timing. The compiler groups compatible work by dependency wave and law kind, subject to data hazards. This prevents both per-limb pipelines and a mixed-law megakernel.
10. **Portability.** Pre-record bundles with one indirect draw per compatible geometry batch. Count each draw even when empty. This avoids indirect-count and bindless requirements and makes differing topologies visible in the budget.
11. **Change · editing.** Interpret one dirty, eligible render kind while native code compiles, keep simulation and shipping frames native, and install a complete generation atomically. A global VM or a promise of immediate arbitrary native recompilation fails the measured edit cost.
12. **Change · reproducibility.** Require bit equality for qualified per-device motion, scoped numeric tolerances across devices and tiers, and complete expected frame sets. Abandon the legacy byte-for-byte port and reject missing samples.
13. **Worlds.** Give each ray class an explicit world with six readings. A kind supplies geometric or optical functions, a world answers observers. This keeps field evaluation distinct from nearest-hit and segment transmittance distinct from primary coverage.
14. **Records.** Carry identity, a codec-owned reconstruction payload and depth across primary visibility. Tile-classify opaque samples, draw transmitting surfaces forward and integrate along traced rays. This removes the closed material G-buffer and permits another codec.
15. **Geometry.** Prefer equivalent analytical hits, then indexed raster/accelerated triangles, then bounded implicit work, as cost and observer allow. A change in form, silhouette or shadow needs its own declared approximation.
16. **Materials.** Expose scattering evaluation, sampling measures, emission and directional albedo. Layer and compensate against the lobe actually used. A fixed paint struct and an unchanged GGX lookup cannot describe an arbitrary replacement.
17. **Lighting.** Integrate overlapping media jointly and compare estimators at identical observations, path depth and linear exposure against a converged reference. This eliminates author-order fog, proxy skeleton shadows, and beauty-space bias tests.
18. **History.** Let the viewer reuse declared history and invalidate on disocclusion, reconfiguration or seek. Stills, clips and training get independent reproducible frames. A hidden temporal cache would change a frame drawn alone.
19. **Change · formulation.** Give each degree of freedom one writer and order intent, support, stepping, IK and secondary motion. Use constrained numerical solves where a local law needs them. A global objective would arbitrate the animator’s beat implicitly.
20. **Motion state.** Body, mutable state and final pose are typed tables. Advance the same fixed-tick entry in live and baked playback, record input prefixes, event cursors and dependent tick spans. Frame-rate-dependent forces and repeated hits are invalid.
21. **Change · constraints.** Produce a dimensioned, scoped verdict per obligation, site and domain, from the pose the renderer observes. A passing head-height check says nothing about gaze, and correct target state cannot establish a planted toe.
22. **Solvers.** Prove disjoint writes per color or gather Jacobi contributions in fixed order. Pins use posed parents at the corresponding simulation time. A barrier or an unordered float atomic cannot turn racing updates into deterministic motion.
23. **Ownership.** The host controls its queue, targets, depth convention and frame lifecycle. Typed raw entries receive bound views and source maps. This allows embedding and full engine replacement without forced submission or readback.
24. **Change · delivery.** Build six vertical slices, with pictures and all five gates from the spine onward. Regenerate the corpus, then re-pin training and embed. A successful layer test or a passing fraction of a corpus cannot authorize cutover.

The algorithm/schedule distinction follows Halide, the visibility record follows Burns and Hunt, and the physical estimators, capture point, least-jerk trajectories and XPBD follow established methods. Loft's contribution is the contract between them: row-staged code identity, observer-specific equivalence and post-pose evidence, all subject to an executable cost gate. New defaults earn their place with the fixtures below.

## 2. The language

### Source and binding

A UTF-8 file is a module. The project root contains the entry module. A path denotes a file relative to the root or a pinned library root. Its last component can select a public definition in the enclosing module. No module executes on import. Definitions are private unless marked `pub`. Import cycles, recursive shader calls and unbound names are errors. Re-exports of the same declaration coalesce. For different declarations imported by wildcard, an ambiguous *use* requires qualification. Parameter types disambiguate explicitly declared overloads. A module-local declaration shadows a wildcard import. A scene exports one build-time function returning `Scene`. The CLI selects a named export when a module has several. Library interfaces below use bodyless signatures as declarations in an interface listing. Semantics next to each declaration belong to that function.

~~~ebnf
module      = { import | decl } ;
import      = "import", path, [ "::", ("*" | "{", names, "}") | "as", id ], ";" ;
path        = id, { "::", id } ;
names       = id, { ",", id }, [ "," ] ;
decl        = attrs, [ "pub" ], ( wgsl_decl | alias | enum | space | unit | iface
            | kind | scan | solver | routine | implementation ) ;
space       = "space", names, ";" ;
unit        = "unit", id, [ "=", dimension ], ";" ;
alias       = ("type" | "alias"), id, [ generics ], "=", ty, ";" ;
enum        = "enum", id, [ generics ], "{", variant, { ",", variant }, [ "," ], "}" ;
variant     = id, [ "(", types, ")" | fields ] ;
iface       = "interface", id, [ generics ], "{", { signature, ";" }, "}" ;
implementation = "impl", ty, "for", ty, "{", { routine }, "}" ;
kind        = "kind", id, [ generics ], "{", "row", fields,
              { routine | "query", signature_head, block }, "}" ;
routine     = [ "comptime" ], ("fn" | "law" | "constraint" | "schedule"),
              signature_head, [ effects ], (block | ";") ;
signature   = [ "comptime" ], [ "fn" | "query" ], signature_head, [ effects ] ;
signature_head = id, [ generics ], "(", [ params ], ")", [ "->", ty ] ;
params      = param, { ",", param }, [ "," ] ;
param       = [ "comptime" ], id, ":", ty, [ "=", expr ] ;
generics    = "[", formal, { ",", formal }, "]" ;
formal      = [ "comptime" ], id, [ ":", ty ] ;
fields      = "{", { id, ":", ty, ("," | ";") }, "}" ;
ty          = atomtype, { ("*" | "/"), atomtype } ;
atomtype    = wgsl_type | scoped_space | "1" | path, [ "[", args, "]" ] | "fn", "(", [ types ], ")", "->", ty
            | fields | "(", types, ")" ;
types       = ty, { ",", ty } ;
dimension   = unit_factor, { ("*" | "/"), unit_factor } ;
unit_factor = unit_name, [ "^", rational ] ;
effects     = [ "reads", refs ], "writes", refs ;
refs        = path, { ",", path } ;
scan        = "scan", id, [ generics ], "(", [ params ], ")", attrs, "->", ty,
              "{", "init", ":", expr, ";", "step", "(", id, ",", id, ")",
              ":", expr, ";", "}" ;
solver      = "solver", id, "over", ty, attrs, "{", { "row", fields | "state", fields
            | "config", fields_init | routine }, "}" ;
fields_init = "{", [ inits ], "}" ;
inits       = id, ":", expr, { ",", id, ":", expr }, [ "," ] ;
attrs       = { "@", id, [ "(", [ args ], ")" ] } ;
block       = "{", { stmt }, "}" ;
stmt        = wgsl_stmt | "map", id, "=", expr, ";"
            | [ "comptime" ], "for", id, "in", expr, block
            | "comptime", ("if", expr, block, [ "else", block ] | "const", binding, ";")
            | "emit", path, fields_init, ";" | "pass", id, "=", expr, ";"
            | "match", expr, "{", { pattern, "=>", block, [ "," ] }, "}" ;
expr        = WGSL_expression_extended_with_atoms_and_infix_below ;
atom        = path | number, suffix | string | "[", [ args ], "]" | fields_init
            | type_constructor, "(", [ args ], ")" | expr, fields_init
            | "|", [ lambda_params ], "|", (expr | block)
            | "raw", "wgsl", "(", [ params ], ")", "->", ty, wgsl_block ;
infix       = "|>" | ".." ;              (* pipeline and half-open range *)
args        = (ty | expr | id, ":", expr), { ",", (ty | expr | id, ":", expr) }, [ "," ] ;
binding     = id, [ ":", ty ], "=", expr ;
scoped_space = "'", id ;                   (* lexical space introduced by map id *)
lambda_params = id, [ ":", ty ], { ",", id, [ ":", ty ] } ;
pattern     = "_" | path, [ "(", names, ")" ] ;
~~~

WGSL supplies identifiers, literals, arithmetic, structures, attributes, member selection, calls, indexing, block statements, loops, pointers restricted by address space, and operator precedence. Its grammar is reused with `ty`, `expr`, `stmt` recursively extended at their corresponding productions. Generic structs use the same formal list as routines. A final field separator is optional. In type positions accept `1` as the numerator of a reciprocal dimension. Unit suffixes on numeric tokens come from the unit namespace. The range operator binds below arithmetic and above the pipeline. The left operand of a pipeline becomes the first argument of the call on its right. Named arguments are reordered by the declared parameter list and checked for duplicates. A typed array literal has the expected element type, including when empty. `Array[T]` is a build-time growable list or a read-only runtime slice resolved from a table. GPU-local fixed arrays retain their WGSL type. Angle brackets retain WGSL's template meaning. For Loft brackets followed by a call, the checker resolves arguments against the callee’s generic formals. Ordinary indexing takes a value, never a type. [WGSL language specification](https://www.w3.org/TR/WGSL/)

A `Name` is an interned build-time string. A tuple or struct can hold functions at build time. A lambda becomes a named IR function plus a typed capture record. Its body participates in analysis and its continuous captures occupy rows, even for a single instance. Functions passed as policies are statically linked, or dispatched by a finite kind tag. There are no heap closures or indirect calls on the GPU. A function argument expecting a time sample implicitly binds its sample time from the declared rate; it cannot read wall time. A typed handle records any dependency on a live table. `let` binds a handle, and mutation through the handle has the effects listed on its primitive. Build-local `var` and `push(list,value)` support row construction. `emit` appends a row of a declared kind. The library function `add` also appends and returns the new row handle. For the same geometry/material/cover functions it uses the same generated kind. Structural specialization records only function identities and finite options. Literals *inside a function definition* are part of its code. Values supplied by a scene invocation, a captured outer binding or a generator induction variable are data.

The prelude defines `Meters` (`m`), `Sec` (`s`), `Kg` (`kg`), `Rad` (`rad`), `Hz`, `N`, the integer and float types, and dimensionless `Unit` refined to the closed unit interval. `Rgb` is a linear three-channel value, `vec2f` and `vec3f` are unframed numerical vectors. Space constructors, `World`, and the standard modules' named space types are in scope wherever their signatures appear. `Pt[S](x,y,z)` is an affine point, its components lengths. `Vec[S]` defaults to a displacement; `Dir[S]` and `Norm[S]` carry frame and variance and unit norm. `Vec[S,U]` carries components in dimension U. Multiplication by an appropriate dimension changes vector dimension. Adding two points is ill-typed. `identity[S]()` yields a rigid map S to S. The prelude also admits the dimensionally checked constructors/functions below. All type parameters used only in argument/result positions can be inferred. The same import syntax selects a fork.

~~~loft
import std::render::*;
import local::pearlescent as pearl;

fn rigid[A,B](offset:Vec[B])->Xform[A,B];       // parallel axes, A origin at offset from B origin
fn identity[S]()->Xform[S,S];                   // exact identity
fn inverse[A,B](x:Xform[A,B])->Xform[B,A];       // requires nonsingular linear part
fn xyz[S](p:Pt[S])->Vec[S];                     // displacement from this frame's origin
fn coords[S,U](v:Vec[S,U])->vec3f;               // components only for dimensionless U
fn name(prefix:Name,i:u32)->Name;                 // build-time stable suffix, never a shader literal per row
fn push[T](a:Array[T],item:T);                    // append to an owned build list
fn time()->Sec;                                  // current evaluation instant, included in effects
fn param[T](id:Name,initial:T)->T;                // named instance input row with initial value and a host-writable schema
fn level[T](id:Name,script:fn(Sec)->T)->T;        // sample the named tick port, scripted until its first logged override
fn events[T](id:Name)->Array[T];                  // read this tick's ordered, deduplicated event payloads at the owning instance
fn tabulate[T](comptime n:u32,at:fn(u32)->T)->Table[T]; // materialize one row per index after the fill's dependencies are ready
~~~

Use a local file with `pub` definitions to fork any standard module. Import its functions at the consumer site or assemble a schedule using the fork. Identical definitions can share compiled code across paths. Diagnostics retain both definition and instantiation spans. Lock resolution by content digest through transitive sources and assets. `loft context` enumerates public declarations and contracts from that resolved graph. `loft show` emits lowered shaders with source spans, the chosen schedules, approximations and changes to derived facts.

### Type judgments

**Frames and units.** Subtract two points in the same frame to get a displacement. Add a displacement to a point to translate it. A map A to B acts by its affine part on points, by its linear part on vectors, and by the inverse transpose on normals before normalization. Normalize transformed directions. At a singularity return an error or a named runtime failure. Joins unify the space and the rational exponents of length, time, mass and angle. Numeric literals take dimensions from context. Products/quotients add/subtract exponents, trigonometry consumes radians, exponentials consume dimensionless values. Numerical angles can explicitly be made dimensionless by dividing by a radian. `f32(x)` changes a numeric representation, never a frame or dimension. Ties in unconstrained unit equations require a suffix at the ambiguous boundary. An input to a refined type needs a proof from its declared range, a checked conversion, or explicit clamping. A zero length cannot be normalized to a unit vector. Optics uses finite, nonnegative linear RGB for energy, and signed unrestricted working RGB where a look requests it.

**Domain maps.** A `map q = …` binding introduces a lexical space fresh to that application, a point `q.p` (also `q` in point context), the ordered cell witnesses `q.cell`, a forward differential at its parent sample and, when constructed, its inverse branch. A witness records the side, sector, translation or iteration path needed to invert the chain in reverse order. The branch convention at a mirror plane is positive; repeated cells are half open with the lower face owned. Inverse rotation/translation, and inverse scale for nonzero scale, are synthesized. Custom warps expose the forward map and can supply an inverse whose branch/domain roundtrip is proved or marked as an approximation. No fabricated inverse exists for a many-to-one warp. A point can leave the scope only by lifting it or packaging the point with its witness in an existential pair. On a known branch, map directions by its Jacobian at the point. Pulling a child normal back to the parent uses the transpose of the *forward* Jacobian; pushing a parent normal forward uses its inverse transpose. At a seam evaluate the chosen one-sided branch and name the normal convention. For the face, `eyeWorld = bodyPose * (q.to_parent * eyeLocal)`; project the world gaze back down the same branch before taking its dot product with the globe normal.

**Smoothness and AD.** Grade each expression on its active domain, with respect to each independent variable. `Cinf > C2 > C1 > C0 > Cneg1` is the regularity guarantee. Constants in that variable are smooth even if selected by another variable. Addition, composition and multiplication take the weakest input grade, subject to their domains. Bounded-away-from-zero division and normalization are smooth. Min, max, exact distance at medial axes, unfiltered Voronoi distances and abs guarantee continuity. Runtime discontinuous choice, floor, fract and lookup of cell IDs give `Cneg1` unless boundary jets can be matched. Cubic smoothstep joins constant branches at order one, quintic at order two. Domains must prove ordered distinct edges. Decreasing masks use one minus the increasing mask. For `diff(f,x)`, x names a formal, and the result is a callable with the same parameters whose output is the x partial; for a local expression x names the independent variable. One derivative consumes one order, with `Cinf` stable. Gradient, Jacobian and directional jets are derived. `subgrad` is an explicit piecewise shading convention (abs at zero gives zero, min ties choose the left branch); it grants no second derivative or proof of uniqueness. A bounded finite-difference normal is available as an observational approximation. The directional normal curvature along u is normal·Pᵤᵤ/(Pᵤ·Pᵤ); mean curvature requires both fundamental forms. A normal to a displaced shell comes from the two partials of the *displaced* surface. Differentiating the macro shape twice for a grime mask and the micro shape once for a normal is legal.

**Scans.** The scan result is a table of a specified number of stations in domain order. Evaluate `init` at the first station and `step(previous,parameter)` at the remaining stations. Parallel transport takes successive tangents and an initial orthogonal axis, rotates the prior frame by the minimal rotation, then orthogonalizes. For an antiparallel pair subdivide to resolve the tangent path or return `DegenerateFrame`. Degenerate spines require an authored frame. A scan over static arguments can bake, a scan depending on pose runs after that pose, and a supplied closed form replaces the scan only within its declared equivalence. Tangents with respect to upstream parameters propagate through the entire recurrence. Lookup is differentiated as the chosen interpolant. For a `sample_c2` table store value, first and second derivatives at each knot in parameter units; use quintic Hermite with shared jets, periodic matching for a periodic axis. A builder can derive shared jets by centered differences with one-sided endpoints; this makes a smooth approximation to the path. Hemisphere-align frame quaternions, fit components and normalize only where the interpolation norm is bounded from zero. The spatial error is scored separately from continuity. Baking never turns an unproved spline error into exact parallel transport.

**Bounds on an implicit.** Define F(p) with exterior positive. Over a closed finite box D a proof returns an outward-rounded value enclosure I, a spatial Lipschitz upper bound L (or `Unbounded(cause,span)`) and its domain. Carry numerical error of evaluation in I. Conservative interval arithmetic includes roundoff, transcendental enclosure and uncertainty in row values or texture filtering. Specialize the *certificate data* at build/load/frame rate for changing row ranges. A stale certificate cannot be read after a dependency changes. Domain bounds may be intrinsic, clipped by construction, or verified enclosures. An author-proposed cull box requires that verification or explicit clip semantics.

| Expression on D | Upper bound or proof rule |
|---|---|
| Constant, row independent of p, coordinate | Zero for constants and one for a coordinate. |
| Exact Euclidean signed distance, affine plane with unit normal | One, including at creases. Exactness refers to that shape under the specified coordinate metric. |
| Ellipsoid implicit | Use min(radius) × (length(p/radius) − 1), all radii positive. The zero set is the ellipsoid and L ≤ 1. The ray quadratic gives exact geometric intersection. |
| Addition, subtraction, constant scaling | Add upper bounds, multiply by the absolute scalar. |
| Product and composition | For fg use sup abs(f)·Lg + sup abs(g)·Lf. For h∘f multiply Lf by an enclosure for sup abs(h′) over I(f). Reciprocals need a range excluding zero. |
| Vector map and scale | An affine map uses its induced spectral-norm bound, rotation and reflection use one, diagonal scale uses the largest absolute axis. A variable Jacobian uses interval AD with an upper Frobenius norm or √(matrix 1-norm × infinity-norm). |
| Hard and smooth booleans | Min/max/negation preserve the maximum child L. A constant-width smooth min with nonnegative partial weights summing to one has the same rule. For quadratic smooth min add L(width)/4 if width varies and remains positive; for other blends enclose the actual width partial. |
| Repetition or inversion | Piecewise bounds hold up to the next certified cell boundary or singularity. A global L needs matching boundary values. DDA crosses exact boundaries with half-open ownership, testing neighboring motifs whose boxes overlap the cell; no epsilon jump through an unexamined interval. |
| Noise and fBm | For a basis n with proved Lₙ and octaves aᵢ n(Aᵢp+bᵢ), sum abs(aᵢ)·‖Aᵢ‖·Lₙ. Negative gains use absolute values. Unbounded octave count needs a proved convergent tail or a finite cap. |
| Loops, texture reads, opaque leaves | Interval recurrence over a bounded domain/trip count, or a proved invariant. Bound texel differences for the stated reconstruction, address mode, mip and seam. A point-sampled or opaque texture can give `Unbounded`. |

For a concrete, reproducible geometry-noise basis, v1 supplies `wave_noise` with n(x) = (sin x₁ + sin x₂ + sin x₃)/3, after an orthogonal rotation and phase. It is smooth, in [−1,1], with exact global L = 1/√3 in Euclidean coordinates. Interval forward AD bounds each gradient component by 1/3, then takes the Euclidean norm of the vector of bounds. It can derive this certificate directly from the function body in §7. Rotate and offset successive octaves to reduce alignment. The original claw uses the unnormalized sum of three octaves with gains 1, ½, ¼ and frequency ratios 1, 2, 4. The resulting costed geometry has the following **predicted** certificate values (exact expressions are the test oracle, stored bounds round upward).

| Term in local metres | Bound before arithmetic enclosure |
|---|---:|
| Original tooth, cubic ramp width 0.8 m | 0.045 × (26 + 1.5/0.8) = 1.254375 |
| Unit-L jaw capsule minus tooth, then constant-width smooth union with palm and studs | 2.254375 |
| Original 0.025 m three-octave engraving at 6.5/m | 0.025 × 6.5 × 3 / √3 ≈ 0.28146 |
| Whole crusher | ≤ 2.53584 (round upward again for implemented arithmetic) |

The field keeps the tooth and the fBm amplitudes in the geometry. The noise basis above fixes what the old unspecified `fbm` means for this v1 scene. For other noise laws use their own certificates. A changed noise body invalidates any hand-verified library axiom keyed to the old implementation. A fast global proof can be tightened by subdividing D and bounding derivative components in Bernstein form on each polynomial cell. Point sampling is a falsification test, never a bound generator.

**Finding a hit.** From a unit ray at p, advance by at most distance(0,I(F(p)))/L within the current certified segment. The same formula with an inside starting point finds its first *exit*. A zero lower bound or L=0 is handled explicitly. An interval excluding zero throughout a ray segment proves empty space there. To claim a nearest root use an analytic solve, or isolate a root to the distance tolerance by ordered interval subdivision, prove existence (e.g. opposite signs under continuity or interval Newton inclusion), and exclude earlier intervals. Tangencies can require analytic handling. A flat residual alone cannot locate a root. Exhaustion produces `Unknown(RootIsolation,span)`; an unresolved earlier interval prevents returning a later hit. A finite fixed step can skip an entire tooth. For an unbounded derivative use interval subdivision if the field admits interval evaluation, or require a new exact intersector or explicit approximate rendering adapter. A visual adapter can turn a footprint-sized unresolved region into partial coverage under its own scored contract. Reference picks and truth checks retain the uncertainty.

**Staging and effects.** Stage zero executes finite build code on CPU/WASM and may emit rows, declare resources, schedules or groups of kinds. Stage one specializes a definition by discrete shape of code (a bool, enum, bounded integer or finite lobe schema); v1 allows at most four *used* structural variants per declared kind by default. Generic type/space variables erase if layout and implementation agree. A different budget is an explicit project decision recorded with the module budget. Floating-point kind parameters are illegal even when just one value happens to occur. Stage two executes kernels over typed rows. A runtime trip count never creates graph copies. A large bound or opaque loop can be legal natively while ineligible for proofs or tape. Rate/effect inference lifts an expression only to a rate which provides all its binders, clock and resources. Every storage write names an owned region; an unscheduled data dependence is a diagnostic, including through a closure. The language checks shape and units at `raw wgsl` boundaries, injects binding/layout declarations, validates stage restrictions, and marks math, derivatives and access patterns inside raw code as opaque unless independently verified. Raw code cannot establish a claimed motion or field guarantee. A build binding whose value depends on a later clock binds a typed expression for that clock. For example, binding a neck station from a posed take in a scene function schedules its evaluation after pose publication and before the capsule row is read. Making that station control a build-time branch or row count raises E403. Frame-inferred constructors such as `Vec(…)` take the frame of the consuming operation, including the input chart at that point in a map pipeline.

~~~loft
space Leg;
kind LegStock {
  row { length:Meters, socket:Pt[Leg], }
  fn reach(self:Row[LegStock])->Meters { return self.length; }
}
comptime fn stock(lengths:Array[Meters])->u32 {
  comptime for i in 0u..lengths.len() {
    emit LegStock { length:lengths[i], socket:Pt[Leg](0,0,0) };
  }
  return lengths.len();
}
~~~

Inputting twelve lengths emits twelve rows with one schema and one reach body. Attempting `kind LegStock[comptime length:Meters]` gives E401; specializing on twelve discrete leg indices gives E402 at the fifth distinct instantiation, with all instantiation sites in the report.

### Diagnostics

| Code | Condition and repair |
|---|---|
| E001 / E011 | Syntax or unresolvable/ambiguous module, name, import cycle or bad entry. Quote the origin and legal declaration. |
| E104 / E105 | Distinct spaces or unavailable/inapplicable inverse branch. Show both bindings and the lift or required change of map. |
| E111 / E112 | Unit equations conflict or are underdetermined; failed range/nonzero/normalization refinement. Give a suffix or check at the boundary. |
| E211 / E212 | Required bound missing or an exact query demands an opaque/approximate observer. Name the operator, domain, query and exact adapter required. |
| E307 / E308 | Insufficient regularity or degenerate frame/table jets. Name the variable, knot/seam and available derivative order. |
| E401 / E402 / E403 | Continuous specialization, structural variant budget or rate escape. Move instance values into rows or bind at the consuming rate. |
| E501 / E502 | Pass cycle, read-before-write, conflicting ownership or unproved disjoint solver writes. Name both sites and an intervening version, color or gather. |
| E601 / E602 | Absent query/codec/capability or resource, layout or capacity mismatch. State the failed ABI, observer and allowed replacement. |
| E701 / E702 | Stale snapshot/log, unsupported deterministic profile, overflow or incompatible edit. State the last good generation and recovery action. |
| E801 | Invalid bind, chain/retarget map or missing site. Name the source joint, destination site and frame. |

A diagnostic carries severity, code, file, UTF-8 byte span, display line/column, primary and related spans, instantiated kinds, failing obligation and one actionable fix. `show --json` preserves these fields. A runtime `Unknown` carries its reason and evidence; it is never downgraded to a compiler warning that permits a proof.

~~~text
error[E104] crab.loft:19:33  expected Pt[BodySpace], found Pt['eye]
  17 | map eye = mirror_x(p) |> translate(-socket);
  19 | let world_eye = body_pose * eye_c;
                                    ^^^^^  eye_c belongs to the branch introduced at line 17
  fix: Lift the centre with body_pose * (eye.to_parent * eye_c).

error[E211] claw.loft:12:26  sphere tracing needs a finite spatial bound
  11 | let gap = max(0m, length(q) - 0.5m);
  12 | return palm - 0.04m * (1m / gap);
                                     ^^^  gap contains zero in the active box
  fix: Give the denominator a physically meaningful positive lower limit and rebuild its certificate.

error[E307] shell.loft:25:12  second derivative in u requires C2
  22 | let relief = voronoi_cone(uv);
                    ^^^^^^^^^^^^  only C0 at cell boundaries
  25 | let k = diff(diff(displaced,u),u);
               ^^^^^^^^^^^^^^^^^^^^^^^^^  displaced includes relief from line 22
  fix: Compute curvature on the smooth macro shell and use the relief only for first-order or explicit shading detail.
~~~

## 3. The core

**Execution boundary.** Layer 0 accepts a versioned package of validated programs, resources and commands. It owns the resident bytes and their lifetimes. Layer 1 defines their meaning and the tests for a conforming implementation. Layer 2 can change any lowering that passes those tests. Geometry, light, contact, control and the selection of passes are Layer 3 source. The CPU twin interprets typed functions without a graphics device. A raw function reports its unavailable analyses at its source span.

The native host boundary is below. Names such as `Package`, `Schema` and `Error` denote the records specified in this section. Handles belong to their creating device. The standalone runner supplies the event loop, clock, device, presentation and queue submission. An embedding host supplies its own.

```rust
pub trait Runner: Sized {
    fn attach(device: wgpu::Device, queue: wgpu::Queue, cache: Cache) -> Result<Self, Error>;
    async fn prepare(&self, pkg: &Package, targets: &TargetLayout) -> Result<Prepared, Error>;
    fn instantiate(&mut self, code: &Prepared, inputs: &InitialRows,
                   imports: &[Imported<'_>]) -> Result<Instance, Error>;
    fn patch(&mut self, i: &mut Instance, at: Tick, edits: &[RowPatch]) -> Result<(), Error>;
    fn log(&mut self, i: &mut Instance, packets: &[TickPacket]) -> Result<(), Error>;
    fn install(&mut self, i: &mut Instance, next: Prepared, migration: Migration) -> Result<(), Error>;
    fn seek(&mut self, i: &mut Instance, to: Tick) -> Result<(), Error>;
    fn encode(&mut self, i: &mut Instance, at: FrameTime, targets: &[TargetView<'_>],
              into: &mut wgpu::CommandEncoder) -> Result<FrameReceipt, Error>;
    fn submitted(&mut self, i: &mut Instance, frame: FrameReceipt,
                 submission: wgpu::SubmissionIndex) -> Result<(), Error>;
    fn abandon(&mut self, i: &mut Instance, frame: FrameReceipt) -> Result<(), Error>;
    fn read(&mut self, i: &Instance, slice: TableSlice,
            into: &mut wgpu::CommandEncoder) -> Result<ReadTicket, Error>;
    async fn resolve(&self, ticket: ReadTicket) -> Result<OwnedRows, Error>;
}
pub struct RowPatch { pub slot: Slot, pub row: u32, pub generation: u32, pub bytes: Vec<u8> }
pub struct TickPacket { pub slot: Slot, pub tick: Tick, pub sequence: u64, pub bytes: Vec<u8> }
pub struct FrameTime { pub tick: u64, pub fraction: f32, pub sample: u32 }
pub enum DepthOrder { NearIsLess, NearIsGreater }
pub struct TargetView<'a> { pub slot: Slot, pub view: &'a wgpu::TextureView }
pub enum Imported<'a> { Buffer(Slot, &'a wgpu::Buffer, std::ops::Range<u64>),
                        Texture(Slot, &'a wgpu::TextureView), Sampler(Slot, &'a wgpu::Sampler) }
pub enum Migration { PreserveSchema, Convert(FunctionId), RestartAt(Tick) }
pub enum Cache { Memory, NativeDirectory(std::path::PathBuf), BrowserOrigin }
pub enum Error { Package(Diagnostic), Unsupported(Diagnostic), History(Diagnostic),
                 Resource(Diagnostic), DeviceLost(Diagnostic) }
```

`TargetLayout` declares dimensions, attachment formats, sample counts, depth convention, projection, ownership, load/store and resolve for every slot. Loading a host colour or depth attachment preserves its prior contents. Clear is an explicit schedule operation. Validate imported handles against declared usages, byte ranges, strides and sample types. The host orders producers before Loft readers in its encoder and supplies a revision digest for externally written tables, including POSE. A host that needs replay also retains the external data or a reproducible producer. Encoding appends commands, including staging copies, to the supplied encoder. Only the owner submits, presents or polls for completion. Read tickets resolve asynchronously after submission. The WASM entry points use handles to these same resources. A resize invalidates extent-dependent allocations and bundles. Device loss retires handles and reports the last recoverable checkpoint and log prefix. Native and browser share the package reader, pass semantics and compiler.

Treat frame encoding as a transaction. A seek requests a restore/replay in the next encoding. That encoding pins one package generation and the needed buffer versions, and stages its prospective new tick, event cursor and frame identity in a single-use receipt. The caller either submits the encoder on its owning queue and acknowledges that submission, or discards the encoder and abandons the receipt. Allow one unacknowledged encoding per instance; a pending instance reports its receipt if another encoding or install is attempted. Acknowledgment publishes the new cursor in queue order. Abandonment permits retry from the last committed tick. Preflight all validations before appending commands; a mid-encode device failure invalidates the encoder and its provisional state. Queue writes to shared published storage must respect older outstanding encoders; prefer per-receipt staging copies. Recycle resources only after every lease is abandoned or its acknowledged submission has completed. A read ticket similarly pins its source version until completion or cancellation.

**Package model.** Serialize the following typed sections in little-endian order with magic `LOFT`, format version, capability/limit requirements and a directory of (section tag, offset, byte length, content digest). Check lengths with bounded integer arithmetic, ids, acyclicity where required, binding limits, resource ranges, schemas, and entry signatures before allocating or encoding. An unsupported format or feature returns a diagnostic and required fix. The artifact is relocatable and contains source spans and the library dependency closure.

| Section | Record and meaning |
|---|---|
| Graph | A function has typed parameters, result, effects, blocks and source spans. Block parameters carry SSA values across branches and back edges. Instructions include constants, field/row/resource reads, arithmetic, constructors, direct/open calls and explicit opaque calls. Terminators branch, loop, return or yield. Hash-cons pure expressions in their dominance scope. Distinguish reads of different resource versions. Calls and loop bodies have a single definition. |
| Kind | Canonical key = normalized typed body + resolved imports/primitive certificates + row schema + finite structural arguments + capability profile. A kind owns a family of entry points. Row counts, continuous values, spawn order, visible counts and LOD choices are data. Shared schedule kernels, render state and target layouts have independent keys. |
| Rows | A typed row handle is (kind, logical id, generation). The generation belongs to the table epoch. Stable logical ids resolve through indirection to packed offsets. A written reference must resolve in its recorded epoch. The build rejects dangling parents, duplicate keys, arithmetic overflow and exhausted id ranges. Compaction copies ids and keeps source identity. |
| Tables | Schema = field (name, type, unit/space, offset or column, stride), element count/capacity, address space, access, clock and retention. Compute physical offsets from WGSL host-shareable address-space layout, including vector alignment and array stride. Lower bools/enums to specified integer codes. Use one schema to generate packers, WGSL declarations and twin loads. A resize replaces backing storage and bind groups at a frame boundary. |
| State | An instance owns its state schema, initial rows, current tick, ordered input log with per-stream sequence cursors, retained checkpoints and frame-local scratch. `Previous` reads the completed prior version. Derived poses, bounds and draw arguments are scratch unless a later tick explicitly reads them. |
| Schedule | A pass has a domain, clock, input versions, output versions, iteration bound, row footprint, kernel and target state. Commands are resource clear/copy, direct or indirect compute, raster over fixed draw buckets, and bundle replay. The DAG admits cycles only across an explicit delayed version or bounded iteration. A compiler diagnostic identifies the two accesses of an illegal cycle, alias or conflicting write. |
| Bindings | Each used entry has generated group/binding indices and an explicit typed read/write footprint, also for raw WGSL. Layout is independent of table length. Pack compatible reads into typed arenas. Validate device-layout limits before preparing a pipeline. External textures/buffers appear as named imports in the manifest. |
| Identity | One frame digest covers module and asset closure, row contents, compiler/format/precision, schedule and approximation choices, camera/projection, targets, time, instance checkpoint and input-prefix digest, sample/seed/lens/shutter, history/reset epoch, device/driver and active revision/tier. A verdict records its scope, writer, dependencies and this identity. A content digest collision is resolved by comparing descriptors. |

**Time and ownership.** Tick intervals are half-open. Tick input is the last level at its start plus the events assigned to that tick, ordered by stream then sequence. Order within a stream is monotone. A duplicate sequence with identical payload is idempotent, conflicting data is an error. Script inputs evaluate at that tick. Each port schema declares its payload type and whether it supplies a level or events. Decode packet bytes using that schema. After a scripted level switches to live input, its last value holds until replaced unless its schema requires a fresh sample. A row patch replaces a complete row in the named host-writable slot. Resolve the whole batch against the indicated epoch, validate lengths, bounds and unique destinations, and commit it atomically at the named tick. Log its values before evaluating any dependent pose or rendering tables. Editing a retained tick forks its history by the same rule as editing a packet. A declared hold-last packet carries its age. A late edit to a retained input creates a new history branch at its tick and invalidates subsequent checkpoints, poses and image history on that branch. Missing input outside a declared hold or retained prefix produces `HistoryGap`.

Choose a closed form when a value depends only on named inputs available for that time. A scripted stateful law bakes through the same advance entry used live. A host override continues from its matching scripted boundary. Checkpoint all state read in future ticks, including event cursors, plans, solver multipliers with persistence, RNG state when used, and stateful environment dependencies. Include tick, schema/law digests and input-prefix hash. Seek restores the latest matching checkpoint at or before the target and replays the actual logged inputs. An absent checkpoint can use the initial state only with its complete prefix. Dropping old data is an explicit retention choice. Present the solved span at the requested fraction. Drawing and checking consume that same post-solve pose version. A request spanning several advances is chunked without dropping ticks. An incompatible source edit requires a typed state conversion or an explicitly selected restart boundary.

**Lowering in dependency order.**

1. Resolve source, infer types and effects, and elaborate compile-time topology into rows. Produce functions with explicit calls and loops, then intern canonical kinds. An optimizer may inline small sites and unroll a short proven constant loop on emit. Show the recorded size, emitted size and decision. Arbitrary instance inputs never specialize shaders.
2. Specialize open calls to the set of reachable implementations. Known targets become direct calls. Unknown targets lower to a checked switch over the present kinds, or kind-homogeneous work lists. Derive readings, forward derivatives, intervals and bounds before stripping phantom types. Simplifications respect the selected arithmetic semantics. Preserve raw opacity across calls. The twin consumes the same SSA and data layout. A typed refusal identifies the first unsupported operation and its consumer.
3. Schedule at the slowest clock that has all dependencies and meets the requested observation. Determine RAW, WAR and WAW order from versioned resource footprints. Topologically number dependency waves. Bucket homogeneous row work by (wave, law kind, clock, compatible ABI). Fuse ticks into an invocation or workgroup only if the entire recurrent island and its dependencies are local. Cross-island reads need an immutable prior version and a pass boundary before its replacement can be read. Multiple independent writers require disjointness or an explicit deterministic reduction. The author may replace grouping, storage and pass topology. The compiler checks hazards and charges the resulting work.
4. Emit one canonical module per reachable kind family plus the finite set of global schedule modules. A module may contain multiple entry points, including geometry and shade. Count every distinct compiled module and every actual compute or render pipeline descriptor. Draw buckets are (pass, geometry kind, topology/LOD, raster state). Use one indirect draw per kind for the common single-topology case. For differing index counts, use fixed LOD/topology buckets or a geometry-pulling schedule; the ledger counts each draw. The invariant is independence from row counts for a fixed kind/LOD/schedule set.
5. Cull against current pose and shutter bounds. The portable small-bucket cull assigns one workgroup per bucket, scans source rows in stable order in successive tiles, uses a local exclusive prefix, and overwrites the full indirect argument including zero for an empty bucket. A parallel expansion uses distinct clear/count/scan/scatter passes where it needs device-wide ordering. Reserve bounded output slices and surface overflow. Raster indexes the compacted logical-row list using an explicit bucket base. Nonindexed indirect arguments have four words, indexed five, including signed base vertex. Encode zero first instance. Set pipeline and issue the single indirect draw within a cached RenderBundle. A native multi-draw lowering must pass the same observer and budget gates. Bundle replay saves encoder work and every contained draw still counts. [WebGPU indirect commands](https://www.w3.org/TR/webgpu/#indirect-first-instance)
6. Lower typed IR to Naga with a source-span map. Validate new code and retain errors against source and generated WGSL. Native consumes validated IR when supported. Browser receives WGSL and the device performs its required validation. Reuse compiler validation by module key. Pipeline-cache keys include backend, adapter/driver, compiler, capabilities, entry points and complete binding/raster/target layouts. Persist binaries only when supported; other profiles persist source/IR and let the driver manage its cache. Report cache-hit level and preparation times. `loft show` prints WGSL, layouts, source spans, rates, dependencies, adapters and costs, including the first cause of an opaque or approximate result.

**Visibility ABI.** The core knows typed attachments, load/store and sample count. The standard library defines the following forkable record. At each covered sample a single `rgba16uint` holds lanes (K, R low, R high | P low shifted, P high). Precisely, lane X is K, Y is bits 0…15 of R, Z is bits 16…23 of R in the low byte and bits 0…7 of P in the high byte, W is bits 8…23 of P. K=0 means background. Other K values identify the dense kind directory for the current world epoch. R is the unsigned logical row within kind. P is a primitive index for triangles, two unorm12 values for a grid chart, or a kind-defined local token. Round unorm components to nearest even after saturating. Reject overflow or select a declared wider codec. A mesh reconstructs barycentrics from projected primitive vertices, a grid refines the chart when its quantization error matters, and an analytic/field proxy reconstructs from hit depth and the kind's reading. Encode coverage or ink requiring more payload in a declared side attachment or a different codec. Culling never changes R.

Use a separate depth attachment and projected hit depth in the caller's convention. Multisampled integers are read per sample and resolved explicitly. Depth resolve takes the nearest covered sample in that convention. The standard classifier uses 8 × 8 pixel tiles, including partial tiles at the frame edges. It enumerates every (tile, kind) pair present across covered samples. Shading groups samples only when the complete reconstructed observation is the same under the selected sampling rule. Sum radiance weighted by coverage, including background. Forward and secondary observers use matching geometry and epoch. A resized or forked codec invalidates its reader set and history at one revision boundary.

**Execution tiers.** Tier 1 is the published path for release, CI, motion and solvers. Tier 0 is a transient preview for the one render kind currently edited. Scene rows edited as data need only a table patch. The VM ABI is a header (version, register and instruction counts, kind-schema hash, constant and binding ranges), immutable constant pool, and fixed four-word instructions (opcode/result type, destination, packed operand register pair, third operand or immediate). Validate every operand, declared resource, initialized register, branch target and structured loop bound before upload. Registers carry scalar or vector words and a lane type. Deterministic noise and interpolation kernels use versioned library bodies shared with Tier 1.

| Tape instruction family | Semantics and eligibility |
|---|---|
| Data and arithmetic | Constant, parameter, row or checked table load, move, vector build/swizzle, conversion, comparisons, select, add/subtract/multiply/divide, explicit fused multiply-add, clamp, mix, dot, cross, length, normalize and scalar elementary functions. Expand other pure supported expressions to these. |
| Domain instructions | Affine point/vector/normal transform, mirrored/repeated cell with witness, signed-distance primitives and smooth combinations, fused verified noise. Implement the very same map, cell boundary, noise and f32 conventions as the native form. |
| Sampling and AD | Checked table interpolation, certified spline sampling and explicit-LOD texture sampling on a finite statically bound slot set. Lower forward AD to ordinary operations before emitting a tape. Implicit pixel derivatives, raw code, atomics, shared memory, arbitrary stores and barrier-dependent kernels require Tier 1. |
| Control | Uniform structured loop count, or a proven upper trip count with masking of inactive lanes. Predicate lane-varying conditionals including effect-free guarded loads. All lanes execute a single PC. Arbitrary data-dependent exit, unsafe speculative evaluation, unsupported resource ABI or exceeding the verified limits yields `NativePending`. |

| Edit contract | Predicted limit or rule |
|---|---|
| Eligible patch | At most 64 loop iterations, 256 vector registers, 512 tape instructions and 8 KiB instruction bytes. Target p95 under 2 ms from source receipt to verified enqueue on a resident ABI. Measure edit-to-first-present separately, including vsync. |
| Activation and promotion | Each workgroup receives one uniform tape id. Activate only when every affected observer can use a coherent revision. Start async Tier 1 compile after 400 ms quiet, immediately on explicit commit, load, CI or release. Every surviving edit promotes. |
| Scalar agreement | Finite field and shading components in their declared base units must satisfy absolute error ≤10⁻⁵ **or** distance ≤8 representable f32 values. Check sign classifications outside certified uncertainty and hit identity/depth at named probe rays. |
| Frame-boundary swap | Compare matching frames with identical pose, inputs, resources and keys. At least 99.9% of covered pixels have max sRGB channel difference ≤1/255 and ΔE00 ≤0.5. Any remaining error must be on a reference silhouette within one pixel and ≤2/255 per channel. Release each old resource after retirement of the last referencing submission. |
| Measured calibration | The measured tape overhead was +2.0 ms on the 63-op paint graph and +5.5 ms on the larger graph. Charge interpreted invocation count, including repeated field evaluations and secondary queries, when forecasting an edit. |

Never run mixed tape/native revisions across observers of the edited kind. An ineligible edit may keep displaying the previous complete revision labelled with its digest and `NativePending`. The first revision without an eligible preview remains pending until complete. A newer edit supersedes pending promotions. Install only a verified package whose revision, tables, schema, record codec and dependency digests still match. Compile failure keeps the last complete frame available with the diagnostic. When a state writer changes, an explicit preserve/convert/restart decision establishes a new checkpoint and history branch at the installation tick, recording the parent identity and migration digest. Include solver-owned persistent values in the compatibility check. Stage the entire migration into separate storage and publish the checkpoint and code handles together on success. A failed conversion leaves the prior revision selectable. Undo can reactivate a matching cached package and its matching state history.

**Numerical agreement.** Motion marked deterministic uses binary32 words for persisted floats and a frozen evaluation order. The same compiled `advance` handles bake, seek and live. Strict primitive operations have round-to-nearest, ties-to-even, signed zeros canonicalized to +0 and subnormals flushed to zero on entry and exit. Invalid/nonfinite results set a sticky per-row failure with source span and never become a successful hold. Evaluate reductions in an explicit stable tree, contact tests in stable site-id order and constraint colors in a fixed order. Jacobi gathers incident contributions in stable edge-id order. Randomness is counter-based or checkpointed. Opacity, races and unlogged external reads make an entry ineligible for the deterministic profile.

Implement the portable strict arithmetic as `u32` sign/exponent/significand operations with guard/round/sticky bits. Exact integer significand multiplication uses split limbs. Division uses quotient/remainder and square root an integer-root residual before rounding. An explicit FMA retains the product through the addition before the one round. Expand vectors into those scalar operations. For elementary functions use the identical versioned range reduction, coefficients/tables and strictly ordered arithmetic in both backends; the module digest includes these bytes. Qualify accelerated replacements only where a backend guarantees those exact results over the admitted operand range. A default native WGSL transcendental, native contraction or passing a few probe points supplies no such guarantee. This profile can be replaced by a consumer law with an explicit `Unknown(OpaqueTwin)` or replay capability refusal in the affected tools. [WGSL floating-point evaluation](https://www.w3.org/TR/WGSL/#floating-point-evaluation)

| Observable | Predicted acceptance criterion |
|---|---|
| Replay and CPU twin on one device profile | Identical STATE and post-IK POSE words at every scored tick and presentation fraction of each deterministic fixture, including restore and catch-up. Compare input/event cursors and verdict site ids exactly. |
| Across separately qualified devices | On the same named trace, categorical events/site identities agree; positions within 0.1 mm, rotations within 0.0001 rad geodesic angle, normalized other finite scalars within the scalar tier tolerance. A discrepancy is a reported failed portability check. Across-device pictures use the stated picture gate. Each checked scope names its device. |
| Arithmetic and scope | Changed law, input prefix, geometry, precision or projection invalidates the dependent verdict. A trace establishes evidence only on its stated observed samples. A continuous-time hold needs a certificate on the entire domain. A nonfinite result, exhausted proof/solver or precision ambiguity is Unknown with its actual cause. |

**Five release gates.** The same executable and manifest of expected scene/view/time results operate from the first slice. The fixtures grow with the implementation. A required unavailable device, empty observation set, missing image, stray substitute camera, duplicate identity, missing negative-fixture result or skipped pass makes the run incomplete and exits nonzero.

| Gate | Instrument | Predicted pass criterion |
|---|---|---|
| Picture | Explicit nonempty frame-key manifest; approved light-free ids/ink/depth, independent beauty plates, converged reference for radiometry, declared artistic changes and owner review. Compare every expected frame, individually. | Geometry reference: every off-edge sRGB channel within 1/255, ≤0.01% pixels exceed that value, each within one pixel of a reference depth discontinuity, maximum 4 pixels per four-connected error cluster. For a deliberate new look, approve a new plate while retaining analytic geometry and light fixtures. |
| Twin | Exact strict-motion rows and event traces, CPU/GPU intersection oracles, field/swap thresholds above, positive and negative diagnostics. | All named cases conform, with nonempty tick/ray/sample counts. Phase 0 exercises a deterministic numeric row recurrence even before there is a motion library. |
| Speed | Record GPU timestamped frame and pass spans, draws/dispatches after bundle expansion, plus separate clears, copies, submissions and CPU encode times. Count zero-instance draws. | Per-scene p95 and command budget both pass at the declared configuration. Unresolved spans below the measured M4 timestamp quantum of 0.065 ms are recorded as unresolved; repeat the same work in long batches for an amortized estimate. |
| Load | Observe fresh-driver-text cold and reused-text warm loads after device creation, plus build CPU, transfer, first draw, distinct module/descriptor hashes, cache state and emitted WGSL size. Report data upload separately from compile. | Within each scene limit; module and pipeline identity sets unchanged under row duplication from one to twenty crabs. Training build ≤2 s CPU and p95 GPU ≤50 ms/frame for 512² × 16 frames. An uncontrolled cache run cannot certify a cold-load budget. |
| Truth | Assert the exact expected per-site result, witness/failure reason and scope. Compare interval certificates and sampled margins separately. Score light transport against analytic furnaces/media and independent seed batches. | A positive requirement passes only on Held. An explicitly declared Missed or Unknown with matching reason tests correct reporting and confers no success on the underlying hold. Missing frames, unlabelled refusals and timeouts cannot pass. |

## 4. The render and light sub meta engines

**World, objects and observations.** The following are the source interfaces exported by `std::render`. Declaration-only listings are library headers. A handle names a typed value, table or pass output. A function-valued argument is resolved at composition time and its numeric captures become row fields. A material can capture its object frame or local chart. In a hit, geometric normal controls side tests and ray offsets. Shading normal controls the response. The shutter instant and ray footprint travel with a ray.

```loft
struct Ray[S] { o:Pt[S], d:Dir[S], lo:Meters, hi:Meters, radius:Meters, spread:f32, time:Sec }
struct LocalHit[S] { t:Meters, n:Norm[S], ns:Norm[S], uv:vec2f, prim:u32 }
enum Crossing[T] { HitValue(T), Miss, Unknown(Reason) }
struct Coverage { pixel:vec2u, sample:u32, depth:f32, time:Sec, coverage:f32 }
struct HitId { kind:u32, row:u32, prim:u32 }
struct Vertex[S] { p:Pt[S], ns:Norm[S], uv:vec2f }
enum Extent[S] { Finite(Box[S]), Infinite }
struct Weights { indices:Table[vec4u], weights:Table[vec4f] }
enum Index { Flat, UniformGrid, Groups, Bvh }
enum Interp { Hold, Linear, Cubic }
enum ColorSpace { Linear, Srgb, Data }
struct Reproject { previous_xy:vec2f, id:HitId, valid:bool }
struct Camera { eye:Pt[World], aim:Pt[World], up:Dir[World], projection:Projection,
                size:vec2u, coverage:u32, shutter:TimeRange, aperture:Meters, focus:Meters,
                samples:u32, bounces:u32, peels:u32, viewer_history:bool, depth:DepthOrder }
enum Projection { Perspective(Rad), Orthographic(Meters) }
enum DepthOrder { NearIsLess, NearIsGreater }
struct FrameSource[A,B] { at:fn(Sec)->Xform[A,B] }
struct Hit { p:Pt[World], n:Norm[World], ns:Norm[World], uv:vec2f, prim:u32, thing:Thing, t:Meters, time:Sec }
struct Box[S] { lo:Pt[S], hi:Pt[S] }
struct Beam { ray:Ray[World], key:Key }                     // open endpoints for emitter visibility
struct Grid { u:u32, v:u32 }                      // numbers of cells in the two directions
struct Mesh[S] { vertices:Table[Vertex[S]], triangles:Table[vec3u] }
struct GridImage[T] { size:vec2u, texels:Table[T] }
enum Rate { Build, Load, Tick, Frame, Row, Vertex, Pixel, Sample }
alias Rgb = vec3f;
alias Paint = fn(Hit)->Bsdf;
alias Engine = fn(World,Camera)->Schedule;
alias Field[S] = fn(Pt[S])->Meters;
alias Curve[S] = fn(f32)->Pt[S];
alias Patch[S] = fn(vec2f)->Pt[S];
alias Height = fn(vec2f)->Meters;
alias Sky = fn(Dir[World])->Rgb;

fn world()->World;                                           // Empty build handle with black escape radiance.
fn at_time(w:World,t:Sec)->World;                            // Sample the same shutter time for frame sources, moving lights and acceleration.
fn subset(w:World,keep:fn(Thing)->bool)->World;              // Bind an observer to an explicit alternative world.
fn add[S](w:World,g:Geometry[S],m:Paint,x:FrameSource[S,World])->Thing; // Append an opaque row or an alpha-cut row.
fn add_forward[S](w:World,g:Geometry[S],m:Paint,x:FrameSource[S,World])->Thing; // Append a surface evaluated in the forward pass.
fn frame(w:World,c:Camera,e:Engine)->Scene;                  // Freeze the world and reachable motions, select view and engine.
fn view(s:Scene,name:Name,c:Camera)->Scene;                   // Add an independently named camera to a scene.
fn identity[S]()->Xform[S,S];                                // Identity frame.  // Prelude re-export.
fn rigid[A,B](offset:Vec[B])->Xform[A,B];                    // Parallel local axes, origin at the offset from B's origin.  // Prelude re-export.
fn instances[S](w:World,g:Geometry[S],m:Paint,x:Table[FrameSource[S,World]])->Table[Thing]; // Stable rows for enumerated copies.
fn rgb(r:f32,g:f32,b:f32)->Rgb;                              // Linear working RGB.
fn box[S](lo:Pt[S],hi:Pt[S])->Box[S];                       // Local closed domain, validated componentwise.

fn trace(w:World,r:Ray[World])->Crossing[Hit];                 // Closest accepted crossing on [lo,hi), including exits.
fn visible(w:World,b:Beam)->Rgb;                             // Straight-path transmittance incl cutouts, thin filters and all media.
fn material(w:World,h:Hit)->Bsdf;                            // Evaluate material at the actual surface chart and shutter time.
fn light(w:World,p:Pt[World],k:Key)->LightSample;            // Draw one emitter and a point/direction, with full selection probability.
fn sky(w:World,d:Dir[World])->Rgb;                           // Emitted radiance reached on escape in direction d.
fn medium(w:World,p:Pt[World],t:Sec)->Coefficients;          // Sum overlapping coefficients and keep their phase weights.
fn pick(w:World,c:Camera,px:vec2f,t:Sec)->Crossing[Hit];       // Cast the view ray on the same pose and world.
```

The geometry body, material body and other reachable readings form the row's canonical kind. Numeric captures (including size, dimensions and pigment values) are row data. Changing geometry code or its material function can change the kind, even if its topology is the same. The six queries are open over this world. A static transform lifts to a constant frame source. World handles are immutable during execution. Re-emitting rows never changes stable IDs of surviving rows. A new geometry can supply these open kind readings. The overload set is the library's contract for constructing `Geometry[S]`. The cover and ray intersector agree on range, sidedness, accepted alpha, pose and actual geometry. An adapter states its observer and error. The planner fails at an unavailable reading, with its observer and the providing kind.

```loft
interface Bounded[Self,S] { query bound(row:Self,t:Interval[Sec])->Extent[S]; }                  // Enclose the swept object or return Infinite; unknown bounds disable culling.
interface Intersector[Self,S] { query hit(row:Self,r:Ray[S])->Crossing[LocalHit[S]]; }         // First crossing and outward normal or a checked failure.
interface Implicit[Self,S] { query dist(row:Self,p:Pt[S])->Meters; }                  // Zero set and sign, carrying the derived spatial certificate.
interface Parametric[Self,S] { query place(row:Self,uv:vec2f,t:Sec)->Pt[S]; }               // Parametric point for a grid, with declared edge topology.
interface Silhouette[Self,S] { query gap(row:Self,r:Ray[S])->Meters; }           // Signed miss distance for a traced outline or analytic coverage.
interface CoverageLaw[Self,S] { query mask(row:Self,h:LocalHit[S],t:Sec)->f32; }         // Shared cutout criterion and threshold for each observer.
fn geometry[S,K](row:RowId[K])->Geometry[S];                  // Bind a kind’s available shape readings and derive only certified adapters.
fn analytic[S](b:Extent[S],h:fn(Ray[S])->Crossing[LocalHit[S]])->Geometry[S]; // Custom exact intersection and its conservative extent.
fn field[S](b:Box[S],d:fn(Pt[S])->Meters)->Geometry[S];     // Restrict the zero set to b; sphere-trace certified pieces inside b.
fn grid(u:u32,v:u32)->Grid;                                  // Validate positive cell counts and indexed triangle winding.
fn surface[S](p:fn(vec2f)->Pt[S],g:Grid)->Geometry[S];       // Triangulate p on g; ray, shadow and pick hit those same triangles.
fn deformed[S](p:fn(vec2f,Sec)->Pt[S],g:Grid)->Geometry[S];  // Re-evaluate and refit at the posed time; cull over shutter bounds.
fn relief[S](base:Patch[S],h:Height,g:Grid)->Geometry[S];    // Root-isolate height crossings in a certified shell, including silhouettes.
fn brick[S](g:Geometry[S],spacing:Meters)->Geometry[S];      // Bake interval lower bounds, skip empty space, refine using original law.
fn surface_nets[S](g:Geometry[S],spacing:Meters)->Mesh[S];   // Extract an approximate polygonal zero set with a declared resolution.
fn spatial[S](parts:Table[Geometry[S]],index:Index)->Geometry[S]; // Query potentially contributing bounds via grid, group tree or BVH.
fn cell_cast[S](cell:Vec[S],motif:fn(vec3i)->Geometry[S],b:Box[S])->Geometry[S]; // DDA in ray order, clip to cell and include protruding neighbours.
```

An intersection returns one of `HitValue`, `Miss`, and `Unknown`. Only a certified empty interval yields `Miss`. If an unexcluded region precedes the first known hit, the result is `Unknown`. All world, lighting and engine functions have a checked observation effect. A nested unresolved crossing, failed reconstruction, invalid coefficient or invalid PDF propagates a reason and source site to its enclosing observation, including a pick or reference sample. The GPU writes this status to the sample receipt. The reference gate fails on any such sample. Replacing a failed sample with black or omitting it from the mean is invalid. Only a named preview adapter may substitute a value, recorded as approximate. Plain RGB signatures represent the successful value of this effect. `World` in a point parameter names the spatial frame, and in a function argument names the sampled world handle.

At a transform, retain world ray parameter or scale **both** endpoints and convert the returned distance. Clip a field to its true evaluation domain. A small residual alone does not bound distance to a root of a general field. Use the step lower bound to exclude space, an interval/bracket to certify a crossing, and the ray footprint to bound rendering error. A finite-precision tangent or exhausted budget can produce `Unknown` in checks and a recorded approximation in a preview. A leaky proxy, padded depth, or a drawable's rest skeleton cannot stand in for its posed hit. The exact secondary geometry for a raster patch is the tessellation actually drawn. Recover barycentrics for that triangle before interpolating attributes. An analytic patch can supply a separate exact hit reading and declare the permitted difference from its raster cover.

**Primitives, distances, folds and profiles.** Lengths below are positive where they parameterize radii, cell widths or periods. Coincident capsule endpoints reduce to a sphere. A singular transform is a typed error at inversion. The finite solid constructors use analytic roots with explicit tests for inside, tangent, parallel, cap and nearest root. The torus restricts to a ring torus and isolates ordered roots of its ray quartic, validating roots against the implicit equation. A quadric takes a symmetric homogeneous matrix in local coordinates scaled to metres. A general quadric supplies its extent separately.

```loft
fn sphere[S](r:Meters)->Geometry[S];                          // Sphere at the local origin, quadratic hit.
fn ellipsoid[S](axes:Vec[S])->Geometry[S];                   // Positive semiaxes, affine sphere hit with inverse-transpose normal.
fn solid_box[S](half:Vec[S])->Geometry[S];                   // Slab intersection, deterministic tied edge normal.
fn plane[S](n:Dir[S],offset:Meters)->Geometry[S];            // Infinite oriented plane dot(p,n)=offset.
fn capsule[S](a:Pt[S],b:Pt[S],r:Meters)->Geometry[S];       // Side cylinder and hemispherical caps.
fn round_cone[S](a:Pt[S],b:Pt[S],ra:Meters,rb:Meters)->Geometry[S]; // Tangent cone and sphere caps, handle containment.
fn cylinder[S](r:Meters,half:Meters)->Geometry[S];           // Finite capped cylinder on local y.
fn torus[S](major:Meters,minor:Meters)->Geometry[S];         // Ring around local y, major > minor.
fn quadric[S](q:mat4x4f,b:Extent[S])->Geometry[S];           // Solve ray quadratic or linear degeneracy; extent is a clip domain.
fn washer[S](inner:Meters,outer:Meters)->Geometry[S];        // Plane hit restricted to the annulus, including axial hole.

fn sd_sphere[S](p:Pt[S],r:Meters)->Meters;                   // Exact signed distance, spatial L≤1.
fn sd_box[S](p:Pt[S],half:Vec[S])->Meters;                  // Exact box distance, L≤1.
fn sd_round_box[S](p:Pt[S],half:Vec[S],r:Meters)->Meters;   // Box offset by r, L≤1.
fn sd_capsule[S](p:Pt[S],a:Pt[S],b:Pt[S],r:Meters)->Meters; // Distance to segment minus radius, L≤1.
fn sd_round_cone[S](p:Pt[S],a:Pt[S],b:Pt[S],ra:Meters,rb:Meters)->Meters; // True round-cone distance incl contained sphere, L≤1.
fn sd_cylinder[S](p:Pt[S],r:Meters,half:Meters)->Meters;    // Exact finite cylinder distance, L≤1.
fn sd_torus[S](p:Pt[S],major:Meters,minor:Meters)->Meters;  // Exact ring-torus distance, L≤1.
fn sd_plane[S](p:Pt[S],n:Dir[S],offset:Meters)->Meters;     // Oriented plane distance, L=1.
fn sd_ellipsoid[S](p:Pt[S],axes:Vec[S])->Meters;            // min(axes) times distance from the unit sphere in scaled space, L≤1.
fn sd_circle(p:vec2<Meters>,r:Meters)->Meters;              // Exact planar disk distance.
fn sd_rect(p:vec2<Meters>,half:vec2<Meters>)->Meters;       // Exact rectangle distance.
fn sd_segment(p:vec2<Meters>,a:vec2<Meters>,b:vec2<Meters>)->Meters; // Unsigned point-to-segment distance.
fn sd_ring(p:vec2<Meters>,r:Meters,half:Meters)->Meters;    // Exact annulus distance for r≥half.
fn sd_triangle(p:vec2<Meters>,v:array<vec2<Meters>,3>)->Meters; // Exact distance to oriented triangle boundary with winding sign.
fn smootherstep[T](lo:T,hi:T,x:T)->Unit;                 // Clamped quintic fade on strictly increasing bounds in the same units.
fn stroke(d:Meters,half:Meters)->Meters;                    // abs(d)-half, also strokes capsule profiles.
fn coverage(d:Meters,footprint:Meters)->f32;                // Filter across one positive pixel footprint.
fn unite(a:Meters,b:Meters)->Meters;                        // Minimum of signed fields.
fn intersect(a:Meters,b:Meters)->Meters;                    // Maximum of signed fields.
fn subtract(a:Meters,b:Meters)->Meters;                     // max(a,-b).
fn smin_poly(a:Meters,b:Meters,k:Meters)->Meters;           // Quadratic compact blend for k>0; convex gradient weights.
fn ssub_poly(a:Meters,b:Meters,k:Meters)->Meters;           // Smooth difference by negated smooth union.
fn smin_exp(a:Meters,b:Meters,k:Meters)->Meters;            // Stable soft min -k log(exp(-a/k)+exp(-b/k)), k>0.
fn round_field(d:Meters,r:Meters)->Meters;                  // d-r, an offset zero set.
fn shell_field(d:Meters,thickness:Meters)->Meters;          // abs(d)-thickness/2.
fn elongate[S](d:Field[S],half:Vec[S])->Field[S];           // Extend a centred shape along clamped local coordinates.
fn revolve[S](d:fn(vec2<Meters>)->Meters,offset:Meters)->Field[S]; // Rotate an offset planar profile about y.
fn extruded_field[S](d:fn(vec2<Meters>)->Meters,half:Meters)->Field[S]; // Intersect profile extrusion with two caps.
```

All exact signed distances above have the unit Lipschitz axiom. The scaled ellipsoid field is a conservative distance bound with the exact ellipsoid zero set. A general quadratic residual gets interval AD over its domain. Booleans preserve sign and an upper slope bound across kinks. Their numeric result need not equal distance everywhere. A smooth intersection is the negation of a smooth union of the negated operands.

```loft
fn mirror_x[S](p:Pt[S])->Mapped[S];                          // Reflect into x≥0, with a sign witness for the return branch.
fn translate[S](p:Pt[S],by:Vec[S])->Mapped[S];              // Shift into a fresh chart, inverse subtracts by.
fn rotate[S](p:Pt[S],axis:Dir[S],a:Rad)->Mapped[S];         // Rotate into a fresh chart, inverse uses -a.
fn rot_x[S](p:Pt[S],a:Rad)->Mapped[S];                     // Rotate the coordinate sample about the x axis.
fn rot_z[S](p:Pt[S],a:Rad)->Mapped[S];                     // Rotate the coordinate sample about the z axis.
fn repeat_grid[S](p:Pt[S],cell:Vec[S])->Mapped[S];          // Centred cells with signed integer xyz addresses.
fn repeat_finite[S](p:Pt[S],cell:Vec[S],extent:vec3u)->Mapped[S]; // Clamp chosen cell index to the declared range.
fn repeat_polar[S](p:Pt[S],n:u32)->Mapped[S];               // Rotate about y into one sector, preserve height and sector index.
fn kaleidoscope[S](p:Pt[S],planes:Table[Plane[S]])->Mapped[S]; // Sequence of plane reflections, witness contains ordered branches.
fn screw[S](p:Pt[S],pitch:Meters,sector:Rad)->Mapped[S];    // Screw symmetry with axial advance per angular turn.
fn spiral[S](p:Pt[S],scale:f32,angle:Rad)->Mapped[S];       // Log-radial cells and angular shear around y; reject axis singularity.
fn log_sphere[S](p:Pt[S],r0:Meters)->Mapped[S];             // Log-radius/direction chart with bounded radial intervals.
fn inversion[S](p:Pt[S],r:Meters)->Mapped[S];               // Spherical inversion with its local inverse, singular at origin.
fn warp[S](p:Pt[S],f:MapFn)->Mapped[S];                    // One-way chart incl twists/bends/noise, derive Jacobian enclosure.
fn invertible[S](p:Pt[S],f:MapFn,back:MapFn)->Mapped[S];    // Expose lift where both compositions verified on witnessed branch.
fn kifs[S](p:Pt[S],n:u32,step:MapFn)->Mapped[S];            // Fixed repeat of fold, scale and offset with a witness per level.
fn place_in[S,T](g:Geometry[S],chart:Xform[S,T])->Geometry[T]; // Carry geometry, attachments and normals through a frame.
```

Here `Mapped[S]` existentially binds the fresh local space from §2. `MapFn` is a map body, instantiated with distinct input and output spaces; it exposes coordinate evaluation to AD. Use a witness to lift only points on its valid branch. For normals pulled back through a coordinate map use its Jacobian transpose; pushing a normal forward through a placement uses the inverse transpose. For a repeat with discontinuous cell values intersect the ray with cell faces, assign the exact boundary by a half-open ownership rule, traverse every crossed cell and evaluate overlapping neighbours. A positive epsilon across a wall can skip a thin surface. An arbitrary conformal point scale alone does not certify a finite step across a singularity. Piecewise interval Jacobian bounds do. Parametric curves, paint, heights and density compose with the point map. Enumerated copies take a row generator; branching trees emit rows or use certified traversal. Screen-width ink derives the footprint before a discontinuous fold.

**Curves, surfaces, assets, noise.** Grid spacing is itself a reparameterisation, including crowded-end grids. The default grid has two triangles per cell. A joined periodic seam duplicates attributes where paint needs a seam. The library specifies cap, point, rounded and open ends through the generating profile and the closing faces. A geometric pole needs a one-sided normal construction.

```loft
fn keys[T](times:Table[Sec],v:Table[T],mode:Interp)->fn(Sec)->T; // Hold, linear or cubic keyed interpolation.
fn bspline[T](knots:Table[f32],v:Table[T])->fn(f32)->T;       // B-spline with continuity from degree and knot multiplicity.
fn through[S](points:Table[Pt[S]])->Curve[S];                // Interpolating curve in the given order.
struct Basis[S] { origin:Pt[S], tangent:Dir[S], normal:Dir[S], binormal:Dir[S] }
fn frame_seed[S](p:Pt[S],t:Dir[S],up:Dir[S])->Basis[S];      // Orthogonalise up against a nonzero tangent; require a noncollinear seed.
fn transport_step[S](last:Basis[S],p:Pt[S],t:Dir[S])->Basis[S]; // Minimal rotation from prior tangent; diagnose unresolved reversal/cusp.
fn sample_c2[S](tab:Table[Basis[S]],u:Unit)->Basis[S];       // Match value/first/second derivatives, sign-lift/fix quaternion spline and renormalize.
fn parallel_transport[S](c:Curve[S],up:Dir[S],n:u32,rate:Rate)->Table[Basis[S]]; // Ordered rotation-minimising scan; bake static spines, update moving spines before draws.
fn sample_c2[T:Euclidean](t:Table[T],u:f32)->T;                      // C² spline fit with matching knot derivatives; frames use a continuous sign lift and regular normalisation.
fn grid_box[S](p:Patch[S],domain:Box2)->Extent[S];          // Interval envelope of a patch or Infinite if uncertified.
fn sweep[S](spine:Curve[S],section:fn(vec2f)->vec2<Meters>,frames:Table[Basis[S]])->Patch[S]; // Carry the section in transported frames.
fn loft[S](sections:Table[Curve[S]])->Patch[S];             // Interpolate registered curves in the transverse parameter.
fn lathe[S](profile:fn(f32)->vec2<Meters>)->Patch[S];       // Revolve a radius/height curve.
fn extrude[S](edge:Curve[S],by:Vec[S])->Patch[S];           // Sweep the edge along a vector.
fn sheet[S](a:Curve[S],b:Curve[S])->Patch[S];               // Ruled sheet between two posed edges.
fn thicken[S](p:Patch[S],half:Meters,g:Grid)->Geometry[S];  // Offset both sides and stitch open boundary edges.
fn displaced[S](p:Patch[S],height:Height)->Patch[S];        // Offset along geometric normal; differentiate the resulting parameterisation.
fn sculpt(h:GridImage[ Meters ],wrap:vec2<bool>)->Height;  // Bilinear sampled relief with explicit seam wrapping and C⁰ certificate.
fn ridge(curve:fn(f32)->vec2f,width:f32,amount:Meters)->Height; // Chart-distance profile around a path; negative amount cuts a crease.
fn toroidal_offset(a:vec2f,b:vec2f,wrap:vec2<bool>)->vec2f; // Shortest signed chart displacement for wrapped patch editing.
fn mesh[S](v:Table[Vertex[S]],tri:Table[vec3u])->Geometry[S]; // Indexed triangles with chart, normal and material slots.
fn import_gltf(uri:Name)->Asset;                             // Read buffers, mesh attributes, images, skins, nodes and animation channels; diagnose unsupported required extensions.
fn export_gltf(s:Scene,times:Table[Sec],uri:Name)->Result;   // Sample evaluated mesh and pose, write rest binds and local TRS with scale.
fn skin_mesh[S](m:Mesh[S],j:Table[Xform[S,World]],weights:Weights)->Geometry[World]; // Apply skinned positions consistently to all observers.
fn indexed[S](m:Mesh[S])->Geometry[S];                     // Draw a mesh value from extraction, import or explicit assembly.
fn points[S](p:Table[Pt[S]],billboard:fn(u32,vec2f)->Vertex[S])->Geometry[S]; // Instanced point sprites/ribbons, with cap/cull/coverage explicit.
fn texture(uri:Name,space:ColorSpace)->Texture;              // Load image/mips; decode colour or linear data as declared.
fn sample_lod(tex:Texture,uv:vec2f,lod:f32)->vec4f;         // Explicit mip and sampler rules; bounded filtering when texel bounds are available.
fn hash_cell(i:vec3i,seed:u32)->u32;                       // Integer keyed hash, discontinuous in the cell address.
fn value_noise(p:vec3f,seed:u32)->f32;                     // Quintic tensor interpolation of hashed lattice values in [0,1], C².
fn worley(p:vec3f,seed:u32)->f32;                          // Nearest-feature distance, C⁰ and L≤1; visit all cells whose box can beat the current distance.
fn smooth_worley(p:vec3f,sites:Table[vec3f],k:f32)->f32;    // Stable exp smooth-min of regularised distances sqrt(r²+k²) to fixed sites, k>0.
fn fbm(p:vec3f,n:u32,lac:f32,gain:f32,seed:u32)->f32;     // Sum value-noise octaves with deterministic seeds and weights gain^i.
```

For value noise a coordinate partial is a convex combination of neighbouring corner differences times the quintic fade derivative. Bound each component by the largest absolute edge difference on the relevant cell times the maximum fade slope on the active interval. Subdivide cells intersecting the footprint; bound the Euclidean norm by the root-sum-square of the component bounds. Across all cells the bound is at most 15√3/8. Fixed-site smooth Worley has derivative norm at most one by convex weights; a feature-list change across cells requires a new seam certificate. Cellular IDs are discontinuous. A code change invalidates the certificate. Filtering or moving displacement to shading changes the surface and must be a deliberate author edit.

**Material and light.** A `Bsdf` is a closure over one open response kind. Continuous evaluation returns BSDF times absolute incident cosine in the local shading frame, with geometric-side and shading-normal consistency. Sampling returns the same measure in its weight. An atom carries probability **mass**, a continuous direction a solid-angle **density**. Selection accounts for the probability of a lobe, emitter and sampled point. Delta mass never participates in a solid-angle density comparison.

```loft
struct Scatter { wi:Dir[World], weight:Rgb, pdf:f32, delta:bool }
struct LightSample { wi:Dir[World], Le:Rgb, beam:Beam, pdf:f32, delta:bool }
alias Radiance = Rgb;  // Linear scene-referred radiance in the chosen energy normalization.
struct Coefficients { absorption:Rgb/Meters, scattering:Rgb/Meters, source:Radiance/Meters, phase:PhaseMixture }
struct Segment { L:Rgb, T:Rgb }
alias DirW = Dir[World];
interface Response[Self] {
    query eval(m:Self,h:Hit,wo:DirW,wi:DirW)->Rgb;          // Continuous non-delta f|n·wi|, zero off support.
    query pdf(m:Self,h:Hit,wo:DirW,wi:DirW)->f32;           // Marginal density of the continuous sampling law per steradian.
    query sample(m:Self,h:Hit,wo:DirW,k:Key)->Scatter;      // Sample a tagged event; weight contains f|cos|/pdf or atomic coefficient/mass.
    query emitted(m:Self,h:Hit,wo:DirW)->Rgb;               // Outgoing emitted radiance, including its sidedness.
    query albedo(m:Self,h:Hit)->Rgb;                        // Descriptive diffuse colour used by looks; energy checks integrate eval instead.
}
fn matte(base:Rgb)->Bsdf;                               // Lambert diffuse with cosine sampling.
fn metal(base:Rgb,rough:f32)->Bsdf;                     // Compensated conductor GGX.
fn ggx(base:Rgb,rough:f32,metalness:f32)->Bsdf;         // Energy-partitioned dielectric/conductor blend.
fn microfacet(D:Distribution,G:Masking,F:Fresnel)->Bsdf; // Compose a lobe, derive/cache directional albedo for these exact terms.
fn compensate(b:Bsdf)->Bsdf;                            // Add rough microfacet multiple-scattering term from lobe-specific directional albedo.
fn mix_bsdf(a:Bsdf,b:Bsdf,weight:f32)->Bsdf;            // Convex mixture with marginal PDF accounting for both proposals.
fn coat(base:Bsdf,weight:f32,rough:f32,ior:f32)->Bsdf;  // Fresnel coat with attenuation of the underlying response.
fn sheen(base:Bsdf,tint:Rgb,rough:f32)->Bsdf;           // Cloth-grazing layer with integrated base attenuation.
fn thin_film(base:Bsdf,ior:f32,thickness:Meters)->Bsdf; // Thin-film RGB reflectance projected under the declared working illuminant.
fn glass(ior:f32,tint:Rgb)->Bsdf;                       // Dielectric reflection/refraction, TIR and radiance-mode eta factor.
fn thin_glass(ior:f32,tint:Rgb)->Bsdf;                   // Parallel thin sheet with Fresnel reflection and tinted straight-through transmission incl internal reflections.
fn sss(base:Bsdf,albedo:Rgb,radius:Meters)->Bsdf;       // Refractive boundary and interior medium, random-walk reference; marked diffusion fit allowed in realtime.
fn emission(base:Bsdf,Le:Rgb)->Bsdf;                    // Attach directional radiance; retain scattering, register surface for light sampling when area sample exists.
fn custom_response[T](x:T)->Bsdf;                       // Bind open readings of T; compile the reachable cases only.
fn attributes[T](w:World,th:Thing,paint:fn(Hit)->T);   // Attach typed chart/thing paint words for a look.
fn read_attributes[T](w:World,h:Hit)->T;                // Resolve the required reading at build, fail on a missing producer.
fn anisotropic(base:Rgb,rough:vec2f,tangent:DirW)->Bsdf;     // Elliptical microfacet slopes in the projected tangent frame.
fn ggx_D(nh:f32,rough:f32)->f32;                        // Trowbridge–Reitz normal distribution.
fn smith_G(nl:f32,nv:f32,rough:f32)->f32;               // Height-correlated GGX masking-shadowing.
fn schlick(vh:f32,f0:Rgb)->Rgb;                         // Schlick Fresnel fit at given normal-incidence reflectance.
fn directional_albedo(b:Bsdf,nv:f32)->Rgb;              // Numerically integrate actual response per lobe signature and parameters.
```

`Distribution`, `Masking` and `Fresnel` are respectively the function types of the last three analytic terms. A custom continuous lobe may use a cosine proposal only on a support it covers. Transmission needs a proposal on the transmitted hemisphere as well. A custom delta lobe must supply its sampler. Exact emission and sampling remain available independently. An emitter without an area sampler is visible to continued paths with competing light density zero. Record which proposals exist. A planet-sized sky, directional delta sun, point light and finite emitter are distinct distributions. Summing unweighted lobe terms is permitted in a custom response, with a furnace verdict describing the resulting material.

```loft
interface Source[P] {
    query sample_light(p:P,at:Pt[World],t:Sec,k:Key)->LightSample; // Conditional measure given this emitter (area to solid-angle Jacobian for finite surface).
    query light_pdf(p:P,at:Pt[World],wi:DirW,t:Sec)->f32;         // Conditional density for the corresponding non-delta path.
    query power(p:P,t:Sec)->f32;                                 // Finite nonnegative proposal weight; the world normalizes mixture probabilities.
}
fn add_emitter[P](w:World,source:P);                        // Register a custom Source and its readings, including for MIS.
interface MediumLaw[P,S] {
    query coefficients(p:P,x:Pt[S],t:Sec)->Coefficients;    // Absorption, scattering, emitted source and phase per length.
    query majorant(p:P,b:Box[S],t:Interval[Sec])->Rgb/Meters;      // Return a verified outward bound for channel-wise total extinction.
}
fn custom_medium[P,S](p:P)->Medium[S];                     // Bind coefficients and a certified majorant; raw bounds cannot supply proof.
fn add_sun(w:World,toward:DirW,E:Rgb);                  // Directional delta emitter with prescribed normal irradiance.
fn point_light(w:World,p:Pt[World],intensity:Rgb);      // Intensity divided by squared distance, discrete selection probability.
fn area_light(w:World,th:Thing);                        // Register or reuse the same emitter id (deduplicate by thing); use surface sampling and transform area PDF to solid angle.
fn environment(w:World,s:Sky);                          // Register escape radiance and an environment sampling proposal.
fn env_map(tex:Texture,rotation:Rad)->Sky;              // Latitude-longitude HDR with solid-angle importance sampling.
fn daylight(sun:DirW,turbidity:f32)->Sky;               // Directional sky radiance, explicit sun-disc ownership.
fn add_medium[S](w:World,b:Extent[S],m:Medium[S],x:FrameSource[S,World]); // Register transformed coefficients and boundary.
fn homogeneous[S](a:Rgb/Meters,s:Rgb/Meters,j:Radiance/Meters,g:f32)->Medium[S]; // Constant absorption/scattering per metre and emission per metre; HG phase.
fn density[S](c:fn(Pt[S],Sec)->Coefficients)->Medium[S]; // Heterogeneous coefficients for bounded quadrature or tracking.
fn gaussian[S](centre:Pt[S],radii:Vec[S],c:Coefficients)->Medium[S]; // Gaussian-scaled coefficients with analytic segment optical depth.
fn optical(w:World,b:Beam)->Rgb;                        // Integrate summed extinction along segment; exact component integrals when available.
fn compose(front:Segment,back:Segment)->Segment;       // (Lf+Tf*Lb, Tf*Tb), in physical front-to-back order.
fn inscatter(w:World,b:Beam,k:Key)->Segment;            // Integrate summed emission and direct in-scattering inside the same transmittance.
fn direct(w:World,h:Hit,wo:DirW,k:Key)->Rgb;            // Sample one direct source with true visibility and inverse proposal probability.
fn many_lights(w:World,h:Hit,wo:DirW,n:u32,k:Key)->Rgb; // RIS over n proposals; use aggregate weights, one shadow sample.
fn ao(w:World,h:Hit,r:Meters,k:Key)->f32;              // Hemisphere occlusion in the same posed world for every geometry.
fn reflection(w:World,h:Hit,wo:DirW,k:Key)->Rgb;        // Sample glossy continuation and trace real offscreen geometry to direct+emitted or sky.
fn gi(w:World,h:Hit,wo:DirW,k:Key)->Rgb;                // One diffuse indirect continuation with the same receiver queries.
fn shadow_map(w:World,c:Camera,size:vec2u)->Image;      // Draw all casters from a light; finite resolution and bias are priced approximations.
fn temporal(old:History,now:Image,reproject:Reproject)->Image; // Validate history before merging; statistics and disocclusion mask travel with it.
```

A `LightSample` produced by an individual source has a conditional PDF or atomic mass. The world-level `light` multiplies it by the selected emitter probability. Its `Le` carries the incident radiance factor for that sample (point sources include inverse-squared distance). A positive contribution outside an explicit light proposal's support still receives the correct hit-emission weight from path continuation. The source factory registers an emissive surface once by stable thing identity. `Ray.lo`/ `Ray.hi` clip the visibility interval; a finite emissive target lies at its excluded far endpoint. Refractive and subsurface *interiors* require closed geometry or an explicitly bound closed volume. Thin sheets supply their own reflection and straight-through response.

Units on coefficients are carried at their constructors. `source` is radiance per unit length. Phase is a scattering-coefficient-weighted mixture per colour channel. Extinction adds before evaluation of transmittance. In a uniform segment with extinction σ and source j, (T=e^{-σd}), (L=(1-T)j/σ), using the continuous limit at zero. Forward surfaces split the segment at actual depths. A path through thick glass refracts and must continue as a scattering path. A straight shadow beam can cross null boundaries, coverage holes and declared thin filters. A refracting interface terminates the straight beam: bent caustic paths require another technique. In the reference, solid-angle PDFs, continuous/delta measures and reciprocal emitter densities are exposed for MIS. An approximation such as temporal reservoir reuse, a shadow map or a diffusion fit is named in the plan, and scored against the reference with the same bounce policy.

**Camera, sampling, non-PBR and post.** The camera declares projection, view size, depth convention, coverage pattern, shutter, aperture, light/path sample count and path depth. The profile also carries target format, viewer-history request and maximum forward layers. These are invocation data except when a target layout or schedule specialization actually changes. Camera constructors default to 1280 × 800, near-is-less depth, focal distance equal to the eye–aim separation, pinhole aperture, instantaneous shutter at the requested time, four coverage samples, one radiance sample, eight visited path vertices, one allowed forward layer and history off. The caller can override each field for a named view or host target. The print benchmark requests one coverage sample. The compiler infers fragment footprints through screen derivatives only in permitted uniform control flow; compute uses explicit ray differentials or finite footprints.

```loft
fn look_at(eye:Pt[World],aim:Pt[World],up:DirW,fovy:Rad)->Camera; // Perspective camera; refuse a zero baseline or collinear up.
fn orthographic(eye:Pt[World],aim:Pt[World],up:DirW,height:Meters)->Camera; // Parallel rays with constant footprint.
fn depth_of_field(c:Camera,focus:Meters,aperture:Meters)->Camera; // Lens sampling focused on the named plane.
fn motion_blur(c:Camera,open:Sec,close:Sec)->Camera;       // Sample geometry, lights, acceleration and motion at shutter time.
fn ray(c:Camera,px:vec2f,k:Key)->Ray[World];                // Map film, lens and time samples to a world ray.
fn project(c:Camera,p:Pt[World])->vec3f;                    // Screen xy and camera-convention depth.
fn expose(im:Image,ev:f32)->Image;                          // Linear multiplier 2^ev.
fn tone(im:Image,f:fn(Rgb)->Rgb)->Image;                    // Apply selected display mapping once on output.
fn aces(c:Rgb)->Rgb;                                       // Fitted filmic curve for the declared linear working space.
fn bloom(im:Image,r:f32,gain:f32)->Image;                   // Linear HDR multiscale bright-pass before display mapping.
fn grain(im:Image,strength:f32,k:Key)->Image;               // Reproducible film or paper texture at chosen display stage.
fn grade(im:Image,lut:Texture)->Image;                      // Apply the named colour-space transform/LUT.
fn outline[Record](r:Records[Record],method:InkMethod)->Image; // Silhouette/gap, depth/ID or hull ink as a separate mask.
fn ink_mask(value:f32,cut:f32,width:f32)->f32;             // Band edge with an explicit filter width.
fn overprint(paper:Rgb,ink:Rgb,coverage:f32)->Rgb;          // Multiply by ink transmittance at the chosen coverage.
fn light_fold[T](w:World,h:Hit,z:T,term:fn(T,LightSample)->T,k:Key)->T; // Enumerate lights with a receiver-keyed sample per emitter.
fn sheet_image(views:Table[Image],columns:u32)->Image;      // Assemble camera views or times into a contact sheet.
```

Materials and looks can carry typed paint channels such as wood fibre or a foil mark using `attributes`. A look can inspect the channel, use only an emitter silhouette, replace the light fold by a maximum or replace the entire shade function. Toon uses filtered steps in the light fold; ink uses gap, hull or screen contours. Woodblock uses ordered ink coverage and overprint. Painterly and sketch looks attach brush or stroke fields in surface coordinates. Papercraft uses folded normals, paper and crease passes. Neon and bioluminescence use emission, bloom and deliberate reflection. None requires a change to the record. Real shadow or reflection observations remain available whenever the look reads them.

**Schedules and integrator primitives.** `World` is a typed binding for all observers, `Schedule` a build-time list of versioned pass declarations. These are public functions that construct passes using §3's general compute and raster commands. A pass builder that returns a handle appends its producer by dependency. Repeated uses of a handle read one version. `Image` here is a linear image handle unless it has passed through a display transform. `Key` is a counter-based stream with independent named dimensions derived from pixel, sample, bounce and experiment seed.

```loft
fn plan(w:World,c:Camera)->Schedule;                         // Collect only reachable passes and frame dependencies.
fn clear_counts(s:Schedule,w:World)->Counts;                 // Zero compacted list counters, including tile append counters.
fn cull(s:Schedule,w:World,c:Camera,counts:Counts)->Args;    // Build fixed slices, visible-row lists and indirect arguments from posed bounds.
fn record[R](s:Schedule,w:World,c:Camera,a:Args,codec:Codec[R])->Records[R]; // Render opaque coverage in one indirect draw per active batch.
fn vis64()->Codec[Vis64];                                   // Public encoder, reconstruction, background and target formats from §3.
fn classify[R](s:Schedule,r:Records[R],counts:Counts)->Tiles; // Append one work item per distinct (tile,kind); update indirect dispatch counts atomically in this pass.
fn deferred[R](s:Schedule,w:World,r:Records[R],tiles:Tiles,f:Shade)->Image; // Per-kind indirect dispatch shades into separate coverage samples.
fn reflect_pass[R](s:Schedule,w:World,c:Camera,r:Records[R],im:Image)->Image; // Add traced glossy continuation, supply sky on uncovered samples.
fn peel(s:Schedule,w:World,c:Camera,a:Args,r:Depths,layer:u32,old:Layers)->Layers; // Select next forward interface deeper than old and before opaque.
fn no_layers()->Layers;                                     // Empty forward layer list.
fn media_forward(s:Schedule,w:World,c:Camera,r:Depths,ls:Layers,opaque:Image)->Image; // Interleave layer responses and integrated segments in depth order.
fn depths[R](r:Records[R])->Depths;                          // Borrow per-sample depths, preserving their camera convention.
fn history[R](s:Schedule,w:World,c:Camera,r:Records[R],im:Image)->Image; // Reproject validated data in a viewer; without history returns im.
fn print_pass[R](s:Schedule,w:World,c:Camera,r:Records[R],f:Shade,bg:Rgb)->Image; // Reconstruct each covered sample, apply f, resolve coverage with bg.
fn output(s:Schedule,im:Image,display:fn(Rgb)->Rgb)->Schedule; // Resolve and present, with one chosen output transform.
fn linear(c:Rgb)->Rgb;                                      // Preserve linear values up to the target's declared transfer encoding.
fn any_forward(w:World)->bool;                              // Test whether the build contains a forward batch.
fn screen_samples(s:Schedule,c:Camera,f:fn(vec2u,u32)->Rgb)->Image; // Dispatch one sample/pixel, add to the sum, repeat c.samples times, retain count.
fn average(im:Image)->Image;                                // Resolve linear sum/count during the consuming output pass.
fn inspect_pixel(w:World,r:Ray[World])->TraceReport;         // Bounded list of candidates, intervals, accepted hit, medium and source sites.

alias Shade = fn(World,Hit,DirW,Key)->Rgb;
enum VertexEvent { Escape(DirW), Surface(Hit), Volume(VolumeSite), Absorbed }
struct VolumeSite { p:Pt[World], time:Sec, channel:u32, phase:PhaseMixture }
struct PhaseSample { wi:DirW, weight:f32, pdf:f32 }
interface Phase[P] {
    query phase_eval(p:P,wo:DirW,wi:DirW)->f32;            // Radiance scattering density per steradian.
    query phase_pdf(p:P,wo:DirW,wi:DirW)->f32;             // Probability density for this proposal.
    query phase_sample(p:P,wo:DirW,k:Key)->PhaseSample;    // Weight equals eval/pdf and sampled support must cover phase support.
}
fn henyey_greenstein(g:f32)->PhaseMixture;                 // Normalized anisotropic phase about the incoming propagation direction, |g|<1.
fn phase_mix(weights:Table[Rgb],phases:Table[PhaseMixture])->PhaseMixture; // Normalize scattering weights independently for each channel.
struct Walk { at:VertexEvent, j:Rgb, weight:Rgb }
struct Connection { value:Rgb, light_pdf:f32, scatter_pdf:f32, delta:bool }
fn key(pixel:vec2u,sample:u32,bounce:u32,dimension:u32)->Key; // Disjoint named substreams, no dependence on queue order.
fn channel(k:Key)->u32;                                     // Uniform choice of one RGB transport channel.
fn reconstruct(c:u32,x:f32)->Rgb;                           // Return a vector with 3*x in the selected channel, zero elsewhere.
fn first_event(w:World,r:Ray[World],c:u32,k:Key)->Walk;      // Nearest boundary or sampled volume event over r; integrate source along survived distance.
fn event_emission(w:World,v:VertexEvent,wo:DirW)->Rgb;       // Surface emission, sky on escape, zero on a scatter/absorb event.
fn terminal(v:VertexEvent)->bool;                           // True on escape or absorption.
fn emitter_pdf(w:World,r:Ray[World],v:VertexEvent)->f32;    // Directional light proposal density from previous vertex to reached emitter, zero off its support.
fn connect(w:World,v:VertexEvent,wo:DirW,c:u32,k:Key)->Connection; // Draw NEE source, evaluate scattering/phase × Le × transmittance; full proposal measures.
fn scatter(w:World,v:VertexEvent,wo:DirW,c:u32,k:Key)->Scatter;    // BSDF or phase continuation in the selected channel; tag atoms and absorption.
fn spawned(v:VertexEvent,wi:DirW,t:Sec)->Ray[World];         // Geometric-side origin offset with error bound, preserve shutter instant.
fn power_weight(a:f32,b:f32)->f32;                          // a²/(a²+b²) on a sampled technique with a>0.
```

**Public pass and record constructors.** A `Resource` is a handle and a type/layout descriptor. A `Version` pairs it with its writer serial, region and history offset. Read-after-write order follows those edges. A pass builder writes new versions of its destinations. Schedule bodies and these functions can be combined in one engine. Input and output tuples of typed entry functions flatten to declared resources in parameter order. The validator checks count, dimension, sample type, format, stage access, shape and alignment. Device requirements and all reads, including sampler and raw reads, must be declared.

```loft
enum Clock { Build, Load, Tick, Frame }
enum Domain { Direct(vec3u), Indirect(Version,u32) }
enum Count { Fixed(u32,u32), Indexed(Version,u32), Vertices(Version,u32) }
enum Load { Keep, Clear(vec4f), ClearUint(vec4u), ClearDepth(f32) }
enum Format { Rgba8Unorm, Rgba16Uint, Rgba16Float, Rgba32Float, Depth32Float, R32Uint }
enum Blend { Opaque, Add, PremultipliedOver }
enum Cull { None, Back, Front }
enum Topology { TriangleList, TriangleStrip, LineList, PointList }
enum Test { Always, Less, LessEqual, Greater, GreaterEqual, Equal }
enum InkMethod { Hull, Gap, IdDepth }
struct RasterState { topology:Topology, cull:Cull, depth:Test, depth_write:bool, blend:Blend }
struct Attachment { destination:Resource, load:Load, store:bool }
struct Entry { module:ModuleId, function:Name, abi:Layout }
enum Indices { Unindexed, U16(Version), U32(Version) }
struct MeshInputs { vertex_buffers:Array[Version], indices:Indices }
struct Program { vs:Entry, fs:Entry }
struct Codec[R] { planes:Array[Format], background:R,
                  pack:fn(World,Hit,Coverage)->R, id:fn(R)->HitId,
                  unpack:fn(World,Camera,R,Coverage)->Crossing[Hit],
                  reproject:fn(World,Hit,Coverage)->Reproject }

fn buffer[T](s:Schedule,n:u32,initial:Array[T])->Resource;    // Allocate typed storage; initial may be empty for a later fill.
fn target(s:Schedule,dims:vec2u,f:Format,samples:u32)->Resource; // Allocate a typed attachment or storage view.
fn current(r:Resource)->Version;                              // Bind the version produced by the last declared writer.
fn previous(r:Resource,delay:u32,initial:Version)->Version;    // Retain a positive-delay version, initialize explicitly on reset.
fn entry[T](f:T)->Entry;                                      // Resolve a typed or raw function, fixed layout and source spans.
fn compute(s:Schedule,clock:Clock,d:Domain,k:Entry,reads:Array[Version],writes:Array[Resource])->Array[Version]; // Append dispatch and return new versions.
fn raster(s:Schedule,clock:Clock,c:Camera,p:Program,inputs:MeshInputs,args:Count,state:RasterState,reads:Array[Version],outputs:Array[Attachment])->Array[Version]; // Append draw and return colour/depth versions.
fn build_args(s:Schedule,w:World,a:Table[Thing],f:fn(Thing)->bool)->Args; // Select rows with a consumer culler, compact and fill indirect args.
fn geometry_shader[S](g:Geometry[S],p:Program,inputs:MeshInputs,state:RasterState)->Geometry[S]; // Override a kind's primary cover, retain separately supplied equivalent ray readings.
fn pack_lights(w:World)->Table[Emitter];                       // Access every registered emitter, its sampler, density and static/time-dependent power.
fn as_image(v:Version)->Image;                                // Typed view of a single immutable colour image version.
```

`Count::Fixed(vertices,instances)` is a direct nonindexed draw. `Indexed` and `Vertices` carry an indirect buffer version and byte offset. A raw function uses the same entry constructor and declared resources. A larger compute or raster shader can supply explicit vertex, fragment or workgroup interfaces. Pass bodies, row extraction and projections are source. `record` pairs the selected cover, encoder and depth writer in the generated family. The codec’s planes concatenate the scalar or packed-vector fields of R in declaration order, with explicit padding for unused channels. `unpack` receives camera/sampling data and the posed row directory. Record overflow and a decoded row outside the current epoch are diagnostic failures. Indexed reconstruction of geometry-specific attributes may return `Unknown` with the offending primitive. A custom record can include motion vectors or extra attachments by changing R, its plane formats and these functions together. A decoder can re-run the same hit from a depth interval when direct reconstruction is insufficient.

A surface supplying a different `Codec[R]` writes a covered-sample payload, decodes it using sample coordinate, depth, camera and posed rows, enumerates kind/row, and defines background and reprojection identity. This is a typed producer/consumer contract. Indirect offsets, counters and generation mismatches fail validation. A shaded tile may contain several kinds. Each invocation writes disjoint covered samples or disjoint per-kind partials with an explicit resolve. A contested non-atomic addition to a pixel is invalid.

The standard forward pass is depth peeling with a declared layer budget; it selects the nearest remaining interface using depth tests and stable tie order. Transparent backgrounds and media split the ray into ordered intervals. Each composition pass reads a predecessor version, so sampling scene colour or depth cannot alias an attachment being written. A cap that can hide an interface requires a proved depth-complexity bound, an exact overflow pass, or an explicitly accepted approximation; the default exact profile refuses a scene without a bound or exact overflow handler. Refraction in the realtime pass follows one traced refracted ray, with the truncation named in the plan. Shadows march every intersecting geometry and medium at its current pose. A moving light can instead bind a shadow map found from its camera. At thin boundaries coverage, transmission and media are evaluated once. Registered lights with precisely zero power can be excluded; every positive-contribution emitter has sample support or is reached by the continuation estimator.

The `first_event` reference implementation chooses one RGB channel per camera sample. It samples unit exponential optical depth on that channel through the **summed** extinction up to the nearest surface. Homogeneous intervals use inverse optical depth, Gaussian intervals use monotone inversion of the analytic integral, other fields use null tracking with a certified majorant. Draw a distance, accept with σₜ/majorant, then scatter with probability σₛ/σₜ or absorb. This analogue walk has continuation weight one at a real event; sample its mixed phase conditional on scattering. A zero-extinction interval is crossed directly. Integrate emission density j along the actually travelled distance without a second survival factor, analytically or with unbiased uniform position sampling (bounded intervals). On unbounded media require a finite integral and an integrable sampling distribution. At the real vertex `connect` separately estimates direct illumination. Transmittance for a reference connection is analytic or unbiased ratio tracking, with independent random dimensions. An invalid coefficient bound, nonfinite sample or missing medium interval is a failed reference sample, never silently discarded. The selected channel determines the phase proposal at a volume vertex. At a surface the sampler uses its RGB proposal and the path takes the selected component of its weight. Both MIS densities refer to the proposals actually drawn. Bounce limits specify the finite-depth quantity being estimated. A convergence verdict compares at that same depth. [PBRT, A Better Path Tracer](https://pbr-book.org/4ed/Light_Transport_I_Surface_Reflection/A_Better_Path_Tracer)

History belongs to the viewer. Reproject the same local sample through the prior pose and camera, then require matching epoch/row/primitive or chart, consistent surface depth/normal, and a valid material/light revision. Reject at disocclusion, state seek, cut, engine change or invalid motion correspondence. A cold still, clip or training frame uses only its declared within-frame samples. A temporal estimator carries its bias and reset history in the identity. Coverage samples, path/light samples, lens samples and shutter samples are separate sampling dimensions.

**Legacy capability coverage.** These are porting destinations for capabilities in the signature inventory. A recipe uses the declared geometry, motions or passes directly. It can stay local to a scene until a second author needs the same law.

| Legacy family | v1 construction and disposition |
|---|---|
| Chart, station table, warp, sweep, symmetry, tree, grow, branch lists | Compose witnessed maps for queries; enumerate transformed rows for draws. A screw adds rotation and axial advance, a helix evaluates the screw at successive indices. Branch visibility uses an indexed list or bounded spatial traversal. A spiral, coil and tube use curves, a section law and the transported frame. |
| Strand, ropes, fan, crest, sheets, thick skins, patches, brush/relief | Sweep a shaped section, use a catenary between two curves, or interpolate the sheet's edges. A framed surface exposes its point, two tangents and normal, also for placing another shape. Patch height functions combine a path distance, bump profile, region mask, clamping, flattening or inflation. Surface-net baking and glTF export keep an explicit mesh. |
| Traced, copied, posed, line, lattice impostor, field, relief, mesh, glazed, grid detail | Geometry readings supply per-observer intersections. Cull rows by projected bound, choose grid LOD and hysteresis in a schedule; quads cover points, trails and line strokes. Glazing reads earlier colour/depth or traces a ray. Custom hull and inner-gap masks preserve authored ink width. |
| Sea, whirlpool, afloat, ring, ground and underwater looks | A projected patch and its height/normal law provide swell, deformation and support frames. Place an object using sampled bow/stern/beam positions. Ring deforms use a time-shifted radial wave. Foam/whitewater, curl heads, waterlines and wakes use slope, contact and phase masks, then ordered ink layers. An absent ground is an empty world region. |
| Gaussian puffs/banks, marched volumes, cloud, rain/splashes, lightning, glow/flare, sprays, dust | Compose densities and bounded optical-depth functions; split folded Gaussian integrals at cell walls. Seeded instances/curves draw streaks, branches and halos. Time profiles provide flashes, afterglow and matching illumination. Cache grain-to-view optical depth on a grid for expensive veils and report interpolation error. |
| Style, sky, paint, light recipes and printing | Typed channels and arbitrary estimators support countershade, bands, spots, hue accents, mottling, halo, rim, overprint, wash/bokashi, contact, hatching, cross-hatching, palette keys and block relief. RGB output, depth, edge data and another camera's render can feed any post pass. |
| Variant/design/lineup, parameter sweeps, staging, independent views, readback | A build function takes typed numeric and discrete choices, emits rows, combines worlds and constructs named cameras or view sheets. Log choice/asset/seed in scene identity. Parameter bounds validate changes at ingress. Legacy string variant syntax and JS graph reflection are dropped because typed calls and printed IR expose the same choices and definitions. |
| Host packs, graphs, inline shader templates, static/frame/recur tables, custom draws, clips and checks | §2–3 cover calls, intervals, derivatives, schedules at build/load/frame/step, raw shader and compute, checkpointed feedback and source maps. §5 covers posed/animated sites and timed checks. A user-defined derivative rule needs an independently validated contract; sampled Lipschitz probes remain falsifiers. Legacy wire encodings, style-dependent AO, compulsory seas/suns and skeleton/time-zero shadow surrogates are dropped because the actual posed scene supplies the observation. |

## 5. The motion sub meta engine

### Bodies, inputs and poses

A body is a rest skeleton with named sites. A site is a point and frame on a segment, such as the underside of a toe or the eye of a bird. The engine reads the same final segment frames as the artist's mesh, the exporter and the checker. Use *planted* for a controller decision and *held* for an observation on a drawn site.

```loft
// std::motion. Names identify rest sites inside one BODY. World, BodySpace and Joint are spaces.
fn trunk(size:Vec[BodySpace],height:Meters,mass:Kg)->Trunk;      // Box inertia by default, height is rest COM height.
fn chain(id:Name,base:Pt[BodySpace],lengths:Array[Meters],end:End)->Chain; // Order links from socket to tip.
fn foot(home:Pt[BodySpace])->End;                                // Rest pad position relative to the trunk.
fn head(home:Pt[BodySpace],yaw:Rad,pitch:Rad,glance:Rad)->End;   // Separate desired place, skull frame and eye ray.
fn hand(home:Pt[BodySpace])->End;                                // May acquire a scored world target and release it.
fn loose(home:Pt[BodySpace],hz:Hz,zeta:f32)->End;                // A root-driven damped secondary chain.
fn strand_end(home:Pt[BodySpace],compliance:Meters/N)->End; // Loose chain with per-edge compliant distance constraints.
fn body(t:Trunk,chains:Array[Chain],forward:Dir[BodySpace]=Dir[BodySpace](1,0,0))->Body;                      // Lower shorthand to segments, sites, limits and end laws.
fn rig(segments:Array[Segment],sites:Array[SiteDef],ends:Array[EndDef],edges:Array[Edge])->Body; // Full rest description.
fn root()->Site;                                                // The trunk origin.
fn socket(id:Name,i:u32)->Site;                                 // Joint i in the chain, zero at the attachment.
fn tip(id:Name)->Site;                                          // The last joint or the named effector site.
fn controls(go:fn(Sec)->Velocity[World],look:fn(Sec)->Target[World],hits:Array[Event])->Inputs; // Script defaults for the typed per-instance go, look and hits ports.
fn strike(tick:u32,seq:u32,at:Pt[World],j:Impulse[World])->Event; // One impulse at a world point on this actor's tick.
fn hop(i:Inputs,takeoff:Sec,flight:Sec,delta:Vec[World])->Inputs; // Add a planned take-off, airborne interval and landing displacement.
fn terrain(i:Inputs,supports:SupportWorld)->Inputs;               // Bind candidate surfaces, motion and friction; controls defaults to the static y=0 plane.
fn beat(id:Name,t:Sec,at:Target[World],v:Velocity[World])->Beat;  // Aim to arrive at a place, time and velocity.
fn release(id:Name,t:Sec)->Beat;                                // End a scored channel on this tick.
fn score(bs:Array[Beat])->Score;                                 // Ordered typed channels; reject conflicting writers/times.
fn move[S](b:Body,c:Controller[S],i:Inputs)->Take;                     // Allocate an instance and its input, state and pose views.
fn pose(t:Take,s:Site)->Xform[Joint,World];                     // Drawn post-IK frame at the current sample.
fn site_at(t:Take,s:Site)->Pt[World];                            // That frame's origin.
fn neck_path(t:Take,id:Name,u:Unit)->Pt[World];                   // Drawn arc at unit fraction of the solved neck length.
fn attachment[S](t:Take,s:Site,x:Xform[S,Joint])->FrameSource[S,World]; // Read pose(t,s)*x in the rendering pass.
fn mount[S](p:Xform[Joint,World],x:Xform[S,Joint])->Xform[S,World]; // Compose a sampled pose with its rigid attachment.
fn tracked_center(f:fn(Sec)->Pt[World])->FrameSource[Joint,World]; // Translation driven by the same sample time as motion.
```

Velocity and impulse above are vectors with units of length/time and mass·length/time respectively. Space tags survive storage erasure. A `Target[World]` is a desired position. An `Attach[Joint]` is a permanent local offset. Each segment in `rig` specifies parent (root or parent index), rigid rest transform, mass/inertia, twist basis and swing/twist limits. Each site names a segment and its rest attachment. Each end names its site, kind and parameters. Edges express lift inhibition or strand connectivity. Validate the tree, unique names, positive finite masses and lengths, unit bases, ordered limits and complete incidence at build. The shorthand places the trunk with its COM at (0,height,0), takes its horizontal forward axis from the argument (default +x) and projects rest feet onto the support below them. It chooses the rest bend in the plane through the socket, home and body up; at collinearity use body forward, then side. Imported twist and explicit poles in a full rig take precedence. Initialize the eye bearing from the tick-zero world target and record the initial targets and default input levels. A host writes typed go, look and hits packets to the instance by these names, each with its logical tick. The first host write to a scripted channel switches that dependency to the live log at the specified tick.

```loft
// The callable seams are ordinary definitions that a consumer can copy. A Tick is the packet for exactly one logical tick.
struct Controller[S] { init:fn(Body,Tick)->S, step:fn(Body,S,Tick)->S, present:fn(Body,S,Unit)->Pose }
fn controller[S](init:fn(Body,Tick)->S,step:fn(Body,S,Tick)->S,present:fn(Body,S,Unit)->Pose)->Controller[S];
fn initial_state(b:Body,i:Tick)->State;                          // Construct the rest span, initial pins, zero momentum and event cursor.
fn steer(b:Body,s:State,i:Tick)->Intent;                         // Velocity, gaze and planned hop requests.
fn perform(prior:Score,b:Body,s:State,i:Tick)->Intent;            // Add per-channel artist targets and authority to steer.
fn impulses(b:Body,s:State,i:Tick)->State;                       // Apply not-yet-consumed events, including angular impulses.
fn contacts(b:Body,s:State,i:Tick)->Contacts;                     // Snapshot support frames, candidate pads, slips and loads.
fn balance(b:Body,s:State,i:Tick,g:Intent,k:Contacts)->Forces;    // Feasible wrench, yaw torque and per-pad shares.
fn repin(b:Body,s:State,i:Tick,g:Intent,k:Contacts,f:Forces)->Pins; // Select lifts and advance every planted/swing/scored end.
fn integrate(b:Body,s:State,i:Tick,g:Intent,f:Forces,p:Pins)->State; // Commit span, pins, plans, momentum and counters.
fn secondary(b:Body,s:State,i:Tick)->State;                      // Attached secondary laws follow this tick's resolved trunk span.
fn present_body(b:Body,s:State,fraction:Unit)->Pose;              // Sample spans, solve IK and publish frames, eye rays and surfaces.
fn solve_near(keys:Score,free:Array[Spline],holds:Array[Condition],cost:Weights)->Score; // Build-time constrained curve fit.
fn spring(k:SpringState,driver:Polynomial,dt:Sec,hz:Hz,zeta:f32)->SpringState; // Exact forced linear transition.
fn stretch(axis:Dir[Joint],ratio:f32)->Xform[Joint,Joint];        // Scale by r along the axis and r^(-1/2) across it, r>0.
fn share_range(c:Contacts,f:Forces,pad:Site)->Interval[Unit];    // Feasible min/max share at the same resultant wrench from two linear programs.
fn margin[U](site:Site,id:Name,value:U,scope:Scope)->Verdict[U]; // Signed evidence for one named observation.
fn holds(t:Take)->Report;                                        // Site × constraint × scope, with provenance.
fn require_margin[U](t:Take,s:Site,id:Name,span:TimeRange,signed:fn(Sec)->U); // Add a scoped check evaluated from the pose at that sample time.
fn clip(src:ClipSource,map:RigMap,b:Body)->Score;                 // Retarget a file, sample array or tick-stamped live joint stream.
fn read_pose(t:Take,at:Sec)->Readback[Pose];                     // Asynchronous frame and site readback.
fn export_gltf(t:Take,span:TimeRange,position_error:Meters)->Asset; // Rest skeleton and adaptive local TRS keys.
```

`Intent` carries per-channel targets, desired velocity, requested yaw/pitch, deadlines and giver (trunk, tip or release). `State` in these signatures denotes this standard controller's schema. A replacement controller supplies its own state constructor and step/present pair. The initial tick snapshot seeds state before its queued events are consumed; each event enters the first applicable step exactly once. `Pose` publishes one rigid frame per segment, root and effector sites, the drawn neck evaluator and any deformation channels. A scale applied downstream must be included in the observation transform. A renderer that deforms a pad independently supplies its final site evaluator or that hold becomes `Unknown(UnobservedSite)`. Buffers and generated dispatchers know only their types.

### Author-defined laws, observations and solvers

These are the interchange records used at the seams. A controller may replace both its state type and its end kinds. `Body` conveniences construct the same reader-facing tables. The pose channels and evidence schema remain available to a custom solver.

```loft
type Velocity[S] = Vec[S,Meters/Sec];
type Impulse[S] = Vec[S,Kg*Meters/Sec];
type Accel[S] = Vec[S,Meters/Sec/Sec];
struct Segment { id:Name, parent:Option[u32], rest:Xform[Joint,Joint], mass:Kg, inertia:Inertia,
                 pole:Dir[Joint], swing:Rad, twist:Interval[Rad] }
struct SiteDef { id:Name, segment:u32, offset:Xform[Joint,Joint] }
struct EndDef { site:Site, end:End }
struct Edge { a:Site, b:Site }
interface EndLaw[P] {
    fn demand(p:P,b:Body,s:State,i:Tick)->Intent;
    fn realize(p:P,b:Body,s:State,u:Unit)->Xform[Joint,World];
}
fn end[P](params:P,law:EndLaw[P])->End;                         // Open new end case by appending typed parameters and two readings.
struct Anchor { surface:SupportId, local:Pt[Support] }
struct Contact { pad:Site, anchor:Anchor, at:Pt[World], n:Norm[World], v:Velocity[World], mu:f32, load:N }
type Contacts = Array[Contact];
type SupportWorld = fn(Pt[World],Sec)->Array[ContactPatch];
struct ContactPatch { surface:SupportId, to_world:Xform[Support,World], n:Norm[World],
                      at:Pt[World], v:Velocity[World], mu:f32 }
fn observed_slip(i:Inputs,tick:u32,pad:Site,vrel:Velocity[World])->Inputs; // Log measured pad velocity relative to its surface.
struct SwingPlan { from:Pt[World], aim:Anchor, start:Sec, duration:Sec, lift:Meters, up:Dir[World] }
enum PadMode { Planted(Anchor), Swing(SwingPlan) }
struct PadState { mode:PadMode, foot_world:Pt[World] }
struct Launch { aim:Anchor, duration:Sec, lift:Meters, up:Dir[World] }
enum PadDecision { Stay(Anchor), Lift(Launch), Continue(SwingPlan) }
fn at_support(a:Anchor,t:Sec)->Pt[World]; // Evaluated from a logged support frame; its absence invalidates the site.
fn along_swing(p:SwingPlan,t:Sec)->PadState {
    let end = at_support(p.aim,t);
    let u = saturate((t-p.start)/p.duration);
    if (u == 1.0) { return PadState(PadMode::Planted(p.aim),end); }
    let q = u*u*u*(10.0+u*(-15.0+6.0*u));
    let x = p.from + (end-p.from)*q + p.up*(p.lift*16.0*u*u*(1.0-u)*(1.0-u));
    return PadState(PadMode::Swing(p),x);
}
pub law step_pad(old:PadState,d:PadDecision,t:Sec,dt:Sec)->PadState {
    match d {
        PadDecision::Stay(a) => { return PadState(PadMode::Planted(a),at_support(a,t+dt)); }
        PadDecision::Lift(k) => {
            let p = SwingPlan(old.foot_world,k.aim,t,k.duration,k.lift,k.up);
            return along_swing(p,t+dt);
        }
        PadDecision::Continue(p) => { return along_swing(p,t+dt); }
    }
}
```

`repin` loads the old row and writes the *entire returned* pad row to next-state storage, including the actual foot location. `present_body` re-evaluates the tick's chosen span at its fraction; IK and the drawn mesh read that result. `Site` is a plan-local stable handle, and `SupportId` selects the runtime instance supplying the chart named `Support`. A structural socket frame has +y along its outgoing bone and a parallel-transported transverse pair carrying rest twist. The final structural socket continues the previous tangent. At the same final position, a planted foot's distal effector frame has +y along its actual support normal and +x along the projected foot heading. A head effector has +x on the eye ray. This fixes the axes of attached toe and beak geometry. A nonzero tangential `observed_slip` packet on a planted pad ends the old stance on that tick, preserves the observed endpoint as the start of recovery and triggers a new support solve. An event on a swinging pad is recorded as inapplicable. The fixture below consumes the same event log in live, bake and replay.

```loft
fn two_hits(pad:Site)->Inputs {
    let i = controls(|t|Velocity[World](0,0,0),|t|Target[World](Pt[World](8,2,0)),[
        strike(60u,1u,Pt[World](0.4,1,0),Impulse[World](3,0,0)),
        strike(75u,2u,Pt[World](0,1,0.3),Impulse[World](0,0,-2))
    ]);
    return observed_slip(i,61u,pad,Velocity[World](0,0,0.8));
}
```

For this reaction fixture use a body initially at rest with the nominated pad planted. Check the first event exactly once, the loss of the old anchor on the next tick, re-selection of support, and the second transverse impulse on the surviving momentum. A fixture that never entered the relevant planted state fails its precondition.

```loft
enum Observation[T] { Observed(T), Unavailable(Reason) }
enum DomainEvidence { Instant(Sec), Samples(Table[Sec]), Continuous(TimeRange) }
struct Scope { instance:u32, obligation:Name, site:Site, domain:DomainEvidence }
struct Evidence[U] { scope:Scope, margin:Interval[U], witness:DomainEvidence, revision:Digest }
enum Verdict[U] { Held(Evidence[U]), Missed(Evidence[U]), Unknown(Scope,Reason) }
struct PlantReading[S] { surface:Xform[S,World], drawn:Xform[Joint,World], pad:Pt[Joint], anchor:Pt[S] }
fn unknown[U](site:Site,id:Name,reason:Reason,scope:Scope)->Verdict[U]; // Record a named unmet evidence dependency.
pub constraint drawn_plant[S](site:Site,r:Observation[PlantReading[S]],eps:Meters,scope:Scope)->Verdict[Meters] {
    match r {
        Observation::Unavailable(why) => { return unknown[Meters](site,"plant",why,scope); }
        Observation::Observed(x) => {
            let p = inverse(x.surface) * (x.drawn*x.pad);
            return margin(site,"plant",eps-distance(p,x.anchor),scope);
        }
    }
}
fn always[U](domain:TimeRange,m:fn(Sec)->Observation[U])->Verdict[U];       // Universal signed margin with time/site evidence.
fn eventually[U](domain:TimeRange,m:fn(Sec)->Observation[U])->Verdict[U];   // Existential nonnegative margin or proof all are negative.
struct Spline { channel:Name, knots:Array[Sec], keys:Array[f32], scale:f32 }
struct Weights { near:f32, velocity:f32, acceleration:f32, jerk:f32 }
struct Condition { id:Name, site:Site, range:TimeRange, unit:UnitId, unit_scale:f32, signed:fn(Sec,Score)->f32 }
struct JointMap { source:Name, target:Site, rest_basis:Rotation, length_scale:f32 }
struct RigMap { joints:Array[JointMap], to_world:Basis, unit_scale:f32, root_policy:RootPolicy }
enum RootPolicy { InPlace, Scaled, WorldTrack }
enum ClipSource { Gltf(Asset), Bvh(Asset), Samples(Table[JointSample]), Stream(Input[JointSample]) }
struct JointSample { tick:u32, joint:Name, local:Xform[Joint,Joint], contact:Option[bool] }
```

A report keys each verdict by its complete scope and frame/input identity. The evidence margin encloses the signed residual on its witness, in the author's units. Zero meets a nonnegative requirement. A sampled witness lists the actual requested instants. A continuous witness carries the outward enclosure or a certificate reference in the report, and a counterexample identifies the offending instant or interval. The report retains the first missing dependency, cap, opaque expression, or unresolved interval on Unknown. A compile-time `loft check` or `loft build` fails when a required static proof is unavailable. Runtime gates check the exact requested domain against the available observations.

Scope a plant to the promised stance intervals, including any detected lost contact. A missing expected frame is `Unavailable(MissingFrame)`. A deliberate release ends an obligation at that tick and the report names it. A sample observed after release cannot certify the preceding stance. `always` on continuous time requires a nonnegative interval lower bound for Held or a negative witness for Missed. `eventually` needs one nonnegative witness for Held or a strictly negative upper bound over the full interval for Missed. Anything undecided remains Unknown. `Condition.signed` normalizes a physical residual by its declared channel scale; retain its source units for reports. A consumer can substitute new `EndLaw`, `Controller` or `solver` code independently.

The following is the complete fixed-size strand solver used for the boss fixture. Its layout is one row per strand; the library selects this definition for the matching capacity. Longer strands use contiguous table slices and the identical predict/project/commit equations with dispatch boundaries between colours when one workgroup cannot hold the island. `SocketSpan` supplies the upstream attachment as a function of time within the current tick. A zero-length collapsed edge uses its rest direction. Any nonfinite value invalidates the solver evidence.

```loft
struct StrandRow {
    rest:array<Meters,15>, rest_dir:array<Dir[World],15>,
    w:array<1/Kg,16>, compliance:Meters/N, gravity:Accel[World]
}
struct StrandState { p:array<Pt[World],16>, v:array<Velocity[World],16> }
type SocketSpan = fn(Unit)->Pt[World];

pub solver Antenna over StrandRow @checkpoint(60u) {
    config { strategy: colored(2u), substeps: 4u, iterations: 6u }
    fn step(r:StrandRow,s:StrandState,root_at:SocketSpan,dt:Sec)->StrandState {
        var next = s;
        let h = dt/4.0;
        let alpha = r.compliance/(h*h);
        for k in 0u..4u {
            let before = next.p;
            next.p[0] = root_at(saturate(f32(k+1u)/4.0));
            for j in 1u..16u {
                next.v[j] = next.v[j] + r.gravity*h;
                next.p[j] = next.p[j] + next.v[j]*h;
            }
            var lambda:array<Kg*Meters,15>;
            for sweep in 0u..6u {
                for colour in 0u..2u {
                    for (var e=colour; e<15u; e=e+2u) {
                        let d = next.p[e+1u]-next.p[e];
                        let length_now = length(d);
                        var n = r.rest_dir[e];
                        if (length_now>0m) { n=normalize(d); }
                        let dl = (r.rest[e]-length_now-alpha*lambda[e])/(r.w[e]+r.w[e+1u]+alpha);
                        lambda[e] = lambda[e]+dl;
                        next.p[e] = next.p[e]-n*(r.w[e]*dl);
                        next.p[e+1u] = next.p[e+1u]+n*(r.w[e+1u]*dl);
                    }
                }
            }
            for j in 0u..16u { next.v[j]=(next.p[j]-before[j])/h; }
        }
        return next;
    }
}
```

`colored(n)` in a solver config declares the iteration over its disjoint colour classes. The body specifies sequential reference semantics; codegen proves nonoverlap of edge destinations, maps each class to lanes, keeps p, v and λ in workgroup storage and inserts barriers between prediction, colours and velocity reconstruction. A shared `before` buffer survives the projections. Lambda initializes to zero for each substep by value semantics. Require root inverse mass zero and every movable node finite positive inverse mass, so denominators stay positive. Check distance residuals on final coordinates and the moving root exactly at each tick. A consumer may replace this solver with a Jacobi gather or a different constitutive law and keep the same socket and post-solve site contracts.

### The nine motion decisions

1. **Formulation.** Compose absolute performance targets, contact forces, repinning, IK and secondary response in causal order. One writer owns each degree of freedom per tick, with blending inside that writer. The commanded trunk acceleration comes from the velocity error divided by a response time. Project the desired wrench onto the *joint* feasible contact set, including unilateral normal load, tangential friction, joint reach, available yaw torque and push strength. Integrate the resulting trunk span. This buys direct control of a hand's deadline through a body hit and exposes the cause of a slip. The standard controller gives up globally optimal articulated dynamics and limb–limb collision response. An island solver with the same input/pose seams can supply those.
2. **Bodies and stepping.** Derive neighbouring feet by sorting their rest sockets along each side and matching nearest opposing partners, with stable site order for geometric ties. A different neighbourhood is an input to `rig`. For coplanar supports use their tangent plane, effective normal gravity and COM height. Predict the capture point from the COM plus tangential COM velocity *relative to the support* times √(height/gravity). For stationary level feet the demanded pressure point is COM minus horizontal acceleration times height/gravity. Its hull class is flight, point, segment or polygon (including degenerate sets). Choose continuous visible shares by minimizing squared deviation from preferred loads subject to the required wrench, nonnegative shares and friction. On noncoplanar terrain solve the same wrench in world space using each contact's own normal and point velocity including deck rotation. Approximate circular Coulomb cones by inscribed regular octagons oriented by each surface frame, include each force's torque about COM and constrain normal force to [0,push limit]. First minimize squared requested acceleration/torque residual in normalized units; among equal wrenches minimize squared deviation from preferred shares. Use a bounded deterministic active-set QP with feasible descent, entering and leaving constraints and ties resolved by site and facet ID. If support is insufficient, unload or slip the active pin with the largest traction deficit, recompute on that tick and report the giving site. In flight integrate gravity and conserve angular momentum. Pose pins in deck coordinates. Rank planted candidates by normalized capture error, then site ID. Tentatively remove an unloaded candidate, recompute support, honour neighbour inhibition, commit only a feasible lift. Swing from its *current* pad along a least-jerk horizontal arc with quartic lift, commit a reachable landing point in the destination support's frame, then latch that anchor. Reversal changes the next decision from the current state. Body geometry and loads select available groups of feet. A leaping or galloping policy replaces the lift law and can choose flight; gait styles are not guaranteed by anatomy.
3. **GPU solve.** Exact two-link IK and fixed-length arcs handle common reach; for long strands use XPBD. Predict positions with substep length h, sample the posed root at each substep, give that vertex zero inverse mass and set it to the attachment. For a distance residual C and endpoint gradients g, let α=compliance/h² and Δλ=(−C−αλ)/(Σ w‖g‖²+α); accumulate λ and apply w·g·Δλ. Reset λ at each substep, retain it across sweeps, and reconstruct velocity from corrected positions. Store previous corrected position if using Verlet. Alternate disjoint edge colours with a workgroup barrier after each colour. Refuse invalid colourings. General meshes use Jacobi from one position snapshot, one correction per constraint and gathers in fixed incident-edge order; renormalize by incident weight. Split a multi-workgroup island at dispatch boundaries. The shipped solver fixes substeps, sweeps, tie order and residual tolerances in the program identity. A missed iteration certificate yields `Unknown(IterationCap)` for the solver's convergence claim. A measured site outside tolerance still yields its own `Missed`. Cross-tick fusion requires local state or per-tick upstream spans; emit all socket spans before a downstream strand consumes them, or interleave waves per tick. Heterogeneous laws bucket by wave and kind. The crowd budget applies to the benchmark body mix in §9.
4. **Performance with weight.** Treat artist keys and imported channels as priors, hard position/velocity beats as explicit constraints and declared priority as the conflict rule. Eliminate flight analytically before fitting cubic B-spline freedoms to keys with position, velocity, acceleration, posture and clearance holds. Minimize normalized squared distance to keys plus weighted derivative penalties using damped Newton and an augmented Lagrangian; return the fitted curve with residuals, never a proof from the objective. Run interval checks independently. For a displacement D, end speed v and duration T, the rest-to-arrival quintic is D(10u³−15u⁴+6u⁵)+vT(−4u³+7u⁴−3u⁵). It reverses first for collinear velocity with vT/D>2.5. Replan on discrete changes in aim, contact or impulse from the current position and speed. A scored hand owns its beat while reach permits. Infeasibility produces a measured miss or the named giver. A requested hop has v₀=Δ/T−gT/2 with gravity as a vector; no midair contact impulse can enforce an impossible posture preference.
5. **Gaze.** Hold head place in world space until deviation from its rest target consumes slack, then thrust toward the moving rest target with a critically damped follower and velocity feed-forward. Bearing is unwrapped relative to the previous bearing across the rear seam. Yaw and pitch are computed from the *actual eye* to the world target. Clamp skull pitch by the glance cap and neck pitch range, keep roll level, and clamp yaw to the declared neck interval. Grounded yaw request is sign(bearing)·max(|bearing|−0.6·range,0)/√(height/gravity), limited by available torque. Flight can keep existing spin. Place the head with an S of two equal-length, oppositely curved arcs between socket and target; solve sin θ/θ=chord/rest length on [0,π] with bounded bracketed iteration and a straight-limit expansion. Specify the bend plane by the rest neck pole. Chord above rest length gives a reach miss, and the drawn tip stops at full extension. The final eye ray determines the gaze report.
6. **Secondary motion.** For a rest-relative damped oscillator driven by actual root acceleration, integrate the constant-coefficient state transition over each polynomial driver span. Bake the transition coefficients once per plan. When a driver is reactive, compute the same transition per fixed tick; each chain link reads its parent's resulting acceleration. Nonlinear whip/contact uses the strand solver. Squash/stretch uses the volume-preserving transform above on the artist-selected channels. Loose antennae can whip while the scored claw and deliberately still face hold their beats.
7. **Reaction and history.** Consume each tick's events ordered by sequence. On hit at p, update v by J/m and angular momentum by (p−COM)×J; use the world inertia tensor for angular velocity. Shock initially reduces trunk score authority by min(1,|Δv|√(height/gravity)/strain-radius), then decays with that time constant. A second hit composes with the remaining shock as 1−(1−old)(1−new), and adds momentum to the *current* body. The measured contact set after a slip selects the recovery law. Save trunk pose, velocity, angular momentum, span, all modes and deck-local anchors, lift/landing plans, support IDs, head bearing, spring/strand state, score phase, shock, cursor, seed and input-prefix hash. A checkpoint in this profile is at a fixed tick boundary after all dependent waves. At presentation evaluate x+vδ+½aδ² inside the chosen span, with no future input. A controller with only time reads is closed form, a step reading only declared scripts can bake, a host-written stream is live. Both stepped homes call the same specialization. A live edit branches at the last valid checkpoint and replays the associated immutable input prefix. Missing or expired inputs give a named history error.
8. **Truth.** Every reachable end carries finite values, reach and joint limits; feet add contact and timing, heads add neck length, skull pitch and gaze. Enable head-above-hips on an upright heron during the chosen interval. A diving bird or headless body has a different posture contract. For a foot, compute ε−‖D⁻¹Fp−anchor‖ from final segment frame F, site p and support D at the drawn sample. For gaze use ε−atan2(‖eye×targetDir‖,eye·targetDir). At zero target distance return Unknown. Record units, signed margin/error, site and hold ID, giver, tick/fraction span, worst sample, algorithm hashes, input-prefix digest and scope (proved range or finite trace with device). Empty sample sets, nonfinite values, missing contacts where required, unobserved geometry and exhaustion are explicit unknowns. A confirmed miss includes a witness even if another tick was unknown. Summaries retain unknowns and the underlying site rows. Expected failures are keyed by site, reason and time window with a bound on magnitude; other failures fail the gate.
9. **Interop.** A rig map supplies source and target joint or end roles, rest bases including twist, handedness, axes, length scale and root-motion policy. Normalize glTF, BVH, arrays and generated packets at ingress. For a rest-relative source rotation Δq apply qTargetRest·B·Δq·B⁻¹ in target joint coordinates. Scale root displacement by the chosen hip-height ratio and end targets by chain-length ratio. Nonhuman many-to-one mappings name their writer or fail with a map-entry diagnostic. Contact tags and speed/height dwell tests propose pins; compare them and report disagreements. Correct the prior through the same controller, expose corrected POSE and trace; stale live pose repeats the last sample with its age and loses any freshness claim. External models run outside Loft. Export parent-relative TRS and inverse binds, split keys at impulses/contact transitions, subdivide until interpolation on held sites meets tolerance or report an export-budget refusal.

| Evidence that selected the controller | Measured in the supplied look rounds |
|---|---|
| Heron, unconstrained posture versus near the animator's keys | Least jerk alone pitched the body 67° and put the head 0.39 m below the hips. Fitting near keys with a posture bound gave 12° maximum pitch and 0.17° head tilt. Adding world gaze and neck IK brought peak head jerk from 670 to 428 m/s³ |
| Eight-foot crab, reference motion take | Weight-driven swings produced 23 steps with variable timing. The Rust controller produced 22 steps, 0.0008 mm post-IK planted-toe drift, and caught a 20 mm fault on the injected toe. Baked and live histories agreed at 30, 60 and 120 Hz with a six-tick stall. These figures describe the original body and controller |

| Standard profile quantities | Predicted design values and gates |
|---|---|
| Time and replay | 120 ticks/s, checkpoint every 60 ticks, maximum 59 replay ticks to an available target checkpoint interval; at 60 Hz normally two steps and one present |
| Trunk and contact | g=9.81 m/s², T=√(height/g), horizontal drive (vRequest−v)/(T/2), default μ=0.6, maximum net push 2.1g, no tensile normal force, lean toward atan2(horizontal acceleration,g) at 2 Hz with damping ratio 0.7. Per-pad push cap = total push capacity unless BODY overrides it |
| QP | Feasible initial zero contact forces, eight-sided friction cone, at most 64 active-set transitions. Scale force by mg and torque by mg·height, require primal/dual/KKT residual ≤1e−5 in normalized units. Nonconvergence records Unknown and applies the last primal-feasible iterate, with its unachieved intent retained |
| Head and IK | Release head-place pin when displacement exceeds 0.15 neck lengths; critically damped tracking, ω=2π·2 Hz, target velocity feed-forward 1/ω. Bracketed S-neck solve on [0,π], 24 bisections or equivalently certified residual ≤10 µm; apply skull pitch and yaw limits independently |
| Swing profile | R=0.18 link-chain length, urgency u=saturate(strain−1). For planned step length d choose Tstep=clamp((0.6−0.3u)√(d/g),0.12 s,0.35 s), lift=clamp((0.18+0.10u)d,0.05R,0.75R). Project rest home plus velocity·T onto support, store support-local aim on lift-off; use the minimum-jerk horizontal blend and quartic lift in the law above |
| Observed slip | For each lost planted pad, integrate observed tangential vrel over the reporting tick in its support's tangent plane; change mode to Swing with this displaced start, erase its old anchor, and aim at the nearest reachable support in the desired direction. Subsequent plant creates a fresh anchor. Other pads stay eligible to carry load |
| Strand fixture | 16 nodes per strand, four substeps, six sweeps of the two edge colours; one workgroup per strand, with deterministic gathers for general graphs |
| Foot fixture | Normalize strain by 0.18 of reach, require strain >1, minimum stance 0.12 s, maximum preferred lift share 1.1 divided by support count; derive a support inset in units of pad size |
| Posture fixture | Check planted drift ≤0.1 mm; adding 20 mm to the drawn toe after IK fails that toe alone. A moving deck carries its correctly pinned toe in world space |
| Recovery fixture | Hits 15 ticks apart apply exactly twice across 30/60/120 Hz and a six-tick stall; compare snapshots and input digests over the entire take |
| Clip export fixture | Check all held sites at output keys and span midpoints to 1 mm; continuous guarantees need interval bounds, otherwise report the exact sampling scope |
| Directed solve and interop | Default near-key weight 1, normalized jerk weight 1e−7, remaining soft weights zero, hard signed conditions checked independently. Clip stance candidates need three consecutive samples with foot speed below 0.15 m/s and site height below 0.02 m, both scaled by source-to-target leg length. Reject invalid or nonfinite map scales |

The legacy clocked walk, carriage blends, pose scores, breathing, squash, anticipation, restlessness, tremor, struck poses and directed holds remain closed curves and score recipes. Use end-target tracks and attachment/release events for stance, stamp, lunge, sweep, rear, grasp, carry, set-down, throw and canon/choreography. Compose signed shape fields with the same event clock for hit contours, telegraphs, rings, wakes and sampled smears. View size, expression readability, notice, clearance, reach, grip, projectile flight, release and landing checks become named measurements on the relevant world-space site, screen projection or support and time scope. This preserves the ability to author a deliberately phased walk as well as adaptive stepping.

## 6. Example engines

### Realtime, reference and woodblock

Each exported function is the entry point of its own module under `engines`. The ordinary source uses only the §4 library. Kind-specialized library calls in a schedule contribute real draws/dispatches. A user can inspect the lowering or replace any one of the calls.

```loft
// engines/realtime.loft
import std::render::*;

fn sun_and_lamps(w:World,h:Hit,wo:DirW,k:Key)->Rgb {
    let b = material(w,h);
    return emitted(b,h,wo) + direct(w,h,wo,k);
}

pub fn realtime(w:World,c:Camera)->Schedule {
    let s = plan(w,c);
    let n = clear_counts(s,w);
    let a = cull(s,w,c,n);
    let v = record(s,w,c,a,vis64());
    let tiles = classify(s,v,n);
    let base = deferred(s,w,v,tiles,sun_and_lamps);
    let opaque = reflect_pass(s,w,c,v,base);
    var layers = no_layers();
    if any_forward(w) {
        for (var i=0u; i<c.peels; i+=1u) {
            layers = peel(s,w,c,a,depths(v),i,layers);
        }
    }
    let hdr = media_forward(s,w,c,depths(v),layers,opaque);
    let display = history(s,w,c,v,hdr);
    return output(s,display,aces);
}
```

Each invocation of `peel` emits the forward batches for that layer. It stores true depth, normal, local response, scattered lighting and continuation data. `media_forward` evaluates one segment down to opaque/background and one extra segment for each peeled layer; the far-to-near recurrence evaluates a layer's transmitted contribution once. Thick refraction reads traced radiance at the refracted destination and suppresses the straight-through transmitted term. The glossy pass accounts only for indirect light after a bounce; directly visible area/environment emission already sampled by `direct` is excluded for competing non-delta paths (or both proposals use MIS). Camera rays missing every opaque object evaluate sky even if a layer or medium lies in front of it. The schedule's last step applies the target's transfer function after film mapping.

```loft
// engines/reference.loft
import std::render::*;

fn path(scene:World,c:Camera,px:vec2u,sample_id:u32)->Rgb {
    let channel_id = channel(key(px,sample_id,0u,0u));
    var r = ray(c,vec2f(px),key(px,sample_id,0u,1u));
    let w = at_time(scene,r.time);
    var L = 0.0;
    var beta = 1.0;
    var prev_pdf = 0.0;
    var prev_delta = true;
    for (var b=0u; b<c.bounces; b+=1u) {
        let walk = first_event(w,r,channel_id,key(px,sample_id,b,2u));
        L += beta * walk.j[channel_id];
        beta *= walk.weight[channel_id];
        let v = walk.at;
        let wo = -r.d;
        var we = 1.0;
        if !prev_delta { we = power_weight(prev_pdf,emitter_pdf(w,r,v)); }
        L += beta * we * event_emission(w,v,wo)[channel_id];
        if terminal(v) || b+1u==c.bounces { break; }

        let n = connect(w,v,wo,channel_id,key(px,sample_id,b,3u));
        if n.light_pdf>0.0 {
            var wm = 1.0;
            if !n.delta { wm = power_weight(n.light_pdf,n.scatter_pdf); }
            L += beta * n.value[channel_id] * wm/n.light_pdf;
        }
        let q = scatter(w,v,wo,channel_id,key(px,sample_id,b,4u));
        if q.weight[channel_id]==0.0 { break; }
        beta *= q.weight[channel_id];
        prev_pdf = q.pdf;
        prev_delta = q.delta;
        r = spawned(v,q.wi,r.time);
    }
    return reconstruct(channel_id,L);
}

pub fn reference(w:World,c:Camera)->Schedule {
    let s = plan(w,c);
    let samples = screen_samples(s,c,|px,i| path(w,c,px,i));
    return output(s,average(samples),aces);
}
```

A reference image declares the positive sample and maximum visited-vertex counts. The last vertex contributes emission, sky or segment source. For scoring illumination read linear sum/count before the display transform. An emitter-only camera path, a specular chain to a light, a mix of two materials, and a path through overlapping media each have a corresponding finite-depth reference experiment. The keyed sampler uses a reserved stream for any variable-length tracking or rejection loop.

```loft
// engines/woodblock.loft
import std::render::*;

pub struct Inks { lit:Rgb, dark:Rgb, pitch:Meters }

fn pull(w:World,h:Hit,wo:DirW,k:Key)->Rgb {
    let inks = read_attributes[Inks](w,h);
    let src = light(w,h.p,k);
    var lit = 0.0;
    if src.pdf>0.0 {
        lit = max(0.0,dot(h.n,src.wi)) * visible(w,src.beam).x;
    }
    // The carved groove is registered to the block; two pigments share its mask.
    let phase = (h.p.x + 0.31*h.p.z + 0.12*h.p.y)/inks.pitch;
    let carved = ink_mask(fract(phase),0.77,0.05);
    let shadow = 1.0-ink_mask(lit,0.36,0.035);
    let paper = rgb(0.92,0.82,0.62);
    let plate = overprint(paper,inks.lit,0.82-0.34*carved);
    return overprint(plate,inks.dark,shadow*(0.94-0.50*carved));
}

pub fn woodblock(w:World,c:Camera)->Schedule {
    let s = plan(w,c);
    let n = clear_counts(s,w);
    let a = cull(s,w,c,n);
    let v = record(s,w,c,a,vis64());
    let print = print_pass(s,w,c,v,pull,rgb(0.92,0.82,0.62));
    return output(s,print,linear);
}
```

This print engine requests opaque coverage only and a pigment reading on each covered thing. Its paper, two ink strikes and key-light shadow determine the final colour. It uses no BSDF evaluation. A print edition that requires only the rim, or reads a different world for shadows, changes `pull` alone.

### Adaptive locomotion and scored performance

`engines::roam` exports the order of operations. A step returns a replacement for its own state row. The spatial foot goal, contact mode, swing origin, elapsed time and final anchor all advance, and the renderer samples the resulting pad from `present_body`.

```loft
import std::motion::*;

pub fn advance(b:Body, s:State, i:Tick, intent:Intent)->State {
    let old = contacts(b,s,i);
    let wanted = balance(b,s,i,intent,old);
    let pins = repin(b,s,i,intent,old,wanted);
    var next = s;
    next.pins = pins;
    let actual = contacts(b,next,i);
    let applied = balance(b,next,i,intent,actual);
    return secondary(b,integrate(b,next,i,intent,applied,pins),i);
}

fn step_roam(b:Body,s:State,i:Tick)->State {
    let hit = impulses(b,s,i);
    return advance(b,hit,i,steer(b,hit,i));
}

pub fn roam()->Controller[State] {
    return controller(initial_state,step_roam,present_body);
}
```

`engines::act` retains the performer’s timing and uses the identical contact machinery. A law value captures the score table handle. Different keys produce different rows in the same compiled controller.

```loft
import std::motion::*;
import engines::roam::advance;

pub fn act(track:Score)->Controller[State] {
    return controller(initial_state,|b,s,i| {
        let hit = impulses(b,s,i);
        let intent = perform(track,b,hit,i);
        return advance(b,hit,i,intent);
    },present_body);
}
```

## 7. Scenes

### The hermit crab at the tideline

The shell grows from a tight umber spiral into a pale, ribbed aperture. The crusher and feeder have different scales, the same serrations and a wet outer coat. Each white corneal highlight moves across a dark pupil aimed at the same world target. The four antennae trail the torso through the impacts. This file uses only the public render and motion modules. The two hits occur before the claw releases its first beat. The stick reversal later in the same take exercises the walking controller.

~~~loft
import std::render::*;
import std::motion::*;
import engines::act::act;
import engines::realtime::realtime;
space Shell, Claw, Face;

fn axis(u:Unit)->Pt[Shell] {
    let t = u * 17rad;
    let r = 0.96m * exp((u-1.0)*3.0);
    return Pt[Shell](r*cos(t),r*sin(t),0.63m*u);
}
scan whorl(u:Unit) @schedule(bake_lut,256u) -> Table[Basis[Shell]] {
    init: frame_seed(axis(0),normalize(diff(axis,u)(0)),Dir[Shell](0,0,1));
    step(last,u_i): transport_step(last,axis(u_i),normalize(diff(axis,u)(u_i)));
}
fn macro_shell(u:Unit,v:Unit)->Pt[Shell] {
    let f = sample_c2(whorl,u);
    let a = v * 6.283185307rad;
    let r = 0.49m * exp(3.0*(u-1.0));
    let rib = 0.08*pow(0.5+0.5*cos(102rad*u),4.0)*(0.6+0.4*cos(5.0*a));
    return f.origin + f.normal*(r*(1.10+rib)*cos(a)) + f.binormal*(r*(0.79+rib)*sin(a));
}
fn shell_at(uv:vec2f)->Pt[Shell] {
    let u = saturate(uv.x); let v = saturate(uv.y);
    let a = v*6.283185307rad;
    let cells = (0.5+0.5*cos(80rad*u+2.0*sin(3.0*a)))*(0.5+0.5*cos(11.0*a+4rad*u));
    let barn = 0.039m*(smootherstep(0.58,0.84,cells)-0.70*smootherstep(0.88,0.99,cells));
    let du = diff(macro_shell,u)(u,v); let dv = diff(macro_shell,v)(u,v);
    let n = normalize(cross(dv,du));
    return macro_shell(u,v) + n*barn;
}
fn nacre(h:Hit)->Bsdf {
    let u=saturate(h.uv.x); let v=saturate(h.uv.y);
    let a=diff(macro_shell,u)(u,v); let b=diff(macro_shell,v)(u,v);
    let n=normalize(cross(b,a));
    let k=dot(diff(diff(macro_shell,u),u)(u,v),n)/dot(a,a);
    let grime=smoothstep(-2.5,-0.2,k*1m);
    let base=mix(rgb(0.58,0.32,0.16),rgb(0.10,0.17,0.09),grime);
    return thin_film(coat(ggx(base,0.26+0.35*grime,0.0),0.8,0.09,1.36),1.42,0.00000038m);
}

fn wave_noise(x:vec3f)->f32 { return (sin(x.x*1rad)+sin(x.y*1rad)+sin(x.z*1rad))/3.0; }
fn engraving(x:vec3f)->f32 {
    let a = vec3f(x.y,-x.z,-x.x);
    let b = vec3f(-x.z,x.x,-x.y);
    return wave_noise(x)+0.5*wave_noise(2.0*a+vec3f(1,2,3))
                        +0.25*wave_noise(4.0*b+vec3f(4,1,2));
}
fn crusher(p:Pt[Claw],angle:Rad)->Meters {
    let q=xyz(p);
    let palm=0.62m*(length(vec3f(q.x/0.95m,q.y/0.62m,q.z/1.35m))-1.0);
    let palm_cut=ssub_poly(palm,sd_box(p-Vec[Claw](0.10,-0.35,0.2),Vec[Claw](0.7,0.3,0.9)),0.06m);
    map jaw=translate(p,-Vec[Claw](0,-0.22,0.85)) |> rotate(Dir(1,0,0),-angle);
    let z=jaw.p.z;
    let teeth=0.045m*sin(26rad*(z/1m))*smoothstep(0.1m,0.9m,z);
    let moving=sd_capsule(jaw.p,Pt['jaw](0,0,0),Pt['jaw](0,-0.15,1.15),0.24m)-teeth;
    let fixed=sd_capsule(p,Pt[Claw](0,0.28,0.73),Pt[Claw](0,0.19,1.92),0.18m);
    map ring=repeat_polar(p,7u);
    let studs=length(ring.p-Pt['ring](0.75,0.36,0))-0.11m;
    let solid=smin_poly(smin_poly(smin_poly(palm_cut,fixed,0.06m),moving,0.08m),studs,0.04m);
    return solid-0.025m*engraving(coords(q/1m)*6.5);
}
fn wet_response(h:Hit,base:Bsdf)->Bsdf {
    let grain=0.5+0.5*wave_noise(coords(xyz(h.p)/1m)*9.0);
    return emission(coat(base,0.85,0.065,1.34),
                    rgb(0.16,0.025,0.005)*smoothstep(0.72,0.95,grain));
}
fn chitin(h:Hit)->Bsdf {
    let grain=0.5+0.5*wave_noise(coords(xyz(h.p)/1m)*9.0);
    return ggx(rgb(0.47,0.063,0.023),0.16+0.19*grain,0.03);
}
fn wet(h:Hit)->Bsdf {
    return wet_response(h,sss(chitin(h),rgb(0.83,0.16,0.045),0.018m));
}
fn wet_surface(h:Hit)->Bsdf {
    return wet_response(h,chitin(h));
}
fn eye_sway(t:Sec)->Meters { return 0.035m*sin(2.1rad*(t/1s)); }
fn face_dist(p:Pt[Face],t:Sec)->Meters {
    map e=mirror_x(p) |> translate(Vec( -0.38,-0.17,-0.08 ));
    let c=Pt['e](eye_sway(t),0.27,0.17);
    let stem=sd_capsule(e.p,Pt['e](0,-0.16,0),c,0.073m);
    let globe=length(e.p-c)-0.128m;
    map arch=translate(e.p,Vec(0,-0.44,-0.16)) |> rotate(Dir(0,0,1),-0.29rad);
    let brow=sd_round_box(arch.p,Vec(0.16,0.042,0.12),0.025m);
    return smin_poly(smin_poly(globe,stem,0.028m),brow,0.021m);
}
fn ship(t:Sec)->Pt[World] { return Pt[World](3m*sin(0.7rad*(t/1s)),2.15m,5.2m); }
fn pigment(h:Hit,xf:Xform[Face,World])->Bsdf {
    let p=inverse(xf)*h.p;
    map e=mirror_x(p) |> translate(Vec(-0.38,-0.17,-0.08));
    let c=Pt['e](eye_sway(h.time),0.27,0.17);
    let origin=xf*(e.to_parent*c);
    let gaze=e.from_parent.dir(inverse(xf)*normalize(ship(h.time)-origin));
    let v=e.p-c;
    var pupil=0.0;
    if length(v)>0.0001m {
        pupil=smoothstep(0.86,0.94,dot(normalize(v),gaze))*
              (1.0-smoothstep(0.002m,0.018m,abs(length(v)-0.128m)));
    }
    return coat(ggx(mix(rgb(0.73,0.45,0.12),rgb(0.012,0.009,0.006),pupil),0.28,0),0.7,0.075,1.36);
}
fn link_ring(take:Take,id:Name,j:u32,turn:Rad,r:Meters)->Pt[World] {
    return pose(take,socket(id,j))*Pt[Joint](r*cos(turn),0,r*sin(turn));
}
fn skin(take:Take,id:Name,n:u32,uv:vec2f,r:Meters)->Pt[World] {
    let h=saturate(uv.x)*f32(n);
    let j=min(u32(floor(h)),n-1u);
    let a=6.283185307rad*uv.y;
    let p=link_ring(take,id,j,a,r*(1.0-0.68*f32(j)/f32(n)));
    let q=link_ring(take,id,j+1u,a,r*(1.0-0.68*f32(j+1u)/f32(n)));
    return p+(q-p)*(h-f32(j));
}
fn stick(t:Sec)->Velocity[World] {
    if t<1.35s { return Velocity[World](0,0,0); }
    if t<2.35s { return Velocity[World](1.2,0,0); }
    if t<3.45s { return Velocity[World](-2.1,0,0); }
    return Velocity[World](0,0,0);
}
pub comptime fn scene()->Scene {
    var chains:Array[Chain]=[];
    let femur=[0.62m,0.78m,0.88m,0.84m,0.70m,0.54m];
    comptime for i in 0u..12u {
        let side=select(-1.0,1.0,i%2u==1u); let j=i/2u;
        let z=(f32(j)-2.5)*0.29m;
        push(chains,chain(name("leg",i),Pt[BodySpace](side*0.47m,-0.10,z),
            [0.32m,femur[j],1.18*femur[j]],foot(Pt[BodySpace](side*1.30m,-0.88m,z*1.42))));
    }
    comptime for i in 0u..2u {
        let side=select(-1.0,1.0,i==1u);
        push(chains,chain(name("arm",i),Pt[BodySpace](side*0.49m,0.14,0.43),
            [0.87m,1.02m],hand(Pt[BodySpace](side*1.17m,0.18,0.71))));
    }
    comptime for i in 0u..4u {
        let side=select(-1.0,1.0,i%2u==1u); let z=0.74m+0.07m*f32(i/2u);
        var links:Array[Meters]=[];
        comptime for j in 0u..15u { push(links,0.088m); }
        push(chains,chain(name("whip",i),Pt[BodySpace](side*0.26m,0.38,z),links,
            strand_end(Pt[BodySpace](side*0.65m,0.77,z+1.16m),0.002m/1N)));
    }
    let b=body(trunk(Vec[BodySpace](1.32,0.67,1.78),0.88m,12kg),chains,Dir[BodySpace](0,0,1));
    let hand_id=name("arm",1u);
    let script=score([beat(hand_id,0.70s,Target[World](Pt[World](1.28,0.46,1.05)),
                           Velocity[World](0,-3.8,0)),release(hand_id,1.30s)]);
    let io=controls(stick,|t|Target[World](ship(t)),[
        strike(60u,1u,Pt[World](-0.7,1.13,0.22),Impulse[World](8.0,0,-1.2)),
        strike(75u,2u,Pt[World]( 0.7,1.04,0.40),Impulse[World](-3.0,0,5.0))]);
    let take=move(b,act(script),observed_slip(io,61u,tip(name("leg",0u)),Velocity[World](0,0,0.8)));
    let w=world();
    let base=attachment(take,root(),identity[Joint]());
    add(w,surface(shell_at,grid(96u,32u)),nacre,
        attachment(take,root(),rigid[Shell,Joint](Vec[Joint](0,0.35,-0.74))));
    add(w,capsule(Pt[Joint](0,0,-0.54),Pt[Joint](0,0,0.53),0.49m),wet,base);
    comptime for i in 0u..12u {
        let id=name("leg",i); let r=0.105m-0.004m*f32(i/2u);
        add(w,surface(|uv|skin(take,id,3u,uv,r),grid(9u,8u)),wet_surface,identity[World]());
    }
    comptime for i in 0u..2u {
        let id=name("arm",i);
        let scale=select(0.39,0.76,i==1u);
        let f=field(box(Pt[Claw](-1.0,-0.95,-1.42),Pt[Claw](1.0,0.85,2.30)),|p| {
            let q=Pt[Claw](p.x/scale,p.y/scale,p.z/scale);
            let a=0.22rad+0.19rad*cos(6rad*(time()/1s));
            return scale*crusher(q,a);
        });
        add(w,f,wet,attachment(take,tip(id),rigid[Claw,Joint](Vec[Joint](0,0,0))));
        add(w,surface(|uv|skin(take,id,2u,uv,0.14m),grid(6u,8u)),wet_surface,identity[World]());
    }
    let seat=rigid[Face,Joint](Vec[Joint](0,0.14,0.71));
    let face=field(box(Pt[Face](-0.72,-0.22,-0.20),Pt[Face](0.72,0.81,0.49)),
                   |p|face_dist(p,time()));
    add(w,face,|h|pigment(h,mount(pose(take,root()),seat)),attachment(take,root(),seat));
    comptime for side in [-1.0,1.0] {
        let lens=surface(|uv| {
            let a=(uv.x-0.5)*3.14159265rad; let e=(uv.y-0.5)*3.14159265rad;
            return Pt[Face](side*(0.38m+eye_sway(time()))+0.132m*cos(e)*sin(a),
                            0.44m+0.132m*sin(e),0.25m+0.132m*cos(e)*cos(a));
        },grid(12u,8u));
        add_forward(w,lens,|h|thin_glass(1.34,rgb(0.98,1.0,1.0)),attachment(take,root(),seat));
    }
    comptime for i in 0u..4u {
        let id=name("whip",i);
        add(w,surface(|uv|skin(take,id,15u,uv,0.018m),grid(15u,5u)),
              |h|coat(matte(rgb(0.35,0.09,0.035)),0.78,0.09,1.34),identity[World]());
    }
    add(w,plane(Dir[World](0,1,0),0m),|h|matte(rgb(0.19,0.21,0.16)),identity[World]());
    add_sun(w,normalize(Vec[World](-3,5,2)),rgb(3.3,2.85,2.35));
    environment(w,|d|mix(rgb(0.19,0.23,0.20),rgb(0.49,0.67,0.77),saturate(d.y)));
    return frame(w,look_at(Pt[World](4.4,2.65,6.5),Pt[World](0,1.04,0),
                          Dir[World](0,1,0),0.61rad),realtime);
}
~~~

Here `socket(id,n)` is the tip of an n-link chain. Its final frame continues the transported basis of the last segment. Root placement and leg-length differences are row data. UV normals on the skinned grids come from the drawn patch; the analytic observer for this cover intersects those same triangles. The shell's increasing v and u determine the outward winding; validate against a ray from outside. The declared eye coats are thin sheets. The profile measures overlaps of their projected coverage and charges the peel depth selected for that view.

At the first kick the trunk acquires positive x momentum and turns about its COM. At the next tick the supplied contact observation releases the first toe’s old deck anchor. The second kick has negative x and positive z components while the first recovery is still active. The gate checks that both event sequences occur once, the detached anchor is retired, the scored hand is evaluated at its deadline, the face stays attached and each antenna root follows its posed socket. A comparison take with the second event removed must differ in the affected trunk/antenna rows after that tick and agree before it.

### Heron hunting at the water's edge

The fish is both a rendered object and a world-space target sampled from the same curve. It veers during the hop and then holds its new bearing. The landing puts the feet back in a position to turn the trunk. The beak is drawn on the skull's forward axis, so a saturated neck can be read directly from the picture.

```loft
import std::motion::*;
import std::render::*;
import engines::roam::roam;
import engines::realtime::realtime;

fn fish_at(t:Sec)->Pt[World] {
    let a = 0.68rad + 0.45rad * smoothstep(0.76s,1.45s,t);
    return Pt[World](12.0m*cos(a),0.12m,12.0m*sin(a));
}

pub comptime fn scene()->Scene {
    let b = body(trunk(Vec[BodySpace](1.00m,0.38m,0.32m),1.30m,2.0kg), [
        chain("left", Pt[BodySpace](-0.08,-0.12,-0.12),[0.62m,0.72m],foot(Pt[BodySpace](0.02,-1.30,-0.18))),
        chain("right",Pt[BodySpace](-0.08,-0.12, 0.12),[0.62m,0.72m],foot(Pt[BodySpace](0.02,-1.30, 0.18))),
        chain("neck", Pt[BodySpace]( 0.34, 0.10, 0.00),[1.55m],head(Pt[BodySpace](0.90,1.08,0),0.80rad,0.62rad,0.26rad))
    ]);
    let input = controls(|t|Velocity[World](0,0,0), |t|Target[World](fish_at(t)), []);
    let bird = move(b,roam(),hop(input,0.85s,0.50s,Vec[World](0.65m,0,0)));
    require_margin(bird,tip("neck"),"upright",TimeRange(0s,2.4s),
        |t|(site_at(bird,tip("neck"))-site_at(bird,root())).y-0.15m);
    let w = world();
    let ivory = |h|matte(rgb(0.81,0.83,0.76));
    let slate = |h|matte(rgb(0.19,0.27,0.33));
    let ochre = |h|matte(rgb(0.68,0.37,0.08));
    let dark = |h|matte(rgb(0.035,0.045,0.055));
    let body_frame = attachment(bird,root(),identity[Joint]());
    let skull = attachment(bird,tip("neck"),identity[Joint]());
    add(w,capsule(Pt[Joint](-0.35,0,0),Pt[Joint](0.27,0.05,0),0.20m),ivory,body_frame);
    comptime for side in [-1.0,1.0] {
        add(w,capsule(Pt[Joint](-0.48,-0.04,side*0.11m),Pt[Joint](0.10,0.05,side*0.15m),0.10m),slate,body_frame);
        add(w,sphere(0.024m),dark,attachment(bird,tip("neck"),rigid[Joint,Joint](Vec[Joint](0.039,0.024,side*0.083m))));
    }
    add(w,sphere(0.10m),ivory,skull);
    add(w,capsule(Pt[Joint](0.04,0,0),Pt[Joint](0.24,0,0),0.032m),ochre,skull);
    add(w,capsule(Pt[Joint](0.21,0,0),Pt[Joint](0.40,0,0),0.015m),ochre,skull);
    comptime for id in ["left","right"] {
        comptime for j in 0u..2u {
            add(w,capsule(site_at(bird,socket(id,j)),site_at(bird,socket(id,j+1u)),0.024m),
                ochre,identity[World]());
        }
        comptime for side in [-1.0,1.0] {
            add(w,capsule(Pt[Joint](0,0.012,0),Pt[Joint](0.16,0.012,side*0.045m),0.012m),
                ochre,attachment(bird,tip(id),identity[Joint]()));
        }
    }
    comptime for j in 0u..16u {
        let a = neck_path(bird,"neck",f32(j)/16.0);
        let z = neck_path(bird,"neck",f32(j+1u)/16.0);
        add(w,capsule(a,z,(0.060-0.020*f32(j)/15.0)*1m),ivory,identity[World]());
    }
    let fish = tracked_center(fish_at);
    let silver = |h|metal(rgb(0.54,0.68,0.61),0.27);
    add(w,capsule(Pt[Joint](0,0,-0.19),Pt[Joint](0,0,0.19),0.081m),silver,fish);
    comptime for side in [-1.0,1.0] {
        add(w,capsule(Pt[Joint](0,0,-0.18),Pt[Joint](0,side*0.105m,-0.34),0.027m),slate,fish);
    }
    add(w,plane(Dir[World](0,1,0),0m),|h|matte(rgb(0.12,0.22,0.20)),identity[World]());
    add(w,capsule(Pt[World](0.35,0.08,-0.65),Pt[World](0.35,0.08,0.65),0.045m),dark,identity[World]());
    add_sun(w,normalize(Vec[World](-2,5,2)),rgb(3.1,2.8,2.4));
    environment(w,|d|rgb(0.33,0.42,0.48));
    return frame(w,look_at(Pt[World](5.7,4,-8),Pt[World](2.9,1,2.9),Dir[World](0,1,0),0.70rad),realtime);
}
```

Unwrapped yaw crosses the neck limit in flight. The gaze constraint records a miss at the eye for the unreachable portion, the pitch gate remains separate and the post-landing turn brings the eye back on target. The drawn neck length in this scene is the sum of the capsule centreline chord lengths. Report its shortening against the analytic arc separately, with the measured chord sum deciding the drawn-length verdict. A tighter requested tolerance requires finer tessellation or an analytic swept-arc cover. The planted site is the centre of the toe's two heel contacts on the support, including pad radius.

| Heron trace oracle | Predicted at the standard tick rate |
|---|---|
| Grounded pursuit, flight, reacquisition | With the commanded yaw rate feasible while grounded, the planar bearing model predicts the neck reaches its stop near 1.18 s, peak excess about 0.15 rad near landing, and reacquisition shortly after 1.50 s. Require at least one yaw-limited eye miss inside [1.05,1.70] s, maximum missed angle 0.27 rad. Outside that window require yaw alignment to 0.01 rad, and report pitch separately |
| Drawn and measured | Upright margin positive throughout [0,2.4] s; airborne angular momentum constant until contact; planted pad drift ≤0.1 mm; a separate adverse take moves the target across ±π with the neck at its stop and must preserve its unwrapped side. Neck ideal arc and capsule chord sum are two separate observables |

### Low tide, two printing blocks

This edition shows timber pilings, dark wet stones and long ink shadows in a sand flat. The sky is unprinted paper. The ground and the tall posts receive the same two ink strikes. Light from the low right casts shadows well beyond the stones. The sand, stones and timber have distinct hatch pitches. All pilings share one definition.

```loft
// scenes/low_tide.loft
import std::render::*;
import engines::woodblock::{woodblock,Inks};
space Post, Stone;

fn unused_material(h:Hit)->Bsdf { return matte(rgb(0.0,0.0,0.0)); }

comptime fn block[S](w:World,g:Geometry[S],x:Xform[S,World],inks:Inks) {
    let t = add(w,g,unused_material,x);
    attributes(w,t,|h| inks);
}

pub comptime fn scene()->Scene {
    let w = world();
    let sand = Inks(rgb(0.95,0.82,0.57),rgb(0.18,0.25,0.38),0.17m);
    let indigo = Inks(rgb(0.42,0.62,0.67),rgb(0.09,0.11,0.21),0.09m);
    let red = Inks(rgb(0.82,0.33,0.19),rgb(0.21,0.13,0.16),0.08m);
    block(w,plane(Dir[World](0,1,0),0m),identity[World](),sand);
    comptime for i in 0u..5u {
        let f = f32(i);
        let base = Pt[Post](0m,0.10m,0m);
        let top = Pt[Post](0.12m*(f-2.0),1.85m-0.19m*f,0.11m*f);
        let place = rigid[Post,World](Vec[World](-2.5m+1.05m*f,0m,-0.40m*f));
        block(w,capsule(base,top,0.14m),place,red);
    }
    comptime for i in 0u..7u {
        let f = f32(i);
        let x = -2.7m+0.86m*f;
        let z = 0.90m+0.47m*cos(1.8rad*f);
        let r = 0.25m+0.05m*sin(2.2rad*f);
        block(w,sphere[Stone](r),rigid[Stone,World](Vec[World](x,-0.5*r,z)),indigo);
    }
    add_sun(w,normalize(Vec[World](0.8,0.38,0.44)),rgb(1.0,1.0,1.0));
    let camera = orthographic(Pt[World](5.7m,4.4m,7.9m),Pt[World](0m,0.45m,0m),
                              Dir[World](0,1,0),5.1m);
    return frame(w,camera,woodblock);
}
```

## 8. The escape table

Each row describes a source edit and the evidence available after rebuilding the affected dependency closure. A compatible interface preserves access to an observation. Its numerical verdict is re-earned on the new program. Existing render references never silently approve a changed law.

| Forkable level | What the author writes | What they keep for free | What they lose or must supply |
|---|---|---|---|
| One BSDF lobe | A new distribution, Fresnel term or complete continuous/delta lobe with a matching proposal; pass it to the existing composition. | Geometry, depth, shadow casters, motion sites, integrators and the per-lobe furnace/sampling harness. | The prior lobe's albedo cache and energy result. Reintegrate its directional albedo; delta support requires a discrete sampler. |
| Whole material | Return a new response from the material reading and attach custom attributes where a look needs them. | Hit, chart, motion, depth, direct/indirect observers and visibility in other worlds. | Supply scattering for every measure claimed, emitter sampling as applicable, and new material verdicts. A print-only material can just expose its inks. |
| Light source or light fold | Implement emitter sampling and reciprocal density, or enumerate arrivals with an authored fold. | Current posed occluders, media and receiver geometry. | A different proposal needs new weights. A max, quantised or coloured fold owns the resulting look. |
| Transport estimator | Supply direct, GI, reflective, participating-medium or path integration code over a chosen world. | Six queries, alternate shadow worlds, record reconstruction and scene sampling key. | The previous estimator's bias, convergence and history claims. Declare approximations or score its linear-light reference. |
| Geometry or intersector | Define bound/hit, field or patch readings and provide the observer adapters. | All clients of those readings, including picks, rays from lights, reflections and analysis of typed source. | Supply coverage/side/inside/exit and reconstruction semantics; proof obligations follow the new bound. |
| One shader of one kind | Replace its record, shade or forward entry using typed buffers and the current codec. A raw fragment can return actual hit depth. | All other kind programs; conforming record output can still be shaded and reprojected; unchanged geometric hits still cast shadows. | Raw arithmetic loses AD, intervals and CPU evaluation on the dependent path. Moving a drawn surface also requires its ray/site/depth readers to agree. |
| Visibility record | Supply a codec, attachment schema, covered-sample writer, decoder, identity and history correspondence; pass the codec into the producer and consumers. | Independent world-ray queries, material and motion laws, pass validation and indirect draws. | Rebuild record readers, classifier, sample resolve and cache/history ABI; demonstrate coverage, picking and reconstruction for the new format. |
| Pass, rate or schedule | Reorder public passes or build a pass DAG with different rates, culling, history, targets or raw compute. | Resource typing, hazard checks, batching where legal, geometry/material/pose laws, profiling and host ownership. | New order, sample count and history need their own time, memory, picture and reproducibility evidence. |
| One motion law | Replace a value function in step/present, for example the calculation of swing height from the old pad and target. | BODY, input log, stepping clocks, compatible pins and final-pose observer interfaces. | Dependent bakes, checkpoints and verdicts become stale. Preserve state through an explicit conversion or replay. |
| Solver | Replace the strand/contact solver with a colored or gathered implementation that publishes compatible state and pose. | The controller and render consumers of its public outputs, scheduling, source diagnostics and check machinery. | Prove single-writer or ordered reduction, justify numerical/replay eligibility and score individual residuals. |
| Controller or imported performance | Supply a step and present pair, drive it from a score/clip/live generator, or provide external POSE rows. | Rendering, corrected pose export, observed-site checks and identity for declared inputs. | Own the dynamics, attachment or retarget correspondence and timing. An external pose cannot inherit evidence from the solver it bypassed. |
| Whole engine | Export an engine returning a new schedule, combine different render/light/motion sources, or submit a typed package to the runner. | Device integration, resources, validated layouts, source mapping, explicit clocks and any public law/observer the new engine calls. | Establish its own attachment/depth contract, frame budgets and reference images; any abandoned reading is a named unavailable observation. |

## 9. Predicted numbers

All costs below are **predicted** for native Apple M4 Metal at 1280 × 800 unless explicitly marked **measured**. These are acceptance workloads, including the camera, rather than device-independent claims about a crab. The model charges measured command issue cost once, then separately charges pixels, intersection evaluations, triangle/refit work, motion and attachment traffic. Fit coefficients are tested with pass timestamps and counters. A miss changes the theory before an optimization is attempted.

| Calibration input (**measured**, from the handoff) | Value and scope |
|---|---|
| Render-kind slice | Identical 6 modules/pipelines from 10 to 1,000 things; 2.40 ms at 10 and 4.60 ms at 30, 1280 × 800. |
| Five-crab historical comparison | Old loader 3.0/3.5 ms p50/p95, 305 modules, 32.9/3.6 s cold/warm; redesign 97/109 ms with about 5.54k modules. Different shader work from the fixtures below. |
| Command and topology cost | Corpus fit about 1.3 ms + 15 µs per command over 229 scenes; less predictive at low command counts. Pose batching 24 dispatches/0.83 ms versus 4,884/13.7 ms. Shader sharing 2.2 ms versus per-draw pipelines 17.3 ms. |
| Geometry substitution | Coral close view 3.08 ms listed versus 25.89 ms bricked march, far 0.72 versus 1.90 ms. Method change altered 1.5–9.5% of pixels. Grain veil 120.8 ms versus 1.56 ms with a 32³ grid. |
| Motion cost and instrument | Original eight-foot law at 1/256/1,000 crabs took 0.070/0.101/0.086 ms p95, bake 5.3 ms. Timestamp increment 0.065 ms. Original 1,000-crab number excludes this specification's XPBD antennae, constrained force law and strict arithmetic profile. |

**Capture protocol.** Use linear HDR before film for light tests, four coverage samples for beauty, one light proposal per receiver, one glossy continuation on eligible samples, no screen history or shutter/lens jitter, and Tier 1 everywhere. Orthographic print uses one coverage sample for its edition fixture. Include all engine draws/dispatches inside the timestamped frame, from motion through output. Measure standalone GPU time offscreen; report CPU submit/present latency and vsync waits separately. Use frames from the entire take including impulses, across repeated warmed loops; report at least 600 timed frames after 60 warmup frames, nearest-rank quantiles, and pass spans. Cold load includes parsing, build, static CPU scan/bakes, upload, native compile and first prepared frame after device acquisition. Warm means same process-independent assets and verified cached code with a *new* instance. Report device acquisition, transfer, actual cold-cache evidence and first-present separately. Stamp OS/driver/compiler, resolution, workload counters and scene hash. Prediction assumes resident device, no paging, offline local assets and a quiet machine.

In the realtime schedule let O be the number of opaque draw batches, F forward batches and L the peeled layer budget. Each opaque batch has a cover pipeline and a shade pipeline in one module. Each forward batch has one pipeline. The six shared modules/pipelines are counter clear, cull/args, tile classify, reflection/sky, medium/layer resolve and film/output. Media resolve executes once more for each peeled layer. Each motion bucket M and acceleration/refit bucket B adds a module, a pipeline and a dispatch in these fixtures. Cold-only tables use the CPU twin. History H adds one each when enabled. Thus the draw/dispatch count is M+B+6+2O+L(F+1)+H, with L=0 when F=0. This profile uses the clear compute pass to reset compacted and tile counters; other lowerings price their own reset. Buffer clears as load operations on the attachments are excluded from the draw/dispatch count and reported alongside it. Dispatch dimensions, unused zero-count draws, transferred bytes and query counts are logged. No per-light shadow-map pass is assumed.

**Hermit crab.** The copy fixture translates twenty complete actors with their own input logs, targets and root frames onto the same plane. For rank r in 0…3 and column c in 0…4, translate each actor and its world-space input positions by (4.4(c−2), 0, 4.6(r−1.5)) metres, leaving velocities and impulses unchanged. Use an orthographic camera at (0,16,23) metres aimed at (0,1,0), up +y, vertical span 16 m and the same plane and environment as the single. All twenty enter the view and every uncovered primary ray reaches the plane. Lenses are single-sided convex front patches. For the benchmark require a depth-complexity capture with maximum one forward interface before opaque depth. Charge a further forward draw and a composite dispatch per excess layer, or fail the fixture. Each animal has twelve leg rows and four separate strands. Use offline local-space shell geometry/BVH, one workgroup per deforming limb to produce leaves/subtree bounds, then one shared top-level refit over actor clusters. Shutters are closed at the scored pose.

| Hermit fixture (**predicted**) | One actor | Twenty actors, wide view |
|---|---:|---:|
| Opaque / forward batches; opaque / forward rows (one shared floor) | 8 / 1; 24 / 2 | 8 / 1; 461 / 40 |
| Actual grid triangles before cull (including lens) | 9,048 | 180,960 |
| Primary candidates for field proxies, including overlap; accepted opaque animal pixels excluding floor | 108k; 112k | 285k; 303k |
| Opaque/floor/sky pixels (animal and floor can both cast or receive rays) | 112k / 734k / 178k | 303k / 721k / 0 |
| Mean claw/face field evaluations per candidate, incl refinement and rejected rays; lens pixels | 23 / 15; 1,600 | 21 / 14; 4,100 |
| Direct world rays reaching acceleration, glossy continuations, computed per pixel or distinct coverage observation | 0.76M / 62k | 0.82M / 158k |
| Acceleration + motion buckets, forward layers, history | 2 + 2, 1, 0 | 2 + 2, 1, 0 |
| Modules / pipelines | 19 / 27 | 19 / 27 |
| Draws (record, forward, film) / dispatches / total GPU commands | 10 / 18 / **28** | 10 / 18 / **28** |
| GPU p50 model: motion + refit/cull + raster/record/trace + classify/direct + reflection/lens/media + resolve/issue | 0.22 + 0.25 + 1.65 + 1.20 + 1.38 + 0.68 = **5.38 ms** | 0.30 + 0.78 + 3.75 + 2.25 + 3.04 + 0.68 = **10.80 ms** |
| GPU p50 / p95, native | **5.38 / 6.80 ms** | **10.80 / 14.00 ms** |
| Cold / warm load, incl rows and first prepare | 1.75 / 0.19 s | 1.83 / 0.23 s |
| Browser WebGPU on same M4, modules / pipelines / GPU commands | 19 / 27 / 28 | 19 / 27 / 28 |
| Browser GPU p50 / p95; cold / warm first prepare | 5.8 / 7.5 ms; 2.0 / 0.28 s | 11.5 / 15.4 ms; 2.1 / 0.34 s |
| Isolated full motion, p95 at **1 / 1,000** actors; steady 2 ticks/frame | **0.30 / 2.70 ms** | Same replication fixture, rendering disabled |
| State / POSE budget per boss; scripted bake of 480 ticks for one | 4.5 / 4.0 KiB; 28 ms | Input prefix and ≥1 checkpoint extra per retained window |

The eight opaque batches are shell, torso, legs, arms, claw field, face field, whips and plane. The cover/command ledger therefore charges real work omitted by the earlier thirteen-command forecast. The per-pass split assumes a 60/40 claw/face proxy-fragment mix, near-to-far bounds, four coverage samples shaded once when identical, mixed cells reclassified, and no free early rejection for a shader writing depth. Mean shadow traversal visits six row bounds and one detailed leaf for the single actor, twelve and two for the wide view. Log actual distributions, including root-isolation retries and unresolved segments. Derivative certificates for the pinned field allow the compiler to use segment-local bounds in near-empty regions.

At full strand quality, each boss owes 2 × 4 × 6 × 15 × 4 = **2,880 edge projections/frame** (ticks × substeps × sweeps × edges × strands). The thousand-boss timing assumes roughly 0.75 ns amortized per projection including strict arithmetic/gathers, plus controller and bandwidth overhead. The published sub-half-millisecond crowd target below has its own workload. Removing antenna constraints, relaxing arithmetic or counting only pose writes would change the experiment.

**Heron hunting.** One actor, one visible fish, a ground plane and a low rail. Use the supplied camera with a level horizon; keep the rail and fish in the frame during landing. The neck has a smooth centreline and a sampled capsule cover. Cover/pick rays hit the *same capsules*. Finite analytic objects use bounded proxies; derive row bounds in one packed frame fill. Traverse a small two-level cluster index, then exact capsules and spheres. The dry floor uses actual visibility. Fish highlights contribute short world-ray continuations.

| Heron fixture (**predicted**) | Value |
|---|---:|
| Opaque kinds and rows, forward and L | 8 batches / 37 rows (33 capsules, 3 spheres, 1 plane); 0 / 0 |
| Triangle-equivalent work | 432 bound-proxy triangles (36 finite rows × 12), 2 plane-cover triangles; 0 tessellated character triangles |
| Primary analytic proxy candidates; animal/fish, ground/rail and sky pixels | 73k; 20k / 866k / 138k |
| Direct visibility rays / glossy rays; mean bound/primitive tests per visibility ray | 0.70M / 3.0k; 5 / 0.8 |
| Motion / bound dispatches, history | 1 / 1, 0 |
| Modules / pipelines / GPU commands (draws + dispatches) | **16 / 24 / 24** (9 + 15) |
| GPU p50 components: motion/bounds + records + direct + reflection/media + issue/output; p50 / p95 | 0.19 + 0.56 + 1.02 + 0.48 + 0.56; **2.81 / 3.55 ms** |
| Cold / warm load | **1.03 / 0.13 s** |
| Isolated motion p95 for **1 / 1,000** herons | **0.13 / 0.43 ms** |
| State / POSE allocation budget per actor; scripted one-heron 480-tick bake | 1.0 / 0.75 KiB; 8.5 ms |

The motion estimate assumes a two-leg support solve, one analytic neck, two fixed ticks per rendered frame and a presentation at the fractional tick. Sweep the fish through the saturation interval and include that interval in timing. Across repeated actors the target positions and starting phases vary in rows. No driver work is shared across separate device caches.

**Low tide.** The print engine reads pigment and one actual directional shadow per accepted interior sample. It skips tile classification, BSDF sampling and glossy rays. The low sun lengthens the shadow cast by each piling across the sand. The ground still uses its full covered-pixel budget.

| Low-tide fixture (**predicted**) | Value |
|---|---:|
| Static opaque batches, rows, forward and L | 3 / 13 (ground, 5 pilings, 7 stones); 0 / 0 |
| Triangle-equivalent work; solid proxy candidates | 144 bound triangles + 2 plane triangles; 83k candidate pixels |
| Distinct solid, sand and background pixel counts; real shadow rays | 45k / 979k / 0; 1.024M |
| Bound / detailed tests per shadow ray (flat static list with per-kind bounds or equivalent group skips) | 12 / 0.8 |
| Modules / pipelines / GPU commands (draws + dispatches) | **7 / 7 / 7** (4 + 3, including compute print and film/output draw) |
| GPU p50 components: cull + covers + ink/shadow + issue/output; p50 / p95 | 0.06 + 0.27 + 1.06 + 0.34; **1.73 / 2.15 ms** |
| Cold / warm load | **0.48 / 0.065 s** |
| Isolated motion p95 at **1 / 1,000** replicated subjects; state / pose / bake | **0 / 0 ms** scheduled work; 0 B mutable state, no pose bake |

**Cost assumptions that can fail.** For load, use compiled-text equivalents of 13.5k / 8k / 3.6k WGSL lines (crab/heron/print), 100 μs per fresh line, 8 μs per cached line plus per-pipeline preparation, upload and static bakes. Split first-time driver work from compile, resource creation and row transfer. For p95 use the specified high-work camera frames with approximately 25–30% headroom for the moving fixtures and 24% for print; these margins are predictions to replace with frame distributions. The old command fit prices only about 0.42 ms of the boss frame from commands. It does not predict shader evaluation or permit omitting commands executed inside bundles.

For deterministic crowd simulation publish both the qualified native arithmetic and integer-emulated profiles. As a planning example, **predicted** time for 1,000 eight-foot actors, 7,400 scalar operations per actor/frame and 300 Ginteger-op/s is about 0.025 ms per integer instruction of mean expansion, plus dispatch and memory. At forty integer operations per semantic operation this is already about 1.0 ms. The crowd gate requires an actual qualified fast implementation and separate counters for division, square root, workgroup barriers, active-set iterations, spills and bytes. An adapter missing the gate is reported under its profile.

## 10. Build order

Start a fresh Rust workspace with runner, IR, compiler and CLI crates. Pin wgpu/Naga together with the embedding host. The `lib` and `engines` trees contain readable `.loft` source. Native CLI and WASM call the same compiler, package reader and gate definitions. Keep the legacy tag as evidence. A phase ends with its named picture, machine-readable gate report, scoped negative fixtures and an owner-visible comparison. Each row below is a vertical integration, with one integrator running the full current suite on the combined revision. A missing render, unapproved aesthetic reference, failed threshold or incorrect refusal holds the phase open. Persist the package/identity and last completed fixture so an interrupted gate can resume without silently skipping a frame.

| Phase and artifact | L0 runner | L1 IR and gate definitions | L2 compiler | L3 source library and engines | Delete or retire on acceptance |
|---|---:|---:|---:|---:|---|
| 0 · Spine. CLI and WASM, typed source to analytic washer and implicit sphere plates, real depth/pick and a checked recurrence. | +420 | +470 | +1,700 | +350 | Remove any handwritten shader/row path used to bootstrap these plates. Close the old wire-port milestone at the legacy tag. |
| 1 · Frozen hermit. Implement build-row loops, geometry families, cell-scoped maps, scans/AD, shell, claw, face, twelve legs, basic tiles, material and direct daylight. Pose is a static table. | +100 | +170 | +2,000 | +1,450 | Remove scalar safety guesses, repeated limb modules, temporary analytic-only cover and pre-painted crab proxies. |
| 2 · Edit the visible claw. Incremental spans/ABI, tape, native background compile, last-good package and source-mapped diagnostics. | +125 | +100 | +1,100 | +200 | Remove restart-on-row-edit and unconditional full-world tape/validation paths in the editor prototype. |
| 3 · Play the scramble, interrupted slam and heron take. Logged inputs, snapshots, deterministic arithmetic, build fit, support/IK and strands. | +195 | +230 | +2,100 | +2,450 | Remove static pose fixture from the live scenes, frame-rate stepping and separately compiled bake laws; keep static rows as an external POSE producer. |
| 4 · Light and ink. The same world in realtime, converged reference and Low Tide. Add forward interfaces, true shadow/secondary observers, media, history and radiometry fixtures. | +55 | +180 | +1,300 | +2,600 | Remove constant-light stand-ins and diagnostic-only material decoders from the shipping schedule. Archive input plates so new lighting cannot mask a geometry regression. |
| 5 · Corpus and host. Import/export assets and clips, generate seeds/examples, deliver device-owned embedding, production tools, training and viewer. | +95 | +250 | +1,600 | +6,300 | Switch training and native/browser hosts to the new runner; retire the old TS front end/compiler/host, Rust redesign, test-only adapters, old API map and wire readers from the active product. Preserve archived sources and reference data at their pinned revisions. |
| **Predicted cumulative hand-written lines** | **990** | **1,400** | **9,800** | **13,350** | Runner ceiling 1,000; IR plus shared gate definitions ceiling 1,500. Fixtures, individual scenes, golden images and generated bindings are inventoried separately. |

**Phase acceptance.** These are **predicted** gates for the specified fixture. Profile once against the prediction, diagnose a miss, then record a new hypothesis before changing code. Geometry uses the complete manifest and edge rule in §3 from the first slice. The first approved beauty reference is a human decision; arithmetic or a self-generated golden cannot grant it.

| Phase | Pass/fail checks, including what appears on screen |
|---|---|
| 0 | At 512² orthographic centre samples covering [−4,4]², place the z=1 washer, outer radius 1, over a card at z=0, camera (0,0,3) facing −z. Inner radius 0.5 gives **9,664** washer samples, 0.25 gives **12,080**. With light toward (2,0,1), the card's blocked mask equals the washer mask shifted 128 pixels left; a deliberately solid shadow world gives **12,892** blocked and leaves the primary unchanged. From (0,0,3) pick the card at 3 m; from (0,0.375,3) the edit moves the first hit from 3 to 2 m. For a second plate use a unit implicit sphere centered at the origin, from (0,0,3) toward −z get t=2 m and from inside get the exit at t=1 m within 10⁻⁴ m. Inject an unresolved tangent and a missing frame: both return their named failures. Keep shader hashes for one and twenty identical rows; ≤7 modules/pipelines, ≤8 commands, p95 ≤3 ms at 1280×800, cold/warm ≤0.65/0.10 s. Twin recurrence replays with zero differing words. Render both fixtures on a native and a core browser device. |
| 1 | Freeze the source at one rest pose with opaque primary visibility, one direct lighting pass and an opaque cornea proxy explicitly labelled as the Phase 1 look. Check AD normal/curvature versus independent smooth-chart differences; exact rest axes and seam; full-amplitude claw slope certificate rounds up from 2.535833… before arithmetic enclosure. A mirror pair lifts to distinct world eye centres; twelve leg row sizes generate one leg family. Drive the same edit from one to twenty crabs: identical code hashes/pipeline counts, zero extra commands, row bytes account for the increase. For this **reduced schedule**, ≤16 modules, ≤22 pipelines, ≤23 commands, one/twenty p95 ≤5.5/10.5 ms, cold/warm one ≤1.6/0.2 s. Compare both silhouettes and inner claw opening with approved plates and trace off-camera test rays. |
| 2 | On resident fixtures apply a row edit, eligible 63-op pigment edit, opaque/ABI edit, then rapid undo/redo. Row edit changes zero shaders. Eligible host patch p95 <2 ms; price first presentation at the following refresh separately, and measure extra GPU time ≤0.50 ms for a dirty pigment evaluated once on 12% of the frame. Native is active after compilation; swap meets §3 bounds. Opaque/ABI edit explicitly shows the prior revision until the complete replacement is ready. No stale async result installs after undo. E104/E211/E307 identify the failing author expressions. E401/E402 reject continuous or excessive discrete specialization. Gate the actual pre/post-swap image, cache hit level, code count and query witnesses. |
| 3 | Draw one 4 s legacy-dimension eight-foot fixture and the §7 boss/heron excerpts. The eight-foot legacy implementation **measured** 22 steps, 0.0008 mm post-IK drift and caught a 20 mm fault; it has no signed-off owner verdict on weight. V1 **predicts** 20–25 steps, ≤0.1 mm drift, zero inhibited neighbour overlap; compare acceleration, braking and swing timing by eye. At 30/60/120 Hz and a six-tick stall get **0/481** state-row differences for the same tick inputs, including seek around both hits. Only the injected toe fails the planted-site test. No unsupported yaw impulse in flight; the hunting bird exceeds its neck yaw briefly and recovers on planted support, and drawn eye, pitch, neck and posture have independent results. The constrained build take with ballistic flight and a 20° pitch hold stays inside that envelope; the omitted-hold fixture must expose the omission. Three render-independent populations of 1,000 each: eight-foot roam p95 ≤0.45 ms; herons ≤0.50 ms; full boss strands ≤3.0 ms. A mixed crowd of 750 eight-foot crabs and 250 herons, two law buckets, must also reach **<0.50 ms**. Profile all of them with the §3 strict twin enabled for comparisons and no readback in the timed span. A single live eight-foot crab's bake ≤9 ms. Owner reviews weight and gaze at game distance. |
| 4 | All §9 full-schedule module/pipeline/command counts and p95/load budgets pass, including twenty native and browser bosses. In a closed diffuse furnace with wall emission E, albedo ½ and D reflections, compare linear radiance to E·Σᵢ₌₀ᴰ ½ⁱ. At D=1,2 expect 1.50E,1.75E within 1%. White rough metal under a uniform 0.5 sky returns 0.50 within 2% at centre/rim. One metre of dye at (0.2,0.7,1.4)/m gives transmittance (0.81873,0.49659,0.24660). Coincident homogeneous media, each with neutral extinction σₜ=ln(2)/m and emission j=σₜ in its own red or blue channel, length 1 m, over black gives (0.375,0,0.375), T=0.25, in either insertion order within 10⁻⁴ absolute. Test 1,000 emissive shapes with ≤4 shadow rays/receiver. Require independent-batch 95% half-width ≤0.005 per *linear* test patch; double samples through 4,096, then return Unknown on nonconvergence. For matched-bounce unbiased estimators require the confidence interval for relative mean bias on nonblack interiors within ±1%, and absolute error ≤0.005 on black; separately price each explicitly biased approximation. Check no-history frame-alone equality and viewer resets at cut, in particular frames 1, 4 and 16. The ink edition retains its apparent cast shadows under camera motion. |
| 5 | Regenerate all **125 examples and 170 seeds (measured inventory)** and freeze new expected frame/view/time keys and owner-approved reference hashes. **Predicted** acceptance: 295/295 entries accounted for, 100% of required frames/checks and all declared negative fixtures, zero silent skips. Each scene has its own p95/modules/commands/cold/warm budget signed before it runs. Training on 512² ×16 frames: CPU build ≤2 s and GPU p95 ≤50 ms/frame per seed. Check at least 12 body plans ×6 motions including a biped, many-legged body and non-human retarget; report evidence and look approval per cell. Import glTF mesh/skin/clip and live pose packets, export corrected local TRS, compare held sites within 1 mm at midpoints or split intervals. Exercise native Metal/Vulkan/DX12 and a core WebGPU browser, with capability refusals explicit. Embed in Keel and room-runners on host-owned devices, test reverse-Z and forward-Z over host geometry: zero unauthorized clears, submits and synchronous readbacks; replay stable after resize/seek. Re-pin creative training only after verifying packages through its actual load and render entry point. |

Each phase publishes the lowerings that achieved its score. Changes to an observer, a target layout, a training frame key, or the appearance of a regenerated scene list the affected fixtures and obtain an approval for the intended new image. The product gate tests new v1 semantics, not the historical arrangement of shader bytes.

## 11. Open questions

The following choices depend on a measurement or an artistic judgment unavailable in the handoff. Each default above is sufficient to build and score the next slice.

| Question | Default until the experiment resolves it |
|---|---|
| Does the contact-driven reversal read as weight on the new crab and as recovery on the biped? | Ship the load- and support-gated controller for the trials. Compare locked-camera clips beside the owner-approved heron neck and the crab reversal panels before selecting a training reference. Preserve the measured foot and gaze traces when judging a change of style. |
| Which native operations can accelerate strict motion on each adapter while matching the twin, and does the resulting crowd meet its budget? | Use the bit-defined implementation. Substitute only a range-qualified equivalent lowering. Report each adapter's operation mix, parity and crowd timestamps. A budget miss blocks that profile's release. |
| Which field accelerator wins on the engraved claw and the legacy folded coral at game distance? | Start with per-cell interval bounds, analytic branches where available and an ordered root-isolation path. Benchmark listed branches, bricked bounds and triangulation on the same views, first scoring silhouettes, ID and hit depth. Adopt a substitution only for the observations it passes. |
| What image error does screen-history reuse buy per unit GPU time in animated reflections? | Ship independent samples in stills and training. In the viewer allow revision-keyed reprojection, reset on disocclusion or a cut, and compare the first and steady frames at equal time against the converged reference before enabling longer reuse. |

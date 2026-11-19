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


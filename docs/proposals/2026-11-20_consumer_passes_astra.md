# Consumer passes for resumable motion

Design proposal, 20 November 2026. The decision is open.

## Verdict

**Yes. A consumer should be able to define what happens when execution reaches a block, suspends, and enters it again.** Give the imported library ownership of the meaning of its construct. Give the compiler ownership of the types, effects, storage layout and schedule that make that meaning executable. A motion author should be able to replace “continue immediately when ready” with “resume on the next tick” by forking a library. Today that change needs a compiler fork. [Owner's question](2026-11-20_consumer_passes.md) · [Philosophy §0.5](../PHILOSOPHY.md#05-agentic-development-inverts-the-old-trade)

I disagree with the lead about **where** to rewrite and **what** rechecking establishes. On today's crab scene, `loft ir scenes/crab.loft --sexp` begins with the following excerpt, ending inside the first dispatch.

```text
#("loft-ir-v1" ((bakes) (path) (dispatches ((module . 2) (entry . "advance") (workgroup_x . 64)
```

This is a serialized `loft_passes::program::Program`, including packed rows, dispatches, Naga expression arenas and source origins. Its reader validates Naga, regenerates WGSL and checks indices, strides and bindings. Both metres and seconds can already be `f32` there. An author-approved write to a field and a store into a field owned by another law can both be valid Naga. A suspend point and the lexical binding live across it may already have disappeared. Keep this interchange for executable programs. Give rewriters a view **before** those distinctions disappear. [CLI](../../crates/loft-cli/src/main.rs) · [S-expression codec](../../crates/loft-passes/src/sexp.rs) · [Program](../../crates/loft-passes/src/program.rs)

The registry in `loft-ir` is a reservation. Its type is `BTreeMap<SymId, fn(&mut IrModule) -> Res<()>>`. The module holds a vector whose sole op variant is `Intrinsic { name, args }`. A list of value operands cannot say when to reevaluate a predicate or which statements it guards. The actual compiler indexes parsed modules into `Prog`, checks expressions into a type vector, invokes lowering functions directly, and emits a `Program`. For example, `motion_module` walks scheduled laws and emits the body of `advance`. Exposing that registry as-is would freeze an interface the compiler itself cannot use for this example. [IR reservation](../../crates/loft-ir/src/bundle.rs) · [Compilation pipeline](../../crates/loft-passes/src/lib.rs) · [Motion lowering](../../crates/loft-passes/src/motion.rs)

A checker can establish that either resume policy is a valid program. A twin executing the same lowered program can faithfully reproduce the same *wrong tick*. The library therefore owes the user a source-level contract and a trace with an independent expected answer. An optimization of existing code has a different obligation again. The compiler must establish that the proposed transformation preserves the original observations. Merely typechecking two schedules with different results establishes very little.

## The pass contract

The first public phase is **typed-region lowering**. A region is an ordered block with resolved binders, typed expressions, and the surrounding law's row, stage and allowed writes. It retains the statements in a predicate's lexical scope. The compiler exposes the region after resolving imports and checking the declared signatures, before AD, motion fusion, packing or shader emission. The checker may assume an imported marker's signature while checking the body. Only a successful lowering discharges the obligation to remove that marker.

A package exports a qualified block symbol and any marker signatures it owns. An ordinary explicit import binds those names in the author's module. Resolving a use of the block selects one exact, content-pinned export. The claim covers that occurrence and the markers bound to it. It grants reads of its typed operands, replacement of its own body, and fresh storage on its target row. The host takes the enclosing law's public write set from source. A library cannot authorize a second writer, replace a sibling law, or manufacture a waiver by including one in a reply. Two imports with the same local name get the normal ambiguity diagnostic at both imports. A qualified export supplied by two different package digests is a resolution error. An author can fork the package and bind its export in their module.

The host enumerates original sites in canonical module, declaration and statement order. First-version claims are disjoint. Nested claimed regions are diagnosed at both sites. Markers belong to their enclosing claim. Each replacement runs once and must contain ordinary checked operations at its output boundary. This makes composition finite without priority numbers or import-order precedence. Independent laws can use different passes and compose through the ordinary schedule. Stage-zero evaluation, schedule construction, row-law fusion, derivation and target emission retain their compiler dependencies. A later schedule pass may propose a change at the schedule boundary described below. A library import alone schedules no global optimizer.

**Language.** Write the reference pass in Rust, build it to a deterministic WebAssembly guest, and publish the guest with its source. Consumers can use any language that targets the same ABI. The scene author needs only Loft and the package. The pass author pays for a guest toolchain, marshalling a region once per cold rewrite, and debugging across a module boundary. Native Rust function pointers remain an internal implementation choice. A public native plugin would also make the compiler's process, allocator and release toolchain part of the interface. A new build-time `.loft` pattern language would need binders, region traversal, liveness and diagnostics before it could write this pass. Revisit that language if multiple real passes share a smaller notation. Use the same guest and verifier for the shipped example and its forks.

**Verification.** Decode each reply into a scratch region. Check bounds on IDs and expansion size, scope and dominance, initialization on every path, types including spaces and units, stage, reachable calls, field ownership, row locality and capabilities against the enclosing source contract. Reject a foreign marker or one left in a copied subtree. Discard incoming analysis facts. Install only a checked replacement. Before execution, re-index the combined module, check inter-law conflicts and solver races (E501–E503), redo stage and E504 checks transitively, derive intervals, smoothness and bounds from the resulting operations, and reconstruct the resource DAG (E505). Check actual target accesses and bindings against the plan and validate Naga. Preserve explicit source opt-outs and their truth records. Reject a synthesized opt-out or a `kernel allow(...)` escalation. A motion operation without deterministic CPU support gets a named refusal before bundling. The semantic checks also apply on cache hits. For example, changing `store(row.yaw, saved_angle)` to store a distance must fail E110 even though both store one word. A copied helper with unmarked raw motion WGSL must fail E504 at the authored call. A second law that claims `yaw` must still fail E502 after expansion. [Language](../spec/01_LANGUAGE_V1.md) · [Strict desugaring and opt-outs](../spec/PATCH_V1_2.md) · [CPU stepping](../../crates/loft-std/src/stepping.rs)

Run Pictures, Twin, Speed, Load and Truth on the **composed output**. Running the picture suite between two incomplete lowerings cannot validate the final picture. For the example, a hand-written law supplies both a picture baseline and a speed control. Twin checks a fixture oracle as well as CPU/GPU agreement. Load compares module bytes across row counts. Truth uses the ordinary source checks and verdict rules. `loft show` should show the imported export, its replaced block, allocated fields, and resulting entry. In `profile --json`, link the contributing sites to the fused entry and its §07 module counters. Report cold rewrite CPU time, cache hits and measured GPU time for the whole dispatch. A single timestamp pair cannot apportion GPU milliseconds among fused laws. [Gates](../spec/02_CORE_ABI_AND_LOWERING.md) · [Platform telemetry](../spec/07_PLATFORM_POWER_EXTENSIONS.md#62-moduletelemetry)

**Hygiene and reproducibility.** A copied reference retains its resolved binding ID. The host allocates fresh locals and private fields from the owning site and a guest-local ordinal, in separate namespaces from source names. Each generated node must cite a valid input origin. A copied expression keeps its precise origin. Synthetic guards and saves cite the marker or binding that caused them, with the block, package export and digest as expansion notes. The host owns the mapping to source spans and composes it with the existing Naga expression map. Guests receive site-relative IDs and normalized values, with no filenames, offsets or trivia. On a cache hit, reattach today's source spans to the saved origin IDs. [Emitter provenance](../../crates/loft-passes/src/provenance.rs) · [Derivation cache precedent](../../crates/loft-passes/src/cache.rs)

Instantiate a fresh integer-only Wasm guest per call. Reject imports, start functions, floating-point instructions/types, threads, SIMD and reference types in this ABI. The host supplies bytes and deterministic instruction, memory and output limits fixed by the ABI version. Exhaustion or a guest trap is a compile diagnostic at the block. Time, filesystem, environment, random sources and adapter queries are absent. Use BLAKE3 over length-delimited canonical input, including pass ABI and host checker/lowering version, package manifest and Wasm digest, resolved region with transitive signatures/bodies and refinements, permissions, and any exposed target or build constants. Pin package resolution in the build inputs. Sort declarations by binding identity and preserve statement order, literal bits and units. Publish a complete rewrite and origin map atomically. Downstream bounds and pipelines keep their own keys over verified geometry or emitted modules, layouts and target. A cross-build determinism fixture compiles the same input in fresh processes with the cache disabled, then compares the reply bytes, module hashes and bundle bytes with fixed debug settings. A package edit misses the rewrite cache even if it emits identical shader bytes. A comment edit can reuse the rewrite and still move a diagnostic to the correct line.

## Worked example — walk to the rock, wait for the claw, turn

The following is proposed author source. The second law supplies a small deterministic motion fixture. The kind's six scalar fields are also the entire public state of this fixture.

```loft
use shore::motion::{behaviour, until}

space World
kind Crab @World (
  x: m = 0m, rock_x: m = 0.5m, goal_x: m = 0m,
  bearing: rad = 0.5rad, claw_closed: bool = false, yaw: rad = 0rad,
) {}

law approach(c: Crab, dt) writes c.{goal_x, yaw} {
  behaviour {
    let face = c.bearing
    c.goal_x = c.rock_x
    until(c.x >= c.goal_x)
    until(c.claw_closed)
    c.yaw = face
  }
}

law stride(c: Crab, dt) writes c.{x, bearing, claw_closed} {
  c.x = min(c.goal_x, c.x + 0.25m)
  c.bearing = c.bearing + 0.25rad
  c.claw_closed = c.bearing >= 1.75rad
}
build { emit Crab() }
schedule Main { run_laws(approach, stride) }
```

This library accepts a single `behaviour` as a law's whole body. It accepts straight-line immutable bindings, owned field assignments and standalone `until(bool)` markers with pure row-local predicates. Diagnose other forms at the unsupported statement. A row starts at entry. Run ordinary statements once, in order. A false guard stops this law for this tick. The next tick retries that guard against the then-current row. A true guard continues immediately, including through successive ready guards. Completion is absorbing for the life of that row. Other laws still run on a tick when this law suspends. These choices belong to the package and can be changed in a fork.

**The pass.** This is the complete lowering algorithm in pseudocode over the proposed public view. A segment after a marker starts with its unevaluated guard. The last use of a binding includes uses in guard expressions.

```text
segments = split_at_resolved_marker(body, until)     # entry, then one segment per guard
pc = fresh_field(u, zero, origin = block)
for each binding used in a later segment:
    allocate a private zero-initialized field of its storable type, or diagnose at the binding
k = fresh_local(load(pc))
for (i, segment) in source_order(segments):
    emit if k == i:
        emit if segment.guard (entry uses true):
            replay its statements in order with resolved references
            at a spilled binding's definition, save its value
            substitute a field load for uses of that binding in later segments
            set k = i + 1
emit store(pc, k)
```

Each test reads the *current* local `k`. A false guard leaves it unchanged, so every later segment skips. The forward scan has a static bound given by the number of segments. It emits each source statement once, has no busy wait, and needs no per-row stack or dynamic loop. The slot holding `face` makes the author-visible difference between sampling a bearing on arrival and remembering the bearing from departure.

**Emitted IR.** This is the proposed checked region written as a listing with types and resolved field labels. The successive `if`s are ordinary structured branches. `k` is a mutable local, and `row` is the current invocation's row.

```text
private row fields    %pc : u = 0       %face : rad = 0rad
local k : u = load(row.%pc)
if k == 0 {
    store(row.%face, load(row.bearing))
    store(row.goal_x, load(row.rock_x))
    k = 1
}
if k == 1 {
    if load(row.x) >= load(row.goal_x) { k = 2 }
}
if k == 2 {
    if load(row.claw_closed) { store(row.yaw, load(row.%face)); k = 3 }
}
store(row.%pc, k)
```

The public writes are `{goal_x, yaw}`. The host alone extends that set with the two fresh fields. Within a tick, `approach` sees the incoming `x` and `claw_closed`, and `stride` sees the new `goal_x`. That forwarding is part of the authored schedule. Row values stay in registers across the laws and catch-up ticks where possible. Lower the checked region and `stride` in source schedule order inside the same `advance(from_tick, to_tick, keep_stride)`. The executable is an ordinary state `TableDecl`, a kernel module and a `PassDecl` with `ComputeFusedTicks`, row storage read/write and tick arguments read-only. The state table is also the table named by `StateStoreDecl`. The module has no suspension instruction. [Fixed-tick emit](../../crates/loft-passes/src/motion.rs) · [Bundle construction](../../crates/loft-passes/src/fusion.rs)

For the standalone fixture, the six public words occupy byte offsets 0 through 20. Use offsets 24 and 28 for the private u32 PC and f32 angle, giving a 32-byte state row including both. That is eight bytes of private payload per row and per checkpoint copy. Layout in a larger kind follows its actual fields and padding. Initialize private words on creation and row reuse. Include the owning site, normalized input/rewrite digest, resume ordinals and spill types in the private fields' identity in the state layout hash. This conservatively resets state and checkpoint rings on a changed continuation contract, even when byte offsets agree. Use `hot_swap_bundle` for compiler-produced motion edits so the runner compares that hash. A raw `hot_swap_module` carries only shader text and cannot convey a changed continuation schema. A saved PC must retain its meaning when state survives a swap. [Hot swap](../spec/PATCH_V1_2.md) · [State schema](../../crates/loft-ir/src/schema.rs)

**Predicted tick trace.** Each row shows state *after both laws*. All fractions are exactly representable in f32. A checkpoint after tick 2 is deliberately just before the row changes waits.

| Tick | PC | x (m) | bearing (rad) | closed | saved face (rad) | yaw (rad) |
|---:|---:|---:|---:|:---:|---:|---:|
| 0 | 0 | 0 | 0.5 | false | 0 | 0 |
| 1 | 1 | 0.25 | 0.75 | false | 0.5 | 0 |
| 2 | 1 | 0.5 | 1.0 | false | 0.5 | 0 |
| 3 | 2 | 0.5 | 1.25 | false | 0.5 | 0 |
| 4 | 2 | 0.5 | 1.5 | false | 0.5 | 0 |
| 5 | 2 | 0.5 | 1.75 | true | 0.5 | 0 |
| 6 | 3 | 0.5 | 2.0 | true | 0.5 | 0.5 |

The CPU twin interprets the same validated entry with `step_bundle_cpu`, using the packed state, tick range and binding declarations. `step_pass_cpu` serves synchronous native stepping of live rows. The device-free path also exercises the bundle without a GPU readback. Compare each checkpoint to the table above and compare packed terminal bytes for six single steps, one six-tick catch-up, and a restore at tick 2 followed by four ticks. Also start with both guards ready. The turn must then complete on the first tick. Mutating the lowering to reevaluate `face` at the second guard predicts a final yaw of 1.75 rad. Both backends could agree on that bug. [Runner stepping](../../crates/loft-runner/src/stepping.rs) · [Device-free twin](../../crates/loft-std/src/stepping.rs)

Predicted acceptance on this fixture is zero differing bytes, one fused dispatch per nonempty advance within its tick limit, and byte-identical kernel text at one and twenty rows. Time the hand-written PC law and the guest-produced law on the same quiet device in paired rounds, at identical row counts. Equivalent emitted kernels predict no resolvable p95 GPU difference. Report the timing resolution and actual dispatch count. These are acceptance predictions for a future implementation.

## The other candidates

**Plan lowering.** The source already has a dedicated grammar and a boundary worth copying. `plan_source` checks objectives and holds, and `plan::lower` runs the CPU solve and packs a prior header and Hermite sample rows into `LutTables`. A future stage-zero claim would receive the resolved declaration, constant arguments and reachable data. It would return a proposed static table plus sample function. Independently evaluate the declared holds on those samples under the spec's tolerances before granting a verdict. A hold checked only at collocation points carries only that discrete guarantee. Preserve `Unknown(SolveFailed)` on non-convergence. Hash the solver and arithmetic profile with the inputs. The integer-only guest would need software floating point or a separately specified deterministic numerical profile. This is a good *second* experiment for constant-data output and cost. It barely tests suspension, hygiene or tick scheduling. [Plan contract](../spec/06_ADDENDUM.md) · [Current solver bridge](../../crates/loft-passes/src/plan.rs) · [Source lowering](../../crates/loft-passes/src/plan_source.rs)

**Automatic pass fusion.** Let a later, explicitly selected schedule pass propose an ordered slice of existing pass IDs. A host verifier can construct the fused body itself for adjacent row-local kernels with the same clock, row domain, wave and compatible bindings. Preserve statement order *inside each tick*, field ownership, checkpoints and observable outputs. Reject cross-row dependences, aliasing that prevents the proof, barriers, external observations or event ordering that the transformation cannot preserve. Disjoint writes alone are insufficient when a kernel reads a neighbour's just-written state. A refused proposal leaves the original schedule available with the reason. Compare that schedule and its proved fusion for intermediate observable states, picture, replay and GPU time. Start with required language-level law fusion as the reference. A general schedule optimizer has a bigger proof obligation than a package defining its own block. [Schedule IR](../spec/06_ADDENDUM.md) · [Law fusion](../spec/02_CORE_ABI_AND_LOWERING.md)

## Proposed spec patch

### 1. Surface and resolution (01, 06 §15 and normative `loft.lark`)

Append the final alternative below to the existing `stmt` production in place. Both the parser and the constrained decoder consume this rule. The module, expression, newline and import grammars continue to apply.

```lark
stmt:        "let" LIDENT (":" type_expr)? "=" expr
           | "var" LIDENT (":" type_expr)? "=" expr
           | "map" LIDENT "=" expr
           | build_for_stmt
           | "for" LIDENT "in" expr ".." expr block
           | "yield" expr
           | "return" expr
           | lhs "=" expr
           | expr
           | LIDENT block
```

The parser recognizes a statement headed by a non-keyword identifier followed immediately by a block, even without an installed package. Resolution requires a region export. Missing exports use E011. A marker is an ordinary resolved call whose signature and owning block are exported by the same package. Use outside its owner or an unsupported placement inside the block is a diagnostic at the call. The example's exported signatures are `behaviour : law-statement(block) -> unit` and `until : marker(bool) -> unit`. The block's body retains expression trees until lowering. In particular, the host may not hoist a marker argument across entry or suspension. `loft fmt` can format either form using its CST. A second consumer can export a differently named block and marker with this same grammar. A fixed name such as `behaviour` never enters the lexer or the runner.

### 2. Compiler IR and ABI (PATCH_V1_2 §§3, 6)

Keep the reserved value op `Op::Intrinsic { name, args }` for leaf intrinsics. Add a **compiler-side** statement `RegionIntrinsic { name: SymId, body: BlockId, site: OriginId }` whose body is an ordered lexical region. Call nodes in that region use ordinary resolved callee IDs, including owned markers. These nodes are legal only before typed-region lowering. Extend the internal registry to dispatch by phase and qualified export. Use a read-only projection and a checked replacement for the public ABI. A practical first version can project checked `loft-parse` statements and lower replies back through the existing checker with fresh IDs. Publish neither Rust AST layout nor Naga handles as the wire format.

The following is the v1 guest data model. Lists are ordered. IDs are indices into the host request or into disjoint guest-local namespaces. Every output statement and computed expression carries an origin from the request. The notation elides those repeated origin fields.

```text
Request = (site, export, target_row, stage, allowed_writes, fields, types, bindings, block)
Block   = ordered list of Let(binding, expr) | Set(place, expr) | Mark(export, expr*)
                       | Other(stmt_id)
ExprRef = (expr_id, type_id, free_bindings, reads, effects)
Reply   = Replace(private_fields, statements) | Reject(origin, message, fix)
Field   = (ordinal, type_id, init_literal, origin)
Stmt    = Let(local, expr) | Set(place, expr) | If(expr, then_stmts, else_stmts)
        | Copy(stmt_id, substitution)
Expr    = CopyExpr(expr_id, substitution) | Load(place) | Literal(type_id, bits)
        | Eq(expr, expr)
Place   = Original(binding_or_field_id) | Local(ordinal) | PrivateField(ordinal)
```

`expr` in an input statement is an `ExprRef`. `types` includes the resolved dimensions, spaces and storage types, `bindings` associates IDs with type and mutability, and each input entry has its own origin. `Other` preserves a typed source subtree for copying or a precise refusal. A substitution maps a free binding ID to a typed output expression, checked at the use site. Statement-local binders in a copied subtree are alpha-renamed. `Let(local,...)` introduces a fresh temporary assignable by `Set`. The host admits references to captured input values and owned places only within their original scopes. It assigns offsets after verification, derives effects itself and checks every private read against initialization. No pass may create a new original ID. This small output alphabet suffices for the example because an existing source expression can supply the guard, including its comparison and units. Add output forms only when an independent pass needs them.

The guest exports one bounded, unshared wasm32 `memory`, `alloc(n: i32) -> i32` and `rewrite(ptr: i32, len: i32) -> i64`. The host copies the canonical request into the returned range and copies out one reply. The result packs the unsigned reply address in the low word and byte length in the high word. Validate all ranges with widened arithmetic, then discard the instance. Specify the request/reply as canonical UTF-8 S-expressions with positional fields, the above constructor tags, decimal unsigned IDs, lists in source order, and bit-pattern literals of the resolved type. Diagnostics use quoted UTF-8 strings. Version the codec independently of the executable `loft-ir-v1`. Expose the very same request bytes via `loft ir --sexp --region <site>`, with site IDs listed by `loft show`. Execution continues to use the existing `TableDecl`, module, `StateStoreDecl` and `PassDecl` ABI. The private state identity rule above is the clarification to PATCH_V1_2 §5.

### 3. Packaging and loading (06 §14, 07 §§1, 8)

A scene `.loftb` ships the **result** in MANIFEST, ROWS and MODULES. Loading, replaying and CPU-stepping it require neither the package nor a compiler. An authoring/library `.loftb` may additionally distribute the **rewriter**. Reserve optional section tag `0x06` for the following records, sorted by qualified export. Use the bundle's little-endian, length-prefixed wire conventions and section hashes.

```text
Rewriters = list of (qualified_export, pass_abi:u32, phase, marker_signatures,
                     wasm_digest:[u8;32], wasm_bytes)
phase     = 0   # typed-region lowering, law statement in v1
```

The manifest of this first guest profile has no dynamic imports. Wasm dependencies are statically linked. Source packages are resolved under their qualified paths with locked content digests before compiling a scene. The authoring compiler validates the export, digest and Wasm profile when an explicit import claims a site. Inspecting or running the bundle never invokes the guest. These bytes have no `ModuleId` or GPU binding. `ShaderTarget::Wasm` remains available for *execution* backends. Optional SPANS carries the site-to-origin expansion chain and export digest for inspection. The scene's execution modules retain only lowered operations. Replay pins the physical bundle hash under §07 even if two builds share a shader cache entry.

The current reader accepts exactly the known section sequence and compiler ABI. Write shader-only output in the existing format when it fits. Allocate compiler ABI 2 for files carrying the new section(s), retain the original ABI 1 reader, and allow optional tags 4, 5 and 6 in ascending order after the required 1–3. Derive presence from the directory, honor the spec's debug-spans flag for tag 5 and add no executable-module flag for tag 6. The new reader verifies ranges and hashes, skips tag 6 on the standalone execution path, and rejects unknown ABI/required encodings by name. Older readers correctly refuse the new encoding. Verify both legacy playback and a new bundle on a standalone wasm32 runner whose dependency graph excludes the guest interpreter and `loft-passes`. [Current file codec](../../crates/loft-format/src/bundle.rs) · [Platform replay and standalone loading](../spec/07_PLATFORM_POWER_EXTENSIONS.md#8-deterministic-loftlog-record-and-replay)

### 4. Freeze in this order

**Freeze now** the generic statement grammar, import-local claims, typed pre-fusion input, host-owned binders/effects/origins, strict elimination, and the distinction between a compiler package and an executable module. Reserve the section tag. The first implementation should be an external `behaviour` package run through the public entry point. Its activation must require only an import and a block. Use the trace, negative checker fixtures, hot-swap identity and standalone replay to decide whether the interface works.

**Freeze the distribution ABI after the example runs.** This includes v1 wire tags and golden bytes, instruction accounting and concrete limits, deterministic diagnostic ordering, and the persisted section with a dual-reader fixture. Then try a second independently authored package before promising more statement shapes, nested regions, a numerical Wasm profile, a schedule transformation phase or a rewrite language in Loft. The port's frozen TS engine needs no work.

## Predicted line cost

These are **predicted** hand-maintained added/changed implementation lines against the current root workspace. They count the adapter and validation the empty registry still needs. They exclude vendored Wasm engine source, existing compiler code reused unchanged and generated files. The largest uncertainty is how much of the existing checker can consume a reconstructed region without splitting into two checkers.

| Work | Predicted lines |
|---|---:|
| Grammar, parse, import/export resolution and marker signatures | 140 predicted |
| Typed projection, reply decoding, hygienic reconstruction and rechecking | 420 predicted |
| Guest host, deterministic limits, cache and origin reattachment | 270 predicted |
| Compiler-side state layout, initialization and continuation compatibility | 130 predicted |
| Bundle codec, dual reader, inspection and profile attribution | 180 predicted |
| Public guest helpers and the reference behaviour package | 150 predicted |
| Grammar, negative cases, trace, replay, mutation and standalone fixtures | 230 predicted |
| **First vertical slice total** | **1,520 predicted** |
| Behaviour-specific lines in the runner and executable IR | **0 predicted** |

The plan adapter would add **180 predicted lines** if the solver can be reused with reproducible arithmetic. The schedule proposal and structural fusion verifier would add **300 predicted lines** for the row-local case. Both are outside the first-slice total. If wrapping the current checker requires duplicating its semantic rules, stop at the typed projection and refactor the shared checker before expanding the plugin ABI.

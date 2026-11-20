# Lean1a · Source and checked operations

## Control and prediction

The audit is `1194779163787fb902c0db08c85db4415f7cb717`. The X2
control is `6e30111be103c0a0edbc97ba0993e13d0f2daa7e`, whose native
code and inputs match the scored X2 revision `180c7086ea9c575cb78610cea5e63e8e25df334f`.
X2a, X2b and X2c were held. X2d changed the legacy benchmark.
X2e changed loft-std's wasm32 web module only. All A-owned files have zero
X2 delta.

The audit and control count 2,581 parser lines and 5,087 owned passes
lines with scc 3.7.0. The ceilings require net deletions of 931 and
2,307. The first cut predicts 2,450 parser lines from shared lists,
rows and unit atoms and top-level-only kind scope resolution. Reaching
1,650 would require a further 800-line net reduction in the parser and
CST. The operations and typed source bindings predict 4,900 owned
passes lines. Reaching 2,780 would then require a further 2,120-line
cut, including the checker, evaluator and AD arithmetic or diagnostics.
Both are estimates made before implementation.

Prediction against an unchanged X2 control: zero changed RGBA bytes,
18/18 actual gate hashes, 9/9 bead hashes, the same named outcomes and
Pictures 0/18, Twin 36/36, Speed 12/12, Load 31/33, Truth 34/34.
Expect zero contended samples under the GPU lock. Keep grammar
classifications, AST diagnostics, source replay and rounding order.
The existing Hermit pipeline misses and the Tier 0 promotion timing
failure belong in the control ledger.

## Source traces

- For a kind with `trace` calling a later `radius` helper, and
  `radius` calling `sin`, `Parser::kind` first scans the kind body to
  resolve `radius` to `Kind#radius`. The ordered parse lifts the
  helper at declaration position and emits the scoped symbol at the
  call. `Ck::call` resolves the lifted name in `Prog::fns`, then checks
  `sin` as a builtin inside that helper. `Eval::expr_inner` repeats
  the declaration lookup and passes `sin` into its own builtin
  dispatch for either `f32` or `Iv`.
- `motion_source::solver` checks that a positive `colored_gs(4)` and
  a boolean `shared_mem` are legal. `scheduled` reads both AST nodes
  again and applies the GPU-only limit of 64 after table and field
  checks. `plan_source::check_source` flattens, resolves and binds
  `effort(1.0) + near(keys, pos: 1e-8)` plus the holds. `lower_source`
  decodes both again after closing the parameters and evaluating the
  defaults and horizon. Both paths have diagnostic order to retain.

## Measured change and disposition

Code commit `df8e2910a6474d1a82e7e262b4a82268ed0de75c`. **Hold for X3 ownership,
the size ceilings and timing parity.** Wave X3 waits for `wave2.finished`, which
`wave2.sh` writes after Lean integration. X3 claims escapes in `check.rs`,
`ad.rs` and `eval.rs`, and IX3 is queued to integrate the approved Lipschitz
change in `lip.rs`. See the [ownership request](../../../out/meta/lean1a/ownership.md).

| Scope | Audit | X2 control | Candidate | Ceiling | Over |
| --- | ---: | ---: | ---: | ---: | ---: |
| Parser | 2,581 | 2,581 | 2,544 | 1,650 | 894 |
| Nine passes files | 5,087 | 5,087 | 5,079 | 2,780 | 2,299 |
| Lane | 7,668 | 7,668 | 7,623 | 4,430 | 3,193 |
| Workspace inventory | 31,484 | 31,692 | 31,647 | | |

The X2e web module accounts for all 208 added workspace lines between audit
and control. These are scc 3.7.0 `Code` totals from
`scc --by-file --format csv crates/loft-*/src crates/shared`, including
embedded tests, WGSL, and the existing included/generated inputs. The new
external grammar assertion adds 28 physical test lines. The parser cursor
went 1,761 → 1,695 and CST went 346 → 375. Motion source went 579 → 564
and plan source 438 → 445. The remaining seven passes files still account
for 4,070 lines. [Control inventory](../../../out/meta/lean1a/x2-control-scc.csv) ·
[Candidate inventory](../../../out/meta/lean1a/candidate-scc.csv)

### Producers and consumers

- The CST visitor feeds the parser cursor and source replay. The cursor's
  brace links let `Parser::kind_scope` skip nested bodies while preserving
  forward helpers, forward map headers, and the ordered AST parse.
  Removed the recursive-descent parser's temporary CST token vector,
  the kind-body depth rescan and redundant comma-list and row consumers.
  Kind map expressions still parse in each query's lexical context.
  `Unit::named` now owns atom identity, scale and dimension for lexing,
  literal interpretation and type positions.
- `motion_source::config` produces the existing `SolverConfig` with
  the first scheduled-limit span. It replaces the boolean config
  validator and `scheduled`'s independent syntax switch, including
  `small_count` and its three checked-config panic branches. GPU field
  checks still precede the scheduled limit.
- `plan_source::PlanSource::bind` binds source objectives and holds for
  both `check_source` and `lower_source`, replacing their individual
  binding loops. The legacy adapters still invoke the bindings at both
  stages, so the repeated work remains until the composition path
  carries the bound payloads.

### B handoff

The checked-call API is pending X3 ownership. The proposed entry is
`check::call::CheckedCall { op, operands, span, stage }`, addressed by
`Expr::id` through `CheckedEffects`. Operands carry `Expr::id` and
resolved `Ty`. World calls should carry
`world::WorldCall<Operand>` and its existing `WorldQuery`. Resolve
lifted user names, raw calls, scans and priors at the checked call site.
Preserve syntax for pre-check AD and keep checked derivative rules and
`Eval<S>` arithmetic at their existing boundaries. The remaining call
reclassifiers include `Ck::call` and checker builtin dispatch,
`Eval::expr_inner` and evaluator builtin dispatch, and `ad::d_call`.
B still needs to adopt the table in emit and Tier 0 and forward bound
solver and plan sources through composition.

### Checks and parity

- Static passed on both trees, including Earley and llguidance at 281/281.
  Parser tests passed 6/6 and passes tests passed 68/68, including
  diagnostics, Phase 4, motion and Tier 0. The AST and CST instrument
  matched all 299 inputs, including replayed bytes, serialized ASTs,
  expression IDs and parse diagnostic payloads. Seventeen paired CLI
  probes matched stdout, stderr and exit codes byte-for-byte. They
  cover forward helper shadowing, reciprocal and power units, opted-out
  AD, marked deterministic raw solver and the E504 unmarked control,
  solver validation order and plan diagnostics. Phase 4 front-end tests
  also verify the `AuthorOptOut` proof and the absent surface JVP.
  [Static log](../../../out/meta/lean1a/candidate-static.log) ·
  [Probe outputs](../../../out/meta/lean1a/probes/) ·
  [Replay checksums](../../../out/meta/lean1a/parse-replay.sha256)
- Native workspace tests passed 154/155 compared with 153/154 in X2.
  The inherited `encode::edit_gate::time_edit_and_compare_native_promotion`
  failure was 0.537041 ms p95 against 0.500 ms, compared with 0.520375 ms
  in X2. Both measured 0/140,000 pixels over 1/255. Workspace doctests
  and the wasm32 runner check passed. [Native log](../../../out/meta/lean1a/candidate-native.log)

The locked Apple M4 Metal runs used 1280 × 800, 30 warmups, 300 timed
frames per scene, and the frozen 1/20 instance overrides. The candidate
matched **18/18 actual RGBA hashes and 9/9 bead hashes** against X2.
All six scene shader hashes, command counts, module counts and pipeline
counts matched in both comparisons. Both runs retained all 18 frozen
picture misses and both Hermit pipeline misses (14 against 13).
The Phase 5 archive and frozen pins were kept read-only.

| Run | Pictures | Twin | Speed | Load | Truth | Contended |
| --- | ---: | ---: | ---: | ---: | ---: | ---: |
| Scored X2 control | 0/18 | 36/36 | 12/12 | 31/33 | 34/34 | 0 |
| Candidate, initial | 0/18 | 36/36 | 11/12 | 28/33 | 34/34 | 0 |
| Control, paired | 0/18 | 36/36 | 6/12 | 31/33 | 34/34 | 0 |
| Candidate, paired | 0/18 | 36/36 | 6/12 | 29/33 | 34/34 | 1 |

The initial timing run overlapped builds. Its additional misses were
Blobs-1 p95 and Blobs-20, Hermit-1 and Hermit-20 cold load. The paired
runs excluded lane builds. The main binary then missed all six p95
checks plus its inherited failures. The candidate also missed Blobs-1
and Blobs-20 cold load, and Hermit-20 p95 was marked contended.
The two cold misses include pipeline creation increases of 0.180 s and
0.114 s. The host had active foreground graphics work. These samples
do not establish identical named timing outcomes. The JSON preserves
all p95s, cold and warm timings, overrides by case, frame stamps and
per-check deltas. [Initial score and hashes](../../../out/meta/lean1a/parity.json) ·
[Paired score and hashes](../../../out/meta/lean1a/paired-parity.json) ·
[Bead results](../../../out/meta/lean1a/candidate-bead/score.json)

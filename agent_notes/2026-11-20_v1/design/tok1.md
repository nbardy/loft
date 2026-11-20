# TOK1 · Where Loft spends its authoring tokens

Three ordinary functions earn a place in the library on the measured sources. The largest factors three adjacent keyframe ramps into one call. It saves 51,240 tokens of expressions and bindings across 158 scenes. The largest saving overall is already available to authors. Giving each sweep kind its most frequent paint and section exponent as column defaults saves **985,842** tokens, after paying for the declarations. The second result is about the representation of sampled assets. It does not require language work.

## Population and instrument

The census covers all 10 engine files, six scene files, 16 distinct library files and **all 187 non-quarantined corpus ports** at `66011c1`. This is the denominator for “corpus median”. It includes 3 l5c scenes, 14 l5d scenes, 75 l5e seeds and 95 seeds under `corpus/seeds/`. The six entries with paired quarantine diagnostics and the unported identities in the 295-entry manifest have no weight. The 187 files are authoring workloads, each counted once, independently of how many images have been certified. The two library symlinks to Act and Roam are charged to engines. Reading those aliases a second time would add 1,034 tokens. Both files under `lib/migration/` are in the library inventory.

From the workspace root run this read-only [instrument](tok1.py) against the measured sources. To reproduce later, run the copy of the instrument accompanying this report with the historical checkout as the working directory.

```sh
uv run --with tiktoken==0.14.0 python agent_notes/2026-11-20_v1/design/tok1.py
```

The unit is `o200k_base.encode_ordinary` of the complete checked-in text, including comments, punctuation and final newline. Each token goes to the category at its first non-whitespace byte, or its first byte if entirely whitespace. The classifier follows the enclosing declaration and kind member. Space tags, matched type annotations and unit suffixes take precedence over their host line. Generated build fields have separate categories. A material's eye pigment stays under material, a leg solver under motion, and a mirror map under symmetry. Geometry is tagged by a part name on the line, then on its enclosing declaration. These are reproducible estimates of semantic ownership. The token and file totals are exact.

| Construct | Engines | Scenes | Library | Corpus |
|---|---:|---:|---:|---:|
| Setup, cameras and schedules | 894 | 462 | 0 | 10,956 |
| Lights, sky and transport | 2,065 | 108 | 1,095 | 14,954 |
| Material and palette, including row paint | 1,556 | 1,285 | 1,776 | 1,264,850 |
| Build and row construction | 1,333 | 601 | 111 | 382,924 |
| Kind and row schema | 170 | 331 | 49 | 268,334 |
| Leg geometry | 148 | 428 | 0 | 161 |
| Eye geometry | 140 | 211 | 0 | 298 |
| Shell geometry | 389 | 61 | 0 | 523 |
| Other limb geometry | 316 | 288 | 0 | 100 |
| Symmetry and mirror maps | 248 | 104 | 0 | 64 |
| Motion laws, solver calls, checks and interpolation | 723 | 2,400 | 1,347 | 210,840 |
| Sampled ring centers and axes | 0 | 0 | 0 | 7,825,650 |
| Sampled keyframe deltas | 0 | 0 | 0 | 8,213,888 |
| Sampled section exponents | 0 | 0 | 0 | 352,300 |
| Units and type/space annotations | 672 | 492 | 876 | 83,549 |
| Other geometry and code | 1,800 | 2,134 | 2,348 | 175,221 |
| Comments and blank lines | 526 | 134 | 1,236 | 57,172 |
| **Total** | **10,980** | **9,039** | **8,838** | **18,861,784** |

Capsules spends 332 of 621 tokens on building rows and 127 on setup and schedules. Hermit spends 951 of 3,021 on materials, 562 on row construction and 807 on named part geometry. In the generated seeds, the source span from the first sweep emit to the final build brace accounts for 18,023,476 of 18,835,594 tokens (95.69%). That confirms the prediction that literal rows would occupy over 90% of those files.

The part comments preceding the generated rows let us attribute their six ring vectors and the corresponding keyframe deltas a second way. This cross-tab partitions the two data rows above. It adds zero tokens to the census. “Other” includes environment geometry and names that do not identify a part.

| Label on the sweep | Base ring fields | Keyframe delta fields |
|---|---:|---:|
| Legs and feet | 1,912,800 | 815,938 |
| Eyes, stalks, pupils and brows | 1,050,401 | 1,042,528 |
| Shells and carapaces | 655,769 | 1,116,470 |
| Claws, arms, antennae and other limbs | 2,332,610 | 3,669,766 |
| Other | 1,874,070 | 1,569,186 |
| **Total** | **7,825,650** | **8,213,888** |

## Recurring code worth extracting

Rank is the sum of original block tokens minus fully spelled replacements at qualifying sites. Search units were expressions, consecutive bindings, emit pairs, query bodies and schedule tails. Compare the same operations after identifying local binders, substitutable fields, literal arguments and spaces. Preserve query kind, grids, operation order and state writes. Repetitions in generated input count once per authored block, regardless of row count or how often the shader runs. The instrument contains the exact source patterns and replacements. Its whole-file forecasts also charge tokenizer joins at edit boundaries.

| Rank and function | Qualifying sites / files | Original tokens per site | Call tokens | Complete replacement tokens | Original / replacement | Gross tokens saved | One library definition¹ |
|---|---:|---:|---:|---:|---:|---:|---:|
| 1. `ramp_weights` | 840 / 158 | 81 early / 89 late | 25 / 19 | 27 / 21 | 3.00× / 4.24× | 51,240 | 129 |
| 2. `daylight` | 170 / 170 | 85 | 20 orbit / 19 fixed | 20 / 19 | 4.25× / 4.47× | 11,145 | 167 |
| 3. `yaw_about` | 14 / 14 | 67 | 13 | 19 | 3.53× | 672 | 81 |

¹ Counted once with the proposed body, signature, constants and final newline. Consumer totals exclude prelude text. This fee matters for repository size and for an author who pastes a private copy. There is no recurring import charge in the measured flat prelude.

**Keyframe weights.** Each of 420 animated kind bodies evaluates six scalar windows at the same time. Bind one triple for the first three and another for the tail. Read their xyz components in the original six-delta sums, preserving left-to-right addition and every numeric width. Replacing the 36 `t1`…`t6` references per body with component reads costs the same number of tokens in context. The tail literals live in two ordinary constants, charged above. Widths are seconds and positive at each site. Recomputing a width by subtracting adjacent rounded sample times would change the arithmetic.

**Daylight.** A single stage-0 call normalizes the authored direction in World and emits Sun then Sky. All 170 seeds share the radiance, zenith and horizon. The 75 orbit ports share one direction and the 95 fixed-eye ports share another. The call is charged with all four names. Much of this saving comes from sharing the five literal values. Keeping the two emits and naming the values alone costs 36 (orbit) or 35 (fixed) tokens per block. The emitter also works with a scene's own four literals. Spectral Study is a separately costed adoption (79 → 62, 18 saved in the whole file). It supplies none of the qualifying recurrence. Shrine has a similar pair and is excluded from the rollout after its image control varied.

**Yaw about a pivot.** Fourteen l5d ports translate a point to the view pivot, compute both trigonometric terms, rotate in XZ and translate back. Keep each port's typed local `placement`, constants and final `place(..., VIEW_YAW)`. The table charges that outer constructor and the repeated yaw argument. Port-specific Local spaces and row order stay explicit.

### Typed E+ and forkability

The following bodies are the proposed implementation. Each compiled in a scratch copy of the flat prelude. Copying the functions and constants into the consuming scene under fresh function names also compiled against the unmodified prelude. Both paths use the ordinary scene loader and compiler. The two stage-0 functions execute when constructing rows. The pure interpolation helper runs in the authored surface.

```loft
# lib/motion.loft
const SEED_TAIL_STARTS = [0.75, 0.9375, 1.37]
const SEED_TAIL_WIDTHS = [0.1875, 0.432500005, 1.52999997]
fn ramp_weights(t: s, starts: v3, widths: v3) -> v3 =
  [clamp((t - starts.x) / widths.x, 0, 1),
   clamp((t - starts.y) / widths.y, 0, 1),
   clamp((t - starts.z) / widths.z, 0, 1)]

# lib/light.loft
const tidal_orbit_dir = [-0.609761775, 0.76538378, 0.205859736]
const tidal_fixed_dir = [-0.430528373, 0.76538378, 0.478364885]
const tidal_key = [2.8, 2.6, 2.3]
const tidal_zenith = [0.35, 0.56, 0.88]
const tidal_horizon = [0.82, 0.86, 0.9]
build fn daylight(sun: v3, key: rgb, zenith: rgb, horizon: rgb) {
  emit Sun(dir: dir([sun]@World), color: key)
  emit Sky(zenith: zenith, horizon: horizon)
}

# lib/layout.loft
build fn yaw_about(p: p3@World, pivot: p3@World, yaw: rad) -> p3@World {
  let d = p - pivot
  let c = cos(yaw)
  let s = sin(yaw)
  pivot + [c * d.x + s * d.z, d.y, -s * d.x + c * d.z]
}
```

### What the probes ruled out

- The seed sweep surface looked like the largest opportunity. There are 701 static and 420 animated copies. After excluding comments and retaining the animated bindings, 127/27 and 133/33 suggested 112,100 tokens saved. The unchanged compiler rejected the extraction with E405. A row's exponent is constant for UV differentiation. A helper parameter is live in the generated Jacobian-vector product and the differentiator refuses a varying exponent in `pow`. The original port compiled. The forecast keeps the exponent and its power operation in the surface.
- Boss, Crab and Heron all solve a two-bone knee and union two capsule distances. Including the assignments, actual lengths, radii and pole made representative rewrites 65 → 40 and 78 → 54. The checks, anchors and body rows differ across the three laws. Hermit's shell, face and claw materials similarly have different lobe weights and masks. Existing IK, capsule-chain, frame-transport and BSDF functions already capture much of the shared work.
- A floor field at 29 → 7 and an even-index sign at 19 → 4 pass the numerical cutoff. Both would introduce a name for a single existing operation with a few literals or a parity test. The simpler authoring opportunity here is the origin splat. Capsules and Blobs share a long schedule tail, but ordinary E+ functions cannot emit pass declarations. Their schedules also omit the default Realtime shadow pass. Substituting the default would change the workload.
- Literal rings recurring under a part name are sampled assets. Changes to either end or its deltas change a specific mesh. Treating those decimal payloads as a forkable leg, eye or shell *algorithm* would give misleading library ratios.

## Surface savings outside std

A lower-cost design for the two seed views alone would hard-code their palette in a single `tidal_daylight(direction)` recipe. Keeping four inputs on `daylight` lets an authored scene supply its own palette and keeps all colors visible at the call. The named constants in the seed calls pay for themselves inside the measured corpus.

Each estimate re-encodes complete files with just that edit. The various rows overlap. The last forecast column below applies only the first three edits together, after std extraction.

| Change | Engines | Scenes | Library | Corpus | Core cost and decision |
|---|---:|---:|---:|---:|---|
| Modal paint and exponent column defaults | 0 | 0 | 0 | 985,842 | Existing syntax. Pick the most frequent exact literal per kind per file, declare it once, omit exact matches, retain every override. No change to the row ABI. |
| Omit explicit matching camera defaults | 70 | 17 | 0 | 0 | Existing `orbit` and camera paths default period to 24 s and height to 0.5 m. Keep every varying field and every nondefault period. |
| Write a tagged origin as `[0]@S` | 54 | 18 | 0 | 216 | Existing scalar splat. Restrict the estimate to the exact integer triple with an explicit space. |
| Infer positional fields on complete static sweep emits | 0 | 0 | 0 | 872,872 | Would grow the grammar, row binder and diagnostics. Estimate removes the eight field labels on complete ordered rows only. Decline: changes to field order could silently retarget equal-typed axes. Existing named defaults save more. |
| Infer base units on complete literal radius, height and period camera fields | 3 | 9 | 0 | 269 | Current numeric lowering accepts bare base units. A sound context-sensitive dimension checker would grow the core. Defer and retain degrees, millimetres, nonliteral arithmetic and all conversions. |

## Forecast and image gate

| Workload | Measured | Std only | Lane rollout | Std + three existing surface forms throughout |
|---|---:|---:|---:|---:|
| Capsules | 621 | 621 | **613** | 613 |
| Hermit | 3,021 | 3,021 | **3,009** | 3,009 |
| Corpus median | 96,204 | 95,894 | **95,894** | **89,892** |
| Corpus total | 18,861,784 | 18,798,557 | 18,798,557 | 17,812,499 |

The 17 l5c/l5d ports and the 170 seeds have very different sizes. The l5c, l5d, l5e and remaining seed medians start at 808, 847, 107,663 and 95,566 respectively. Their std forecasts are 808, 799, 107,353 and 95,011. Across all consumer files, the lane forecast saves 63,265 whole-file tokens (63,227 corpus, 18 Spectral Study, 20 engine surface). The three library bodies cost 377 tokens plus separators.

The median is rank 94 after separately re-sorting all 187 whole-file counts in each scenario. Applying a mean percentage to the old median gives a different answer. The lanes adopt the two small camera edits on Capsules and Hermit. Corpus modal defaults and wholesale surface cleanup are estimates only. The data-heavy median explains why large aggregate savings coexist with a small change to the typical file.

The unchanged CLI compiled all 184 rewritten corpus ports (14 l5d, 75 l5e and 95 seeds) through `loft show --plan`. Ten paired 1280×800 stills matched exactly in decoded RGBA on the local Metal adapter, covering Capsules and Hermit at 4 s, Pool at 0.5 s, Spectral Study at 0 s, and `crab-actions-v1__v1__train-16__2` at 0, 0.125, 0.84, 1.1, 2.1 and 2.9 s. This exercises between-key interpolation. A separate Shrine trial differed at 41 pixels at 0 s. the unchanged baseline also produced three different hashes in three replays. Three pre/post replays with helpers appended to existing library files also varied. The cause remains open. Shrine earns zero forecast savings. These probes cover selected consumers. each lane must still score its own files. [Local paired hashes](../../../out/meta/tok1/frames.tsv)

The disjoint Sol briefs divide ownership by consumer. The seed lane owns the shared interpolation and light rig, all 170 generated ports and the authored spectral scene. The layout lane owns the manual view placements and the two engine camera edits. The wave coordinator can carry their measurements into the later stdlib tree design. The paths and copy-paste tests here describe the requested flat `lib/` revision. [Lane IDs](../../2026-11-20_codex/tok1.lanes)

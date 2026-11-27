# Rollout Study & Token Bloat Audit — 27 Nov 2026

Comparative study of **cheap/mini/Spark/corpus rollouts** (`out/spark_pairs/`, `corpus/l5e/`, `corpus/seeds/`, `syntax_bakeoff/`) vs. **Opus character & fur rollouts** (`loft/boss-{otter,walrus,heron,whiskers}`, `loft/integrate-1125:tests/characters/`, `auncel/d`, `loft/fur-strands`, `out/meta/boss/*/trail.jsonl`), accounting for version drift on `loft/integrate-1125`.

---

## 1. Point (`p3@S`) vs. Vector (`v3@S`) — Why the Distinction Exists & Why Loft Leaks It

In 3D affine geometry (homogeneous coordinates):
- **Point (`p3@Space`, $w = 1$):** a *location*. A rigid/affine transform $T = (R, t)$ applies both rotation/scale AND translation: $T \cdot p = R p + t$. Adding two points (`p1 + p2`) is meaningless (`E101`); subtracting two points (`p1 - p2`) gives a displacement vector (`v3@Space`); adding a vector to a point (`p + v`) gives a new point (`p3@Space`).
- **Vector (`v3@Space`, $w = 0$):** a *displacement, velocity, acceleration, or offset*. A transform applies rotation/scale ONLY (translation is ignored): $T \cdot v = R v$. Vectors can be added, scaled, dotted, and crossed freely.
- **Direction (`d3@Space`):** unit-length vector (`ray` direction, pole axis).
- **Normal (`n3@Space`):** surface covector (transforms by $(R^{-1})^T$ under non-uniform scale).
- **Untyped `v3` / `rgb`:** 3 spaceless scalars (colors, box half-extents, raw coordinates).

**Why `[x, y, z]@World - [0.0, 0.0, 0.0]@World` happened:** Loft's syntax only gives `[x, y, z]@Space` the type `p3@Space` (point) and provides **no literal or cast syntax for `v3@Space`** (and no clean `p3@Space(v)` conversion from a `v3`/`rgb`). So authors have to subtract the origin point (`p3 - p3 -> v3`) and unpack components (`[a.x, a.y, a.z]@World`, **83×** across the 6 Opus branches) just to satisfy the typechecker.

---

## 2. Ranked Sources of Obvious Token Bloat (Measured Across Rollouts)

### 1. Verbatim Scene Wrapper Boilerplate — **27–56% of every cheap/mini rollout**
- Every `out/spark_pairs/{models,chains}/*/*.loft` file repeats the same 5 `use` lines, 26-line `Capsule`/`GlossBead` kinds, and 8-line `schedule Main` (**31.7%** of all lines in `models/`, **26.6%** in `chains/`, **56%** in `ash-monk/model.loft`).
- Even Opus round scenes repeat the full 7–9 line `schedule Main` because parameterized schedules imported from a module hit `E505 duplicate cull` (`boss-walrus/LOG.md:36`).
- **Fix (additive):** Ship stock `Capsule`/`GlossBead` in `stdlib::shapes` + fix parameterized imported `schedule` functions + default orbit/realtime recipe. Cuts ~35 lines (~200 tokens) per scene.

### 2. Missing `match` / Const Lookup Tables → Nested `select(...)` Chains & Python Codegen (HUGE in Opus)
- **290 `select(...)` calls** and **52 nested `select(select(...))` chains** across the 6 Opus branches (e.g., 7-deep nested `select` for `alb` in `boss-walrus/walrus.loft:176-177`).
- WGSL's reversed `select(false_val, true_val, cond)` argument order caused an actual bug in `boss-whiskers` r07 (`b_wet`).
- Opus wrote **`build for i in 0..select(0u, 1u, cond)`** 18× in `boss-otter/otter.loft:410-423` because `build` blocks lacked simple `if` gating over emits.
- Missing const float tables forced `boss-otter/mkface.py` to generate 90 lines of piecewise-linear `clamp` functions (`face.loft`).
- **Fix (additive):** `match` expression/statement (lowering cleanly to WGSL `switch` or ternary chain), `if` over `emit` in `build`, and `const` arrays (`const PALETTE = [...]`). Cuts 60–80% on material/preset dispatch.

### 3. `rgb`-as-`v3` + `[a.x, a.y, a.z]@World` + `[...]@World - [0]@World` (HUGE in Opus)
- **357 `rgb` annotations** used for positions/vectors/extents across the 6 Opus branches, **83** `[a.x, a.y, a.z]@World` unpacks, **102** `[x.x, x.y, x.z]` unpacks, plus bespoke wrappers (`at_v` 97× in otter, `wld` 122× in walrus, `cap` 31× in heron).
- **Fix (additive):** `v3(x, y, z)@Space` / `v3(expr)@Space` and `p3(expr)@Space` conversions, plus migrating stdlib geometry/SDF signatures to accept `v3` (keeping `rgb` compatible as alias for 3-float vector so zero rollouts break).

### 4. Missing Struct Returns / Tuples / Fixed Arrays → `v4` & `frame` Packing Hacks
- `stdlib/motion/mood.loft:5` abuses `frame@World` as a 12-float struct; `boss-otter/bodyfur.loft` packs 2 vectors into `frame@World` with 2 dummy zero vectors; `boss-whiskers` packs `(root.xyz, len)` into `v4` bags; `boss-otter/forms.loft` repeats an 11-float face bundle across 28 signatures; `corpus/l5e` repeats 44-column `Sweep14Motion` kinds across 170 files.
- **Fix (additive):** User `struct` return values + small fixed arrays `[f; N]` on locals/rows.

### 5. `c + [...]` & Unmirrored Bilateral `emit` Duplication (Cheap/Mini + Opus `build`)
- `c + [...]` appears **492 times** across 20 `spark_pairs` files (just to avoid writing `@World` twice per `emit Capsule`), and 40–50% of `emit` lines manually duplicate `-x` / `+x` limbs, eyes, and props.
- **Fix (additive):** Optional bilateral `emit_pair` / `mirror_x` helper in `stdlib` and ergonomic positional/short `emit` or scoped space origin.

### 6. No Lobe Composition or Normal-Perturbation Combinator (Opus Materials)
- `boss-otter/mat.loft:24-82` and `loft/fur-strands/otter.loft:31-67` copy-paste ~55 lines of stdlib GGX/sheen/coat math solely because a custom lobe cannot call another lobe or apply a perturbed normal.
- **Fix (additive):** `with_normal(n2, bsdf)` combinator / allowing lobes to call lobes.

### 7. No Parameterized Light Callbacks or Const-Generic Kinds (Opus Multi-Variant)
- `boss-otter/mklights.py` generates 7 copy-pasted 30-line `li_a..li_g` functions differing only in 4 RGB constants because `shade_tiles(light: ...)` only accepts a bare function name.
- `boss-otter/mkforms.py` generates 581 lines of `Head0..Head6` kinds because per-variant constants must stay literal for Lipschitz bound derivation.

### 8. Missing Basic SDFs & Defensive `0.0 - x` Tic
- `el()` (ellipsoid SDF) re-implemented 4× and called **212×**; `smax()` re-implemented 3× → add `ellipsoid` and `smax` to `stdlib::sdf::solid`.
- `0.0 - x` appears **189×** across Opus branches (defensive habit from earlier unary-minus edge cases — verify unary `-` works everywhere on scalars, vectors, and interval/AD paths).

---

## 3. Cheap/Mini Rollouts vs. Opus Rollouts

- **Cheap/mini (`spark_pairs`, Haiku bakeoff):** Monolithic single files (68–103 LOC), cargo-culting `Capsule` + `GlossBead` + `schedule Main` (27–56% boilerplate), flat `ggx` materials, zero SDF blending/maps/custom lighting. In the syntax bakeoff, Haiku struggled with staging/ownership rules (`time` in `build`, duplicate law writes, double-emitting mirrored kinds); in `spark_pairs`, it played it safe with capsules and offloaded photometric work to GLSL/numpy (`LOFT_VS_GLSL.md`).
- **Opus (`boss-*`, `auncel/d`, `fur-strands`):** Multi-file modular libraries (500–2,000 LOC) with custom SDFs, fur/whisker fields, custom BSDF lobes, debug AOV switches, and 2-bone IK/springs. **570/573 logged renders compiled cleanly!** Opus gets blocked not by syntax, but by **compiler/engine ceilings** (Lipschitz `L` blow-up causing march holes, `E212` time-dependent traces, 193k-step interval searches on law-written fields, missing `match`/arrays/structs forcing Python codegen).

---

## 4. Version Drift Checklist (`main` vs. `loft/integrate-1125`)

- **Already FIXED on `loft/integrate-1125` (merge ASAP):**
  1. Named & default `build fn` arguments (`heron` 13 → 2 args, `odd` 6 → 2 args).
  2. Exported `pub world` kind groups (~45% schedule token cut in multi-kind scenes).
  3. Empty-kind elision (`without_desk` needs zero schedule edits).
  4. `select(bsdf_a, bsdf_b, cond)` over `Bsdf`.
  5. `u`/`i` int suffixes + comment-after-brace grammar parity.
  6. Tight certified field bounds & retained visibility (`otter q3_r08` 62.7 ms → 12.4 ms p50).
- **Still unfixed on BOTH `main` and `loft/integrate-1125`:** items #1–#8 in §2 above (best queued as Wave 2 after `loft/integrate-1125` + Wave 1 merge).

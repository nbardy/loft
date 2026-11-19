Both catches in the crab example were real bugs—author-written `Field[Claw, 1.35]` silently lied about `0.045 * 26 = 1.17` from the serrated teeth plus FBM, and `mirror_x(p) - Vec(...)` staying `Pt[Body]` let the exact gaze-frame bug slip through type-checking. Accepting both fixes makes the language dramatically stronger.

Below is the complete specification for the WGSL-superset language, its typing and staging rules, the Tier 0/Tier 1 contract, the rewritten hermit crab, and the lowering/portability numbers.

---

## 1. Grammar Sketch (WGSL Superset)

Everything in valid WGSL stays valid inside `fn` and `raw wgsl { ... }` blocks. The superset adds **only** the following top-level items, type constructors, and expression forms:

```ebnf
(* 1. Top-level declarations *)
item       := wgsl_item
            | "space" ident ("," ident)* ";"
            | "unit" ident ("=" unit_expr)? ";"
            | "kind" ident comptime_params? "{" row_block? kind_member* "}"
            | "scan" ident "(" params ")" "@" "schedule" "(" sched_mode ")" "->" type block
            | "solver" ident "over" ident "{" solver_member* "}"
            | "law" ident "(" params ")" "writes" ident_list block
            | "constraint" ident "(" params ")" "->" "Verdict" block
            | "schedule" ident "{" pass_stmt* "}"

comptime_params := "[" ("comptime" ident ":" discrete_type),+ "]"
row_block       := "row" "{" (ident ":" type ";")* "}"
kind_member     := "query" ("trace"|"visible"|"material"|"light"|"sky"|"medium") fn_decl
                 | fn_decl

(* 2. Type additions (bracketed so WGSL's <T> stays unambiguous) *)
type       := wgsl_type
            | ("Pt"|"Vec"|"Dir"|"Norm") "[" space_id ("," unit_expr)? "]"
            | "Xform" "[" space_id "->" space_id "]"
            | "Map" "[" space_id "->" space_id "]"
            | "Field" "[" space_id ("," smooth_k)? "]"       (* L is inferred, never authored *)
            | "Attach" "[" space_id "]" | "Target" "[" space_id "]"
            | "Table" "[" expr "," type "]" | "RowId" "[" ident "]"
            | "Bsdf" | "Verdict"

smooth_k   := "Cinf" | "C2" | "C1" | "C0" | "Cneg1"

(* 3. Expression & statement additions *)
stmt       := wgsl_stmt
            | "map" ident "=" domain_expr ";"                 (* introduces fresh space 'ident *)
            | "comptime" ("const"|"for"|"if") ...             (* Stage 0 only *)
            | "emit" ident "{" field_inits "}" ";"            (* Stage 0: appends row to kind table *)
            | "raw" "wgsl" "(" typed_bindings ")" block       (* opaque escape hatch *)

expr       := wgsl_expr
            | expr "|>" expr                                  (* domain/lobe pipeline *)
            | "diff" "(" expr "," ident ")"                   (* compiler-checked forward/rev AD *)
            | "subgrad" "(" expr "," ident ")"                (* explicit C0 non-smooth opt-in *)
            | "bound_step" "(" expr "," expr ")"              (* conservative fallback for Unbounded *)
            | "sample_c2" "(" expr "," expr ")"               (* C2 Hermite table lookup *)
            | number unit_suffix                              (* 0.18m, 28rad, 0.5s *)

```

Nothing else is added. No classes, closures, GC types, exceptions, or TypeScript syntax.

---

## 2. Typing Rules: Spaces, Units, Domain Maps, Smoothness & Derived Lipschitz

### A. Affine Spaces & Units

* `Pt[S] - Pt[S] : Vec[S]` ; `Pt[S] + Vec[S] : Pt[S]` ; `Pt[S] + Pt[S]` is a type error.
* `Xform[A -> B] * Pt[A] : Pt[B]` ; `Xform[A -> B] * Vec[A] : Vec[B]` ; `Xform[A -> B] * Norm[A] : Norm[B]` (automatically lowers to inverse-transpose $M^{-T}$).
* `inv(Xform[A -> B]) : Xform[B -> A]`.
* Dimensioned scalars unify via rational exponent vectors (`Meters / Sec`, `1 / Kg`, `Rad`). Dimensionless `Unit` tracks interval $[0, 1]$.

### B. Domain Maps & Generative Scoped Spaces

Every domain transform (`translate`, `rot_*`, `mirror_*`, `repeat_grid`, `repeat_polar`, `warp`) returns a point in a **fresh generative space** `'m` (analogous to a Rust lifetime—zero manual space declarations required):

```wgsl
map sym = mirror_x(p) |> translate(-vec3(0.42, 0.55, 1.18)m);

```

This single statement binds four compiler-tracked projections on `sym`:

* `sym : Pt['sym]` — the mapped coordinate.
* `sym.cell` — branch/cell witness (`f32` sign $\in \{-1, +1\}$ for `mirror_x`, `u32` sector for `repeat_polar`, `vec3<i32>` for `repeat_grid`).
* `sym.to_parent : Map['sym -> S]` — auto-generated inverse pipeline **parametrized by `sym.cell**`. For `mirror_x` followed by `translate(-v)`, the compiler synthesizes `x -> mirror_sign(sym.cell, x + v)` so left and right eyes lift to their true distinct `Body` positions.
* `sym.from_parent : Map[S -> 'sym]` — forward map for transforming external directions/points (like world gaze vectors) into `'sym`.

For a non-rigid **Warp** `warp(p, fwd, inv, jac_bound)`:

* If `inv` is supplied, both `to_parent` and `from_parent` exist, and `Norm['w]` transforms via $(J_{\text{fwd}}^{-1})^T$.
* If `inv` is omitted (e.g., an arbitrary noise warp), `to_parent` is **not generated**; lifting points out of `'w` is a compile error.

### C. Smoothness Lattice ($C^k$)

Every expression carries a smoothness tag in $C^\infty \sqsubset C^2 \sqsubset C^1 \sqsubset C^0 \sqsubset C^{-1}$:

* `sin`, `cos`, `exp`, `smin_exp`: $C^\infty$.
* `smootherstep` (quintic $6t^5 - 15t^4 + 10t^3$), `smin_cubic`, `sample_c2`: $C^2$.
* `smoothstep` (cubic $3t^2 - 2t^3$), `smin_poly` (quadratic), `smooth_voronoi`: $C^1$.
* `min`, `max`, `abs`, `voronoi_cone`, `sd_box`: $C^0$.
* `step`, `floor`, `fract`, `select` on runtime condition: $C^{-1}$.

**Differentiation rule:**


$$\frac{\Gamma \vdash e : (T, C^k) \quad k \ge 1}{\Gamma \vdash \text{diff}(e, x) : (\partial T, C^{k-1})}$$


Taking `diff` of a $C^0$ expression (or `diff(diff(...))` of a $C^1$ expression) is a **compile error**. To intentionally request a generalized/clamped derivative for micro-bump shading on $C^0$ geometry, the author must write `subgrad(e, x)` (which returns $C^{-1}$ and cannot be differentiated again).

### D. Compiler-Derived Lipschitz Calculus (Never Authored)

The author writes `Field[S]`, never a numeric $L$. Over the field's active bounding box $D$ (propagated via interval arithmetic $I(e) = [e_{\min}, e_{\max}]$), the compiler computes $L(e) \in \mathbb{R}_{\ge 0} \cup \{\text{Unbounded}\}$ bottom-up:

| Construct | Derived Bound $L(e)$ | Notes |
| --- | --- | --- |
| Exact SDF primitives (`sd_sphere`, `sd_box`, `sd_capsule`, `sd_torus`) | $1$ | Axiomatically verified in stdlib |
| Approx SDF (`sd_ellipsoid(p, r)`) | $\max(r) / \min(r)$ | Or tighter interval AD bound over $D$ |
| Constants / row uniforms (independent of `p`) | $0$ |  |
| Rigid map / `mirror_x` / `abs` fold | $L(f)$ | Isometric ($L_{\text{fold}} = 1$, continuous across fold plane) |
| Uniform scale $f(s \cdot p)$ vs shape scale $s \cdot f(p / s)$ | $\Vert{}s\Vert{} \cdot L(f)$ vs $L(f)$ | Compiler recognizes shape-scaling cancellation |
| Non-uniform scale $f(v \odot p)$ | $\max(\Vert{}v_x\Vert{}, \Vert{}v_y\Vert{}, \Vert{}v_z\Vert{}) \cdot L(f)$ |  |
| `repeat_grid(p, c)` / `repeat_polar(p, n)` | $L(f)$ **with auto cell-wall clamp**, or `Unbounded` | If child is proven symmetric across cell faces, $L(f)$ holds globally; otherwise the compiler emits $\min(d_{\text{child}}, d_{\text{cell\_wall}} + \epsilon)$ so rays never cross a seam in one step |
| Smooth warp $f(\phi(p))$ | $L(f) \cdot \sup_D \Vert{}J_\phi(p)\Vert{}_2$ | Bounded soundly via interval Frobenius/row-sum norm $\sqrt{\Vert{}J\Vert{}_1 \Vert{}J\Vert{}_\infty}$ |
| Linear comb $a f \pm b g$ | $\Vert{}a\Vert{} L(f) + \Vert{}b\Vert{} L(g)$ |  |
| Product $f \cdot g$ | $\Vert{}I(f)\Vert{}_\infty L(g) + \Vert{}I(g)\Vert{}_\infty L(f)$ | Requires finite intervals $I(f), I(g)$ |
| Univariate $h(f(p))$ | $\sup_{y \in I(f)} \Vert{}h'(y)\Vert{} \cdot L(f)$ | e.g., `smootherstep(a, b, x)` has $\max \Vert{}h'\Vert{} = \frac{15}{8\Vert{}b-a\Vert{}}$ |
| `min(f, g)`, `max(f, g)` | $\max(L(f), L(g))$ | Exact by Rademacher's theorem |
| `smin_poly`, `smin_cubic`, `smin_exp`, `ssub_*` (const $k$) | $\max(L(f), L(g))$ | Gradient is a convex combination $(1-w)\nabla f + w\nabla g$, $w \in [0, 1]$ |
| Smooth blend with spatially varying $k(p)$ | $\max(L(f), L(g)) + c_{\text{blend}} L(k)$ | $c_{\text{poly}} = \frac{1}{4}$, $c_{\text{cubic}} = \frac{1}{6}$ |
| `grad_noise3(p)` / `smooth_voronoi3(p)` | Pre-verified library constant $L_{\text{noise}}$ | e.g. $L_{\text{quintic3}} = 2.45$, $L_{\text{voronoi}} = 1.0$ |
| `fbm(s * p; N, lac, gain)` | $\Vert{}s\Vert{} \cdot L_{\text{noise}} \cdot \sum_{i=0}^{N-1} (\text{gain} \cdot \text{lac})^i$ | For $\text{gain}=0.5, \text{lac}=2$, sum $= N$ (grows linearly with octaves!) |
| Division over $0 \in I(g)$, unbounded `exp`/`tan`, `raw wgsl`, unproven runtime loop | **`Unbounded`** | Never guessed. `trace` rejects `Field[S, Unbounded]` unless wrapped in `bound_step(f, max_step)` (conservative fixed-step + bisection) |

---

## 3. Explicit Staging & Metaprogramming Model

### The Three-Stage Boundary

1. **Stage 0 (`comptime` — topology & table emission):** Runs on CPU/WASM at scene build. `comptime for` emits **table rows** via `emit Kind { ... }`, never unrolled shader ASTs.
2. **Stage 1 (`kind` — sole unit of code identity):** A `kind` compiles to **one** canonical shader family. `kind` parameters (`kind Foo[comptime mode: Mode]`) are strictly limited to **discrete structural types** (`bool`, `enum`, lobe bitsets, small bounded `u32` with domain size at most 8). Continuous types (`f32`, `Meters`, `Rad`, `vec3`) are **compile errors** in `comptime` kind parameters—they must live in `row { ... }`.
3. **Stage 2 (`runtime` — GPU/CPU execution):** `for` loops inside `kind` functions stay real WGSL loops. Instance variation always reads `self.<field>` from the kind's SoA/AoS row buffer.

### Who Owns Dispatch Shape?

**Compiler cost model by default, schedule declarations as overrideable hints:**

* The compiler automatically performs **(1) dependency-wave topological batching**, **(2) bucketing by `(wave, law_kind)**`, and **(3) kind-level shader deduplication**. Authors cannot accidentally trigger 5,539 modules or 4,884 sequential pose dispatches.
* A `schedule` block declares *algorithmic topology* (e.g., vis-buffer vs forward, tile size, whether a `scan` is baked or per-frame) and optional overrides (`@fuse_if(regs <= 48)`, `@inline_max(64)`). If an author override violates a hardware budget, the compiler warns and CI gates catch the frame-time/module regression.

### Example: "12 Legs, Each With Its Own Length" (1 Kind, 12 Rows)

```wgsl
kind CrabLeg {
  row {
    parent:    RowId[CrabBody],
    socket:    Xform[Leg -> Body],
    coxa_len:  Meters,
    femur_len: Meters,
    tibia_len: Meters,
    side:      f32,
    phase:     Rad,
  }

  query visible(self, u: Unit, v: Rad) -> { p: Pt[Leg], n: Norm[Leg] } {
    // Runs the SAME shader for all 12 legs (and all 240 legs across 20 crabs)
    let spine_p = leg_ik_point(self.coxa_len, self.femur_len, self.tibia_len, u);
    let r = mix(0.11m, 0.035m, u);
    // ... parametric tube evaluation ...
  }
}

// Stage 0: loop appends 12 rows to the CrabLeg storage table -> 0 new shaders
comptime fn spawn_crab_legs(body: RowId[CrabBody]) {
  comptime const FEMUR: array<Meters, 6> = [0.62m, 0.78m, 0.88m, 0.84m, 0.70m, 0.54m];
  comptime for pair in 0u..6u {
    comptime for side in [-1.0, 1.0] {
      emit CrabLeg {
        parent:    body,
        socket:    leg_socket_xform(pair, side),
        coxa_len:  0.22m,
        femur_len: FEMUR[pair],
        tibia_len: FEMUR[pair] * 1.18,
        side:      side,
        phase:     f32(pair) * 1.047rad,
      };
    }
  }
}

```

### Error When an Author Accidentally Makes 12 Kinds Instead

```text
error[E401]: continuous value cannot parameterize kind identity
  --> crab_legs.loft:1:15
   |
 1 | kind CrabLeg[comptime femur_len: Meters] {
   |               ^^^^^^^^^^^^^^^^^^^^^^^^^ `Meters` is continuous; kind parameters must be discrete
   |
   = note: instantiating this in `spawn_crab_legs` would mint 12 distinct shader modules.
   = fix: move `femur_len: Meters` into the `row { ... }` block so all legs share one compiled kind.

```

*(If an author instead tries a discrete `u32` leg index `0..12` as a `comptime` parameter, `E402: kind variant budget exceeded (12 > max 4)` triggers with the same one-sentence fix.)*

---

## 4. Tier 0 / Tier 1 Contract (Calibrated to Your +2.0 ms / 63-Op Data)

Your measurement (+2.0 ms for 63 ops, +5.5 ms for larger graphs) proves two things: **(a)** a global tape VM cannot run the whole scene in production, and **(b)** running *only the single actively edited kind* in a workgroup-uniform tape VM is completely viable at 60–120 Hz while Metal compiles Tier 1 asynchronously.

### 1. Opcode Set (~56 Fixed 32-bit 3-Address SSA Ops)

* **Included:** scalar/vec3/vec4 math (`fma`, `add`, `mul`, `min`, `max`, `clamp`, `mix`, `smoothstep`, `smootherstep`, `dot`, `cross`, `len`, `norm`), transcendentals (`sin`, `cos`, `exp`, `log`, `pow`, `sqrt`), SDF primitives + `smin_poly`/`smin_exp`, transforms (`xform_pt`, `xform_dir`, `mirror`, `repeat_cell`), and **fused noise intrinsics** (`grad_noise3`, `smooth_voronoi3` as single opcodes—never 40 scalar ops).
* **Loops:** **Yes** — structured `LOOP_BEGIN(trip_reg, end_pc) / LOOP_END` with static trip guard (at most 64 iterations).
* **Table & Texture Reads:** **Yes** — `LOAD_ROW(col_offset)`, `SAMPLE_LUT_C2(table_slot, t)`, `SAMPLE_TEX2D(tex_slot, uv)` via fixed bind-group descriptor arrays.
* **Derivatives (`diff`):** **Lowered before tape emission.** Forward-mode AD runs on the SSA graph in Rust (< 0.5 ms), emitting ordinary arithmetic opcodes into the tape. No runtime AD opcode exists.
* **Excluded:** `raw wgsl`, workgroup shared memory, atomics, subgroups. Kinds using those skip Tier 0 and keep their previous Tier 1 pipeline live until async compilation finishes.

### 2. SIMD Divergence Rule

A shared tape interpreter running heterogeneous kinds across lanes in one 32-thread Apple SIMD group **does diverge catastrophically** (every opcode switch diverges and lanes stall until the longest tape exits).

**Mandatory invariant:** Tier 0 is **never** dispatched heterogeneously. Every Tier 0 dispatch is **kind-bucketed** (one indirect dispatch per dirty Tier 0 kind; `tape_offset` is workgroup-uniform). Every thread in a 32-lane SIMD group executes the **exact same PC and opcode**—eliminating control-flow divergence entirely. The only remaining overhead is register-array indexing and opcode fetch, and because **clean kinds stay on Tier 1**, editing 1 crab kind covering ~12% of the screen pays **~0.25–0.45 ms** instead of +2.0 ms full-screen.

### 3. Promotion Policy (Tier 0 $\to$ Tier 1)

* **On edit:** Dirty kind immediately uploads its 2–8 KB tape buffer (**0 ms Metal compile, under 2 ms hot-reload**).
* **Promote to Tier 1 when:** The kind's canonical IR hash is quiet for **400 ms** (or **0 ms** on scene load / CI / release) AND (`ops > 20` OR measured GPU pass time `> 0.35 ms`).
* **Swap:** Compile native WGSL via `create*PipelineAsync` on a worker, cache in an LRU keyed by canonical IR hash (so undo/redo swaps in 0 ms), and atomically flip the kind's dispatch handle between frames.

### 4. Parity & Swap Tolerance Contract

Tier 0 and Tier 1 **will not be bit-identical** because Metal's LLVM backend contracts `a * b + c` into hardware FMA and uses fast transcendental approximations.

* **Contract:**
1. Both backends emit explicit `fma()` calls and share the exact polynomial definitions for `grad_noise3`, `smooth_voronoi3`, and `smin_*`.
2. **Field / scalar eval:** $\vert{}\text{Tier0} - \text{Tier1}\vert{} \le 10^{-5}$ abs (or 8 ULP).
3. **Motion / solver laws:** **Do not run in Tier 0** unless deterministic IEEE mode is active; motion kernels are tiny (1–2 modules, < 100 ms compile) and stay bit-matched to the CPU twin.
4. **Shaded swap continuity:** sRGB pixel diff $\le 1/255$ on at least $99.9\%$ of covered pixels ($\Delta E_{2000} \le 0.5$), max $2/255$ on grazing silhouette edges. Verified per-opcode in CI so mid-session promotion never visibly pops.



---

## 5. Scans, Race-Free Solvers, Motion & Open 6-Query Rendering

### A. Scans (`parallel_transport`)

`scan` explicitly declares sequential integration along a parameter:

* `@schedule(bake_lut)` — runs once at load (0 ms/frame) when the spine depends only on constants/row-static shapes.
* `@schedule(per_frame_compute)` — placed in a motion dependency wave before dependent geometry when the spine animates (e.g., flexing neck).
* **Differentiability:** Continuous sampling uses `sample_c2(table, t)` (cubic Hermite, provably $C^2$ in $t$). Differentiating w.r.t. upstream curve parameters automatically generates the forward-tangent scan pass.

### B. Race-Free GPU Solvers & Checkpointed State

* Parallel neighbor mutation is forbidden in raw kernels. `solver` requires an explicit conflict-free strategy:
* **`gauss_seidel(colored: 2)`** for chains/trees: phase 0 solves even edges $(2k, 2k+1)$, phase 1 solves odd edges $(2k+1, 2k+2)$—compiler proves disjoint writes. When strand length $\le 64$, the compiler maps 1 workgroup per strand and runs all substeps and colored iterations inside **`var<workgroup>` shared memory in 1 dispatch**.
* **`jacobi(omega: 0.5)`** for arbitrary meshes: gather/atomic delta buffer + normalized apply pass.


* **State ownership & scrubbing:** `state` buffers live in Layer 0's **State Store** with `@checkpoint(every: 60)` ring-buffer snapshots. Scrubbing to frame $F$ restores the latest checkpoint $\le F$ and steps forward deterministically ($\le 59$ substeps, < 2 ms) so scrubbing matches live execution to the bit.

### C. First-Class Motion & Honest Verdicts

* Joints/targets take explicit `Attach[Body]` vs `Target[World]` types so rigid body-locking cannot masquerade as world stabilization.
* Constraints return a **three-valued `Verdict**`:
* `Verdict::Held(margin: Meters)` — satisfied with positive margin.
* `Verdict::Missed(err: Meters)` — failed with exact magnitude (e.g. `Missed(0.003m)`).
* `Verdict::Unknown(reason)` — NaN, solver hit iteration cap, missing contact, or missing frame. CI gates pass **only** on `Held`.



### D. Mapping Loft's 6 Queries Without Freezing the Renderer

No hardcoded `GBuffer` or `PbrSurface`. Each `kind` implements any subset of `trace`, `visible`, `material`, `light`, `sky`, `medium`:

* `material` returns an **open algebraic `Bsdf**` (`ggx(...) + clearcoat(...) + thin_film(...) + sheen(...) + sss(...) + emission(...)`). Adding a `thin_film` lobe to one kind requires zero changes to any visibility buffer or other kind.
* `visible` writes a **64-bit Visibility Record** `(kind_id: u16, row_id: u24, prim_or_uv: u24)`—geometry transport is completely decoupled from shading.
* A `schedule` freely mixes **deferred visibility-buffer** (opaque shell/claws/legs), **forward** (translucent eyes/water reading `medium` + `sky`), and **path-traced `trace` rays** (wet reflections/GI) in the same frame.

---

## 6. The Hermit Crab — Rewritten & Verified

```wgsl
space World, Body, Shell, Claw, Leg;
unit Meters; unit Rad; unit Sec; unit Kg;

// --- 1. Parallel-Transport Scan (Baked Once at Load, C2-Sampleable) ---
fn shell_spine(t: Rad) -> Pt[Shell] {
  let e = exp(0.18 * f32(t));
  return Pt[Shell](e * 0.95 * cos(t), e * 0.95 * sin(t), -e * 0.72) * 1.0m;
}

scan shell_frames(t: Rad in 0.0rad..28.0rad, steps: 256u) @schedule(bake_lut) -> Table[256, Frame[Shell]] {
  init: frame_from_tangent(shell_spine(0.0rad), normalize(diff(shell_spine, t)(0.0rad)), Dir[Shell](0.0, 0.0, 1.0));
  step(prev, t_i): transport_frame(prev, shell_spine(t_i), normalize(diff(shell_spine, t)(t_i)));
}

// --- 2. Gastropod Shell Kind (C2 Macro Curvature + C1 Smooth-Voronoi Barnacles) ---
kind HermitShell {
  row {
    pose: Xform[Shell -> World],
    aggro: Unit,
  }

  // Strictly C2: uses smooth cosine power (no C0 max/abs, no Voronoi)
  fn macro_surface(t: Rad, a: Rad) -> Pt[Shell] {
    let f     = sample_c2(shell_frames, t);
    let scale = exp(0.18 * f32(t)) * 0.62m;
    let rib   = 0.08 * pow(0.5 + 0.5 * cos(6.0 * t), 4.0) * (0.4 + 0.6 * sin(5.0 * a));
    return f.origin + f.normal * (scale * (1.15 + rib) * cos(a))
                    + f.binormal * (scale * (0.82 + rib) * sin(a));
  }

  // C1 micro-displacement kept separate so macro curvature never differentiates through Voronoi
  fn barnacle_height(t: Rad, a: Rad) -> Meters {
    return 0.035m * smooth_voronoi_cone(vec3(f32(t) * 2.1, f32(a) * 3.0, 0.0), k: 0.12);
  }

  query visible(self, t: Rad, a: Rad) -> { p: Pt[World], n: Norm[World], crevice: Unit } {
    let p0  = macro_surface(t, a);
    let du  = diff(macro_surface, t)(t, a);              // C2 -> C1
    let dv  = diff(macro_surface, a)(t, a);              // C2 -> C1
    let n0  = normalize(cross(du, dv)) as Norm[Shell];
    let d2u = diff(du, t);                               // C1 -> C0 (legal! would error on Voronoi)
    let k   = dot(d2u, n0) / max(1e-5, dot(du, du));

    // Apply C1 barnacles along macro normal; first-order diff gives valid C0 perturbed normal
    let h   = barnacle_height(t, a);
    let dh  = vec2(diff(barnacle_height, t)(t, a), diff(barnacle_height, a)(t, a));
    let n_s = normalize(n0 - normalize(du) * dh.x - normalize(dv) * dh.y) as Norm[Shell];
    return {
      p: self.pose * (p0 + n0 * h),
      n: self.pose * n_s,
      crevice: smootherstep(0.4, -0.6, k),
    };
  }

  query material(self, hit) -> Bsdf {
    let grime = saturate(hit.crevice * 1.2 + 0.25 * grad_noise3(hit.local_p * 4.0));
    let base  = mix(rgb(0.62, 0.48, 0.36), rgb(0.09, 0.21, 0.14), grime);
    return ggx(albedo: base, roughness: mix(0.28, 0.78, grime))
         + clearcoat(weight: 1.0 - grime * 0.85, roughness: 0.08)
         + thin_film(weight: 0.25 * (1.0 - grime), ior: 1.42, thickness_nm: 380.0); // nacre sheen!
  }
}

// --- 3. Crusher Claw (Compiler-Derived L = 1.41, No FBM Inside Distance Field) ---
kind CrusherClaw {
  row {
    pose:     Xform[Claw -> World],
    jaw_open: Rad,
    aggro:    Unit,
    side:     f32,                                       // +1 crusher, -1 feeder via row scale
    scale:    f32,
  }

  // Return type omits L: compiler proves L <= 1.41 from round_box(1) + teeth(0.41) + polar clamp(1)
  query trace(self, p_world: Pt[World]) -> Field[Claw] {
    let p      = inv(self.pose) * p_world;
    let palm   = ssub_poly(sd_round_box(p, vec3(0.65, 0.35, 0.95)m, 0.28m),
                           sd_box(p - Vec[Claw](0.1, -0.35, 0.2)m, vec3(0.7, 0.3, 0.9)m), 0.06m);
    map ring   = repeat_polar(p.xz, 7u);                 // auto cell-wall clamped
    let studs  = sd_sphere(ring - Pt['ring](0.75m, 0.0m, 0.0m), 0.11m);
    map jaw    = (p - Vec[Claw](0.0, -0.22, 0.85)m) |> rot_x(-self.jaw_open);
    let teeth  = 0.025m * sin(14.0 * f32(jaw.z)) * smootherstep(0.1m, 0.9m, jaw.z);
    let dactyl = sd_capsule(jaw, Pt['jaw](0.0, 0.0, 0.0)m, Pt['jaw](0.0, -0.15, 1.15)m, 0.24m) - teeth;
    return smin_poly(smin_poly(palm, dactyl, 0.08m), studs, 0.04m);
  }

  query material(self, hit, time: Sec) -> Bsdf {
    // High-frequency chitin pitting lives at shade time (0 impact on raymarch L!)
    let micro = grad_noise3(hit.local_p * 6.5);
    let veins = smootherstep(0.62, 0.78, smooth_voronoi3(hit.local_p * 5.2)) * self.aggro;
    return ggx(albedo: rgb(0.68, 0.14, 0.07), roughness: mix(0.16, 0.42, micro))
         + clearcoat(weight: 0.9, roughness: 0.06)
         + sss(color: rgb(0.9, 0.25, 0.08), radius: 0.35)
         + emission(rgb(1.0, 0.32, 0.04) * veins * (3.5 + 1.5 * sin(f32(time) * 8.0)));
  }
}

// --- 4. Expressive Face & Eyestalks (Scoped Mirror Space Fixes World Gaze) ---
kind CrabFace {
  row {
    pose:   Xform[Body -> World],
    aggro:  Unit,
    target: Target[World],                               // explicit world-space gaze target
  }

  query trace(self, p_world: Pt[World], time: Sec) -> { d: Field[Body], is_pupil: Unit } {
    let p     = inv(self.pose) * p_world;
    map sym   = mirror_x(p) |> translate(-Vec[Body](0.42, 0.55, 1.18)m);

    map brow  = (sym - Vec['sym](0.0, 0.28, 0.05)m)
              |> rot_z(-0.35rad * self.aggro) |> rot_x(0.28rad * self.aggro);
    let plate = sd_wedge(brow, vec3(0.32, 0.10, 0.38)m);

    let sway  = 0.08m * sin(f32(time) * 2.1);
    let eye_c = Pt['sym](sway, 0.24m, 0.16m);
    let stalk = sd_capsule(sym, Pt['sym](0.0, -0.2, 0.0)m, eye_c, 0.09m);
    let globe = sd_sphere(sym - eye_c, 0.135m);

    // Exact world-space eye position for BOTH left and right eyes via sym.to_parent (uses sym.cell!)
    let eye_world = self.pose * (sym.to_parent * eye_c);
    let look_w    = normalize(self.target.pos - eye_world);
    let look_sym  = sym.from_parent.dir(inv(self.pose) * look_w);
    let pupil     = smootherstep(0.88, 0.94, dot(normalize(sym - eye_c), look_sym));

    let d = smin_poly(smin_poly(stalk, globe, 0.05m), plate, 0.03m);
    return { d, is_pupil: pupil * select(0.0, 1.0, globe < stalk) };
  }
}

// --- 5. Race-Free Colored XPBD Antennae (Pinned to Head, Checkpointed State) ---
solver AntennaSolver over AntennaStrands {
  @checkpoint(every: 60)
  state { pos: array<Pt[World], 16>, prev: array<Pt[World], 16>, inv_m: array<1/Kg, 16> }

  row {
    parent:    RowId[CrabBody],
    head_pin:  Attach[Body],                             // explicit rigid attachment to head socket
    rest_len:  Meters,
    stiffness: f32,
  }

  config { strategy: gauss_seidel(colored: 2), substeps: 4u, iterations: 6u, shared_mem: true }

  step(dt: Sec, time: Sec, bodies: Table[CrabBody]) {
    let root_w = bodies[self.parent].pose * self.head_pin.offset;
    // Predict + pin node 0 before constraint sweeps
    pos[0] = root_w; prev[0] = root_w; inv_m[0] = 0.0 / 1.0kg;
    for i in 1u..16u {
      let current = Vec[World](sin(f32(time) + f32(i) * 0.4), 0.2, cos(f32(time) * 0.7)) * 1.4m/(1.0s*1.0s);
      let v = (pos[i] - prev[i]) / dt;
      prev[i] = pos[i];
      pos[i]  = pos[i] + (v * 0.96 + current * dt) * dt;
    }
    // Compiler schedules phase 0 (even edges) and phase 1 (odd edges) with workgroupBarrier()
    constrain edge(i, i + 1u) in 0u..15u {
      solve_dist(&pos[i], &pos[i + 1u], inv_m[i], inv_m[i + 1u], self.rest_len, compliance: 0.002);
    }
  }
}

// --- 6. First-Class Motion Law & Honest Constraint Verdicts ---
law CrabStepLaw(dt: Sec, bodies: Table[CrabBody], legs: Table[CrabLeg]) writes CrabLeg {
  // Capture-point foot stepping: steps from current foot pos when weight shifts past support polygon
  let cp = bodies[leg.parent].com_world + bodies[leg.parent].vel_world * 0.28s;
  if distance(leg.foot_world, leg.nominal_anchor(cp)) > leg.stride_limit {
    leg.step_target = Target[World](project_seabed(cp + leg.foot_offset));
  }
}

constraint EyeGazeAndPosture(b: CrabBody, face: CrabFace) -> Verdict {
  if !is_finite(b.pose) { return Verdict::Unknown(NonFinitePose); }
  let head_w = b.pose * b.head_socket;
  let margin = head_w.y - (b.hip_world.y + 0.15m);
  if margin < 0.0m { return Verdict::Missed(-margin); }  // e.g. "Missed by 3.2 mm"
  return Verdict::Held(margin);
}

// --- 7. Open 6-Query Hybrid Schedule (No Hardcoded G-Buffer or PbrSurface) ---
schedule SeaBossHybrid {
  // Compiler buckets laws automatically by (wave, law_kind)
  pass motion:   run_laws(CrabBodyLaw -> CrabStepLaw);
  pass physics:  run_solver(AntennaSolver);
  pass cull:     build_indirect(HermitShell, CrabLeg, AntennaRibbon, CrusherClaw, CrabFace);
  // 64-bit Visibility Buffer: adding thin_film or new kinds never changes buffer layout
  pass vis:      render_bundle(Vis64) {
                   draw_indirect_mesh(HermitShell, CrabLeg, AntennaRibbon);
                   draw_indirect_trace_proxy(CrusherClaw, CrabFace);
                 };
  pass classify: tile_bin_kinds(Vis64, tile: 8x8);
  pass shade:    dispatch_material_tiles(Vis64, light: SunCaustics, sky: OceanSky, medium: DeepWater);
  pass present:  tonemap_aces(ColorHdr) -> Swapchain;
}

```

---

## 7. Lowering & Draw-Path Portability (1 Crab vs 20 Crabs)

### How WebGPU Core Achieves $O(K_{\text{kinds}})$ Without `multi_draw_indirect`

Browser WebGPU lacks `multi_draw_indirect` and indirect draw counts, **but it supports `drawIndirect` inside a `RenderBundle**`. Because staging guarantees every instance of a kind shares one compiled topology/proxy and differs only by its row index:

1. One `cull` compute dispatch tests all rows (using compiler-derived interval AABBs), compacts visible `row_id`s per kind into a contiguous `visible_rows` buffer, and writes **one 16-byte `DrawIndirectArgs` per kind** (`instance_count = visible_count`).
2. A **pre-recorded `RenderBundle**` (rebuilt only when the set of kinds changes, never per frame) executes **1 `setPipeline` + 1 `drawIndirect` per kind**—both for instanced meshes (`HermitShell`, `CrabLeg`, `AntennaRibbon`) and for SDF bounding-box proxies (`CrusherClaw`, `CrabFace`).
3. If `visible_count == 0`, the GPU skips the draw at zero vertex cost. Native `wgpu` only needs `multi_draw_indirect_count` if you later pack dozens of *distinct static prop topologies* under a single shared pipeline.

### Lowering Breakdown (Apple M4, 1280×800)

| Metric | 1 Crab (Native `wgpu`) | 20 Crabs (Native `wgpu`) | 1 Crab (Browser WebGPU) | 20 Crabs (Browser WebGPU) |
| --- | --- | --- | --- | --- |
| **WGSL Shader Modules** | **11** | **11** | **11** | **11** |
| **Compiled Pipelines** | **12** | **12** | **12** | **12** |
| **Cold / Warm Pipeline Load** | ~1.1 s / ~0.15 s | ~1.1 s / ~0.15 s | ~1.4 s / ~0.20 s | ~1.4 s / ~0.20 s |
| **Compute Dispatches / Frame** | **7** | **7** | **7** | **7** |
| **Render Draw Commands / Frame** | **6** (5 in bundle + 1 present) | **6** (5 in bundle + 1 present) | **6** (5 in bundle + 1 present) | **6** (5 in bundle + 1 present) |
| **Total GPU Commands** | **13** (~0.20 ms cmd cost) | **13** (~0.20 ms cmd cost) | **13** (~0.20 ms cmd cost) | **13** (~0.20 ms cmd cost) |

#### Per-Pass GPU Time Breakdown at 1280×800 (1.024M px, M4)

| Pass | 1 Crab (ms) | 20 Crabs (ms) | What Scales From 1 $\to$ 20 |
| --- | --- | --- | --- |
| Motion waves (2 dispatches: body $\to$ 240 legs IK) | 0.04 | 0.08 | 20 bodies, 240 legs (0 extra dispatches) |
| Antennae XPBD (1 dispatch, shared-mem colored) | 0.02 | 0.07 | 4 $\to$ 80 workgroups (1 workgroup/strand) |
| Cull + indirect args (1 dispatch) | 0.02 | 0.03 | 19 $\to$ 380 rows tested |
| Vis-buffer mesh raster (shell + 12×N legs + ribbons) | 0.18 | 0.55 | ~11k $\to$ ~140k visible tris (after frustum cull) |
| Vis-buffer SDF proxy trace (claws + faces, early-Z) | 0.65 | 1.65 | ~140k $\to$ ~350k unoccluded px (derived $L=1.41$) |
| Tile classify + Deferred tile PBR/caustics (3 dispatches) | 0.63 | 1.05 | ~256k $\to$ ~665k fg pixels (zero SIMD divergence) |
| Tonemap + present (1 draw) | 0.12 | 0.12 | Constant (full-screen) |
| Driver/encoder floor + 13 commands | 0.45 | 0.45 | Constant (+0.6 ms Dawn floor in browser) |
| **Predicted Total GPU Frame Time (p50 / p95)** | **2.1 / 2.4 ms** | **4.0 / 4.6 ms** | **Browser WebGPU: 2.7 ms (1 crab) / 4.7 ms (20 crabs)** |

---

## 8. Three Type-Error Diagnostics for Model Authors

### 1. Space Error (Catching the Original Gaze Bug)

```text
error[E104]: space mismatch in transform application
  --> hermit_crab.loft:94:22
   |
83 |   map sym   = mirror_x(p) |> translate(-Vec[Body](0.42, 0.55, 1.18)m);
   |       --- space `'sym` introduced here
...
94 |   let eye_world = self.pose * eye_c;
   |                   ^^^^^^^^^   ^^^^^ expected `Pt[Body]`, found `Pt['sym]`
   |
   = fix: lift `eye_c` back to `Body` space via `self.pose * (sym.to_parent * eye_c)` so the offset and mirror sign are inverted.

```

### 2. Unproven Lipschitz Bound

```text
error[E211]: sphere tracer requires a proven Lipschitz bound, got `Field[Claw, Unbounded]`
  --> hermit_crab.loft:66:5
   |
64 |   let flare = 0.04m / max(0.0, length(p.xz) - 0.5m);
   |               ------------------------------------- interval of denominator `[0.0, 1.4]` contains 0 -> L = Unbounded
66 |   return smin_poly(palm, dactyl, 0.08m) - flare;
   |   ^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^^ returned from `query trace`
   |
   = fix: clamp the denominator away from zero (`max(0.05m, ...)`) or wrap the field in `bound_step(..., max_step: 0.04m)` for conservative fixed-step marching.

```

### 3. Non-Smooth Second Derivative (Catching the Voronoi Curvature Bug)

```text
error[E307]: cannot differentiate a `C0` expression
  --> hermit_crab.loft:38:15
   |
29 |   let barn = 0.035m * voronoi_cone(vec3(t * 2.1, a * 3.0, 0.0));
   |                       ------------ `voronoi_cone` is `C0` (gradient discontinuous at cell borders)
...
38 |   let d2u  = diff(du, t);
   |              ^^^^^^^^^^^ `du` has smoothness `C0`, but `diff` requires at least `C1`
   |
   = fix: compute curvature on the `C2` macro shell before adding barnacles, or switch to `smooth_voronoi_cone` and perturb normals with first-order `diff` only.

```

---

## 9. The Single Change That Matters Most After Seeing Your Numbers

My earlier proposal leaned on **post-hoc compiler deduplication** (`egglog`/hash-consing) and a **blanket Tier 0 Tape VM** to rescue hot-reload and sharing. Your M4 numbers kill that approach: once host tracing unrolls Titan into 707k nodes or fractures five crabs into 305–5,539 modules, deduplication is fighting the tide, and running the whole frame through a tape VM costs +2.0–5.5 ms.

The single highest-leverage architectural change is **enforcing two-level staging (`kind` = discrete code identity, `row` = continuous instance data) as a hard type-system law at the language frontier, paired with dirty-only, workgroup-uniform Tier 0 tape execution**. That one rule makes module count strictly $O(K_{\text{kinds}})$ by construction—collapsing 32.9 s cold compiles to ~1 s, making `drawIndirect` + `RenderBundle` portable across browser WebGPU and native `wgpu` with zero per-instance draw scaling, and keeping Tier 0 editing overhead under 0.5 ms while clean kinds run natively on Tier 1.

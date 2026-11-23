# Probe A: the public door

23 Nov 2026, branch `probe-door` on `merge-2026-11-23-r2` (a3cfe2c0). Implements capabilities F
and W of `DESIGN.md` (§5.1, primary checkout). Written before any code; results are appended in
§6 without editing §1–§5.

## 1. What an author writes

A schedule function is a `schedule` with parameters. A call in any schedule inlines it. The stock
frame passes are now these, in `stdlib/core.loft` (in scope everywhere, so no scene changes):

```
pub schedule cull_indirect(..kinds) {
  pass cull reads [CameraSlot, kinds] writes [IndirectBuf] {
    dispatch("core/cull.wgsl", "cull", Instances, draw: if drawn == 1 { "core/cull_single.wgsl" } else { "core/cull_multi.wgsl" })
  }
}
```

- `pass` in a render schedule names frame stores in `reads`/`writes` (`Vis64`, `DepthBuf`,
  `TileLists`, `ColorHdr`, `Swapchain`, `IndirectBuf`, `ShadowMap`, `CameraSlot`, kind names).
  Hazards are checked by the existing schedule-order rule.
- `dispatch(template, entry, extent, hole: value, ...)` and `draw(template, vertex, fragment, ...)`
  (full-screen raster into `Swapchain`). A named argument fills the template hole of the same name
  in capitals. A hole value is a WGSL string (a string ending in `.wgsl` names a fragment file next
  to the schedule), a Loft function (its symbol; the module then carries the Loft library it
  reaches), a number, or `if` over the frame's compile-time facts: `drawn` (drawn kind count),
  `edges` (`none`/`taps`/`coverage`), `aov`, `paper`. Fragments may use the same holes.
- The extent `Instances` or `Pixels` is divided by the entry's own `@workgroup_size`.
- WGSL globals bind to frame stores by name (the frame ABI: `vis_tex`, `hdr`, `hdr_tex`, `tiles`,
  `shade_args`, `draw_args`, `visible`, `insts`, `bases`). A pass may bind only stores it lists.
- W: `find_shadow_map(Casters, size: 1024) on dirty(Casters)` or `... on load`.

## 2. Lowering plan

1. Parser: `schedule name(params)` with `..rest` and `name = default`; `on load | on dirty(...)`
   after any step (`PassDecl.clock`, the existing `BakeClock`).
2. `loft-check`: `link` keeps parametrised schedules from every module as a library; `defaults`
   inlines calls after the recipe is composed (recursively, depth 8), substituting parameters in
   arguments, resource lists and the pass body. This replaces nothing yet; `woodblock(...)` stays
   (it binds worlds to kind lists, which a parameter cannot express without world-typed params).
3. `loft-passes` schedule: an authored pass whose stores are frame stores becomes a frame step;
   its body is classified once into `Compute { entry, extent } | FullScreen { vertex, fragment }`
   plus holes. The frame's sky (which World shading also calls) is the present pass's `sky` hole.
4. Lowering, after `lower_world` fixes `edges`: evaluate holes, fill the template, prepend the
   prelude (and the Loft library when a hole names a function), validate with naga, map bindings
   by name to frame views, check them against the pass's stores. `Pass::Authored` replaces
   `Pass::Cull | Classify | Present`; `Program.cull/classify/present` go away. Path schedules keep
   their own present module as `Pass::Path`.
5. Bundle: one handler maps frame views to tables and dispatches or draws.
6. W: `find_shadow_map` carries its clock to `Program.shadow_clock`; loft-native draws a `Load`
   shadow once. Other steps with `on` are a typed error in this probe.

## 3. Rust that goes away

`emit::cull_module`, `classify_module`, `classify_with_halo`, `present_module`, `emit_present`,
`shade_tiles::{classify_module, present_module}` and the dead edges+AOV rewrite in it, the
`cull_indirect`/`tile_classify`/`tonemap*` arms and effects in `schedule.rs`, the culled list, the
film fields, three fixed bundle handlers. The present resolve variants move from a Rust `match`
to WGSL fragments and an `if` chain in `stdlib`.

## 4. Predictions

| # | Prediction | Falsified if |
|---|---|---|
| A1 | Raw module text (`loft show --json`) of every module of the 45 scenes I captured (7 gate scenes, every `scenes/**`, `corpus/l5d`) is byte-identical, including `cull`, `classify`, `present`. | any hash differs |
| A2 | Module count unchanged for raster scenes; −2 (`cull`, `classify` no longer emitted unused) for path-traced scenes. | other count changes |
| A3 | Pictures 19/19 hash-equal; Twin and Truth unchanged; pipelines 5/5/13 unchanged. | any differs |
| A4 | Frame time unchanged (paired, within noise). | a paired delta beyond noise |
| A5 | Net Rust in `crates/`: **+250 ± 120** lines (the door costs more than three passes' special cases save; the design's −1,200 needs shade, shadow and the kind adapters moved too). `stdlib`: about +45 `.loft`, +5 small WGSL fragments. | outside the band |
| W1 | Static casters: the shadow pass costs **0.00 ms** after the first frame **with or without** `on dirty(Casters)`: loft-native already proves static casters and omits the shadow wave (`loft-native/src/shadow.rs`). Frame saving vs main from the clause: **0.0 ms**. The clause adds a declared contract (and `on load` for moving casters), not speed, on this host. | profile shows a nonzero shadow row after frame 0 without the clause |
| W2 | First-frame shadow cost on the new scene (20 capsules, 1024²): about 0.1–0.3 ms, the per-frame saving a host without inference would see. | outside |

## 5. Unknown 1 (holes chosen by Rust)

Tested on `tonemap`: the resolve choice depends on `aov` (a CLI option), on `edges` (an option
of `shade_tiles`, a later step) and on `paper` (set by `outline_and_paper`). A schedule cannot
see those unless the frame publishes them. The probe publishes four facts. That is the answer to
unknown 1 this probe proposes: one template per variant would need 8 present files and still a
Rust choice of which file; a compile-time `if` over published facts keeps the choice in source.

## 6. Results (appended after the runs)

Instruments and evidence are under `out/probe_door/` (scripts `modhash.py`, `compare.py`,
`gpu_batch.sh`, `profiles.py`; logs and JSON beside them). Baseline binary: the lab `loft` built at
a3cfe2c0 in `_wt/loft-merge-2026-11-23`. One `cargo check` of the workspace, clean on the first
try; two follow-up fixes found by the module comparison (below) before the test build.

| # | Predicted | Measured | Verdict |
|---|---|---|---|
| A1 | every module's raw text identical | 242 of 242 modules identical across 45 scenes (7 gate scenes, all `scenes/**`, `corpus/l5d`) plus all 11 `stdlib/engines` entries, including `cull` (both variants), `classify` (centre and the `edges: taps` halo variant of `realtime_matched`), `present` (ACES, `film:` fn, woodblock paper) | held |
| A2 | −2 modules for path scenes only | `spectral_caustic_study`, `reference`, `spectral_caustic`: 8→6, 3→1, 3→1 (`cull`, `classify` no longer emitted unused); no other count changed | held |
| A3 | pictures 19/19, Twin/Truth unchanged | `loft-verify gate`: Pictures 19/19 held, Twin 36/36 held, Truth 34/34 held. AOV renders (albedo, depth, normal, overdraw × capsules, coral_shrine, hermit) baseline vs candidate: 11/12 byte-equal PNGs; coral_shrine normal differs in 8 of 256,000 pixels, and differs between repeat renders of the baseline too (see note) | held |
| A4 | frame time unchanged | paired, interleaved, 3 rounds, 1280×800, 300 frames; load average 7–9 (contended, p95 tails 6–52 ms): capsules median paired Δp50 −0.06 ms, hermit −0.35 ms; Speed gate tails failed on both sides of the machine's noise, not a quiet window | not measured cleanly; no sign of a change |
| A5 | net Rust +250 ± 120 | `crates/` +950 −276 = **+674**; ratchet 34,285 → 34,868 (+583 code lines). `stdlib`: +36 `.loft`, +12 WGSL (`present_coverage.wgsl`) moved out of Rust | **missed** |
| W1 | shadow 0.00 ms after frame 0 with or without the clause; saving 0.0 ms | static_shadow, 300 frames: shadow row 0.000 ms on frames 1–299 for baseline (no clause), candidate (no clause) and candidate `on dirty(Casters)`; first frame 1.47 / 0.47 / 0.89 ms | held |
| W2 | first-frame shadow 0.1–0.3 ms | 0.47–1.47 ms (first frame includes cold pipeline work; contended) | missed high |

Why A5 missed: the generic path (classification, hole evaluation with typed facts, binding
validation, schedule-function binding and substitution) is about 540 lines; the three special
cases it replaced were about 150. The trade pays only when shade's frame part, shadow and post move
too (§7 below).

Bugs the module comparison caught before any test ran: (1) an explicit `pass` label was linked as
a name, so `pass present` collided with a law named `present` in `gait.loft`; (2) `Drawables` in
a scene with no drawn rows lists no kinds, which the instance-table binding check treated as
"does not read kinds". Both fixed, with comments at the sites.

Note on the coral_shrine normal AOV: 8 isolated pixels with large deltas is the signature of the
compact cull's atomic append order at equal depths (the reason `ordered_cull.rs` exists). Two more
renders per binary settle it: all three baseline renders differ from each other, and so do all
three candidate renders. The coral_shrine normal AOV is nondeterministic on main; not a change.

## 7. How far it got

Moved to `stdlib/core.loft`: `cull_indirect`, `tile_classify`, `tonemap`, `tonemap_aces` (names
and arguments unchanged; `tile:` is now a real hole with the TileLists 8×8 contract checked).
Deleted: `cull_module`, `classify_module`, `classify_with_halo`, `present_module` (two),
`emit_present`, `HALO_BINDINGS`, the dead edges+AOV rewrite, `Pass::{Cull, Classify, Present}`,
`Program.{cull, classify, present}`, the culled list, the film fields, three bundle handlers.

Not moved, and why:
- `find_shadow_map`: it replays every caster kind's own vis pipeline into the shadow targets with
  the light's frame variant. A template pass binds frame stores; it has no form for "draw each
  kind's module". That is the per-kind adapter the design keeps in the compiler.
- `shade_tiles` (frame part): per kind, an indirect dispatch of that kind's module over its tile
  list. Same reason: the work lives in kind modules.
- `outline_and_paper`: not a pass at all; it selects the present variant. It stays as a two-line
  Rust arm that publishes the `paper` fact.
- `woodblock(PrimaryWorld, SolidShadow, MainCam)`: binds worlds to kind lists. A schedule function
  parameter is a name or list; it would need a world-typed parameter.

W: `on load` / `on dirty(...)` parse on every step (shared with `bake_tex`'s clause) and lower for
`find_shadow_map` (`Program.shadow_clock`; loft-native draws a `Load` map once). Every other step
with `on` is a typed error. The portable bundle does not carry the clock yet: runner initializers
accept compute only, and do not apply the light frame variant.

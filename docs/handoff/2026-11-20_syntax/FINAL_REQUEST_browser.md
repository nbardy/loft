# Final request before we build (Loft v1.0)

(Everything this message refers to is either in this message or in the bundle you sent us. You need no files.)

Thank you for the v1.0 bundle. We have saved its seven files as our specification, and it is now the source of truth. The Phase 1.5 work (the E+
parser and port) is starting from it. This is our last request before we commit to Phases 2–5. We want to make sure
the implementer never has to guess. Please answer only what is below. Where a section of yours is already complete,
say "complete" and we will build from it as written.

## A. Contradictions to settle (one line each)

1. **Materials: sum or layering?** Decision 13 and `material` write lobes as a sum (`ggx + sss + coat`). Astra, a second reviewer,
   writes them as nested layers (`coat(sss(ggx(...)))`), where a coat attenuates what lies below it by its own transmittance.
   Which semantics does `+` have? Is it a physical layer stack in order (top first), with Kulla–Conty compensation
   per layer, or an energy sum? How does an author say "this coat sits over that base"?
2. **Old reference pictures.** The Pictures gate scores geometry against R0g. R0 came from the old engine, which we
   are retiring. Should the new engine's first renders, approved by the owner, become the references? Or do we keep
   R0g for the scenes that exist in both?
3. **`@waive` against "no `@attributes`".** Decision 6 reserves `@` for spaces, but the creature invariants say
   "active unless `@waive`d". What is the keyword form?
4. **The `.lark` file.** It sits inside your language file (01_LANGUAGE_V1). Is it complete and normative, meaning a parser built from
   it must accept exactly the language, or is it illustrative?

## B. Missing pieces the implementer would otherwise invent

5. **Schedule IR.** List every schedule statement (`cull_indirect`, `render_bundle`, `draw_indirect`, `tile_bin`,
   `shade_tiles`, `tonemap_*`, `run_laws`, `check`, `orbit`, …). For each, give its typed signature, the resources it
   reads and writes (this drives dependency analysis), and how a custom pass declares its resources.
6. **The `World` queries' types.** Give the exact return types of `trace`, `visible`, `material`, `light`, `sky` and
   `medium` (Hit, Bsdf, Light sample, Medium segment). Give the `shade_fn` contract, so that a forked light transport
   compiles against them.
7. **`std` signatures with types and units.** Your stdlib file (03_SUB_ENGINES_AND_STDLIB) lists names under `std::geom`, `std::light`, `std::stylised`,
   `std::post` and `std::motion`; some have types and some do not. Give the full typed signatures for every exported
   function and kind, including default parameter values.
8. **Motion tables.** Give the exact schemas of BODY, STATE and POSE: fields, units, std430 layout and bytes per
   creature, as in your "2,368 B state". Show how `body` is declared in `.loft`: segments, joints, limits, masses,
   ends, eyes. Show how a custom output table replaces POSE.
9. **`plan`.** Its `.loft` syntax, which holds it accepts (position, velocity, acceleration, posture, timing), its
   objectives (effort, jerk, `prefer near`), and how its result is stored and read.
10. **Determinism rules for motion code.** Which operations are allowed, which are banned or wrapped (transcendentals,
    `fma`, division, atomics, sort order), and how the compiler enforces live-equals-bake per device.
11. **Units.** The base and derived units, how compound units parse (`rad/s`, `m/s^2`, `N/m`), and how numeric
    promotion interacts with them.
12. **Randomness at build time.** Every bakeoff output invented its own `rnd`. Specify a seeded, deterministic `std`
    random API for build code. Is there a GPU-side hash for runtime?
13. **Tier 0 opcode table.** The 56 opcodes with their encodings, or confirm the implementer may define them within
    your stated format (`[op:6][dst:5][a:5][b:5][imm11:11]`).
14. **The bundle on disk.** Its layout and version field: what `load_bundle` reads, what is cached, and what
    invalidates the cache.
15. **Comments and lossless edits** (raised by Astra). Must the parse tree keep comments and spacing so that agent
    rewrites and `loft fmt` round-trip source exactly? Does `loft ir --sexp` carry comments?
16. **`loft fmt`.** What is the canonical formatting: indentation, line width, ordering of kind members?
17. **The CLI and training contract.** The commands and flags for build, render, clip, profile, check and show; exit
    codes; and the interface the creative training pipeline calls. The build budget is 2 s of CPU and the draw budget
    50 ms of GPU per frame at 512² × 16 frames, and the existing exit code 7 means "over budget".
18. **Asset import.** The glTF subset supported in Phase 5 (meshes, skins, clips, materials?), and how an imported
    mesh becomes a kind with rows.

## C. Your open questions

You listed five, with defaults. We accept your defaults unless you want to change one now.

## What to send back

One Markdown file per section A and B, or one file with headed sections. Use code blocks for `.loft`, Rust and WGSL,
and tables for schemas and layouts. Mark anything predicted as predicted. Keep it to what the implementer needs to
build without asking. Do not restate the bundle.

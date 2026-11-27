# Loft standard library

Every source file gets `TAU`, `Sun`, `Sky` and the schedule functions `cull_indirect`,
`tile_classify`, `tonemap` and `tonemap_aces` from [core.loft](core.loft). The prelude also
loads `lighting::emitters`, `sky::environment` and `post::display` for renderer defaults and callbacks,
along with `sdf::solid`, `noise::value`, `materials::bsdf`, `geometry::analytic`, `geometry::layout`,
`frames::transport`, `motion::solve`, `integrators::direct` and `styles::woodblock`. The current
compiler resolves public stock names in loaded modules by bare name. Use explicit imports to record
a file's dependencies or to choose a qualified implementation.

```loft
use stdlib::engines::woodblock
use stdlib::shapes::primitives::{Capsule, GlossBead}

build {
  emit Capsule(a: [-0.4, 0, 0]@World, b: [0.4, 0, 0]@World, r: 0.16m, albedo: [0.43, 0.14, 0.05])
  emit GlossBead(c: [0, 0.26, 0]@World, r: 0.14m, albedo: [0.82, 0.60, 0.24])
}
```

An unaliased module import uses the last path segment as its qualifier. Use `as` to choose a qualifier,
`::{name, other}` to select named exports or `::*` for all public declarations in a module. `pub` marks
exports. Private declarations and import bindings are scoped to their file.

The compiler finds `stdlib/` at the nearest workspace ancestor with `Cargo.toml`. Each modern import
resolves a single `.loft` file. A `self::` path starts beside the importing file. Each module contributes
its declarations once, and only the entry file's anonymous `build` blocks run. A scene with a complete
schedule uses it. Otherwise a directly imported public engine schedule supplies the frame, with
Realtime as the default. Conflicting engine selections and module cycles produce diagnostics.

A schedule with parameters inlines its steps at each call. Stock frame passes use `pass` blocks
with `dispatch(template, entry, invocation_count, hole: value)` and
`draw(template, vertex, fragment, hole: value)` to fill WGSL template holes.

## Directory map

| Path | Contents |
| --- | --- |
| [engines/](engines/) | Render schedules such as Realtime, Woodblock, Reference and SpectralCaustic. Legacy gate scenes keep their paths for [engines/gates.json](../engines/gates.json), and Act and Roam retain compatibility exports here. |
| [shapes/](shapes/) | Stock drawable kinds and reusable shape parts. |
| [sdf/](sdf/) | Signed distance bounds, solids, marks and smooth composition. |
| [geometry/](geometry/) | Analytic intersections, mesh assets, growth, shells, sweeps and layout. |
| [materials/](materials/) | BSDF lobes and material recipes. |
| [lighting/](lighting/) | Emitters and world query interfaces. |
| [integrators/](integrators/) | Direct lighting and ReSTIR reservoir sampling. |
| [sky/](sky/) | Environment light, sky colour and background. |
| [post/](post/) | Display transforms and paper grain. |
| [styles/](styles/) | Shading and pigment recipes. |
| [motion/](motion/) | Act and Roam, body plans, gait laws, solvers, springs and audits. |
| [noise/](noise/) | Value and cellular noise. |
| [math/](math/) | Sampling utilities. |
| [physics/](physics/) | Particle simulation and sorting kernels. |
| [frames/](frames/) | Transport along frames. |
| [core/](core/) | Shared WGSL pass templates and fragments. |
| [migration/](migration/) | Legacy observation and coverage adapters. |

## Fork a renderer or tune a gait

Save the scene above as `scenes/harbor/main.loft` and copy the renderer and its pigment.

```sh
mkdir -p scenes/harbor/ink
cp stdlib/engines/woodblock.loft scenes/harbor/render.loft
cp stdlib/styles/woodblock.loft scenes/harbor/ink/woodblock.loft
```

In `scenes/harbor/render.loft`, change `use stdlib::styles::woodblock as ink` to
`use self::ink::woodblock as ink` and change the pigment in the copied `woodblock_pigment`.
Select `use self::render` in the scene and inspect the linked shader with `loft show`.

For a different foot lift, pass `swing_lift: 0.185m` to `walker`, `walker_plan` or `trunk_plan`.
The default is `0.115m`. Tidal Fork uses the higher lift with the stock `step` and `present`
laws and the same planted check as Tidal. The single-row and split-row layouts share the
touchdown and swing pose math while retaining their own laws, write sets and checks.
Pass `crab_details: false` for a plain torso. The default silhouette comes from
[shapes/crab.loft](shapes/crab.loft), which can be forked as a shape part.
The added row fields and shared helpers change the motion module and may change cached check
and trace identities.

## Migration aliases

The loader maps `std::geom` to geometry, frames and solid SDF exports, `std::light` to the world,
emitters, sky, direct lighting and BSDF exports, and `std::noise` to value noise. `std::math`,
`std::color` and `std::quant` retain their intrinsic spellings. For new source, call language
intrinsics such as `dot4_i8` directly. The old `engines::realtime`, `engines::woodblock`,
`engines::reference` and `engines::spectral_caustic` imports resolve to their recipes in
`stdlib/engines/`. See [REFERENCE.md](REFERENCE.md) for the tagged legacy source index.

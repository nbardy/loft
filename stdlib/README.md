# Loft standard library

`core.loft` supplies `TAU`, `Sun` and `Sky` to every source file. Import each other dependency where you use it.

```loft
use stdlib::engines::woodblock
use stdlib::sdf::solid::{sphere}
use stdlib::materials::bsdf as material

kind Buoy @World (at: p3@World, radius: m, tint: rgb) {
  trace(p) = sphere(p - at, radius)
  material(p, n) = material::ggx(tint, 0.55, 0)
}

build { emit Buoy(at: [0, 0, 0]@World, radius: 0.7m, tint: [0.43, 0.14, 0.05]) }
```

An unaliased module import uses the last segment as its qualifier. `as` gives it a local qualifier. Select named exports with `::{name, other}` or use `::*` for every public declaration in a file. `pub` marks exports. Helpers without `pub` are visible inside their declaring file. Imports stay in that file's scope. Use a qualifier when two modules export the same name.

The compiler finds `stdlib/` at the nearest workspace ancestor with `Cargo.toml`. Each import resolves a single `.loft` file. A `self::` path starts beside the file containing that import. A module reached through multiple paths contributes one set of declarations. Only the entry file's anonymous `build` blocks run. A scene with a complete schedule uses that schedule. Otherwise it selects a directly imported public engine schedule or uses Realtime when there is no selection. The compiler diagnoses conflicting selections and import cycles at their source sites.

## Copy an engine and its ink

Save the scene above as `scenes/harbor/main.loft` and make these copies.

```sh
mkdir -p scenes/harbor/ink
cp stdlib/engines/woodblock.loft scenes/harbor/render.loft
cp stdlib/styles/woodblock.loft scenes/harbor/ink/woodblock.loft
```

In `scenes/harbor/render.loft`, change `use stdlib::styles::woodblock as ink` to `use self::ink::woodblock as ink`. Change a band threshold or the wash pigment in the copied `woodblock_pigment`. In the scene change `use stdlib::engines::woodblock` to `use self::render`. Use `loft show` to inspect the linked shader.

```sh
tools/gpu_lock.sh target/lab/loft still scenes/harbor/main.loft out/harbor.png --t 0
```

The engine's import chooses its ink. Another scene can continue to select the stock Woodblock engine. Act and Roam follow the same source rules. Their public motion helpers are separate from the render schedule.

## Migration aliases

The loader maps `std::geom` to geometry, frames and solid SDF exports, `std::light` to the world, emitters, sky, direct lighting and BSDF exports, and `std::noise` to value noise. `std::math`, `std::color` and `std::quant` retain their intrinsic spellings. For new source, call language intrinsics such as `dot4_i8` directly. The old `engines::realtime`, `engines::woodblock`, `engines::reference` and `engines::spectral_caustic` imports resolve to their named recipes under `stdlib/engines/`.

## Physical materials and styles

Physical transport evaluates the kind's `Bsdf` through realtime, RGB reference or spectral caustic transport. A kind can bind a module function `fn material(hit: SurfaceHit) -> Bsdf` with `material(hit: SurfaceHit) = material(hit)`. The query receives World position, normal and view, local anchor and UV. Pigment and emission are derived from the BSDF after the query, so its input has zeroes in those two fields. The point-and-normal query form remains available.

A style has signature `fn shade_style(hit: SurfaceHit, light: LightSummary) -> rgb`. The hit carries World position, normal and view, a local anchor, UV, pigment and emission. `LightSummary` carries `key_dir`, `key_vis`, `key_radiance`, `indirect_irradiance`, `ao`, `footprint_m`, `depth_m` and packed primary `id`. Use `footprint_m` to filter print marks on both tiled and path schedules. Fragment derivatives are available when the style runs in the forward fragment stage.

The realtime woodblock uses shadow-map visibility and cosine-convolved sky coefficients. Reference woodblock uses traced sun visibility and an irradiance estimate from continued paths. The path groups up to four primary coverage sites by row and averages incident light before applying ink. Spectral caustic transport supplies physical radiance and refuses a `LightSummary` style binding.

An entry schedule calls ordinary fragments with lexically bound functions. Parentheses mark a fragment, including a parameterless post fragment.

```loft
use stdlib::schedules::vis_buffer
use stdlib::integrators::direct as transport
use stdlib::lighting::print as ink
use stdlib::post::display

pub schedule WoodblockRealtime {
  orbit(at: [0, 0, 0], radius: 4, height: 0.5, fov: 40deg)
  vis_buffer::VisBuffer(transport::estimate, ink::shade_style)
  display::PrintPost()
}
```

The style and transport imports are independent. The `toon_engine.loft` example changes the ink import and the `restir_engine.loft` example changes the transport import. Forward and RGB reference woodblock use the same `lighting/print.loft` source. The legacy `Woodblock` and `Realtime` schedules retain their six-argument shade call and pass order.

### Run the examples

```sh
tools/dev.sh build loft-cli --fast
tools/gpu_lock.sh target/lab/loft render scenes/showcase/std1d/woodblock_realtime.loft --out out/woodblock.png --size 256x160 --set buoy_count=20
tools/gpu_lock.sh target/lab/loft render scenes/showcase/std1d/woodblock_forward.loft --out out/forward.png --size 256x160 --set buoy_count=20
tools/gpu_lock.sh target/lab/loft render scenes/showcase/std1d/woodblock_reference.loft --out out/reference.png --size 256x160 --set buoy_count=20 --samples 16
tools/gpu_lock.sh target/lab/loft render scenes/showcase/std1d/luminous.loft --out out/luminous.png --size 256x160 --set buoy_count=20
uv run python scenes/showcase/std1d/controls.py
```

Use `loft show SCENE --plan` to inspect the bound stage code. Add `--attachments out/control` to a beauty render to capture tightly packed little-endian `vis.rg32u`, `depth.f32` and `hdr.rgba16f`. Use `furnace.loft --engine reference_primary` for the physical white furnace and `area_control.loft` for the spherical light control.

Optics are scheduled in source. The luminous example draws forward and runs reflection and bloom before ACES. `StudioAO()` and `DepthOfField()` are selectable fragments in `post/optics.loft`. The optics use clamped gathers and share one extra HDR attachment. [Standard library design](../agent_notes/2026-11-20_v1/design/std1.md#module-interfaces) · [Tagged legacy source index](REFERENCE.md)

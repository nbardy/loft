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

The compiler finds `stdlib/` at the nearest workspace ancestor with `Cargo.toml`. Each import resolves a single `.loft` file. A `self::` path starts beside the file containing that import. A module reached through multiple paths contributes one set of declarations. Only the entry file's anonymous `build` blocks run. A scene with a complete schedule uses that schedule. Otherwise it selects a directly imported public engine schedule or uses Realtime when there is no selection. The compiler diagnoses conflicting selections and import cycles at their source sites. A `schedule` with parameters is a schedule function: a call in any schedule inlines its steps. The stock frame passes `cull_indirect`, `tile_classify`, `tonemap` and `tonemap_aces` are schedule functions in `core.loft`, built from `pass` steps that fill a WGSL template's holes (`dispatch(template, entry, Instances | Pixels | n, hole: value)`, `draw(template, vertex, fragment, hole: value)`). Copy one into a scene to change it.

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

## Contract for lane D

Lane D supplies the typed `Surface` and `Lighting` inputs, statically selected transport, style and preparation callbacks, and compiled schedule fragments for visibility, path, forward and post. The current Woodblock example uses the existing six-argument pigment function and pass list. See the proposed signatures and shared field layout. [Standard library design](../agent_notes/2026-11-20_v1/design/std1.md#module-interfaces) · [Tagged legacy source index](REFERENCE.md)

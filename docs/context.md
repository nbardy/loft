# Loft authoring context

Loft is a meta-engine. Edit the .loft source in `stdlib/engines/`, `stdlib/integrators/`, `stdlib/styles/` and `stdlib/motion/`. Stock schedules live in engine files. Put shared schedules in `stdlib/schedules/`. Own the renderer and motion model when the shot demands it.

1. **Scene.** Define `kind Name @Space (...)` with one instance per row. Use `build for` and `emit` to populate rows. Define MaterialTable rows when the schedule shares material data. Put reusable shape and pose logic in functions. Use `p3@S` for points, `v3@S` for offsets, `d3@S` for directions, `n3@S` for normals and `xf@A>B` for transforms. Only a point literal carries a space: `[x, y, z]@S`. Write offsets and per-axis scales space-less; they take the space of the point or offset beside them: `at + [0, 0.2, 0]`, `(p - at) * [1, 2, 1]`. The midpoint of two points is `mix(a, b, 0.5)`. Centred shapes take the offset from their centre, `sphere(p - c, r)` and `box_sdf(p - c, half)`; segment shapes take the point and two endpoint points, `capsule(p, a, b, r)` and `round_cone(p, a, b, ra, rb)`, so place endpoints from a point (`root + [x, y, z]`) rather than passing `p - root`. Space-less parameters such as `vnoise(p: rgb)` take points and offsets as plain coordinates. A field you divide by needs a range that excludes zero, as in `radius: f > 0`. Keep every surface finite: give a ground a `round_box`, not an endless plane. Arrays are `array[T,N]` and read-only row tables are `[K]`.
2. **Style and material.** Choose the BSDF in the kind's `material(p,n)` query. In `trace(p)` and `material(p,n)`, `p` is a point and `n` a normal in the kind's space (`p3@World`, `n3@World`); lobes and light callbacks take space-less `rgb` directions. A material of the form `fn material(hit) -> Bsdf` describes surface response. A style of the form `fn shade_style(hit,light)` interprets visibility and lighting for a look. Bind a callback with the selected pass's signature. The raster `shade_tiles` example below takes a BSDF, directions, shadow and AO. Lobes take only their authored arguments at a material call, as the catalog prints them. The renderer supplies the final `n,v,l`, so a lobe you declare ends with `n: rgb, v: rgb, l: rgb`. Stack lobes with `+` and blend two BSDFs with `mix_bsdf(a, b, t)`. Either side may be a `let`-bound Bsdf or a call to a helper that returns `Bsdf`, such as `mix_bsdf(shell, iris, t)` over `let shell = carapace(...)`. A material returns one fixed lobe stack: vary lobe arguments and `mix_bsdf` weights with `select`, not which lobes run. Scale a colour with `c * 0.5`; add a constant with `c + 0.1`.
3. **Rewrite a subsystem.** Use `loft fork engines/realtime my_engine.loft` and paste the printed import into a sibling scene. Change the schedule's pass order and inputs, write an integrator or replace the motion solver. Try forward shading, a visibility buffer, ReSTIR reservoir reuse or path tracing according to the shot. Write your own gait, spring or mood law and schedule it with `run_laws`. A law may write only its declared row fields. Laws write scalar fields: animate a scalar such as `phase` or `x` and offset the pose from it (`at + [x, 0, 0]`) rather than writing `at`.
4. **Author an engine.** When stock recipes miss the look or frame budget, build a `pass` graph with declared dependencies. Declare permissions on typed kernels with `allow(atomics, workgroup, subgroups)` as needed. Use `World` queries with hardware ray traversal on supporting backends. Use raw WGSL and native MSL escape hatches where the target supports them. Check the lowered graph and shader before capture.

Catalog names resolve without an import; `use stdlib::folder::module::{name}` or a qualified call states the module. `use self::module` loads a sibling. Comments start with `#`. Loop with `for i in 0..n { ... }`; there are no C-style loops and no loops over arrays. Loop indices are `u`; convert with `f32(i)` or `i32(i)`. Choose with `if c { a } else { b }` or `select(b, a, c)`; there is no `?:`. Separate two statements on one line with `;`. `let` binds once; declare `var` to reassign. The entry's `build` emits rows at load. A complete local schedule owns the frame.

This engine keeps raster visibility, skips the shadow map and writes stepped lighting. `cull_indirect` and `draw_indirect` take every drawn kind in one call, as in `cull_indirect(Shell, Claw)`. A fixed-tick law changes the beacon's tint.

```loft
use stdlib::sdf::solid::{sphere}
use stdlib::materials::bsdf::{lambert}
use stdlib::integrators::direct::{sun_sky}
use stdlib::sky::environment::{sky_color}

fn bands(b: Bsdf, n: rgb, v: rgb, sun: rgb, shadow: f, ao: f) -> rgb {
  let lit = sun_sky(b, n, v, sun, shadow, ao)
  floor(lit * 4.0) / 4.0
}

kind Beacon @World (at: p3@World, radius: m > 0m, phase: s = 0s) {
  trace(p) = sphere(p - at, radius)
  material(p, n) = lambert(mix([0.12, 0.22, 0.48], [0.9, 0.45, 0.1], fract(phase / 2s)))
}

law tick(h: Beacon, dt: s) writes h.{phase} {
  h.phase = h.phase + dt
}

build {
  build for i in 0..3 {
    emit Beacon(at: [f32(i) * 0.8 - 0.8, 0, 0]@World, radius: 0.3m)
  }
}

schedule Print {
  run_laws(tick)
  orbit(at: [0, 0, 0]@World, radius: 3m, height: 0.5m, period: 24s, fov: 40deg)
  cull_indirect(Beacon)
  render_bundle(Vis64) { draw_indirect(Beacon) }
  tile_classify(Vis64, tile: 8)
  shade_tiles(Vis64, light: bands, sun: [0.5, 0.75, 0.35])
  tonemap_aces(ColorHdr, sky: sky_color)
}
```

## Verify the picture and the cost

Run `loft check scene.loft` for types, bounds and scheduled checks. Run `loft lint scene.loft` for authoring diagnostics and `loft show scene.loft --plan` to inspect lowering. Capture `loft still scene.loft shot.png --t 0` and `loft clip scene.loft --out frames --size 512x512 --frames 16 --fps 16`. Run `loft profile scene.loft --size 512x512 --frames 60`: frame time means its warmed median and p95, never the one cold frame a `still` or `clip` reports. Its `--json` output gives the time per pass and a `contended` flag; repeat a contended run. Compare stills, motion and p95 at the same camera, times and resolution. When a surface looks wrong, add `--aov albedo` or `--aov normal` to `loft still` or `loft clip`: they show the base colour and the surface normal of the same frame without light, so you can name the channel that is wrong. The normal view is the normal the renderer hands to `material`; a tilt applied inside a lobe does not appear in it. That tilt, roughness and material masks have no view yet; return them as a flat `lambert` colour from a copy of the material to see them. Fork or replace the failing layer, then repeat.

Exit codes (§17) are 0 success, 1 compile, 2 truth, 3 picture/twin parity, 4 load, 5 usage/missing file, 6 device/environment and 7 budget.

## Five silent picture mistakes

- **Misplaced geometry.** Verify axes, pose direction and metre scale in a still. A map's mirror creates a fresh space. Use its point constructor and map back before mixing spaces.
- **Mismatched coverage.** Keep analytic `visible`, distance `trace` and shadow proxies in agreement. A negative analytic hit means a miss.
- **Wrong light space.** Keep the surface normal, view and light in the same frame. Keep RGB lighting linear until the display pass. Zero light direction requests emission from a lobe stack.
- **Plastic or clay surfaces.** `sun_sky` integrates the sky as twelve point samples and has no environment reflection. A glossy lobe narrower than about 0.6 rad shows each sample as its own highlight, which reads as moulded plastic, and flat water under it is nearly black. Keep lobes that answer the sky broad and let a sharp one answer the sun only. Removing every highlight is the opposite mistake: fur, cloth and skin read as matte clay unless their structure is in the shape and the normal.
- **Frozen motion.** Schedule each law and read its updated columns in the pose or material. Compare multiple times. Authored checks can measure pin drift or missed contacts.

Catalog headings give import paths. Schedule signatures show pass order.

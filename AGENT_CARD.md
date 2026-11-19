# Writing a Loft scene

Each row is an instance. Give its kind a space, declare the columns and put per-instance variation in those columns. A trace returns signed distance in metres. A material returns a BSDF. The mirror introduces a fresh space. Use `sym.pt` for a point in it and `sym.out` to return to the parent space.

```loft
space World Shell
use std::geom::*
use std::light::*
param rough: f = 0.35 in 0.05..1
fn chitin(p, n) = ggx([0.7, 0.22, 0.08], rough, 0)

kind ShellPart @Shell (pose: xf@Shell>World, radius: f > 0, phase: f = 0) {
  map sym(p) = p |> mirror_x
  trace(p) = sphere(sym.p - sym.pt(0.2, 0, 0), radius)
  material(p, n) = chitin(p, n)
}

law turn(h: ShellPart, dt) writes h.{phase} {
  h.phase = h.phase + dt
}

build {
  emit ShellPart(pose: place([0, 0, 0]@World), radius: 0.3)
  emit Sun(dir: dir([1, 2, 1]@World), color: [2, 2, 2])
  emit Sky(zenith: [0.2, 0.3, 0.5], horizon: [0.7, 0.8, 0.9])
}
```

Use `build for` to emit many rows. A positive column bound proves that a divisor stays above zero. Pure helpers infer argument types at their calls. Mark raw motion code `wgsl det (...) -> T { ... }`. Use `loft check` to see a diagnostic at the source line and `loft show` to read the emitted shader.

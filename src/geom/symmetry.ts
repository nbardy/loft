// Bilateral pairs and rows. `mirror` reflects a part across the z = 0 plane of its own coordinates (creatures here face
// +x with y up, so that is the sagittal plane), keeping it a true mirror image: a strand's spine and frame seed are
// reflected, its twist negated and its paint read at v -> 0.5 - v; a surface is reflected and read at u -> 1 - u, so
// its front (d/du x d/dv) still faces out. `array` makes named copies of a strand in a row of charts (ribs, legs, oars).

import { lift, neg, sub, time, vec3 } from "../expr.ts";
import { SurfaceDrawable, type Look } from "../draw/surface.ts";
import { framed } from "../map/surface.ts";
import { newSlices } from "../map/sweep.ts";
import type { Chart, End, P3, Surface, Topology } from "../map/types.ts";
import { Strand } from "./strand.ts";

/** Reflection across z = 0. */
export const reflect = (p: P3): P3 => vec3(p.x, p.y, neg(p.z));

function mirrorStrand(s: Strand, name: string): Strand {
  if (s.slice !== time) throw new Error(`mirror "${name}": mirror the live strand, then take it at another time`);
  const t = s.transport;
  const transport = { ...t, twist: (m: Parameters<typeof t.twist>[0]) => neg(t.twist(m)), up: reflect(t.up) };
  const paint = s.look.paint;
  return new Strand([name, (m) => reflect(s.spine(m)), s.section, transport, s.chart, time, newSlices(), `body_${name}`, { kind: "all" }], { ...s.look, paint: (m, v) => paint(m, sub(0.5, v)) });
}

const swapEnds = (u: Topology["u"]): Topology["u"] => (u === "periodic" ? u : [u[1], u[0]] as readonly [End, End]);

function mirrorSurface(s: Surface): Surface {
  const flip = (u: Parameters<Surface["point"]>[0]) => sub(1, u);
  return {
    point: (u, v) => reflect(s.point(flip(u), v)),
    normal: (u, v) => reflect(s.normal(flip(u), v)),
    anchor: (u, v) => reflect(s.anchor(flip(u), v)),
    topology: { u: swapEnds(s.topology.u), v: s.topology.v },
    skeleton: { joints: s.skeleton.joints, joint: (k) => { const j = s.skeleton.joint(k); return { p: reflect(j.p), r: j.r }; } },
  };
}

function mirrorLook(look: Look): Look {
  switch (look.kind) {
    case "lit": { const p = look.paint; return { ...look, paint: (u, v) => p(sub(1, u), v) }; }
    case "flat": { const p = look.paint; return { ...look, paint: (u, v) => p(sub(1, u), v) }; }
    case "glazed": { const c = look.colour; return { ...look, colour: (f) => c({ ...f, u: sub(1, f.u) }) }; }
  }
}

function mirrorDrawn(d: SurfaceDrawable, name: string): SurfaceDrawable {
  if (d.copies.kind !== "one") throw new Error(`mirror "${name}": mirror the instanced part and its points, not the instances`);
  return new SurfaceDrawable(name, mirrorSurface(d.surface), d.grid, mirrorLook(d.look), d.copies);
}

/** The mirror image of a part across z = 0 of its own coordinates, as a new part named `name`. */
export function mirror(part: Strand, name: string): Strand;
export function mirror(part: SurfaceDrawable, name: string): SurfaceDrawable;
export function mirror(part: Strand | SurfaceDrawable, name: string): Strand | SurfaceDrawable {
  if (part instanceof Strand) return mirrorStrand(part, name);
  if (part instanceof SurfaceDrawable) return mirrorDrawn(part, name);
  throw new Error(`mirror "${name}": can mirror a Strand or a SurfaceDrawable`);
}
mirror.surface = mirrorSurface;

/** A strand under another name (its table and shaders are keyed by name, so copies need their own). */
function renamed(s: Strand, name: string): Strand {
  return new Strand([name, s.spine, s.section, s.transport, s.chart, s.slice, newSlices(), `body_${name}`, { kind: "all" }], s.look);
}

/**
 * `count` copies of a part, copy i placed in `placement(i)` (rigid charts such as `on(host, [m, v, rho])`) and named
 * `<name>_<i>`. Each copy is its own drawable, so each may move differently; for many identical copies that ride
 * together, instance with `draw.points` instead (one draw, but copies cast no shadow).
 */
export function array(part: Strand, count: number, placement: (i: number) => Chart): Strand[];
export function array(part: SurfaceDrawable, count: number, placement: (i: number) => Chart): SurfaceDrawable[];
export function array(part: Strand | SurfaceDrawable, count: number, placement: (i: number) => Chart): (Strand | SurfaceDrawable)[] {
  if (!(Number.isInteger(count) && count >= 1)) throw new Error(`array "${part.name}": count must be a positive integer, got ${count}`);
  return Array.from({ length: count }, (_, i) => {
    const name = `${part.name}_${i}`;
    if (part instanceof Strand) return renamed(part, name).placed(placement(i));
    if (part instanceof SurfaceDrawable) {
      if (part.copies.kind !== "one") throw new Error(`array "${part.name}": cannot place copies of instances`);
      const g = placement(i).frame(lift<"vec3">([0, 0, 0]));
      return new SurfaceDrawable(name, framed(part.surface, { frame: g, scale: lift<"f32">(1) }), part.grid, part.look, part.copies);
    }
    throw new Error(`array "${name}": can copy a Strand or a SurfaceDrawable`);
  });
}


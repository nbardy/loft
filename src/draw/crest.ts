// Drawing a crest: a grid over its (x, f) coordinates, riding the host's stations, two-sided, inked along its edge.

import { variable } from "../expr.ts";
import type { Crest } from "../geom/crest.ts";
import { FRAME_WGSL } from "../geom/frame.ts";
import { FRAME_VARS } from "../geom/frame.ts";
import { emitFn } from "../compile/bind.ts";
import { draw, type CompileContext, type Emitted } from "../compile/context.ts";
import { lit } from "../compile/scene_wgsl.ts";

const COLS = 4;

export function crestDraws(ctx: CompileContext, c: Crest): Emitted {
  const r = ctx.stations(c.host);
  const key = `crest.${c.name}`;
  const X = variable("x", "f32"), F = variable("f", "f32");
  const pt = c.point(FRAME_VARS, X, F);
  const code = [
    `override ROWS: u32 = ${c.rows}u;`,
    `const COLS: u32 = ${COLS}u;`,
    emitFn(ctx.slots, `crest_${c.name}`, [["x", "f32"], ["f", "f32"]], "Surf", [pt.pos, pt.normal], (o) => `Surf(${o[0]}, ${o[1]})`, {
      pre: [`let st = station_at(${r.base}u, ${r.count}u, mix(${lit(c.from)}, ${lit(c.to)}, x));`],
      extra: FRAME_WGSL,
    }),
    emitFn(ctx.slots, `paint_${c.name}`, [["x", "f32"], ["f", "f32"]], "vec3<f32>", [c.paint(X, F)], (o) => o[0]),
    /* wgsl */ `
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) xf: vec2<f32>,
};
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VOut {
  let ij = vec2<u32>((vi / 6u) / (COLS - 1u), (vi / 6u) % (COLS - 1u)) + quad_corner(vi % 6u);
  let x = f32(ij.x) / f32(ROWS);
  let f = f32(ij.y) / f32(COLS - 1u);
  let s = crest_${c.name}(x, f);
  var out: VOut;
  out.clip = frame.view_proj * vec4<f32>(s.pos, 1.0);
  out.world = s.pos;
  out.normal = s.normal;
  out.xf = vec2<f32>(x, f);
  return out;
}
@fragment fn fs(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
  // A membrane has two faces: light the one being looked at.
  let n = select(-1.0, 1.0, front) * normalize(in.normal);
  let eye = -view_dir(in.world);
  let depth = sea_height(in.world.xz) - in.world.y;
  let base = paint_${c.name}(in.xf.x, in.xf.y);
  // Ink along the free edge, drawn in the crest's own coordinates: about two pixels wide at any distance.
  let fw = fwidth(in.xf.y);
  let edge = 1.0 - smoothstep(fw * 1.0, fw * 2.2, 1.0 - in.xf.y);
  return vec4<f32>(mix(shade(base, n, eye, in.world, depth), ink(base, n, eye, in.world, depth), edge), 1.0);
}`,
  ].join("\n");
  const vertices = 6 * c.rows * (COLS - 1);
  return { shaders: { [key]: code }, draws: [draw({ label: c.name, shader: key, vertices, cull: "none", constants: { ROWS: c.rows } })] };
}

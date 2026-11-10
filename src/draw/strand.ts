// Drawing a strand's skin: a grid over (m, v) generated in the vertex stage from the strand's chart, shaded by the
// style, plus the ink outline (the same surface pushed out along its normal, front faces culled). Scatter draws
// instances of the same grid through `bodyShader` with a per-instance placement.

import { variable } from "../expr.ts";
import type { Strand } from "../geom/strand.ts";
import { emitFn } from "../compile/bind.ts";
import { draw, type CompileContext, type DrawSpec, type Emitted } from "../compile/context.ts";

const M = variable("m", "f32"), V = variable("v", "f32");

/** Along the body, rings crowd toward the ends (2.5x denser there) where tips close fast. */
const BODY_M_WGSL = /* wgsl */ `
fn body_m(u: f32) -> f32 {
  return mix(u, 0.5 - 0.5 * cos(3.14159265 * u), 0.6);
}`;

export type BodyShader = {
  readonly key: string;
  readonly label: string;
  readonly strand: Strand;
  /** The chart function the grid is generated from. */
  readonly chart: string;
  /**
   * WGSL defining `fn body_place(ii: u32, pos: vec3<f32>, normal: vec3<f32>) -> Surf`: where instance `ii` puts a
   * point of the chart (identity for a plain strand).
   */
  readonly place: string;
  readonly instances: number;
};

const IDENTITY = /* wgsl */ `
fn body_place(ii: u32, pos: vec3<f32>, normal: vec3<f32>) -> Surf { return Surf(pos, normal); }`;

export function bodyShader(ctx: CompileContext, b: BodyShader): Emitted {
  const s = b.strand;
  const code = [
    `override NU: u32 = ${s.along}u;`,
    `override NV: u32 = ${s.around}u;`,
    `override INK: f32 = 0.0;`,
    BODY_M_WGSL, b.place,
    emitFn(ctx.slots, `paint_${s.name}`, [["m", "f32"], ["v", "f32"]], "vec3<f32>", [s.paint(M, V)], (o) => o[0]),
    /* wgsl */ `
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) mv: vec2<f32>,
};
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
  let ij = vec2<u32>((vi / 6u) / NV, (vi / 6u) % NV) + quad_corner(vi % 6u);
  let m = body_m(f32(ij.x) / f32(NU));
  // v starts on the dorsal line, so a ridge vertex exists at every density; it runs to 1.25 without wrapping so the
  // seam interpolates continuously (paint is periodic in v).
  let v = 0.25 + f32(ij.y) / f32(NV);
  let c = ${b.chart}(m, v, 1.0);
  let s = body_place(ii, c.pos, c.normal);
  let p = s.pos + s.normal * (INK * ink_width(s.pos));
  var out: VOut;
  out.clip = frame.view_proj * vec4<f32>(p, 1.0);
  out.world = p;
  out.normal = s.normal;
  out.mv = vec2<f32>(m, v);
  return out;
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let n = normalize(in.normal);
  let eye = -view_dir(in.world);
  let depth = sea_height(in.world.xz) - in.world.y;
  let base = paint_${s.name}(in.mv.x, in.mv.y);
  // INK is an override, so this branch is uniform (the style's shading takes screen-space derivatives), and the
  // outline pass no longer pays for the shading it used to compute and throw away.
  if (INK > 0.5) { return vec4<f32>(ink(base, n, eye, in.world, depth), 1.0); }
  return vec4<f32>(shade(base, n, eye, in.world, depth), 1.0);
}`,
  ].join("\n");
  const vertices = 6 * s.along * s.around;
  const constants = { NU: s.along, NV: s.around };
  const draws: DrawSpec[] = [
    draw({ label: b.label, shader: b.key, vertices, instances: b.instances, constants: { ...constants, INK: 0 } }),
    draw({ label: `${b.label} outline`, shader: b.key, vertices, instances: b.instances, cull: "front", constants: { ...constants, INK: 1 } }),
  ];
  return { shaders: { [b.key]: code }, draws };
}

export function strandDraws(ctx: CompileContext, s: Strand): Emitted {
  return bodyShader(ctx, { key: `strand.${s.name}`, label: s.name, strand: s, chart: ctx.chart(s).wgsl, place: IDENTITY, instances: 1 });
}

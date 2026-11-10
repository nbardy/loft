// Drawing a scatter: the thing's skin grid, instanced; instance i rides the host's chart at the scatter's `at(i)`.

import { variable, vec4 } from "../expr.ts";
import type { Scatter } from "../geom/scatter.ts";
import { emitFn } from "../compile/bind.ts";
import type { CompileContext, Emitted } from "../compile/context.ts";
import { bodyShader } from "./strand.ts";

export function scatterDraws(ctx: CompileContext, sc: Scatter): Emitted {
  const host = ctx.chart(sc.on).wgsl;
  const at = sc.at(variable("i", "f32"));
  const place = [
    emitFn(ctx.slots, `scatter_${sc.name}_at`, [["i", "f32"]], "vec4<f32>", [vec4(at.m, at.v, at.rho, at.scale)], (o) => o[0]),
    /* wgsl */ `
/// The host's chart where instance ii sits; the frame its copy is placed in.
fn scatter_${sc.name}_root(ii: u32) -> BodyPoint {
  let a = scatter_${sc.name}_at(f32(ii));
  return ${host}(a.x, a.y, a.z);
}
fn body_place(ii: u32, pos: vec3<f32>, normal: vec3<f32>) -> Surf {
  let k = scatter_${sc.name}_at(f32(ii)).w;
  let b = scatter_${sc.name}_root(ii);
  return Surf(b.pos + k * (b.x * pos.x + b.y * pos.y + b.z * pos.z), b.x * normal.x + b.y * normal.y + b.z * normal.z);
}`,
  ].join("\n");
  return bodyShader(ctx, { key: `scatter.${sc.name}`, label: sc.name, strand: sc.thing, chart: ctx.chart(sc.thing).wgsl, place, instances: sc.count });
}

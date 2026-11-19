import type { Source, Declarations } from "../record/passes.ts";
import { world, write, raster, screenDraw, type Scene } from "../render/record.ts";
import { albedoFill, lighting } from "../light/queries.ts";
import screen from "../render/covers/screen.wgsl" with { type: "text" };
import { SceneError } from "../errors.ts";

export const film = `fn present(c:vec3f)->vec4f {
  let mapped=c/(1+c);
  return vec4f(select(1.055*pow(mapped,vec3f(1/2.4))-0.055,12.92*mapped,mapped<=vec3f(0.0031308)),1);
}`;
export function resolve(present: string, average: boolean): Source {
  return {
    text:
      screen +
      present +
      `
@group(2) @binding(0) var input:texture_2d<f32>;
@fragment fn finish(@builtin(position) p:vec4f)->@location(0) vec4f {
  let c=textureLoad(input,vec2i(p.xy),0);
  return present(${average ? "c.rgb/c.a" : "c.rgb"});
}`,
    opens: [],
  };
}
export type Sampling = { samples?: number; depth?: number; present?: string };
export function sampling(o: Sampling) {
  const samples = o.samples ?? 1,
    depth = o.depth ?? 1;
  if (
    !Number.isInteger(samples) ||
    samples < 1 ||
    samples > 65536 ||
    !Number.isInteger(depth) ||
    depth < 0 ||
    depth > 64
  )
    throw new SceneError("path sampling: use 1..65536 samples and 0..64 bounces");
  return { samples, depth };
}
export const jittered = `fn primary(px:vec2f,k:u32)->Ray {
  let p=vec2u(px);
  return camera_ray(floor(px)+vec2f(random(p,k,0,8),random(p,k,0,9)));
}`;
export const reference =
  (o: Sampling & { primary?: string } = {}) =>
  (s: Scene): Declarations => {
    const { samples, depth } = sampling(o),
      w = world(s),
      l = lighting(s, w, 2);
    const path =
      (o.primary ?? jittered) +
      `
@group(3) @binding(0) var<uniform> iteration:vec4u;
@fragment fn path(@builtin(position) px:vec4f)->@location(0) vec4f {
  let k=iteration.x;let p=vec2u(px.xy);
  let ray=primary(px.xy,k);
  return vec4f(transport(trace(ray),-ray.d,p,k,${depth}u),1);
}`;
    return {
      tables: l.tables,
      modules: [l.module(path), resolve(o.present ?? film, true), ...(l.hasMetal ? [albedoFill] : [])],
      schedule: {
        images: [{ format: "Rgba32Float", samples: 1 }],
        passes: [
          raster("path", 0, [screenDraw("path")], [write({ Image: 0 }, 0, "Add")], [], samples),
          raster("resolve", 1, [screenDraw("finish")], [write("Output")], [{ Now: 0 }]),
        ],
      },
    };
  };

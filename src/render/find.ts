import boxes from "./covers/boxes.wgsl" with { type: "text" };
import type { Pass, Image, Source } from "../record/passes.ts";
import { world, raster, write } from "./record.ts";

export function find(w: ReturnType<typeof world>, samples: 1 | 4) {
  const images: Image[] = [
    { format: "Rgba16Uint", samples },
    { format: "Depth32Float", samples },
  ];
  const pass: Pass = raster(
    "find",
    0,
    [{ vertex: "boxes", fragment: "record", vertices: 36, instances: { Rows: 0 }, cull: "Front" }],
    [write({ Image: 0 }), write({ Image: 1 }, 1)],
  );
  return { images, module: w.cover(boxes), pass };
}

/** The record owns coverage and depth. A reading supplies the normal and chart at each distinct covered thing. */
export function shadePixel(samples: 1 | 4, shade: string, background: string): string {
  const ms = samples === 4 ? "_multisampled" : "";
  return `
@group(2) @binding(0) var ids:texture${ms}_2d<u32>;
@group(2) @binding(1) var depths:texture_depth${ms}_2d;
${shade}
${background}
@fragment fn shade_pixel(@builtin(position) px:vec4f)->@location(0) vec4f {
  let ray=camera_ray(px.xy); let p=vec2i(px.xy);
  var tags:array<u32,${samples}>;var counts:array<u32,${samples}>;var zs:array<f32,${samples}>;var used=0u;
  for(var s=0;s<${samples};s++){
    let id=textureLoad(ids,p,${samples === 1 ? "0" : "s"});let tag=id.x|(id.y<<16u);
    let z=textureLoad(depths,p,${samples === 1 ? "0" : "s"});
    var slot=0u;while slot<used && tags[slot]!=tag {slot++;}
    if slot==used {tags[slot]=tag;zs[slot]=z;used++;}
    counts[slot]++;
  }
  var color=vec4f(0);var nearest=1e30;
  for(var j=0u;j<used;j++) {
    let tag=tags[j];var c:vec4f;
    if tag==0u { c=background(ray); }
    else {
      let i=(tag&0x7fffffffu)-1u;
      let p3=unproject(px.xy,zs[j]);let t=dot(p3-ray.o,ray.d);nearest=min(nearest,t);
      let h=intersect(i,ray);
      // Device depth loses precision with distance. Launch secondary rays from the analytic surface.
      let surface=select(p3,ray.o+ray.d*h.t,h.t>=0);
      c=shade(Point(surface,h.n,h.uv,t,i),-ray.d,(tag&0x80000000u)!=0u,px.xy);
    }
    color+=c*f32(counts[j]);
  }
  return finish(color/f32(${samples}),nearest);
}`;
}

export const identityFinish = "fn finish(c:vec4f,t:f32)->vec4f{return c;}";

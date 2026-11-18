// mistake: a misspelt name in a raw WGSL function (u32 bit operations put it outside the text subset: only the GPU reads it)
import { draw, scene, wgsl } from "../../../src/index.ts";
import { ball, cameras, grid, style, water } from "./_parts.ts";

const fx = wgsl`
fn grain(u: f32) -> f32 {
  var h = u32(u * 4096.0);
  h = h ^ (h >> 16u);
  return f32(hsh & 255u) / 255.0;
}
fn clay(u: f32, v: f32) -> vec3f { return vec3(0.9, 0.4, 0.2) * (0.8 + 0.2 * grain(u)); }`;

export default scene({ ground: water, things: [draw.surface("ball", ball, { paint: fx.clay, grid, sides: "one", outline: "hull" })], style, sun: [0.3, 0.8, 0.4], cameras });

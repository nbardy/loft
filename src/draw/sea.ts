// Drawing the sea: a grid that grows away from where the view meets the water, shaded per pixel by the style.
//
// Foam where the water meets a body is found from the opaque scene's depth (`sceneDepth`): a sea pixel is foamy when
// some opaque surface lies within the foam width of it in 3D. A ring of taps around the pixel, at the foam width's
// size on screen, finds the body on either side of the waterline (behind the water and above it), so any opaque thing
// gets its waterline foam, whatever kind it is, at a cost per sea pixel that does not grow with the scene.

import type { Sea } from "../chart.ts";
import { draw, type CompileContext, type Emitted } from "../compile/context.ts";
import { lit } from "../compile/scene_wgsl.ts";

/** Growth of the sea grid's cells away from its centre: offset = reach (e^(K|u|) - 1) / (e^K - 1). */
const SEA_GROWTH = 6;
/**
 * Tap offsets, in units of the band's width on screen: 8 round the outer edge, then 8 at 0.55 turned by half a step,
 * so the nearest-surface estimate has 16 directions (with 8 the foam's outer edge showed an 8-pointed star of streaks).
 */
const RING = Array.from({ length: 16 }, (_, k) => {
  const a = (k * Math.PI) / 4 + (k >= 8 ? Math.PI / 8 : 0), r = k >= 8 ? 0.55 : 1;
  return [r * Math.cos(a), r * Math.sin(a)];
});

export function seaDraws(_ctx: CompileContext, sea: Sea): Emitted {
  const code = /* wgsl */ `
override N: u32 = ${sea.grid}u;
const FOAM_WIDTH: f32 = ${lit(sea.foam)};
const RING = array<vec2<f32>, 16>(${RING.map(([x, y]) => `vec2<f32>(${lit(+x.toFixed(6))}, ${lit(+y.toFixed(6))})`).join(", ")});
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
};
fn grid_offset(u: f32) -> f32 {
  return sign(u) * ${lit(sea.reach)} * (exp(${lit(SEA_GROWTH)} * abs(u)) - 1.0) / ${lit(Math.exp(SEA_GROWTH) - 1)};
}
/// Where the sea grid is centred: where the view centre meets the water (or ahead of the eye), snapped to 2 units.
fn sea_centre() -> vec2<f32> {
  let f = frame.forward.xyz;
  let e = frame.eye.xyz;
  let t = select(60.0, clamp(-e.y / f.y, 0.0, 60.0), f.y < -0.01);
  return floor((e.xz + f.xz * t) / 2.0) * 2.0;
}
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VOut {
  let ij = vec2<u32>((vi / 6u) / N, (vi / 6u) % N) + quad_corner(vi % 6u);
  let u = vec2<f32>(ij) / f32(N) * 2.0 - 1.0;
  let xz = sea_centre() + vec2<f32>(grid_offset(u.x), grid_offset(u.y));
  let p = vec3<f32>(xz.x, sea_height(xz), xz.y);
  var out: VOut;
  out.clip = frame.view_proj * vec4<f32>(p, 1.0);
  out.world = p;
  return out;
}
/// Distance from p to the nearest opaque surface seen at pixel q.
fn opaque_gap(p: vec3<f32>, q: vec2<f32>) -> f32 {
  return distance(scene_position(q), p);
}
/// Foam where the water at p (seen at pixel px) meets an opaque body: 0..1, antialiased by the pixel footprint fp.
fn contact_foam(p: vec3<f32>, px: vec2<f32>, fp: f32) -> f32 {
  // The band's width on screen; at least a pixel (far away), at most 40 (a body filling the view).
  let r = clamp(FOAM_WIDTH / pixel_world(p), 1.0, 40.0);
  // The ring turns from pixel to pixel (interleaved gradient noise, fixed in screen space): a fixed ring overestimates
  // the gap between its directions and drew radial streaks out of the band's edge; a turning one leaves fine grain.
  let turn = 6.2831853 * fract(52.9829189 * fract(dot(floor(px), vec2<f32>(0.06711056, 0.00583715))));
  let rot = mat2x2<f32>(cos(turn), sin(turn), -sin(turn), cos(turn));
  var gap = opaque_gap(p, px);
  for (var k = 0u; k < 8u; k++) { gap = min(gap, opaque_gap(p, px + rot * RING[k] * r)); }
  // Only near a body is the inner ring worth reading.
  if (gap < 2.0 * FOAM_WIDTH) {
    for (var k = 8u; k < 16u; k++) { gap = min(gap, opaque_gap(p, px + rot * RING[k] * r)); }
  }
  // Full at the body, gone at the foam width; the threshold wobbles slowly so the edge reads as lapping foam.
  let t = frame.time.x;
  let wobble = 0.12 * sin(p.x * 2.3 + t * 1.9) * sin(p.z * 2.9 - t * 1.4);
  let band = 1.0 - gap / FOAM_WIDTH;
  let aa = max(fp / FOAM_WIDTH, 1e-3);
  let core = smoothstep(0.42 + wobble - aa, 0.42 + wobble + aa, band);
  let fringe = smoothstep(0.12 + wobble - aa, 0.12 + wobble + aa, band);
  return core * 0.85 + fringe * 0.15;
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let xz = in.world.xz;
  let fp = sea_footprint(in.world);
  // Where a pixel covers a good part of the shortest wave, its slope and crest shading can only alias (bands of
  // stripes toward the horizon): fade that detail out by the pixel's footprint.
  let detail = 1.0 - smoothstep(0.04, 0.2, fp / SEA_SHORTEST);
  let g = sea_gradient(xz) * detail;
  let n = normalize(vec3<f32>(-g.x, 1.0, -g.y));
  let eye = -view_dir(in.world);
  // Per pixel, not the interpolated vertex height: far grid cells are large and their edges would show.
  let crest = sea_height(xz) / SEA_AMPLITUDE * detail;
  return water(n, eye, in.world, crest, distance(in.world, frame.eye.xyz), contact_foam(in.world, in.clip.xy, fp));
}`;
  const vertices = 6 * sea.grid * sea.grid;
  return {
    shaders: { sea: code },
    // Transparent: what shows through the water is the submerged body (tinted by its own shader) or, where nothing
    // is below, the sky shader's under-horizon colour.
    draws: [draw({ label: "sea", shader: "sea", vertices, phase: "water", cull: "none", depthWrite: false, blend: "alpha", sceneDepth: true, constants: { N: sea.grid } })],
  };
}

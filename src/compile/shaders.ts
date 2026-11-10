// WGSL generation. Hand-written WGSL is limited to plumbing (vertex index to grid coordinates, table reads, decal
// loops); every formula that describes the scene comes from the expression graph through `wgslFn`, so the CPU and
// the GPU evaluate the same description.

import type { Draw } from "../artefact.ts";
import { lift, variable } from "../expr.ts";
import type { Sea } from "../chart.ts";
import type { Crest } from "../geom/crest.ts";
import { STATIONS_WGSL } from "../geom/stations.ts";
import { FRAME_VARS, FRAME_WGSL, type Strand } from "../geom/strand.ts";
import type { Style } from "../style.ts";
import { wgslFn, type Slots } from "./bind.ts";
import { draw, type CompileContext, type Emitted, type StationRange } from "./context.ts";

const M = variable("m", "f32"), V = variable("v", "f32");
const lit = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));
const vec3Lit = (v: readonly number[]): string => `vec3<f32>(${v.map(lit).join(", ")})`;

// ---- scene-wide WGSL -------------------------------------------------------------------------------------------

export const TABLES = { stations: 0, collars: 1, shadows: 2 } as const;

/** Bind group 1, shared types and functions, and the style and sea, emitted once for every shader. */
export function sceneWgsl(slots: Slots, style: Style, sea: Sea, sun: readonly [number, number, number]): string {
  const c = <T extends "f32" | "vec3">(name: string, ty: T) => variable(`c_${name}`, ty);
  const shadeCtx = { base: c("base", "vec3"), normal: c("normal", "vec3"), sun: lift<"vec3">(sun), view: c("view", "vec3"), world: c("world", "vec3"), depth: c("depth", "f32") };
  const shadeArgs = [["c_base", "vec3"], ["c_normal", "vec3"], ["c_view", "vec3"], ["c_world", "vec3"], ["c_depth", "f32"]] as const;
  const waterCtx = {
    normal: c("normal", "vec3"), sun: lift<"vec3">(sun), view: c("view", "vec3"), world: c("world", "vec3"),
    crest: c("crest", "f32"), distance: c("distance", "f32"), foam: c("foam", "f32"), shadow: c("shadow", "f32"),
  };
  const waterArgs = [["c_normal", "vec3"], ["c_view", "vec3"], ["c_world", "vec3"], ["c_crest", "f32"], ["c_distance", "f32"], ["c_foam", "f32"], ["c_shadow", "f32"]] as const;
  const xz = variable("xz", "vec2");
  const amplitude = sea.waves.reduce((a, w) => a + w.amp, 0);
  return [
    `@group(1) @binding(${TABLES.stations}) var<storage, read> stations: array<vec4<f32>>;`,
    `@group(1) @binding(${TABLES.collars}) var<storage, read> collars: array<vec4<f32>>;`,
    `@group(1) @binding(${TABLES.shadows}) var<storage, read> shadows: array<vec4<f32>>;`,
    `const SUN: vec3<f32> = ${vec3Lit(sun)};`,
    `const SEA_AMPLITUDE: f32 = ${lit(Math.max(amplitude, 1e-3))};`,
    `const SEA_SHORTEST: f32 = ${lit(sea.waves.reduce((l, w) => Math.min(l, (2 * Math.PI) / w.k), 1e3))};`,
    `struct Surf { pos: vec3<f32>, normal: vec3<f32> };`,
    STATIONS_WGSL,
    wgslFn(slots, "sea_height", [["xz", "vec2"]], "f32", [sea.height(xz)], (v) => v[0]),
    wgslFn(slots, "sea_gradient", [["xz", "vec2"]], "vec2<f32>", [sea.gradient(xz)], (v) => v[0]),
    wgslFn(slots, "shade", shadeArgs, "vec3<f32>", [style.shade(shadeCtx)], (v) => v[0]),
    wgslFn(slots, "ink", shadeArgs, "vec3<f32>", [style.ink(shadeCtx)], (v) => v[0]),
    wgslFn(slots, "water", waterArgs, "vec4<f32>", [style.water(waterCtx)], (v) => v[0]),
    wgslFn(slots, "sky", [["c_dir", "vec3"]], "vec3<f32>", [style.sky({ dir: c("dir", "vec3"), sun: lift<"vec3">(sun) })], (v) => v[0]),
    wgslFn(slots, "outline_world", [], "f32", [style.outline], (v) => v[0]),
    INK_WIDTH_WGSL,
    DECALS_WGSL,
  ].join("\n");
}

/** The outline's offset: the style's world width, but never thinner than 1.5 pixels (so far views keep their ink). */
const INK_WIDTH_WGSL = /* wgsl */ `
fn ink_width(p: vec3<f32>) -> f32 {
  let clip = frame.view_proj * vec4<f32>(p, 1.0);
  // Row 1 of view_proj is the projection's y scale times a unit row of the view rotation (perspective and
  // orthographic alike), so its length is that scale. One pixel at depth w is then 2 w / (height * scale).
  let fy = length(vec3<f32>(frame.view_proj[0][1], frame.view_proj[1][1], frame.view_proj[2][1]));
  let px = 2.0 * clip.w / (frame.viewport.y * fy);
  return max(outline_world(), 1.5 * px);
}`;

/**
 * Pixel-time decals on the sea, from the tables the pose pass writes. Each table starts with a header vec4 whose x is
 * the entry count; entries are two vec4s.
 *   collars: (centre xz, radius along, radius across), (along direction xz, foam width, phase)
 *   shadows: (end a xz, end b xz), (radius a, radius b, strength, softness)
 * `px` is the pixel's footprint in world units (from screen-space derivatives, taken by the caller in uniform
 * control flow), used to antialias every edge.
 */
const DECALS_WGSL = /* wgsl */ `
fn foam_at(p: vec2<f32>, px: f32) -> f32 {
  let count = u32(collars[0].x);
  if (count == 0u) { return 0.0; }
  var d = 1e9;
  var w = 0.0;
  for (var i = 0u; i < count; i++) {
    let a = collars[1u + 2u * i];
    let b = collars[2u + 2u * i];
    let q = p - a.xy;
    let local = vec2<f32>(dot(q, b.xy), dot(q, vec2<f32>(-b.y, b.x)));
    let sd = (length(local / a.zw) - 1.0) * min(a.z, a.w);
    // Smooth union of the ellipses, so a run of cuts reads as one waterline loop without creases.
    let k = 0.25;
    let h = clamp(0.5 + 0.5 * (d - sd) / k, 0.0, 1.0);
    d = mix(d, sd, h) - k * h * (1.0 - h);
    w = mix(w, b.z, h);
  }
  // A band round the waterline: full at the line, gone at w outside it and at 0.45 w inside it. The threshold wobbles
  // slowly round the loop so the edge reads as lapping foam, and both edges are antialiased by the pixel footprint.
  let t = frame.time.x;
  let wobble = 0.12 * sin(p.x * 2.3 + t * 1.9) * sin(p.y * 2.9 - t * 1.4);
  let band = smoothstep(-0.45 * w, 0.0, d) * (1.0 - smoothstep(0.0, w, d));
  let aa = max(px / max(w, 1e-3), 1e-3);
  let core = smoothstep(0.42 + wobble - aa, 0.42 + wobble + aa, band);
  let fringe = smoothstep(0.12 + wobble - aa, 0.12 + wobble + aa, band);
  return core * 0.85 + fringe * 0.15;
}

fn shadow_at(p: vec2<f32>, px: f32) -> f32 {
  let count = u32(shadows[0].x);
  var s = 0.0;
  for (var i = 0u; i < count; i++) {
    let a = shadows[1u + 2u * i];
    let b = shadows[2u + 2u * i];
    let pa = p - a.xy;
    let ba = a.zw - a.xy;
    let h = clamp(dot(pa, ba) / max(dot(ba, ba), 1e-8), 0.0, 1.0);
    let dist = length(pa - ba * h);
    let r = mix(b.x, b.y, h);
    let soft = max(b.w, px);
    s = max(s, b.z * (1.0 - smoothstep(r - soft, r + soft, dist)));
  }
  return s;
}

/** Where the sea grid is centred: where the view centre meets the water (or ahead of the eye), snapped to 2 units. */
fn sea_centre() -> vec2<f32> {
  let f = frame.forward.xyz;
  let e = frame.eye.xyz;
  let t = select(60.0, clamp(-e.y / f.y, 0.0, 60.0), f.y < -0.01);
  return floor((e.xz + f.xz * t) / 2.0) * 2.0;
}`;

/** Grid corner offsets of the six vertices of a quad: triangles (0,0)(1,0)(1,1) and (0,0)(1,1)(0,1). */
const QUAD_WGSL = /* wgsl */ `
fn quad_corner(c: u32) -> vec2<u32> {
  return vec2<u32>(select(0u, 1u, c == 1u || c == 2u || c == 4u), select(0u, 1u, c == 2u || c == 4u || c == 5u));
}`;

// ---- strands ---------------------------------------------------------------------------------------------------

/** `surface_<name>(m, v) -> Surf`: the strand's skin at body coordinates, posed. Also used by the crest and the tests. */
export function strandSurfaceWgsl(slots: Slots, s: Strand, r: StationRange): string {
  const surf = s.surface(FRAME_VARS, M, V);
  return wgslFn(slots, `surface_${s.name}`, [["m", "f32"], ["v", "f32"]], "Surf", [surf.pos, surf.normal], (o) => `Surf(${o[0]}, ${o[1]})`, {
    pre: [`let st = station_at(${r.base}u, ${r.count}u, m);`],
    extra: FRAME_WGSL,
  });
}

/** Along the body, rings crowd toward the ends (2.5x denser there) where tips close fast. */
const BODY_M_WGSL = /* wgsl */ `
fn body_m(u: f32) -> f32 {
  return mix(u, 0.5 - 0.5 * cos(3.14159265 * u), 0.6);
}`;

export function strandDraws(ctx: CompileContext, s: Strand): Emitted {
  const r = ctx.stations(s);
  const key = `strand.${s.name}`;
  const code = [
    `override NU: u32 = ${s.along}u;`,
    `override NV: u32 = ${s.around}u;`,
    `override INK: f32 = 0.0;`,
    QUAD_WGSL, BODY_M_WGSL,
    strandSurfaceWgsl(ctx.slots, s, r),
    wgslFn(ctx.slots, `paint_${s.name}`, [["m", "f32"], ["v", "f32"]], "vec3<f32>", [s.paint(M, V)], (o) => o[0]),
    /* wgsl */ `
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) mv: vec2<f32>,
};
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VOut {
  let ij = vec2<u32>((vi / 6u) / NV, (vi / 6u) % NV) + quad_corner(vi % 6u);
  let m = body_m(f32(ij.x) / f32(NU));
  // v starts on the dorsal line, so a ridge vertex exists at every density; it runs to 1.25 without wrapping so the
  // seam interpolates continuously (paint is periodic in v).
  let v = 0.25 + f32(ij.y) / f32(NV);
  let s = surface_${s.name}(m, v);
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
  return vec4<f32>(select(shade(base, n, eye, in.world, depth), ink(base, n, eye, in.world, depth), INK > 0.5), 1.0);
}`,
  ].join("\n");
  const vertices = 6 * s.along * s.around;
  const constants = { NU: s.along, NV: s.around };
  const draws: Draw[] = [
    draw({ label: `${s.name}`, shader: key, vertices, constants: { ...constants, INK: 0 } }),
    // The ink outline: the same surface pushed out along its normal, front faces culled, so only the rim of the hull
    // shows behind the body.
    draw({ label: `${s.name} outline`, shader: key, vertices, cull: "front", constants: { ...constants, INK: 1 } }),
  ];
  return { shaders: { [key]: code }, draws };
}

// ---- crests ----------------------------------------------------------------------------------------------------

const CREST_COLS = 4;

export function crestDraws(ctx: CompileContext, c: Crest): Emitted {
  const r = ctx.stations(c.host);
  const key = `crest.${c.name}`;
  const X = variable("x", "f32"), F = variable("f", "f32");
  const pt = c.point(FRAME_VARS, X, F);
  const code = [
    `override ROWS: u32 = ${c.rows}u;`,
    `const COLS: u32 = ${CREST_COLS}u;`,
    QUAD_WGSL,
    wgslFn(ctx.slots, `crest_${c.name}`, [["x", "f32"], ["f", "f32"]], "Surf", [pt.pos, pt.normal], (o) => `Surf(${o[0]}, ${o[1]})`, {
      pre: [`let st = station_at(${r.base}u, ${r.count}u, mix(${lit(c.from)}, ${lit(c.to)}, x));`],
      extra: FRAME_WGSL,
    }),
    wgslFn(ctx.slots, `paint_${c.name}`, [["x", "f32"], ["f", "f32"]], "vec3<f32>", [c.paint(X, F)], (o) => o[0]),
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
  const vertices = 6 * c.rows * (CREST_COLS - 1);
  return { shaders: { [key]: code }, draws: [draw({ label: c.name, shader: key, vertices, cull: "none", constants: { ROWS: c.rows } })] };
}

// ---- the sea and the sky ---------------------------------------------------------------------------------------

/** Growth of the sea grid's cells away from its centre: offset = reach (e^(K|u|) - 1) / (e^K - 1). */
const SEA_GROWTH = 6;

export function seaDraws(sea: Sea): Emitted {
  const code = /* wgsl */ `
override N: u32 = ${sea.grid}u;
${QUAD_WGSL}
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
};
fn grid_offset(u: f32) -> f32 {
  return sign(u) * ${lit(sea.reach)} * (exp(${lit(SEA_GROWTH)} * abs(u)) - 1.0) / ${lit(Math.exp(SEA_GROWTH) - 1)};
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
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let xz = in.world.xz;
  let px = length(fwidth(xz));
  // Where a pixel covers a good part of the shortest wave, its slope and crest shading can only alias (bands of
  // stripes toward the horizon): fade that detail out by the pixel's footprint.
  let detail = 1.0 - smoothstep(0.04, 0.2, px / SEA_SHORTEST);
  let g = sea_gradient(xz) * detail;
  let n = normalize(vec3<f32>(-g.x, 1.0, -g.y));
  let eye = -view_dir(in.world);
  // Per pixel, not the interpolated vertex height: far grid cells are large and their edges would show.
  let crest = sea_height(xz) / SEA_AMPLITUDE * detail;
  return water(n, eye, in.world, crest, distance(in.world, frame.eye.xyz), foam_at(xz, px), shadow_at(xz, px));
}`;
  const vertices = 6 * sea.grid * sea.grid;
  return {
    shaders: { sea: code },
    // Transparent: what shows through the water is the submerged body (tinted by its own shader) or, where nothing
    // is below, the sky shader's under-horizon colour.
    draws: [draw({ label: "sea", shader: "sea", vertices, pass: "transparent", cull: "none", depthWrite: false, blend: "alpha", constants: { N: sea.grid } })],
  };
}

export function skyDraws(): Emitted {
  const code = /* wgsl */ `
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) ndc: vec2<f32>,
};
@vertex fn vs(@builtin(vertex_index) vi: u32) -> VOut {
  let xy = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u)) * 2.0 - 1.0;
  var out: VOut;
  // Just inside the far plane, with no depth write: everything drawn later is nearer.
  out.clip = vec4<f32>(xy, 0.99999, 1.0);
  out.ndc = xy;
  return out;
}
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let near = frame.inv_view_proj * vec4<f32>(in.ndc, 0.0, 1.0);
  let far = frame.inv_view_proj * vec4<f32>(in.ndc, 1.0, 1.0);
  let dir = normalize(far.xyz / far.w - near.xyz / near.w);
  return vec4<f32>(sky(dir), 1.0);
}`;
  return { shaders: { sky: code }, draws: [draw({ label: "sky", shader: "sky", vertices: 3, cull: "none", depthWrite: false })] };
}

// draw.surface: one drawable for every `Surface` (DESIGN.md 7.4). A grid of `grid.u.count x grid.v.count` cells is
// evaluated in the vertex stage from the surface's own expressions (point and normal), optionally once per instance
// of a set of `Points`; the fragment stage evaluates the look per pixel. The surface coordinates (u, v) reach the
// fragment stage, so paint is a function of the surface's coordinates (body coordinates for a body).

import { add, cos, f, mix, mul, variable, vec2, vec3, vec4, type Expr } from "../expr.ts";
import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type Bakeable, type CompileContext, type DetailSpec, type Drawable, type DrawSpec, type Emitted, type Phase, type Shader, type V3 } from "../compile/context.ts";
import { boundTable, gridDetail } from "./grid_detail.ts";
import { carry, rotate } from "../map/chart.ts";
import { framed } from "../map/surface.ts";
import type { Chart, F, P3, Paint, Points, Skeleton, Surface } from "../map/types.ts";
import { readsBody, type Style } from "../style.ts";
import { lit } from "../compile/scene_wgsl.ts";

/** How a grid direction is sampled: `count` cells, the cell corner at s in [0, 1] going to the coordinate `at(s)`. */
export type Sampling = { readonly count: number; readonly at: (s: F) => F };
export type Grid = { readonly u: Sampling; readonly v: Sampling };

/** Ways to sample [0, 1]. Sampling is a reparametrisation (a warp of the domain): it moves vertices, not paint. */
export const spacing = {
  even: (count: number): Sampling => ({ count, at: (s) => s }),
  /** Crowded toward both ends (2.5x denser there at the default), where tips close fast. */
  ends: (count: number, crowd = 0.6): Sampling => ({ count, at: (s) => mix(s, mul(cos(mul(s, Math.PI)), -0.5).add(0.5), crowd) }),
  /** Even, from `start` to `start + 1` without wrapping (round a tube: a vertex on the line `start`, a seam that interpolates continuously). */
  from: (count: number, start: number): Sampling => ({ count, at: (s) => s.add(start) }),
};

/** What a glazed surface's colour may read in the fragment stage. */
export type Fragment = {
  readonly world: P3;
  /** Pixel position (`@builtin(position).xy`). */
  readonly px: Expr<"vec2">;
  readonly u: F;
  readonly v: F;
  readonly style: Style;
  readonly sun: V3;
};

/** Ink: the surface pushed out along its normal and drawn back faces only (`hull`), ink along its v = 1 edge (`border`, for sheets), or none. */
export type Outline = "hull" | "border" | "none";

export type Look =
  /** Base colour from `paint`, lit by the scene's style (with underwater tint), opaque. */
  | { readonly kind: "lit"; readonly paint: Paint; readonly sides: "one" | "two"; readonly outline: Outline }
  /** `paint` as the final colour, unlit, opaque: strokes and ink lines. */
  | { readonly kind: "flat"; readonly paint: Paint }
  /** Its own colour and alpha, blended over what is behind it (the sea): no depth write. */
  | { readonly kind: "glazed"; readonly colour: (f: Fragment) => Expr<"vec4">; readonly phase: Extract<Phase, "water" | "on-water" | "transparent">; readonly sceneDepth: boolean; readonly detail: GlazedDetail };

/** A glazed surface's grid: as authored, or laid over the screen by the host (the sea). */
export type GlazedDetail = { readonly kind: "fixed" } | Extract<DetailSpec, { kind: "projected" }>;

/** One copy, or one per point. */
type Copies = { readonly kind: "one" } | { readonly kind: "points"; readonly points: Points };

const S = variable("s", "f32"), T = variable("t", "f32"), I = variable("i", "f32");
const U = variable("u", "f32"), V = variable("v", "f32");

export class SurfaceDrawable implements Drawable {
  readonly skeletons: readonly Skeleton[];
  readonly surfaces: readonly Bakeable[];

  constructor(
    readonly name: string,
    readonly surface: Surface,
    readonly grid: Grid,
    readonly look: Look,
    readonly copies: Copies,
  ) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new Error(`draw.surface "${name}": names must be identifiers (they name shaders)`);
    for (const [axis, s] of [["u", grid.u], ["v", grid.v]] as const) {
      if (!(Number.isInteger(s.count) && s.count >= 1)) throw new Error(`draw.surface "${name}": grid.${axis}.count must be a positive integer, got ${s.count}`);
    }
    if (look.kind === "lit" && look.sides === "two" && look.outline === "hull") throw new Error(`draw.surface "${name}": a two-sided surface has no back faces to draw a hull outline with; use "border"`);
    this.skeletons = skeletonsOf(surface, copies);
    this.surfaces = bakeablesOf(this, copies);
  }

  /** The same surface and look, one copy per point. */
  instanced(name: string, points: Points): SurfaceDrawable {
    return new SurfaceDrawable(name, this.surface, this.grid, this.look, { kind: "points", points });
  }

  /**
   * This drawable carried through a chart, for parts of something that is itself being placed there whole (the
   * serpent's fins and teeth, coiled into a whirlpool with it). Copies keep their shape: each copy's frame is carried
   * (placement acts on the skeleton). A single surface has no skeleton to carry, so its points are mapped through the
   * chart and its normals turned with the chart's frame: right for thin things riding a placed body (a fin), which
   * the chart bends with that body; a solid would be sheared (DESIGN.md 7.6), so place a body instead.
   */
  through(chart: Chart): SurfaceDrawable {
    return new SurfaceDrawable(this.name, carriedSurface(this.surface, this.copies, chart), this.grid, this.look, carriedCopies(this.copies, chart));
  }

  draws(ctx: CompileContext): Emitted {
    const key = `surface.${this.name}`;
    const vertex = vertexFn(ctx, this, `surf_${this.name}`);
    const frag = fragment(ctx, this);
    const instances = countOf(this.copies);
    const NU = this.grid.u.count, NV = this.grid.v.count;
    const detail = detailOf(ctx, this);
    const code = [
      cellsWgsl(detail, NU, NV),
      frag.overrides,
      `const ID: f32 = ${lit(nameId(this.name))};`,
      vertex.code, frag.fns.code,
      /* wgsl */ `
struct VOut {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
  @location(2) uv: vec2<f32>,
  @location(3) anchor: vec3<f32>,
  @location(4) @interpolate(flat) id: f32,
};
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
  let n = grid_cells(ii);
  let ij = vec2<u32>((vi / 6u) / n.y, (vi / 6u) % n.y) + quad_corner(vi % 6u);
  let s = surf_${this.name}(f32(ij.x) / f32(n.x), f32(ij.y) / f32(n.y), f32(ii));
  let p = ${frag.push};
  var out: VOut;
  out.clip = frame.view_proj * vec4<f32>(p, 1.0);
  out.world = p;
  out.normal = s.normal;
  out.uv = s.uv;
  out.anchor = s.anchor;
  out.id = copy_id(ID, ii);
  return out;
}
${frag.main}`,
    ].join("\n");
    const base = { label: this.name, shader: key, vertices: 6 * NU * NV, instances, constants: constantsOf(detail, NU, NV), detail: detail.spec };
    const shader: Shader = { code, libs: [...vertex.libs, ...frag.fns.libs] };
    return { shaders: { [key]: shader }, draws: frag.draws(base) };
  }
}

/**
 * How a drawn surface's grid size is chosen: fixed at the authored counts, or by the host from the surface's size on
 * screen (lit surfaces: bodies, sheets, hulls, sails). Glazed surfaces (the sea's own grid is its business; markers and
 * smears on it) and flat strokes (whose grids carry structure, such as a ship's ropes joined by zero-width cells) stay
 * fixed.
 */
type SurfaceDetail = { readonly kind: "fixed" | "grid" | "projected"; readonly spec: DetailSpec };

function detailOf(ctx: CompileContext, d: SurfaceDrawable): SurfaceDetail {
  switch (d.look.kind) {
    case "lit": {
      const at = { u: d.grid.u.at, v: d.grid.v.at };
      const bound = boundTable(d.name, d.surface, at, d.copies);
      ctx.table(bound);
      return { kind: "grid", spec: gridDetail(bound, d.surface, at, d.copies, d.grid.u.count, d.grid.v.count) };
    }
    case "flat": return { kind: "fixed", spec: { kind: "fixed" } };
    case "glazed": return d.look.detail.kind === "fixed" ? { kind: "fixed", spec: d.look.detail } : { kind: "projected", spec: d.look.detail };
  }
}
/** `fn grid_cells(ii) -> vec2<u32>`: the cell counts this instance is drawn with. */
function cellsWgsl(d: SurfaceDetail, nu: number, nv: number): string {
  switch (d.kind) {
    case "fixed": return `override NU: u32 = ${nu}u;\noverride NV: u32 = ${nv}u;\nfn grid_cells(ii: u32) -> vec2<u32> { return vec2<u32>(NU, NV); }`;
    case "grid": return `override LOFT_DETAIL_SLOT: u32 = 0u;\nfn grid_cells(ii: u32) -> vec2<u32> { return vec2<u32>(loft_detail[LOFT_DETAIL_SLOT + ii].xy); }`;
    case "projected": return `override LOFT_DETAIL_SLOT: u32 = 0u;\nfn grid_cells(ii: u32) -> vec2<u32> { return vec2<u32>(loft_detail[LOFT_DETAIL_SLOT].xy); }`;
  }
}
function constantsOf(d: SurfaceDetail, nu: number, nv: number): Readonly<Record<string, number>> {
  switch (d.kind) {
    case "fixed": return { NU: nu, NV: nv };
    case "grid": case "projected": return {};
  }
}

const countOf = (c: Copies): number => {
  switch (c.kind) {
    case "one": return 1;
    case "points": return c.points.count;
  }
};

function carriedSurface(s: Surface, c: Copies, chart: Chart): Surface {
  switch (c.kind) {
    case "one": return {
      point: (u, v) => chart.frame(s.point(u, v)).o,
      normal: (u, v) => rotate(chart.frame(s.point(u, v)), s.normal(u, v)),
      anchor: s.anchor, // carried, not moved: its pattern rides it
      topology: s.topology,
      skeleton: { joints: s.skeleton.joints, joint: (k) => { const j = s.skeleton.joint(k); return { p: chart.frame(j.p).o, r: j.r }; } },
    };
    case "points": return s; // the copies' frames are carried instead
  }
}
function carriedCopies(c: Copies, chart: Chart): Copies {
  switch (c.kind) {
    case "one": return c;
    case "points": {
      const pts = c.points;
      return { kind: "points", points: { count: pts.count, at: (i) => { const a = pts.at(i); return { frame: carry(chart, a.frame), scale: a.scale }; } } };
    }
  }
}

function skeletonsOf(s: Surface, c: Copies): readonly Skeleton[] {
  switch (c.kind) {
    case "one": return [s.skeleton];
    // Each copy casts with the surface's own chain carried by its placement (copies used not to cast at all, so a
    // fleet of instanced ships or a ring of spikes left no shadow on the water).
    case "points": return s.skeleton.joints < 2 ? [] : Array.from({ length: c.points.count }, (_, i) => {
      const at = c.points.at(f(i));
      return { joints: s.skeleton.joints, joint: (k: number) => { const j = s.skeleton.joint(k); return { p: add(at.frame.o, mul(rotate(at.frame, j.p), at.scale)), r: mul(j.r, at.scale) }; } };
    });
  }
}

function bakeablesOf(d: SurfaceDrawable, c: Copies): readonly Bakeable[] {
  const look = d.look;
  switch (look.kind) {
    case "glazed": return []; // its colour reads the camera and the scene's depth: there is nothing to bake
    case "lit": return copiesOf(d, c, look.paint, look.sides);
    case "flat": return []; // flat surfaces are strokes facing the camera: they have no shape to bake
  }
}
function copiesOf(d: SurfaceDrawable, c: Copies, paint: Paint, sides: "one" | "two"): readonly Bakeable[] {
  switch (c.kind) {
    case "one": return [{ name: d.name, surface: d.surface, paint, grid: d.grid, sides }];
    case "points": return Array.from({ length: c.points.count }, (_, i) => ({ name: `${d.name}_${i}`, surface: framed(d.surface, c.points.at(f(i))), paint, grid: d.grid, sides }));
  }
}

/** `fn <name>(s, t, i) -> SurfV`: grid corner (s, t) of instance i to a world point, normal and (u, v). */
function vertexFn(ctx: CompileContext, d: SurfaceDrawable, name: string): Shader {
  const u = d.grid.u.at(S), v = d.grid.v.at(T);
  const s = placedSurface(d.surface, d.copies);
  const normal = vertexNormal(d.look, s, u, v);
  // Only for a style that reads it: a body's anchor fetches its stations again, which toon would pay for nothing.
  const anchor = readsBody(ctx.style) ? s.anchor(u, v) : vec3(0, 0, 0);
  return emitShaderFn(ctx.slots, {
    name, args: [["s", "f32"], ["t", "f32"], ["i", "f32"]], out: "SurfV",
    roots: [s.point(u, v), normal, vec2(u, v), anchor], ret: (o) => `SurfV(${o.join(", ")})`,
  });
}

/** A glazed surface's colour does not read the vertex normal, so it is not computed. */
function vertexNormal(look: Look, s: Surface, u: F, v: F): P3 {
  switch (look.kind) {
    case "lit": case "flat": return s.normal(u, v);
    case "glazed": return vec4(0, 0, 0, 0).xyz;
  }
}

function placedSurface(s: Surface, c: Copies): Surface {
  switch (c.kind) {
    case "one": return s;
    case "points": return framed(s, c.points.at(I));
  }
}

type Base = Pick<DrawSpec, "label" | "shader" | "vertices" | "instances" | "constants" | "detail">;
type Frag = { readonly overrides: string; readonly fns: Shader; readonly push: string; readonly main: string; readonly draws: (b: Base) => DrawSpec[] };

const LIT_PRE = /* wgsl */ `
  let eye = -view_dir(in.world);
  let depth = sea_height(in.world.xz) - in.world.y;`;

function fragment(ctx: CompileContext, d: SurfaceDrawable): Frag {
  const look = d.look;
  const paintFn = (paint: Paint): Shader => emitShaderFn(ctx.slots, { name: `paint_${d.name}`, args: [["u", "f32"], ["v", "f32"]], out: "vec3<f32>", roots: [paint(U, V)], ret: (o) => o[0] });
  switch (look.kind) {
    case "lit": {
      const face = look.sides === "one" ? "1.0" : "select(-1.0, 1.0, front)";
      const head = `@fragment fn fs(in: VOut, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
  let n = ${face} * normalize(in.normal);${LIT_PRE}
  let base = paint_${d.name}(in.uv.x, in.uv.y);`;
      const cull = look.sides === "one" ? "back" : "none";
      switch (look.outline) {
        case "hull": return hullOutline(ctx, head, paintFn(look.paint), cull);
        case "border": return {
          overrides: "", fns: paintFn(look.paint), push: "s.pos",
          // Ink along the v = 1 edge, in the surface's own coordinates: about two pixels wide at any distance.
          main: `${head}
  let fw = fwidth(in.uv.y);
  let edge = 1.0 - smoothstep(fw * 1.0, fw * 2.2, 1.0 - in.uv.y);
  return vec4<f32>(mix(shade(base, n, eye, in.world, depth, in.uv, in.anchor, in.id), ink(base, n, eye, in.world, depth, in.uv, in.anchor, in.id), edge), ${ctx.markOut("mark(base, n, eye, in.world, depth, in.uv, in.anchor, in.id)", false)});
}`,
          draws: (b) => [drawSpec({ ...b, cull })],
        };
        case "none": return {
          overrides: "", fns: paintFn(look.paint), push: "s.pos",
          main: `${head}
  return vec4<f32>(shade(base, n, eye, in.world, depth, in.uv, in.anchor, in.id), ${ctx.markOut("mark(base, n, eye, in.world, depth, in.uv, in.anchor, in.id)", false)});
}`,
          draws: (b) => [drawSpec({ ...b, cull })],
        };
      }
    }
    case "flat": return {
      overrides: "", fns: paintFn(look.paint), push: "s.pos",
      main: `@fragment fn fs(in: VOut) -> @location(0) vec4<f32> { return vec4<f32>(paint_${d.name}(in.uv.x, in.uv.y), 1.0); }`,
      draws: (b) => [drawSpec({ ...b, cull: "none" })],
    };
    case "glazed": {
      const W = variable("f_world", "vec3"), PX = variable("f_px", "vec2");
      const colour = look.colour({ world: W, px: PX, u: U, v: V, style: ctx.style, sun: ctx.sun });
      return {
        overrides: "", push: "s.pos",
        fns: emitShaderFn(ctx.slots, { name: `colour_${d.name}`, args: [["f_world", "vec3"], ["f_px", "vec2"], ["u", "f32"], ["v", "f32"]], out: "vec4<f32>", roots: [colour], ret: (o) => o[0] }),
        main: `@fragment fn fs(in: VOut) -> @location(0) vec4<f32> { return colour_${d.name}(in.world, in.clip.xy, in.uv.x, in.uv.y); }`,
        draws: (b) => [drawSpec({ ...b, phase: look.phase, cull: "none", depthWrite: false, blend: "alpha", sceneDepth: look.sceneDepth })],
      };
    }
  }
}

/** A closed surface's ink: a second draw pushed out along the normal (hull), or a flag for the screen pass. */
function hullOutline(ctx: CompileContext, head: string, paint: Shader, cull: DrawSpec["cull"]): Frag {
  switch (ctx.outline) {
    case "hull": return {
      overrides: "override INK: f32 = 0.0;", fns: paint, push: "s.pos + s.normal * (INK * ink_width(s.pos))",
      // INK is an override, so this branch is uniform (the style's shading takes screen-space derivatives), and the
      // outline pass does not pay for the shading it would throw away.
      main: `${head}
  let mk = mark(base, n, eye, in.world, depth, in.uv, in.anchor, in.id);
  if (INK > 0.5) { return vec4<f32>(ink(base, n, eye, in.world, depth, in.uv, in.anchor, in.id), mk); }
  return vec4<f32>(shade(base, n, eye, in.world, depth, in.uv, in.anchor, in.id), mk);
}`,
      draws: (b) => [
        drawSpec({ ...b, cull, constants: { ...b.constants, INK: 0 } }),
        drawSpec({ ...b, label: `${b.label} outline`, cull: "front", constants: { ...b.constants, INK: 1 } }),
      ],
    };
    case "screen": return {
      overrides: "", fns: paint, push: "s.pos",
      main: `${head}
  return vec4<f32>(shade(base, n, eye, in.world, depth, in.uv, in.anchor, in.id), ${ctx.markOut("mark(base, n, eye, in.world, depth, in.uv, in.anchor, in.id)", true)});
}`,
      draws: (b) => [drawSpec({ ...b, cull })],
    };
  }
}

/** Options for a lit surface. */
export type SurfaceOptions = { readonly paint: Paint; readonly grid: Grid; readonly sides: "one" | "two"; readonly outline: Outline };

/** A surface, lit by the scene's style. */
export function drawSurface(name: string, surface: Surface, o: SurfaceOptions): SurfaceDrawable {
  return new SurfaceDrawable(name, surface, o.grid, { kind: "lit", paint: o.paint, sides: o.sides, outline: o.outline }, { kind: "one" });
}

/** A surface with its own colour and alpha over what is behind it (the sea; a decal surface). */
export function drawGlazed(name: string, surface: Surface, o: { readonly colour: (f: Fragment) => Expr<"vec4">; readonly grid: Grid; readonly phase: Extract<Phase, "water" | "on-water" | "transparent">; readonly sceneDepth: boolean; readonly detail?: GlazedDetail }): SurfaceDrawable {
  return new SurfaceDrawable(name, surface, o.grid, { kind: "glazed", colour: o.colour, phase: o.phase, sceneDepth: o.sceneDepth, detail: o.detail ?? { kind: "fixed" } }, { kind: "one" });
}

/** A surface whose paint is its final colour (a stroke, an ink line). */
export function drawFlat(name: string, surface: Surface, o: { readonly paint: Paint; readonly grid: Grid }): SurfaceDrawable {
  return new SurfaceDrawable(name, surface, o.grid, { kind: "flat", paint: o.paint }, { kind: "one" });
}


/** A stable identity in [0, 1) for a drawable, from its name (FNV-1a): the same body gets the same patterns everywhere. */
export function nameId(name: string): number {
  let h = 0x811c9dc5;
  for (let i = 0; i < name.length; i++) h = Math.imul(h ^ name.charCodeAt(i), 0x01000193) >>> 0;
  return (h >>> 8) / 16777216;
}

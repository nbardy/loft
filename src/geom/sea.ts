// The sea: the root chart, local (x, z, height above the surface) with a swell that moves in time, and the water drawn
// as a glazed `Surface` in that chart. The surface is a projected grid (`Detail` kind "projected"): cells of about
// SEA_CELL x `View.cellPixels` pixels laid over the image of the still water out to `reach`, each vertex the point of
// the still water its pixel sees, lifted to the swell there (exactly the height function at that point). Its cells
// are the same size on screen everywhere, so the far sea is no longer drawn with triangles far smaller than a pixel.
// It is evaluated on the GPU only, coloured per pixel by the style's `water`, with foam where the water meets any
// opaque body found from the scene's depth.
//
// Vertices move over the water as the camera moves (a projected grid "swims"). What that can change is small: colour,
// crest, foam and slope are evaluated per pixel at the interpolated point, and a point of the swell sampled anywhere
// is still on the swell; only the straight edges between vertices cut the crests, by at most A (pi c / L)^2 / 2 for
// cells c across a wave of length L and amplitude A (0.003 for the serpent's swell at 1-unit cells).

import {
  add, cross, distance, lift, max, mul, normalize, sin, smoothstep, sub, substitute, time, variable, vec2, vec3, type Expr, type In,
} from "../expr.ts";
import { gradient } from "../grad.ts";
import { CELL_PIXELS } from "../artefact.ts";
import type { Bakeable, CompileContext, Drawable, Emitted } from "../compile/context.ts";
import { drawGlazed, spacing, type Fragment, type SurfaceDrawable } from "../draw/surface.ts";
import { eyePosition, viewDir, wgslFn } from "../lib/wgsl.ts";
import { NO_SKELETON, type Chart, type F, type Frame, type Skeleton, type Surface } from "../map/types.ts";

export type SwellSpec = {
  /** Direction of travel in the xz plane (normalised here). */
  readonly direction: readonly [number, number];
  /** Wavelength, world units. */
  readonly length: number;
  /** Crest-to-trough height, world units. */
  readonly height: number;
  /** Phase speed, world units per second. */
  readonly speed: number;
  /** Phase offset, radians. Default 0. */
  readonly phase?: number;
};

/**
 * A deformation of the sea (a whirlpool, src/geom/whirlpool.ts): a map of the sea plane that is exactly the identity
 * outside its reach. The sea evaluates its swell, and the deformation its own marks, in the deformation's coordinates,
 * so everything on the water winds with it; the surface is also pushed down by `dip`.
 */
export interface SeaDeform {
  /** Where the water at sea-plane point xz (world x, z) at time t sits in the deformation's own plane coordinates. */
  toLocal(xz: Expr<"vec2">, t: In<"f32">): Expr<"vec2">;
  /** How far the surface is pushed down at sea-plane point xz (0 outside the reach). */
  dip(xz: Expr<"vec2">): F;
  /**
   * How much of the swell survives at sea-plane point xz, 0..1 (1 outside the reach). A whirlpool smooths the swell
   * toward its throat, where the swell wound into it would otherwise crowd below a pixel.
   */
  calm(xz: Expr<"vec2">): F;
  /**
   * Its marks on the water at sea-plane point xz, for a pixel footprint `fp` (world units): foam 0..1, and a shade
   * added to the swell's crest value (-1 trough .. 1 crest). Both 0 outside the reach. GPU or CPU.
   */
  look(xz: Expr<"vec2">, t: In<"f32">, fp: F): { readonly foam: F; readonly shade: F };
}

export type SeaSpec = {
  readonly swell: readonly SwellSpec[];
  /** Deformations of the water, applied in order (each sees the previous one's coordinates). Default none. */
  readonly deform?: readonly SeaDeform[];
  /** How far the water is drawn, as view depth in world units. Default 400. */
  readonly reach?: number;
  /** Width of the foam band where the water meets a body, world units. Default 0.3. */
  readonly foam?: number;
};

/** One sinusoidal component, canonical: unit direction, wavenumber, amplitude, angular frequency, phase. */
type Wave = { readonly dir: readonly [number, number]; readonly k: number; readonly amp: number; readonly omega: number; readonly phase: number };

/** The free variable the sea's gradient is taken with respect to (then replaced by the point asked about). */
const SEA_XZ = variable("loft_sea_xz", "vec2");

/** Sea cells on screen, in units of `View.cellPixels`: shading is per pixel, so the cells carry only the swell's shape. */
const SEA_CELL = 2.25;

/**
 * The point of the still water (y = 0) that grid corner (u, v) of the projected grid sees: NDC x across the screen
 * widened by the host's margin, NDC y over the rows the host found meeting the water (src/host/detail.ts).
 * `LOFT_DETAIL_SLOT` is the sea draw's override; the host chose only rows whose rays meet the water ahead.
 */
const seaPlane = wgslFn<"vec2">({
  name: "loft_sea_plane", ret: "vec2", wgsl: /* wgsl */ `
fn loft_sea_plane(u: f32, v: f32) -> vec2<f32> {
  let rows = loft_detail[LOFT_DETAIL_SLOT];
  let m = loft_detail[LOFT_DETAIL_SLOT + 1u].x;
  let ndc = vec2<f32>(mix(-1.0 - m, 1.0 + m, u), mix(rows.z, rows.w, v));
  let a = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
  let b = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  let o = a.xyz / a.w;
  let d = b.xyz / b.w - o;
  return (o + d * (-o.y / d.y)).xz;
}`,
});

/** Longest extent of one pixel's footprint on the sea at p (`sea_footprint` in sceneWgsl). */
const seaFootprint = wgslFn<"f32">({ name: "loft_sea_footprint", ret: "f32", wgsl: "fn loft_sea_footprint(p: vec3<f32>) -> f32 { return sea_footprint(p); }" });

/**
 * Tap offsets, in units of the band's width on screen: 8 round the outer edge, then 8 at 0.55 turned by half a step,
 * so the nearest-surface estimate has 16 directions (with 8 the foam's outer edge showed an 8-pointed star of streaks).
 */
const RING = Array.from({ length: 16 }, (_, k) => {
  const a = (k * Math.PI) / 4 + (k >= 8 ? Math.PI / 8 : 0), r = k >= 8 ? 0.55 : 1;
  return [r * Math.cos(a), r * Math.sin(a)];
});
const fl = (x: number): string => (Number.isInteger(x) ? `${x}.0` : String(x));

/**
 * How close the water at p (seen at pixel px) is to an opaque body, for a band `width` wide: 1 at the body, 0 at the
 * band's edge, below 0 beyond it. A sea pixel is near a body when some opaque surface lies within the band of it in
 * 3D: a ring of taps round the pixel, at the band's size on screen, finds the body on either side of the waterline
 * (behind the water and above it), so any opaque thing gets its waterline, at a cost per pixel that does not grow
 * with the scene. It reads the scene's depth, so it may only be called from a draw with `sceneDepth` (its definition
 * goes in that draw's module, not in sceneWgsl).
 */
const contactBand = wgslFn<"f32">({
  name: "loft_contact_band", ret: "f32", wgsl: /* wgsl */ `
const LOFT_FOAM_RING = array<vec2<f32>, 16>(${RING.map(([x, y]) => `vec2<f32>(${fl(+x.toFixed(6))}, ${fl(+y.toFixed(6))})`).join(", ")});
fn loft_contact_band(p: vec3<f32>, px: vec2<f32>, width: f32) -> f32 {
  // The band's width on screen; at least a pixel (far away), at most 40 (a body filling the view).
  let r = clamp(width / pixel_world(p), 1.0, 40.0);
  // The ring turns from pixel to pixel (interleaved gradient noise, fixed in screen space): a fixed ring overestimates
  // the gap between its directions and drew radial streaks out of the band's edge; a turning one leaves fine grain.
  let turn = 6.2831853 * fract(52.9829189 * fract(dot(floor(px), vec2<f32>(0.06711056, 0.00583715))));
  let rot = mat2x2<f32>(cos(turn), sin(turn), -sin(turn), cos(turn));
  var gap = distance(scene_position(px), p);
  for (var k = 0u; k < 8u; k++) { gap = min(gap, distance(scene_position(px + rot * LOFT_FOAM_RING[k] * r), p)); }
  // Only near a body is the inner ring worth reading.
  if (gap < 2.0 * width) {
    for (var k = 8u; k < 16u; k++) { gap = min(gap, distance(scene_position(px + rot * LOFT_FOAM_RING[k] * r), p)); }
  }
  return 1.0 - gap / width;
}`,
});

/**
 * Foam from the contact band, 0..1, antialiased by the pixel footprint fp: full at the body, gone at the foam width;
 * the threshold wobbles slowly so the edge reads as lapping foam.
 */
const contactFoam = wgslFn<"f32">({
  name: "loft_contact_foam", ret: "f32", wgsl: /* wgsl */ `
fn loft_contact_foam(p: vec3<f32>, band: f32, fp: f32, width: f32) -> f32 {
  let t = frame.time.x;
  let wobble = 0.12 * sin(p.x * 2.3 + t * 1.9) * sin(p.z * 2.9 - t * 1.4);
  let aa = max(fp / width, 1e-3);
  let core = smoothstep(0.42 + wobble - aa, 0.42 + wobble + aa, band);
  let fringe = smoothstep(0.12 + wobble - aa, 0.12 + wobble + aa, band);
  return core * 0.85 + fringe * 0.15;
}`,
});

export class Sea implements Chart, Drawable {
  readonly name = "sea";
  readonly skeletons: readonly Skeleton[] = [];
  /** The water follows the camera and reads the scene's depth: nothing to bake. */
  readonly surfaces: readonly Bakeable[] = [];
  private readonly drawn: SurfaceDrawable;

  constructor(readonly waves: readonly Wave[], readonly reach: number, readonly foam: number, readonly deforms: readonly SeaDeform[] = []) {
    // Transparent: what shows through the water is the submerged body (tinted by its own shader) or, where nothing
    // is below, the sky shader's under-horizon colour. The host sizes the grid to the screen each frame; the counts
    // here are only its declared size (`stats.maxTriangles`): what it draws at 1280 x 800 with water to every edge.
    const lift = waves.reduce((a, w) => a + w.amp, 0);
    const declared = (pixels: number) => Math.ceil((pixels * 1.1) / (SEA_CELL * CELL_PIXELS));
    this.drawn = drawGlazed("sea", this.surface(), {
      colour: (fr) => this.water(fr), grid: { u: spacing.even(declared(1280)), v: spacing.even(declared(800)) }, phase: "water", sceneDepth: true,
      detail: { kind: "projected", cell: SEA_CELL, plane: 0, reach, lift },
    });
  }

  draws(ctx: CompileContext): Emitted { return this.drawn.draws(ctx); }

  private phaseOf(w: Wave, xz: Expr<"vec2">, t: In<"f32">): F {
    return add(sub(mul(add(mul(xz.x, w.dir[0]), mul(xz.y, w.dir[1])), w.k), mul(t, w.omega)), w.phase);
  }

  /** The swell alone, in the plane coordinates it is evaluated in (the deformations' local coordinates). */
  private swellAt(xz: Expr<"vec2">, t: In<"f32">): F {
    return this.waves.reduce<F>((h, w) => add(h, mul(sin(this.phaseOf(w, xz, t)), w.amp)), lift<"f32">(0));
  }

  /** Where the water at a sea-plane point sits in the last deformation's coordinates (xz itself with none). */
  local(xz: Expr<"vec2">, t: In<"f32"> = time): Expr<"vec2"> {
    return this.deforms.reduce((p, d) => d.toLocal(p, t), xz);
  }

  /** Surface height at a point of the xz plane: the swell where that water came from, less every deformation's dip. */
  height(xz: Expr<"vec2">, t: In<"f32"> = time): F {
    const swell = this.deforms.reduce<F>((h, d) => mul(h, d.calm(xz)), this.swellAt(this.local(xz, t), t));
    return this.deforms.reduce<F>((h, d) => sub(h, d.dip(xz)), swell);
  }

  /**
   * d(height)/dx and d(height)/dz. Symbolic (src/grad.ts) through the deformations' maps, which no hand-written
   * gradient could follow; with none, it reduces to the swell's analytic gradient (the constructors simplify).
   */
  gradient(xz: Expr<"vec2">, t: In<"f32"> = time): Expr<"vec2"> {
    const g = gradient(this.height(SEA_XZ, t), SEA_XZ);
    return substitute([g], SEA_XZ, xz)[0] as Expr<"vec2">;
  }

  /** Floating on the open sea: a point given in sea coordinates is where it is and has not turned (afloat.ts). */
  carry(at: Expr<"vec2">): { readonly xz: Expr<"vec2">; readonly turn: F } {
    return { xz: at, turn: lift<"f32">(0) };
  }

  /** (x, z, height above the surface) to the point and a frame there: y the water's normal, x along +x on the slope. */
  frame(c: Expr<"vec3">): Frame {
    const xz = vec2(c.x, c.y), g = this.gradient(xz);
    const x = normalize(vec3(1, g.x, 0)), y = normalize(vec3(mul(g.x, -1), 1, mul(g.y, -1)));
    return { o: vec3(c.x, add(this.height(xz), c.z), c.y), x, y, z: cross(x, y) };
  }

  at(t: F): Chart {
    const slice = (fr: Frame): Frame => { const [o, x, y, z] = substitute([fr.o, fr.x, fr.y, fr.z], time, t) as Expr<"vec3">[]; return { o, x, y, z }; };
    return { frame: (c) => slice(this.frame(c)), at: (t2) => this.at(t2), atNear: (t2) => this.at(t2) };
  }
  atNear(t: F): Chart { return this.at(t); }

  /** The water: the projected grid (seaPlane), laid in this chart at height 0. */
  private surface(): Surface {
    const xz = (u: F, v: F) => seaPlane(u, v);
    return {
      point: (u, v) => { const p = xz(u, v); return this.frame(vec3(p.x, p.y, 0)).o; },
      normal: (u, v) => { const p = xz(u, v); return this.frame(vec3(p.x, p.y, 0)).y; },
      anchor: (u, v) => vec3(u, v, 0), // the water follows the camera: nothing of it is fixed but its grid
      topology: { u: ["free", "free"], v: "free" },
      skeleton: NO_SKELETON,
    };
  }

  /** The phase of the largest swell at xz, in cycles (0 everywhere for a sea without swell: it has no crests). */
  private dominantPhase(xz: Expr<"vec2">): F {
    const w = this.waves.reduce<Wave | undefined>((best, v) => (best === undefined || v.amp > best.amp ? v : best), undefined);
    return w === undefined ? lift<"f32">(0) : mul(this.phaseOf(w, xz, time), 1 / (2 * Math.PI));
  }

  /** The water's colour at a pixel: the style's `water`, with the swell's slope and crest, and contact foam. */
  private water(fr: Fragment): Expr<"vec4"> {
    const amplitude = Math.max(this.waves.reduce((a, w) => a + w.amp, 0), 1e-3);
    const shortest = this.waves.reduce((l, w) => Math.min(l, (2 * Math.PI) / w.k), 1e3);
    const xz = fr.world.xz;
    const fp = seaFootprint(fr.world);
    const band = contactBand(fr.world, fr.px, this.foam);
    // Where a pixel covers a good part of the shortest wave, its slope and crest shading can only alias (bands of
    // stripes toward the horizon): fade that detail out by the pixel's footprint.
    const detail = sub(1, smoothstep(0.04, 0.2, fp.div(shortest)));
    const g = mul(this.gradient(xz), detail);
    // The deformations' marks, each in its own coordinates (foam arms winding into a whirlpool, a darker throat).
    const marks = this.deforms.map((d) => d.look(xz, time, fp));
    const swell = this.deforms.reduce<F>((h, d) => mul(h, d.calm(xz)), this.swellAt(this.local(xz), time));
    return fr.style.water({
      normal: normalize(vec3(mul(g.x, -1), 1, mul(g.y, -1))),
      sun: lift<"vec3">(fr.sun),
      view: mul(viewDir(fr.world), -1),
      world: fr.world,
      // Per pixel, not the interpolated vertex height: far grid cells are large and their edges would show. The crest
      // is the swell's alone (where the water came from); a dip is shading the deformation chooses, not a trough.
      crest: marks.reduce<F>((c, m) => add(c, m.shade), mul(swell.div(amplitude), detail)),
      distance: distance(fr.world, eyePosition()),
      foam: marks.reduce<F>((f, m) => max(f, m.foam), contactFoam(fr.world, band, fp, this.foam)),
      contact: band,
      phase: this.dominantPhase(xz),
    });
  }
}

export function sea(spec: SeaSpec): Sea {
  const waves = spec.swell.map((s, i): Wave => {
    const l = Math.hypot(s.direction[0], s.direction[1]);
    if (!(l > 0)) throw new Error(`sea: swell ${i} has a zero direction`);
    if (!(s.length > 0)) throw new Error(`sea: swell ${i} needs a positive length, got ${s.length}`);
    const k = (2 * Math.PI) / s.length;
    return { dir: [s.direction[0] / l, s.direction[1] / l], k, amp: s.height / 2, omega: k * s.speed, phase: s.phase ?? 0 };
  });
  const foam = spec.foam ?? 0.3;
  if (!(foam > 0)) throw new Error(`sea: foam width must be positive, got ${foam}`);
  return new Sea(waves, spec.reach ?? 400, foam, spec.deform ?? []);
}

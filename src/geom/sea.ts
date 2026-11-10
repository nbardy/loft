// The sea: the root chart, local (x, z, height above the surface) with a swell that moves in time, and the water drawn
// as a glazed `Surface` in that chart. The surface is a grid that grows away from where the view meets the water (so
// it is evaluated on the GPU only), coloured per pixel by the style's `water`, with foam where the water meets any
// opaque body found from the scene's depth.

import {
  add, cross, distance, exp, lift, max, mul, normalize, sign, abs, sin, smoothstep, sub, substitute, time, variable, vec2, vec3, type Expr, type In,
} from "../expr.ts";
import { gradient } from "../grad.ts";
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
  /** Cells per side of the sea grid. Default 192. */
  readonly grid?: number;
  /** Reach of the grid from its centre, world units. Default 400. */
  readonly reach?: number;
  /** Width of the foam band where the water meets a body, world units. Default 0.3. */
  readonly foam?: number;
};

/** One sinusoidal component, canonical: unit direction, wavenumber, amplitude, angular frequency, phase. */
type Wave = { readonly dir: readonly [number, number]; readonly k: number; readonly amp: number; readonly omega: number; readonly phase: number };

/** The free variable the sea's gradient is taken with respect to (then replaced by the point asked about). */
const SEA_XZ = variable("loft_sea_xz", "vec2");

/** Growth of the sea grid's cells away from its centre: offset = reach (e^(K|u|) - 1) / (e^K - 1). */
const SEA_GROWTH = 6;

/** Where the sea grid is centred: where the view centre meets the water (or ahead of the eye), snapped to 2 units. */
const seaCentre = wgslFn<"vec2">({
  name: "loft_sea_centre", ret: "vec2", wgsl: /* wgsl */ `
fn loft_sea_centre() -> vec2<f32> {
  let f = frame.forward.xyz;
  let e = frame.eye.xyz;
  let t = select(60.0, clamp(-e.y / f.y, 0.0, 60.0), f.y < -0.01);
  return floor((e.xz + f.xz * t) / 2.0) * 2.0;
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
 * Foam where the water at p (seen at pixel px) meets an opaque body, 0..1, antialiased by the pixel footprint fp, for
 * a band `width` wide. A sea pixel is foamy when some opaque surface lies within the band of it in 3D: a ring of taps
 * round the pixel, at the band's size on screen, finds the body on either side of the waterline (behind the water and
 * above it), so any opaque thing gets its waterline foam, at a cost per pixel that does not grow with the scene. It
 * reads the scene's depth, so it may only be called from a draw with `sceneDepth` (its definition goes in that
 * draw's module, not in sceneWgsl).
 */
const contactFoam = wgslFn<"f32">({
  name: "loft_contact_foam", ret: "f32", wgsl: /* wgsl */ `
const LOFT_FOAM_RING = array<vec2<f32>, 16>(${RING.map(([x, y]) => `vec2<f32>(${fl(+x.toFixed(6))}, ${fl(+y.toFixed(6))})`).join(", ")});
fn loft_contact_foam(p: vec3<f32>, px: vec2<f32>, fp: f32, width: f32) -> f32 {
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
  // Full at the body, gone at the foam width; the threshold wobbles slowly so the edge reads as lapping foam.
  let t = frame.time.x;
  let wobble = 0.12 * sin(p.x * 2.3 + t * 1.9) * sin(p.z * 2.9 - t * 1.4);
  let band = 1.0 - gap / width;
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

  constructor(readonly waves: readonly Wave[], readonly grid: number, readonly reach: number, readonly foam: number, readonly deforms: readonly SeaDeform[] = []) {
    // Transparent: what shows through the water is the submerged body (tinted by its own shader) or, where nothing
    // is below, the sky shader's under-horizon colour.
    this.drawn = drawGlazed("sea", this.surface(), {
      colour: (fr) => this.water(fr), grid: { u: spacing.even(grid), v: spacing.even(grid) }, phase: "water", sceneDepth: true,
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
    return { frame: (c) => slice(this.frame(c)), at: (t2) => this.at(t2) };
  }

  /** The water: a grid round the view centre whose cells grow away from it, laid in this chart at height 0. */
  private surface(): Surface {
    const offset = (u: F): F => {
      const a = sub(mul(u, 2), 1);
      return mul(sign(a), mul(this.reach / (Math.exp(SEA_GROWTH) - 1), sub(exp(mul(abs(a), SEA_GROWTH)), 1)));
    };
    const xz = (u: F, v: F) => add(seaCentre(), vec2(offset(u), offset(v)));
    return {
      point: (u, v) => { const p = xz(u, v); return this.frame(vec3(p.x, p.y, 0)).o; },
      normal: (u, v) => { const p = xz(u, v); return this.frame(vec3(p.x, p.y, 0)).y; },
      topology: { u: ["free", "free"], v: "free" },
      skeleton: NO_SKELETON,
    };
  }

  /** The water's colour at a pixel: the style's `water`, with the swell's slope and crest, and contact foam. */
  private water(fr: Fragment): Expr<"vec4"> {
    const amplitude = Math.max(this.waves.reduce((a, w) => a + w.amp, 0), 1e-3);
    const shortest = this.waves.reduce((l, w) => Math.min(l, (2 * Math.PI) / w.k), 1e3);
    const xz = fr.world.xz;
    const fp = seaFootprint(fr.world);
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
      foam: marks.reduce<F>((f, m) => max(f, m.foam), contactFoam(fr.world, fr.px, fp, this.foam)),
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
  const grid = spec.grid ?? 192;
  if (!(Number.isInteger(grid) && grid >= 8)) throw new Error(`sea: grid must be an integer >= 8, got ${grid}`);
  const foam = spec.foam ?? 0.3;
  if (!(foam > 0)) throw new Error(`sea: foam width must be positive, got ${foam}`);
  return new Sea(waves, grid, spec.reach ?? 400, foam, spec.deform ?? []);
}

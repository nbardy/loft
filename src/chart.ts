// Charts are coordinate systems; composing them is the scene graph (DESIGN.md 2.3). M1 has the root chart, the sea:
// local (x, z, height above the surface) with a swell that moves in time. Whirlpool, compose and placement are M2.

import { add, cos, lift, mul, sin, sub, time, vec2, vec3, type Expr, type In } from "./expr.ts";
import type { CompileContext, Drawable, Emitted } from "./compile/context.ts";
import { seaDraws } from "./draw/sea.ts";

export interface Chart {
  /** Local coordinates to world. Always available. */
  toWorld(local: Expr<"vec3">): Expr<"vec3">;
  /** World to local, where the chart has an inverse. */
  toLocal?(world: Expr<"vec3">): Expr<"vec3">;
}

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

export type SeaSpec = {
  readonly swell: readonly SwellSpec[];
  /** Vertices per side of the sea grid (a WGSL override on the draw). Default 192. */
  readonly grid?: number;
  /** Reach of the grid from its centre, world units. Default 400. */
  readonly reach?: number;
  /** Width of the foam band where the water meets a body, world units. Default 0.3. */
  readonly foam?: number;
};

/** One sinusoidal component, canonical: unit direction, wavenumber, amplitude, angular frequency, phase. */
type Wave = { readonly dir: readonly [number, number]; readonly k: number; readonly amp: number; readonly omega: number; readonly phase: number };

export class Sea implements Chart, Drawable {
  readonly name = "sea";
  constructor(readonly waves: readonly Wave[], readonly grid: number, readonly reach: number, readonly foam: number) {}

  draws(ctx: CompileContext): Emitted { return seaDraws(ctx, this); }

  private phaseOf(w: Wave, xz: Expr<"vec2">, t: In<"f32">): Expr<"f32"> {
    return add(sub(mul(add(mul(xz.x, w.dir[0]), mul(xz.y, w.dir[1])), w.k), mul(t, w.omega)), w.phase);
  }

  /** Surface height at a point of the xz plane. */
  height(xz: Expr<"vec2">, t: In<"f32"> = time): Expr<"f32"> {
    return this.waves.reduce<Expr<"f32">>((h, w) => add(h, mul(sin(this.phaseOf(w, xz, t)), w.amp)), lift<"f32">(0));
  }

  /** d(height)/dx and d(height)/dz, analytic. */
  gradient(xz: Expr<"vec2">, t: In<"f32"> = time): Expr<"vec2"> {
    return this.waves.reduce<Expr<"vec2">>(
      (g, w) => add(g, mul(vec2(w.dir[0], w.dir[1]), mul(cos(this.phaseOf(w, xz, t)), w.amp * w.k))),
      lift<"vec2">([0, 0]),
    );
  }

  /** (x, z, height above the surface) to world. */
  toWorld(local: Expr<"vec3">): Expr<"vec3"> {
    return vec3(local.x, add(this.height(vec2(local.x, local.y)), local.z), local.y);
  }

  toLocal(world: Expr<"vec3">): Expr<"vec3"> {
    return vec3(world.x, world.z, sub(world.y, this.height(world.xz)));
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
  return new Sea(waves, grid, spec.reach ?? 400, foam);
}

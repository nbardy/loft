// Water where an attack strikes it (docs/art_direction/DIRECTION.md: "foam as cream claws round whatever breaks the
// water"). At the moment of impact a boil of clawed cream foam bursts on the water at each splash and runs out as a
// ring that breaks up into fingers and is gone; a key-black line runs round the cream. Flat on the water, one glazed
// draw for any number of splashes inside one area (the attack's own hit shape, so the patch is the area the game
// already knows).
//
// The fingers are the woodblock sea's own claws (src/style/woodblock.ts: foam along the zero lines of a noise field).
// The Reefback Titan's "crash" was the first of these; this is that crash for every boss.

import { abs, length, lift, max, mix, mul, saturate, smoothstep, sqrt, step, sub, vec2, vec3, vec4, draw, spacing, type Drawable, type F, type In, type Rgb, type Sea } from "../../../src/index.ts";
import { noise2 } from "../../../src/lib/index.ts";
import { hitPatch, type Hit } from "../../../src/spacetime/index.ts";
import { edgeOf, rgb } from "../../../src/style/shared.ts";
import { NO_SKELETON, type Surface } from "../../../src/map/types.ts";

/** One splash: where it lands on the water (x, z) and the seconds since it landed (negative before). */
export type Splash = { readonly at: In<"vec2">; readonly since: F };

export type ImpactSpec = {
  /** The water drawn on: every splash's ring must stay inside it (pad included). */
  readonly area: Hit;
  /** How far past the area the patch reaches, world units. */
  readonly pad: number;
  readonly splashes: readonly Splash[];
  /** How far the ring runs out from a splash, world units. */
  readonly reach: number;
  /** Seconds from impact until the foam is gone. */
  readonly seconds: number;
  readonly foam: Rgb;
  readonly line: Rgb;
};

/** The foam of a set of splashes, drawn on the water over `area`. */
export function impactFoam(name: string, water: Sea, o: ImpactSpec): Drawable {
  if (o.splashes.length < 1) throw new Error(`impactFoam "${name}": no splashes`);
  if (!(o.reach > 0 && o.seconds > 0)) throw new Error(`impactFoam "${name}": reach and seconds must be positive`);
  // While no splash is live the patch collapses to its middle (no area, no fragments): it costs nothing between strikes.
  const pad = lift<"f32">(o.pad);
  const live = o.splashes.reduce<F>((acc, s) => max(acc, mul(step(0, s.since), step(s.since, o.seconds))), lift<"f32">(0));
  const middle = hitPatch(o.area, lift<"f32">(0.5), lift<"f32">(0.5), pad);
  const surface: Surface = {
    point: (u, v) => { const xz = mix(middle, hitPatch(o.area, u, v, pad), live); return vec3(xz.x, water.height(xz), xz.y); },
    normal: () => vec3(0, 1, 0), anchor: (u, v) => vec3(u, v, 0),
    topology: { u: ["free", "free"], v: "free" }, skeleton: NO_SKELETON,
  };
  return draw.glazed(name, surface, {
    colour: (fr) => {
      const p = vec2(fr.world.x, fr.world.z);
      // The union of every splash's shape (0..1), then one cut into ink and cream. The claws live in the upper half of
      // the shape's falloff only: cut lower, the zero lines of the noise printed as black scribbles wherever the falloff
      // was faintly positive, far outside the foam (first version, 2026-11-11).
      const shape = o.splashes.reduce<F>((acc, s) => max(acc, splashShape(o, p, s)), lift<"f32">(0));
      const fingers = abs(noise2(mul(p, 1.3)));
      const cut = sub(sub(shape, 0.5), mul(fingers, 0.6));
      const foam = edgeOf(cut, 0, 0.004);
      const line = mul(edgeOf(cut, -0.07, 0.004), sub(1, foam));
      return vec4(mix(rgb(o.line), rgb(o.foam), foam), max(foam, mul(line, 0.9)));
    },
    grid: { u: spacing.even(24), v: spacing.even(24) }, phase: "on-water", sceneDepth: false,
  });
}

/**
 * One splash's foam band at p: a boil filling the middle for the first moment, and a ring whose front runs out fast
 * and slows (a square root of time), thinning as it goes; 0 before the impact and after `seconds`.
 */
function splashShape(o: ImpactSpec, p: In<"vec2">, s: Splash): F {
  const k = saturate(mul(s.since, 1 / o.seconds));
  const d = length(sub(lift<"vec2">(p), lift<"vec2">(s.at)));
  const front = mul(mix(0.2, 1, sqrt(k)), o.reach);
  const ring = sub(1, smoothstep(0, mul(mix(0.45, 0.12, k), o.reach), abs(sub(d, front))));
  const boil = mul(sub(1, smoothstep(0.1, 0.35, k)), sub(1, smoothstep(mul(front, 0.5), front, d)));
  const live = mul(step(0, s.since), sub(1, smoothstep(0.7, 1, k)));
  return mul(max(ring, boil), live);
}

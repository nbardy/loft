// The smear: the striking part over the recent past, as fading translucent copies of itself (DESIGN.md 8). Copy k is
// the part's body taken at `time - k dt` (`body.at`, so its stations are posed by the pose pass like any body's),
// lit by the scene's style with the part's own paint, and shown only while the part moves fast: its opacity follows
// the speed of the part's tip between that copy and the next younger one. Back faces are dropped so each copy is one
// layer of translucency, not two.

import { distance, dot, f, lift, mix, mul, smoothstep, step, sub, time, vec2, vec4, type Expr } from "../expr.ts";
import type { CompileContext, Drawable, Emitted } from "../compile/context.ts";
import { drawGlazed, nameId, spacing, type Fragment } from "../draw/surface.ts";
import { viewDir } from "../lib/wgsl.ts";
import type { Strand } from "../geom/strand.ts";
import { NO_SKELETON, type Surface } from "../map/types.ts";
import type { Part, Strike } from "./attack.ts";

type F = Expr<"f32">;

export type SmearSpec = {
  /** Seconds of the recent past the copies cover. */
  readonly over: number;
  /** How many copies. */
  readonly samples: number;
  /** Tip speed (world units per second) at which copies start to show, and at which they are fully shown. */
  readonly speed?: readonly [number, number];
  /** Opacity of the youngest copy at full speed. */
  readonly opacity?: number;
};

/** The part's stretch of a body (the slice), as its own surface: u runs from the part's `from` to its `to`. */
function partSurface(body: Strand, p: Part): Surface {
  const m = (u: F): F => mix(p.from, p.to, u);
  return { point: (u, v) => body.point(m(u), v), normal: (u, v) => body.normal(m(u), v), anchor: (u, v) => body.anchor(m(u), v), topology: { u: ["free", "free"], v: "periodic" }, skeleton: NO_SKELETON };
}

/** The tip's spine point (the nearest station: one table read). */
function tipOf(body: Strand, p: Part): Expr<"vec3"> {
  return body.skeleton.joint(Math.round(p.tip * (body.skeleton.joints - 1))).p;
}

class Smear implements Drawable {
  readonly skeletons = [];
  readonly surfaces = [];
  constructor(readonly name: string, readonly strike: Strike, readonly spec: Required<SmearSpec>) {}

  draws(ctx: CompileContext): Emitted {
    const { part } = this.strike, s = this.spec, dt = s.over / s.samples;
    const slices = Array.from({ length: s.samples + 1 }, (_, k) => (k === 0 ? part.body : part.body.at(sub(time, k * dt))));
    const emitted: Emitted[] = [];
    // Oldest first: drawn back to front in time, so younger copies lie over older ones.
    for (let k = s.samples; k >= 1; k--) {
      const body = slices[k];
      const speed = distance(tipOf(body, part), tipOf(slices[k - 1], part)).div(dt);
      const fade = mul(s.opacity, 1 - (k - 1) / s.samples);
      const show = mul(fade, smoothstep(s.speed[0], s.speed[1], speed));
      const colour = (fr: Fragment): Expr<"vec4"> => {
        const m = mix(part.from, part.to, fr.u);
        const normal = body.normal(m, fr.v);
        const view = mul(viewDir(fr.world), -1);
        // The copy is the part itself an instant ago: its own coordinates, anchor and identity, so its pattern matches.
        const lit = fr.style.shade({
          base: part.body.look.paint(m, fr.v), normal, sun: lift<"vec3">(fr.sun), view, world: fr.world, depth: sub(ctx.sea.height(fr.world.xz), fr.world.y),
          uv: vec2(m, fr.v), anchor: body.anchor(m, fr.v), id: f(nameId(part.body.name)),
        });
        return vec4(lit, mul(show, step(0, dot(normal, view))));
      };
      emitted.push(drawGlazed(`${this.name}_${k}`, partSurface(body, part), {
        colour, grid: { u: spacing.ends(24), v: spacing.from(16, 0.25) }, phase: "transparent", sceneDepth: false,
      }).draws(ctx));
    }
    return { shaders: Object.assign({}, ...emitted.map((e) => e.shaders)), draws: emitted.flatMap((e) => e.draws) };
  }
}

/** Fading copies of the attack's striking part over the last `over` seconds, shown while it moves fast. */
export function smear(strike: Strike, spec: SmearSpec): Drawable {
  if (!(Number.isInteger(spec.samples) && spec.samples >= 1)) throw new Error(`smear: samples must be a positive integer, got ${spec.samples}`);
  if (!(spec.over > 0)) throw new Error(`smear: over must be positive, got ${spec.over}`);
  const full: Required<SmearSpec> = { over: spec.over, samples: spec.samples, speed: spec.speed ?? [5, 11], opacity: spec.opacity ?? 0.5 };
  if (!(full.speed[1] > full.speed[0])) throw new Error(`smear: speed must rise, got [${full.speed.join(", ")}]`);
  return new Smear(`smear_${strike.attack.name}`, strike, full);
}

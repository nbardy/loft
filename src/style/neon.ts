// Neon: a dark ground and light from tubes. A body is near black; its paint becomes a light at full saturation, and
// that light is its outline (the hull is the tube: `ink` is the light), a glow a few pixels in from the silhouette
// (`pixelsInside`), and a fresnel sheen. The water is black glass with faint violet swell lines and cyan rings where a
// body stands in it; the bodies' light is smeared down the water (`reflection`) and everything bright blooms (`bloom`).
//
// Composed of: pixelsInside, swellLines, contactFoam, printLayers, skyOf, reflection, bloom.
//
// Lights (src/light.ts): the light is the paint, and stays so; the sun is not read. A local light lifts the near-black
// body by 0.35 of its colour times N.L, and lays a soft pool of its colour on the dark floor and the black glass, which
// bloom carries.

import { add, div, dot, exp, f, max, min, mix, mul, param, pow, saturate, smoothstep, sub, vec3, vec4, type Expr } from "../expr.ts";
import type { Rgb, ShadeCtx, Style } from "../style.ts";
import { contactFoam, foam, printLayers } from "./foam.ts";
import { bloom, reflection } from "./post.ts";
import { swellLines } from "./sea.ts";
import { pixelsInside, rgb } from "./shared.ts";
import { skyOf, type Sky } from "./sky.ts";

const NIGHT: Rgb = [0.012, 0.01, 0.03];
const WATER: Rgb = [0.014, 0.018, 0.045];
const VIOLET: Rgb = [0.45, 0.2, 0.9];
const CYAN: Rgb = [0.2, 1.0, 1.0];
const SKY: Sky = { horizon: [0.1, 0.035, 0.16], washes: [{ ink: NIGHT, from: 0, to: 0.3 }], below: WATER, sun: { kind: "none" }, clouds: { kind: "none" } };

/** A paint as a tube's light: its hue at full saturation, brighter than 1 so bloom finds it; a grey paint glows cold white. */
function tubeOf(c: Expr<"vec3">): Expr<"vec3"> {
  const hi = max(max(c.x, c.y), c.z), lo = min(min(c.x, c.y), c.z);
  const hue = div(sub(c, lo), max(sub(hi, lo), 1e-3));
  return mul(mix(vec3(0.75, 0.88, 1), hue, smoothstep(0.04, 0.18, sub(hi, lo))), 1.6);
}

/** A soft pool in each local light's colour on the dark floor and the black glass; bloom carries it. */
const pools = (ctx: Pick<ShadeCtx, "lights">, c: Expr<"vec3">): Expr<"vec3"> => ctx.lights("sum", (l, which) => {
  switch (which) {
    case "sun": return c;
    case "local": return mul(l.amount, mul(saturate(l.toward.y), 0.5));
  }
});

/**
 * The neon-tube style: a dark ground where each body is near black and its paint becomes a saturated light, shown as
 * the outline, an inner glow and a sheen. The water is black glass with faint violet swell lines and cyan rings round
 * bodies. Composed of `pixelsInside`, `swellLines`, the `reflection` and `bloom` post passes.
 */
export function neon(): Style {
  /** Light fades under the water. */
  const sunk = (depth: Expr<"f32">) => exp(mul(max(depth, 0), -1.2));
  return {
    name: "neon",
    outline: param("neon.outline", 0.035, { min: 0, max: 0.2 }),
    clear: NIGHT,
    post: [reflection({ gain: 0.12 }), bloom({ radius: 24, gain: 1.4, from: 0.25, to: 0.7 })],
    shade: (ctx) => {
      const glow = tubeOf(ctx.base);
      const tube = sub(1, smoothstep(1.5, 6, pixelsInside(ctx)));
      const fresnel = pow(sub(1, saturate(dot(ctx.normal, ctx.view))), 3);
      const own = add(mul(ctx.base, 0.035), mul(glow, add(mul(tube, 0.7), mul(fresnel, 0.3))));
      // Its light is the paint; a local light only lifts the near-black body a little (the tube still dominates).
      const lifted = ctx.lights("sum", (l, which) => {
        switch (which) {
          case "sun": return own;
          case "local": return mul(mul(ctx.base, l.amount), mul(saturate(dot(ctx.normal, l.toward)), 0.35));
        }
      });
      return mul(lifted, sunk(ctx.depth));
    },
    mark: () => f(1),
    ink: (ctx) => mul(tubeOf(ctx.base), sunk(ctx.depth)),
    water: (ctx) => {
      const lines = swellLines(ctx, { perWave: 4, width: [0.01, 0.03] });
      let c = add(rgb(WATER), mul(rgb(VIOLET), mul(lines.cover, 0.3)));
      c = printLayers(c, contactFoam(foam.rings({ count: 4, drift: 0.5 }), ctx, lines), { fill: CYAN, line: CYAN, edge: CYAN });
      return vec4(pools(ctx, mix(c, rgb(NIGHT), smoothstep(40, 180, ctx.distance))), 1);
    },
    shadow: (ctx) => vec4(0, 0, 0, mul(ctx.amount, 0.5)),
    sky: (ctx) => skyOf(SKY, ctx),
    // A plane is the dark floor; a cast shadow darkens it further.
    ground: (ctx) => pools(ctx, mul(rgb(WATER), sub(1, mul(ctx.shadow, 0.6)))),
  };
}

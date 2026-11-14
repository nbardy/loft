// Toon: banded light from the sun, a rim, a gloss, an ink hull, a translucent stylised sea. Shadows are coloured, not
// grey: this look was criticised as muddy against the water, so every term keeps chroma. Given a lighting recipe
// (`light`, or `--light name` from `LIGHT`), the recipe's treatments are laid over toon's own colour by `litBodies`:
// hatching in its shadows, a printed rim, cast shadows.
//
// Lights (src/light.ts): everything the sun gives (bands, rim, gloss) times its amount; each local light the paint in
// its hue, banded by N.L times the luminance arriving (rings where 0.1 and 0.3 arrive), with a gloss at its half
// vector. The plane takes the same rings. The sky, the fog and the sea are daylight scaled by the sun's amount: a local
// light does not reach the sea.

import { add, div, dot, exp, lift, max, mix, mul, normalize, param, pow, saturate, smoothstep, sub, vec3, vec4, type Expr, type In } from "../expr.ts";
import { luminance } from "../lib/index.ts";
import { fwidth } from "../lib/wgsl.ts";
import type { Rgb, ShadeCtx, Style } from "../style.ts";
import { LIGHT, litBodies, type Bodies, type LightRecipe } from "./light.ts";
import { rgb } from "./shared.ts";

export type ToonSpec = {
  /** Lighting bands. Default 3. */
  readonly bands?: number;
  /** Ink outline width, world units. Default 0.04. Becomes the param `toon.outline`. */
  readonly outline?: number;
  /** Shadow colour multiplier: shadows shift toward this hue instead of toward grey (keeps them saturated). */
  readonly shadow?: Rgb;
  readonly ink?: Rgb;
  readonly water?: { readonly shallow?: Rgb; readonly deep?: Rgb; readonly foam?: Rgb; readonly shade?: Rgb };
  readonly sky?: { readonly zenith?: Rgb; readonly horizon?: Rgb };
  readonly rim?: Rgb;
  /** A plane ground's colour (`scene({ ground: plane })`). */
  readonly ground?: Rgb;
  /** A lighting recipe laid over toon's colour (src/style/light.ts). Absent: toon as it is. Its block, unless stated, is toon's ink at half strength. */
  readonly light?: LightRecipe;
};

/**
 * A hard edge of x at `edge`, antialiased: as wide as x changes over about a pixel, never narrower than `soft`.
 * (A fixed width in n.l was thinner than a pixel wherever the surface turns fast, and those edges stair-stepped.)
 * Uncapped, unlike `edgeOf`: toon's frames predate the cap and stay as they were.
 */
const edgeOf = (x: Expr<"f32">, edge: number, soft: number): Expr<"f32"> => {
  const w = max(mul(fwidth(x), 0.75), soft);
  return smoothstep(sub(edge, w), add(edge, w), x);
};

/** A light's hue: its amount over its largest channel (so a lamp of [1.6, 1.0, 0.5] tints by [1, 0.63, 0.31]). */
const hueOf = (amount: In<"vec3">): Expr<"vec3"> => {
  const a = lift<"vec3">(amount);
  return div(a, max(max(max(a.x, a.y), a.z), 1e-4));
};

/**
 * A colour lit by the sun alone, times its amount (the sky, the sea and the fog are daylight colours): unchanged under
 * a full sun. A local light adds nothing here.
 */
const skyLit = (ctx: Pick<ShadeCtx, "lights">, c: Expr<"vec3">): Expr<"vec3"> => ctx.lights("largest", (l, which) => {
  switch (which) {
    case "sun": return mul(c, l.amount);
    case "local": return vec3(0, 0, 0);
  }
});

/**
 * The cartoon style: sun light in a few flat bands with coloured (not grey) shadows, a rim light, a small gloss, an ink
 * outline hull, and a translucent blue sea with foam. Every colour, the band count and the outline width can be set in
 * `ToonSpec`; a lighting recipe can be laid over it.
 */
export function toon(spec: ToonSpec = {}): Style {
  const bands = spec.bands ?? 3;
  if (!(Number.isInteger(bands) && bands >= 2)) throw new Error(`toon: bands must be an integer >= 2, got ${bands}`);
  const shadowTint = rgb(spec.shadow ?? [0.46, 0.42, 0.78]);
  const inkC: Rgb = spec.ink ?? [0.07, 0.04, 0.13];
  const inkColour = rgb(inkC);
  const shallow = rgb(spec.water?.shallow ?? [0.1, 0.62, 0.7]);
  const deep = rgb(spec.water?.deep ?? [0.03, 0.27, 0.45]);
  const foam = rgb(spec.water?.foam ?? [0.96, 0.99, 0.97]);
  const seaShadow = rgb(spec.water?.shade ?? [0.02, 0.17, 0.33]);
  const zenith = rgb(spec.sky?.zenith ?? [0.32, 0.6, 0.92]);
  const horizonC: Rgb = spec.sky?.horizon ?? [0.78, 0.9, 0.96];
  const horizon = rgb(horizonC);
  const rimColour = rgb(spec.rim ?? [0.75, 0.95, 1.0]);
  const outline = param("toon.outline", spec.outline ?? 0.04, { min: 0, max: 0.2 });

  /** 0..1 in `bands` flat steps over n.l, with narrow antialiased edges. */
  const banded = (ndl: Expr<"f32">): Expr<"f32"> => {
    let lit: Expr<"f32"> = lift<"f32">(0);
    for (let k = 0; k < bands - 1; k++) lit = add(lit, edgeOf(ndl, -0.2 + (0.7 * (k + 0.5)) / (bands - 1), 0.008));
    return mul(lit, 1 / (bands - 1));
  };

  /**
   * Bands of a local light from how much arrives, x = N.L times its luminance, 0 up. x from 0 to 0.4 is laid over the
   * stretch of N.L the bands are cut from (-0.2 to 0.5), so nothing arriving is the darkest band and, with three
   * bands, the rings fall where 0.1 arrives (the lighting recipes' `pool`) and 0.3. Banded on x itself, x = 0 printed
   * the middle band everywhere (N.L's first edge is at -0.025); laid over 0 to 1, a lure's 0.31 on a face lit nothing.
   */
  const lampBands = (x: Expr<"f32">): Expr<"f32"> => banded(add(mul(x, 0.7 / 0.4), -0.2));

  const fogOf = (distance: Expr<"f32">): Expr<"f32"> => { const f = sub(1, exp(mul(distance, -0.008))); return saturate(mul(f, f)); };

  /** Below the sea surface, colour sinks toward the deep water with depth. */
  const submerge = (c: Expr<"vec3">, depth: Expr<"f32">): Expr<"vec3"> =>
    mix(c, deep, mul(sub(1, exp(mul(max(depth, 0), -1.6))), 0.9));

  /** Toon's colour of a body under the sun from `sun`: bands, rim, gloss. */
  const sunlit = (ctx: ShadeCtx, sun: Expr<"vec3">): Expr<"vec3"> => {
    const ndl = dot(ctx.normal, sun);
    const lit = banded(ndl);
    const lightC = mix(shadowTint, vec3(1.08, 1.04, 0.96), lit);
    const facing = saturate(dot(ctx.normal, ctx.view));
    const rim = mul(edgeOf(sub(1, facing), 0.65, 0.01), mul(0.45, add(0.35, mul(lit, 0.65))));
    const half = normalize(add(sun, ctx.view));
    const gloss = mul(edgeOf(dot(ctx.normal, half), 0.9785, 0.0015), 0.16);
    return add(add(mul(ctx.base, lightC), mul(rimColour, rim)), vec3(gloss, gloss, gloss));
  };
  /**
   * A local light on a body: the paint in the light's hue, banded by how much arrives (N.L times its luminance), so a
   * lamp's falloff prints as flat rings; a gloss at its half vector. No rim and no shadow tint: those are the sun's.
   */
  const lamplit = (ctx: ShadeCtx, l: { readonly toward: Expr<"vec3">; readonly amount: In<"vec3"> }): Expr<"vec3"> => {
    const lum = luminance(l.amount);
    const band = lampBands(mul(saturate(dot(ctx.normal, l.toward)), lum));
    const gloss = mul(edgeOf(dot(ctx.normal, normalize(add(l.toward, ctx.view))), 0.9785, 0.0015), mul(saturate(lum), 0.16));
    return mul(add(mul(ctx.base, band), gloss), hueOf(l.amount));
  };
  /** Toon's colour of a body before it sinks: the sun's bands, rim and gloss times its amount, and each local light's. */
  const colour = (ctx: ShadeCtx): Expr<"vec3"> => ctx.lights("sum", (l, which) => {
    switch (which) {
      case "sun": return mul(sunlit(ctx, l.toward), l.amount);
      case "local": return lamplit(ctx, l);
    }
  });
  const own: Bodies = spec.light === undefined
    ? { shade: colour, mark: () => lift<"f32">(1) }
    : litBodies(spec.light, { ink: inkC, amount: 0.5, value: 0.8 }, colour);
  // A block printed after the bodies (cast shadows) prints over the colour before it sinks: a limit of the order.
  const sunk = (s: (ctx: ShadeCtx) => Expr<"vec3">) => (ctx: ShadeCtx) => submerge(s(ctx), ctx.depth);

  return {
    name: "toon",
    shade: sunk(own.shade),
    mark: own.mark,
    ...(own.block === undefined ? {} : { block: { ...own.block, body: sunk(own.block.body) } }),
    // Toon under each recipe of `LIGHT`: what `--light name` chooses.
    lights: Object.fromEntries(Object.entries(LIGHT).map(([name, r]) => [name, () => toon({ ...spec, light: r })])),
    outline,
    clear: horizonC,
    post: [],
    ink: (ctx) => submerge(mix(inkColour, mul(ctx.base, 0.3), 0.25), ctx.depth),
    water: (ctx) => {
      const facing = saturate(dot(ctx.normal, ctx.view));
      // Lighter on crests and toward grazing angles; a banded sun glint; distance fades to the horizon colour.
      const body = mix(deep, shallow, saturate(add(mul(ctx.crest, 0.35), 0.55)));
      const grazing = pow(sub(1, facing), 3);
      let c = mix(body, horizon, mul(grazing, 0.55));
      const half = normalize(add(ctx.sun, ctx.view));
      const glint = smoothstep(0.985, 0.99, dot(ctx.normal, half));
      c = mix(c, foam, mul(glint, 0.8));
      c = mix(c, foam, ctx.foam);
      c = skyLit(ctx, mix(c, horizon, fogOf(ctx.distance)));
      // Opaque where foam or grazing; see-through looking down, where submerged bodies show (already tinted).
      const alpha = saturate(max(add(0.55, mul(grazing, 0.45)), ctx.foam));
      return vec4(c, alpha);
    },
    // Laid over the fogged water: mixing the shadow colour in and then fogging (as one shader did before) equals
    // fogging the shadow colour and mixing it over the fogged water, which is what a layer can do.
    shadow: (ctx) => vec4(mix(seaShadow, horizon, fogOf(ctx.distance)), mul(ctx.amount, 0.55)),
    // A plane: one flat colour under toon's light, its cast shadow in the shadow tint, fogged to the horizon. The colour is
    // made here and not when the style is built: an expression created eagerly renumbers every sea scene's shader text.
    // Each local light lays rings of its hue on the floor, banded by L.y times its luminance, over the sun's colour.
    ground: (ctx) => {
      const floor = rgb(spec.ground ?? [0.86, 0.8, 0.68]);
      const light = ctx.lights("sum", (l, which) => {
        switch (which) {
          case "sun": return mul(mix(mix(shadowTint, vec3(1.08, 1.04, 0.96), banded(l.toward.y)), shadowTint, mul(ctx.shadow, 0.8)), l.amount);
          case "local": return mul(hueOf(l.amount), lampBands(mul(saturate(l.toward.y), luminance(l.amount))));
        }
      });
      return mix(mul(floor, light), skyLit(ctx, horizon), fogOf(ctx.distance));
    },
    // Daylight, scaled by the sun's amount: under a moon of 0.06 the same grade, dark and blue.
    sky: (ctx) => {
      const up = ctx.dir.y;
      const above = mix(horizon, zenith, pow(saturate(up), 0.6));
      // Below the horizon the "sky" is the deep water seen through the sea's translucency.
      return skyLit(ctx, mix(above, mix(horizon, deep, saturate(mul(up, -6))), sub(1, smoothstep(-0.002, 0.0, up))));
    },
  };
}

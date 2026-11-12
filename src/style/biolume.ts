// Biolume: a night sea. Sky and water near black; bodies dark, with their paint's bright and saturated features
// turned into light, cyan or magenta by hue; a rim of cold light on every silhouette; foam and the waterline glowing
// faintly where bodies break the water. Two post passes: the glow smeared down the water as a reflection, then bloom,
// which glows whatever is bright (in this picture, only what emits).

import { abs, add, dot, exp, fract, length, lift, max, min, mix, mul, normalize, param, pow, saturate, sin, smoothstep, sub, time, vec3, vec4, type Expr } from "../expr.ts";
import { hash22, luminance, noise2 } from "../lib/index.ts";
import { fwidth } from "../lib/wgsl.ts";
import type { PostSpec } from "../compile/context.ts";
import type { Contact, Rgb, Style } from "../style.ts";
import { POST_COMMON, rgb } from "./shared.ts";

const C = {
  cyan: [0.15, 0.95, 1.0],
  magenta: [1.0, 0.22, 0.85],
  night: [0.006, 0.012, 0.024],
  horizon: [0.02, 0.04, 0.07],
  water: [0.02, 0.045, 0.08],
  body: [0.035, 0.04, 0.07],
  moon: [0.35, 0.45, 0.7],
} as const satisfies Record<string, Rgb>;

/**
 * The glow's reflection: for a pixel of open water, light from bodies above it in the picture (a reflection in
 * gently moving water lies below what it reflects), gathered along a short column that wavers with the swell.
 */
const REFLECT = /* wgsl */ `${POST_COMMON}
@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> {
  let c = post_color(in.uv).rgb;
  let px = in.px.xy;
  if (sty_water(px) < 0.5) { return vec4<f32>(c, 1.0); }
  let scale = max(frame.viewport.y / 800.0, 0.375);
  var acc = vec3<f32>(0.0);
  for (var k = 1u; k <= 24u; k++) {
    let fk = f32(k);
    let wave = sin(px.y * 0.11 / scale + fk * 0.7 + frame.time.x * 1.3) * 2.5 * scale;
    let q = px + vec2<f32>(wave, -fk * 4.0 * scale);
    let p = scene_position(q);
    // Only bodies above the water reflect.
    if (sty_solid(q) && p.y > sea_height(p.xz)) {
      let s = post_color(q * frame.viewport.zw).rgb;
      acc += max(s - vec3<f32>(0.1), vec3<f32>(0.0)) * exp(-fk * 0.11);
    }
  }
  return vec4<f32>(c + acc * 0.09, 1.0);
}
`;

/**
 * Bloom in one pass: a spiral of taps out to R, weighted by a Gaussian, over the bright part of the picture. The
 * spiral turns from pixel to pixel (interleaved gradient noise, fixed to the screen) so its taps leave fine grain
 * instead of ghost copies.
 */
const BLOOM = /* wgsl */ `${POST_COMMON}
const BL_TAPS: u32 = 40u;
fn bl_bright(c: vec3<f32>) -> vec3<f32> {
  let l = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
  return c * smoothstep(0.12, 0.5, l);
}
@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> {
  let c = post_color(in.uv).rgb;
  let scale = max(frame.viewport.y / 800.0, 0.375);
  let px = in.px.xy;
  let turn = 6.2831853 * fract(52.9829189 * fract(dot(floor(px), vec2<f32>(0.06711056, 0.00583715))));
  var acc = vec3<f32>(0.0);
  var total = 0.0;
  for (var i = 0u; i < BL_TAPS; i++) {
    let t = (f32(i) + 0.5) / f32(BL_TAPS);
    let r = 30.0 * scale * sqrt(t);
    let a = f32(i) * 2.3999632 + turn;
    let w = exp(-3.0 * t);
    acc += bl_bright(post_color((px + vec2<f32>(cos(a), sin(a)) * r) * frame.viewport.zw).rgb) * w;
    total += w;
  }
  let glow = acc / total;
  // Added, then softly compressed so a bright core saturates to white-tinted colour instead of clipping flat.
  let lit = c + glow * 1.6;
  return vec4<f32>(lit / (1.0 + 0.25 * max(lit - vec3<f32>(0.8), vec3<f32>(0.0))), 1.0);
}
`;

export const BIOLUME_POST = {
  reflect: { label: "biolume reflection", wgsl: REFLECT },
  bloom: { label: "biolume bloom", wgsl: BLOOM },
} as const satisfies Record<string, PostSpec>;

/**
 * Light from a paint: as strong as the paint is bright and vivid; magenta for purples, pinks and reds (red and blue
 * both above green), cyan for the rest (creams, yellows, greens, blues).
 */
function glowOf(base: Expr<"vec3">): Expr<"vec3"> {
  const hi = max(max(base.x, base.y), base.z), lo = min(min(base.x, base.y), base.z);
  const vivid = sub(hi, lo);
  const strength = smoothstep(0.42, 0.7, add(mul(luminance(base), 0.45), mul(vivid, 0.6)));
  const purple = smoothstep(0.08, 0.22, sub(mul(add(base.x, base.z), 0.5), base.y));
  return mul(mix(rgb(C.cyan), rgb(C.magenta), purple), strength);
}

export function biolume(): Style {
  const outline = param("biolume.outline", 0.02, { min: 0, max: 0.1 });
  const fog = (d: Expr<"f32">): Expr<"f32"> => saturate(sub(1, exp(mul(d, -0.012))));
  return {
    name: "biolume",
    outline,
    clear: C.night,
    post: [BIOLUME_POST.reflect, BIOLUME_POST.bloom],
    shade: (ctx) => {
      const ndl = dot(ctx.normal, ctx.sun);
      const facing = saturate(dot(ctx.normal, ctx.view));
      // Dark skin under a faint moon, the paint's own hue barely kept.
      const skin = mul(mix(rgb(C.body), mul(ctx.base, 0.08), 0.5), add(0.5, mul(saturate(ndl), 0.8)));
      // Emission is brightest where the surface faces the eye and falls toward the silhouette, as light through
      // translucent tissue does; the rim, not the pattern, draws the outline.
      const glow = mul(glowOf(ctx.base), mul(0.8, add(0.2, mul(mul(facing, facing), 0.8))));
      const rim = mul(rgb(C.cyan), mul(pow(sub(1, facing), 3), 0.55));
      const c = add(add(skin, glow), rim);
      // Under water the body sinks into the dark, its light lasting longer than its skin.
      const sink = exp(mul(max(ctx.depth, 0), -1.1));
      return mix(rgb(C.water), mul(c, add(0.25, mul(sink, 0.75))), sink);
    },
    // The hull is a thin line of the rim's light.
    mark: () => lift<"f32">(1),
    ink: (ctx) => mul(rgb(C.cyan), mul(0.35, exp(mul(max(ctx.depth, 0), -1.5)))),
    water: (ctx) => {
      const facing = saturate(dot(ctx.normal, ctx.view));
      const xz = ctx.world.xz;
      // Faint moonlit swell, so the water reads as a surface and not a void: a broad sheen, glints, and sparse
      // plankton that twinkle (one per cell of a world-space lattice: they stay where they are as the camera moves).
      const half = normalize(add(ctx.sun, ctx.view));
      const spec = dot(ctx.normal, half);
      const sheen = add(mul(smoothstep(0.75, 1.0, spec), 0.22), mul(smoothstep(0.975, 0.99, add(spec, mul(noise2(mul(xz, 4)), 0.012))), 0.3));
      let c: Expr<"vec3"> = add(mul(rgb(C.water), add(0.75, mul(ctx.crest, 0.35))), mul(rgb(C.moon), sheen));
      // Moonlit ripple lines along the swell, faint, fading with distance before they can alias.
      const ripple = add(mul(ctx.phase, 6), mul(ctx.crest, 0.8));
      const rfw = max(fwidth(ripple), 1e-4);
      const rline = mul(sub(1, smoothstep(sub(0.06, rfw), add(0.06, rfw), abs(sub(fract(ripple), 0.5)))), sub(1, smoothstep(0.1, 0.3, rfw)));
      c = add(c, mul(rgb(C.moon), mul(rline, mul(add(0.5, mul(ctx.crest, 0.5)), 0.07))));
      const cell = mul(xz, 2.5);
      const h = hash22(cell);
      const spark = mul(smoothstep(0.93, 0.99, h.x), sub(1, smoothstep(0.0, 0.08, length(sub(fract(cell), add(mul(h, 0.6), 0.2))))));
      const twinkle = add(0.5, mul(sin(add(mul(time, 2.3), mul(h.y, 40))), 0.5));
      c = add(c, mul(rgb(C.cyan), mul(spark, mul(twinkle, 0.5))));
      // Where bodies disturb the water it glows: a soft halo through the contact band, the foam brighter on it,
      // flickering along its edge.
      const flicker = add(0.65, mul(noise2(mul(xz, 3.0)), 0.35));
      c = add(c, mul(rgb(C.cyan), add(halo(ctx.contact), mul(ctx.foam, mul(flicker, 0.75)))));
      c = mix(c, rgb(C.horizon), fog(ctx.distance));
      // Translucent looking down, so light under the surface shows; opaque toward grazing angles.
      const alpha = saturate(max(add(0.6, mul(pow(sub(1, facing), 3), 0.4)), ctx.foam));
      return vec4(c, alpha);
    },
    shadow: (ctx) => vec4(vec3(0, 0, 0), mul(ctx.amount, 0.3)),
    // A plane: a moonlit floor twice the water's tone (so a shadow reads on it), shadows near the night, fogged as the
    // water is.
    ground: (ctx) => mix(mul(rgb(C.water), sub(2.2, mul(ctx.shadow, 1.7))), rgb(C.horizon), fog(ctx.distance)),
    sky: (ctx) => {
      const up = ctx.dir.y;
      const above = mix(rgb(C.horizon), rgb(C.night), pow(saturate(up), 0.5));
      return mix(above, rgb(C.water), sub(1, smoothstep(-0.002, 0.0, up)));
    },
  };
}

/** The glow round a body breaking the water: saturate(band)^2 x 0.22; nothing where no body is near (0 there exactly). */
function halo(contact: Contact): Expr<"f32"> {
  switch (contact.kind) {
    case "clear": return lift<"f32">(0);
    case "near": { const near = saturate(contact.band); return mul(mul(near, near), 0.22); }
  }
}

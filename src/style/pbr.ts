// pbr: bodies lit as physical surfaces. GGX with height-correlated Smith visibility and Schlick's Fresnel for the sun
// and every light, a Lambert body under it, the sky as an environment (projected once per scene, a sharp tap for the
// mirror), and a filmic tone map. Built from parts (src/style/pbr_parts.ts), each a plain function: `pbr({ fresnel })`
// swaps one and leaves the others; `{ ...pbr(), ink }` replaces any member of the Style record.
//
// Words a paint may say: `rough` (0 mirror to 1 chalk; default 1 - `gloss`, so a scene written for toon keeps its
// gloss, and 0.5 when neither is given); `metal` (0 to 1: the specular takes the paint's colour, the diffuse goes);
// `emits` (linear light, added before the tone map, so above 1 it blooms toward white on its own); `normal` (a unit
// vector: the shading normal, for a bump; default the surface's own). A `scatter` takes the place of the diffuse term
// for each light, as in toon. `base` is sRGB as written and is linearised (x^2.2) before it is lit.
//
// Lights (src/light.ts): the sun and each local light through the same term, times its amount; a lamp with a shadow
// map arrives already shadowed. The sky's light is the daylight scaled by the sun's amount, so under a moon of 0.06 the
// sky is dark and a lamp carries the scene. The sea reflects the same sky by Fresnel with a glint per light; a plane
// ground is a rough grey-beige under the same lights, the sun's share darkened by its cast shadow.
//
// Cannot: occlusion (a crevice sees open sky), inter-reflection, transmission; a highlight smaller than a pixel
// sparkles (no antialiasing of specular: keep `rough` above about 0.1 on small curved things).

import { add, dot, evaluate, exp, lift, max, mix, mul, normalize, param, pow, saturate, smoothstep, sub, vec3, vec4, type In } from "../expr.ts";
import type { F, P3 } from "../map/types.ts";
import type { ShadeCtx, Style } from "../style.ts";
import { aces, daylight, ggx, lambert, schlick, skyLight, smithGgx, type PbrParts } from "./pbr_parts.ts";

const DEFAULTS: PbrParts = { distribution: ggx, visibility: smithGgx, fresnel: schlick, diffuse: lambert, environment: skyLight, sky: daylight, tone: aces };

const WATER_DEEP = vec3(0.003, 0.03, 0.07);
const WATER_SHALLOW = vec3(0.01, 0.1, 0.15);
const FOAM = vec3(0.8, 0.82, 0.82);
const FLOOR = vec3(0.66, 0.62, 0.54);
const WATER_ROUGH = 0.12;

/** sRGB as written to linear (x^2.2). */
const linear = (c: In<"vec3">): P3 => pow(max(lift<"vec3">(c), 1e-6), 2.2);

/**
 * The specular lobe's albedo under the default distribution and visibility (`ggx`, `smithGgx`): `e`, the integral of
 * D Vis n.l over the hemisphere of light directions with F = 1, and `t`, the share of it Schlick's (1 - v.h)^5 carries, so f0 e (1 - t)
 * + e t is the split sum's f0 A + B. Fitted by least squares (n.v from 0.05) to a numerical integration of these parts
 * on a 24 x 24 grid of n.v and rough: within 0.06 of f0 A + B at f0 = 1 and 0.04 at f0 = 0.04, mean 0.01. Karis's
 * analytic fit (2014), made for another visibility, is up to 0.47 off for these parts (agent_notes/2026-11-18_pbr_style/NOTE.md).
 * A replaced distribution or visibility keeps this weight: write your own `shade` if its albedo differs much.
 */
function lobeAlbedo(n_v: F, rough: F): { readonly e: F; readonly t: F } {
  const mu = max(n_v, 0.05), r = max(rough, 0.05);
  const poly = (c0: number, c1: number, c2: number): F => add(c0, mul(mu, add(c1, mul(mu, c2))));
  const e = saturate(add(1, mul(mul(r, r), add(poly(-2.407, 5.998, -2.676), mul(r, add(poly(5.343, -18.319, 9.583), mul(r, poly(-3.119, 11.39, -6.479))))))));
  const t = mul(pow(sub(1, mu), add(4.879, mul(r, -3.565))), exp(mul(r, add(-0.698, mul(r, -4.272)))));
  return { e, t };
}

/**
 * The environment's specular weight for a Fresnel law of any shape: e fresnel(c, f0), c the cosine at which Schlick's
 * (1 - c)^5 equals t (`lobeAlbedo`). For `schlick` it is exactly f0 A + B; a thin film or a measured law is read at the
 * angle the split sum says the lobe averages to.
 */
function envWeight(p: PbrParts, n_v: F, rough: F, f0: P3): P3 {
  const { e, t } = lobeAlbedo(n_v, rough);
  return mul(p.fresnel(saturate(sub(1, pow(max(t, 1e-6), 0.2))), f0), e);
}

/** One light's radiance off a surface per the parts: specular pi D Vis F n.l, diffuse (1 - F) times the diffuse part. */
function lightTerm(p: PbrParts, n: P3, v: P3, l: P3, albedo: P3, f0: P3, rough: F, diffuseOf: (term: P3) => P3): P3 {
  const n_l = saturate(dot(n, l)), n_v = max(dot(n, v), 1e-4);
  // A light straight behind the eye's ray would make l + v zero: a little of the normal keeps h defined.
  const h = normalize(add(add(l, v), mul(n, 1e-4)));
  const n_h = saturate(dot(n, h)), v_h = saturate(dot(v, h));
  const F = p.fresnel(v_h, f0);
  const spec = mul(F, mul(mul(p.distribution(n_h, rough), p.visibility(n_l, n_v, rough)), mul(n_l, Math.PI)));
  return add(diffuseOf(mul(sub(1, F), p.diffuse(albedo, n_l, n_v, v_h, rough))), spec);
}

/** The sun's amount at a fold (the sky is daylight: a local light adds nothing to it). */
const sunAmount = (lights: ShadeCtx["lights"]): P3 => lights("largest", (l, which) => {
  switch (which) {
    case "sun": return lift<"vec3">(l.amount);
    case "local": return vec3(0, 0, 0);
  }
});

/**
 * A physically based style from parts. `parts` replaces any of `PbrParts`' defaults; the rest stay. The outline is a
 * thin hull in the body's colour darkened (`pbr.outline`, 0.01 world units, never under the host's 1.5 px).
 */
export function pbr(parts: Partial<PbrParts> = {}): Style {
  const p: PbrParts = { ...DEFAULTS, ...parts };
  const fog = (distance: F): F => { const f = sub(1, exp(mul(distance, -0.008))); return saturate(mul(f, f)); };
  /** What sits on the horizon behind a point seen along `view`: the fog's colour. */
  const haze = (view: P3, sun: P3): P3 => p.sky(normalize(add(vec3(mul(view.x, -1), 0, mul(view.z, -1)), vec3(0, 0.02, 0))), sun);
  /** Under the sea, colour sinks toward the deep water with depth (linear, before the tone map). */
  const submerge = (c: P3, depth: F, amount: P3): P3 => mix(c, mul(WATER_DEEP, amount), mul(sub(1, exp(mul(max(depth, 0), -1.6))), 0.9));

  /** A body's radiance: every light, the sky, its own light. */
  const radiance = (ctx: ShadeCtx): P3 => {
    const base = linear(ctx.base);
    const rough = ctx.channel<"f32">("rough", sub(1, ctx.channel<"f32">("gloss", 0.5)));
    const metal = ctx.channel<"f32">("metal", 0);
    const n = ctx.channel<"vec3">("normal", ctx.normal);
    const albedo = mul(base, sub(1, metal));
    const f0 = mix(vec3(0.04, 0.04, 0.04), base, metal);
    const direct = ctx.lights("sum", (l) => mul(lightTerm(p, n, ctx.view, l.toward, albedo, f0, rough, (d) => ctx.scatter(l.toward, d)), l.amount));
    const env = p.environment({ normal: n, view: ctx.view, sun: ctx.sun }, rough, p.sky);
    const spec = envWeight(p, max(dot(n, ctx.view), 1e-4), rough, f0);
    const sky = add(mul(mul(albedo, env.diffuse), sub(1, spec)), mul(env.specular, spec));
    const lit = add(add(direct, mul(sky, sunAmount(ctx.lights))), ctx.channel<"vec3">("emits", [0, 0, 0]));
    return submerge(lit, ctx.depth, sunAmount(ctx.lights));
  };

  const outline = param("pbr.outline", 0.01, { min: 0, max: 0.2 });
  const up = vec3(0, 1, 0);
  // The clear colour: the default sky on the horizon under a full sun overhead, as the picture shows it.
  const clear = evaluate(p.tone(p.sky(vec3(1, 0, 0), up))) as number[];

  return {
    name: "pbr",
    shade: (ctx) => p.tone(radiance(ctx)),
    ink: (ctx) => p.tone(submerge(mul(linear(ctx.base), mul(sunAmount(ctx.lights), 0.12)), ctx.depth, sunAmount(ctx.lights))),
    mark: () => lift<"f32">(1),
    outline,
    clear: [clear[0]!, clear[1]!, clear[2]!],
    post: [],
    water: (ctx) => {
      const n = ctx.normal, v = ctx.view, n_v = max(dot(n, v), 1e-4);
      const f0 = vec3(0.02, 0.02, 0.02), amount = sunAmount(ctx.lights);
      const env = p.environment({ normal: n, view: v, sun: ctx.sun }, lift<"f32">(WATER_ROUGH), p.sky);
      const reflects = envWeight(p, n_v, lift<"f32">(WATER_ROUGH), f0);
      const body = mix(WATER_DEEP, WATER_SHALLOW, saturate(add(mul(ctx.crest, 0.35), 0.45)));
      // The water's body: light scattered back up from below, from the sky and from each light by how high it stands.
      const under = add(mul(env.diffuse, amount), ctx.lights("sum", (l) => mul(l.amount, saturate(l.toward.y))));
      const glints = ctx.lights("sum", (l) => mul(lightTerm(p, n, v, l.toward, vec3(0, 0, 0), f0, lift<"f32">(WATER_ROUGH), (d) => d), l.amount));
      const water = add(add(mul(mul(body, under), sub(1, reflects)), mul(mul(env.specular, amount), reflects)), glints);
      const foam = mul(FOAM, add(mul(env.diffuse, amount), ctx.lights("sum", (l) => mul(l.amount, saturate(dot(n, l.toward))))));
      const c = mix(mix(water, foam, ctx.foam), mul(haze(v, ctx.sun), amount), fog(ctx.distance));
      // See-through looking down, where submerged bodies show (already sunk); opaque toward grazing and under foam.
      const alpha = saturate(max(add(0.6, mul(pow(sub(1, n_v), 3), 0.4)), ctx.foam));
      return vec4(p.tone(c), alpha);
    },
    shadow: (ctx) => vec4(p.tone(mix(mul(WATER_DEEP, 0.3), haze(ctx.view, vec3(0, 1, 0)), fog(ctx.distance))), mul(ctx.amount, 0.6)),
    ground: (ctx) => {
      const albedo = linear(FLOOR), f0 = vec3(0.04, 0.04, 0.04), rough = lift<"f32">(0.9);
      const direct = ctx.lights("sum", (l, which) => {
        const term = mul(lightTerm(p, up, ctx.view, l.toward, albedo, f0, rough, (d) => d), l.amount);
        switch (which) {
          case "sun": return mul(term, sub(1, ctx.shadow));
          case "local": return term;
        }
      });
      const amount = sunAmount(ctx.lights);
      const env = p.environment({ normal: up, view: ctx.view, sun: ctx.sun }, rough, p.sky);
      const lit = add(direct, mul(mul(albedo, env.diffuse), amount));
      return p.tone(mix(lit, mul(haze(ctx.view, ctx.sun), amount), fog(ctx.distance)));
    },
    // Below the horizon the background is the deep water lit from above, seen through the translucent sea (as toon's
    // is); the environment keeps the sky's own ground, which is what lights a body from below.
    sky: (ctx) => p.tone(mul(mix(p.sky(ctx.dir, ctx.sun), mul(WATER_DEEP, 1.2), smoothstep(0, -0.02, ctx.dir.y)), sunAmount(ctx.lights))),
  };
}


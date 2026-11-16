// Light: the sun, and any light an author writes as a function of a world point (docs/lighting.md, "Lights").
//
// The sun is fixed when the scene is written, or a host input a game or a clip moves. A fixed sun is a literal in every
// shader (as it always was). A movable one is the unit vector along a host input, whose default is the direction the
// scene states: `--input sun=x,y,z`, `--sweep sun=a..b`, or a game writing the input. What is computed on the CPU before
// a frame (a cloud's own light tables, src/soft/medium.ts) reads the stated direction either way and does not follow a
// moved sun. The sun also has an amount: full (1, the sun at noon) unless the scene states one (a moon, or none).
//
// Every other light is a `Light`: for a world point, what arrives there. Styles never see a list of them: a shading
// context offers one fold, `ctx.lights(combine, term)`, in which the style says what one light does and the compiler
// does the loop (so it stays free to unroll it, cull it or defer it later).

import { LIGHT_SAMPLE_INPUT } from "./artefact.ts";
import { hash22 } from "./lib/noise.ts";
import { add, cos, cross, div, dot, fract, input, lift, max, mul, normalize, saturate, select, sin, sqrt, sub, vec2, vec3, abs, lt, type Expr, type In, type NumTy } from "./expr.ts";
import type { TraceBound } from "./draw/trace.ts";

export type V3 = readonly [number, number, number];

export type Sun =
  | { readonly kind: "fixed"; readonly dir: V3 }
  /** `dir` is the input's default, a unit vector. */
  | { readonly kind: "input"; readonly name: string; readonly dir: V3 };

/** A sun a host can move: the input `name` (default "sun"), starting at `dir`. */
export function movable(dir: V3, name = "sun"): { readonly kind: "input"; readonly name: string; readonly dir: V3 } {
  return { kind: "input", name, dir };
}

/** The unit direction toward the sun, as shaders read it. */
export function sunDir(s: Sun): In<"vec3"> {
  switch (s.kind) {
    case "fixed": return s.dir;
    case "input": return normalize(input(s.name, "vec3", s.dir));
  }
}

/** The same, by component (a table's program divides by the height). */
export function sunParts(s: Sun): readonly [In<"f32">, In<"f32">, In<"f32">] {
  switch (s.kind) {
    case "fixed": return s.dir;
    case "input": { const d = sunDir(s) as Expr<"vec3">; return [d.x, d.y, d.z]; }
  }
}

/**
 * What arrives at a point from one light.
 * - `toward`: unit direction from the point toward the light.
 * - `amount`: the light arriving, a linear colour; 1 is the sun at noon, and it may exceed 1.
 * - `distance`: how far along `toward` the light is (where a shadow ray stops); absent means at infinity, as the sun is.
 * - `size`: the light's apparent radius from the point, radians. Only a still or a soft shadow reads it.
 */
export type Arrival = {
  readonly toward: Expr<"vec3">;
  readonly amount: In<"vec3">;
  readonly distance?: In<"f32">;
  readonly size?: In<"f32">;
};

/** Where a light is, for a later shadow-map pass: a point, or a direction for a light at infinity. */
export type LightSource = { readonly kind: "point"; readonly at: Expr<"vec3"> } | { readonly kind: "direction"; readonly toward: In<"vec3"> };

/** A lamp's own shadow, asked for with `shadow: true`: the compiler draws a depth picture from `at` (DESIGN.md 17). */
export type LampShadow = { readonly at: Expr<"vec3">; readonly reach: number; readonly size: number };

/**
 * Where a light is folded: a surface (its unit normal biases a shadow map's test by the receiver's own plane), or a
 * point with no normal (a glazed surface, the sky), which a shadow map treats as facing the light.
 */
export type Receiver = { readonly kind: "surface"; readonly at: Expr<"vec3">; readonly normal: Expr<"vec3"> } | { readonly kind: "point"; readonly at: Expr<"vec3"> };

/** How much of a light's arrival reaches a receiver past whatever stands between: 0 to 1. */
export type ShadowTest = (r: Receiver, lit: Arrival) => Expr<"f32">;

/**
 * A light: for a world point, what arrives there (`Arrival`). Any expression of the point, of time, inputs and tables.
 *
 * The law is yours. Loft evaluates it at every lit fragment (bodies, a plane, the sea) and on the CPU (`litBy`, a cloud
 * lit from inside). `xi`, a sample in the unit square, is for a light with extent (a still averages samples); a plain
 * light ignores it and nothing passes it yet.
 * Cannot: arrive from more than one direction at a point (an area light, a sky dome, two lamps through one function: a
 * function that picks the nearest shows a seam where the nearest changes); be skipped where it is dark (each light is a
 * term in every lit fragment's shader, about 0.02 to 0.05 ms on the largest example); cast a shadow unless it computes
 * one; light a cloud except at its blobs' centres. At most 7 besides the sun.
 * Optional facts a helper attaches so later passes can use them; an author writing a function leaves them out:
 *  - `bound`: no light arrives outside it (lets the compiler skip the light's term; not read yet).
 *  - `source`: where the light is, for a shadow-map pass (not read yet).
 * Relatives: the sum of two neighbours' arrivals by a weighted average of directions (no seam, wrong in a small way
 * where they disagree); a light read from a baked grid (`fetch`); an occluder written inside the function.
 */
export type Light = ((p: Expr<"vec3">, xi?: Expr<"vec2">) => Arrival) & {
  readonly bound?: TraceBound;
  readonly source?: LightSource;
  /** Set by `lamp(..., { shadow: true })`: the compiler shadows this light from a depth picture (DESIGN.md 17). */
  readonly shadow?: LampShadow;
};

/**
 * A point light: `colour` x reach^2 / (reach^2 + d^2) x (1 - x^2)^2, x = d / (4 reach), nothing past 4 reaches. Half of
 * `colour` arrives at `reach`. `colour` 1 is the sun at noon; it may exceed 1.
 *
 * Parameters: `at` and `colour` take expressions (it moves; it flickers: mul(colour, flicker)); `reach` is a positive
 * number of world units; `size` (default 0) the radius of a disc it shines from, facing the lit point: a frame lights
 * from the disc's centre, a still (`--samples`) from a point on the disc per sample (`xi`), so a shadow the light
 * function casts and the edge of its pool come out soft. It carries `bound` (a sphere of 4 reaches, when `at` is a
 * constant) and `source` (the point). `shadow: true` (default false) casts a shadow from a depth picture taken from
 * the lamp each frame (DESIGN.md 17): every opaque thing casts, onto the ground and onto things, its edge as soft as
 * `size` makes it (PCSS; hard but smooth at size 0). The picture is framed on the things' skeletons each frame, at
 * most 70 degrees off its axis; a lamp the things surround (a flame in its own paper) is refused at compile.
 * Cannot: cast shadows without `shadow` (light passes through every thing); be shadowed by a transparent or soft thing,
 * a thing behind the view's camera, or one more than 70 degrees off the picture's axis; be shadowed on the CPU (a cloud
 * lit at its blobs, `litBy`); light the outside of a thing it is inside (the paper of a lantern: give the paper
 * `emits`); be soft in a frame without `shadow` (its size is otherwise sampled only in a still).
 * Relatives: a spot is this times smoothstep over dot(-toward, axis); a gobo is this times a pattern of -toward; a tube
 * is this from the closest point on a segment; a pool from a table is `fetch` of a baked grid at p; a row of lamps is
 * one function that picks the nearest by `floor` (a seam where it changes: fade each pool before the midpoint).
 */
export function lamp(at: In<"vec3">, o: { readonly colour: In<"vec3">; readonly reach: number; readonly size?: number; readonly shadow?: boolean }): Light {
  if (!(o.reach > 0 && Number.isFinite(o.reach))) throw new Error(`lamp: reach must be a positive number of world units, got ${o.reach}`);
  const size = o.size ?? 0;
  if (!(size >= 0 && Number.isFinite(size))) throw new Error(`lamp: size must be a radius of zero or more world units, got ${o.size}`);
  const centre = lift<"vec3">(at);
  const r2 = o.reach * o.reach, edge = 4 * o.reach;
  const light = (p: Expr<"vec3">, xi?: Expr<"vec2">): Arrival => {
    const from = xi === undefined || size === 0 ? centre : onDisc(centre, p, size, xi);
    const v = sub(from, p);
    const d2 = dot(v, v);
    const d = sqrt(d2);
    const x2 = saturate(mul(d2, 1 / (edge * edge)));
    const window = mul(sub(1, x2), sub(1, x2));
    const arrival = { toward: normalize(v), amount: mul(o.colour, mul(window, div(r2, add(d2, r2)))), distance: d };
    return size === 0 ? arrival : { ...arrival, size: div(size, max(d, size)) };
  };
  const bound: TraceBound | undefined = Array.isArray(at) ? { kind: "sphere", centre: [at[0]!, at[1]!, at[2]!], radius: edge } : undefined;
  const shadow: LampShadow | undefined = o.shadow === true ? { at: centre, reach: o.reach, size } : undefined;
  return Object.assign(light, { source: { kind: "point", at: centre } as const, ...(bound === undefined ? {} : { bound }), ...(shadow === undefined ? {} : { shadow }) });
}

/** A point on the disc of radius `r` about `c` facing `p`, uniform in area over xi in the unit square. */
function onDisc(c: Expr<"vec3">, p: Expr<"vec3">, r: number, xi: Expr<"vec2">): Expr<"vec3"> {
  const axis = normalize(sub(p, c));
  const side = select(lt(abs(axis.y), 0.9), vec3(0, 1, 0), vec3(1, 0, 0));
  const t1 = normalize(cross(axis, side)), t2 = cross(axis, t1);
  const rho = mul(sqrt(xi.x), r), phi = mul(xi.y, 2 * Math.PI);
  return add(c, add(mul(t1, mul(rho, cos(phi))), mul(t2, mul(rho, sin(phi)))));
}

/** The sun's amount: full unless the scene states one. */
export type SunAmount = { readonly kind: "full" } | { readonly kind: "given"; readonly amount: In<"vec3"> };

/**
 * How lights are evaluated: at their representative point (a frame: `xi` is not given), or sampled (a still: each light
 * gets `xi`, the sample's point r2(k) from the host input `LIGHT_SAMPLE_INPUT`, rotated per pixel by a hash of the
 * point and per light by the golden ratio (Cranley-Patterson), so the average over a still's samples covers each
 * light's extent).
 */
export type Sampling = { readonly kind: "point" } | { readonly kind: "sampled" };

/**
 * Everything that lights a scene, as `scene()` classified it. `shadows` holds the compiler's test for each light that
 * asked for a shadow map (src/frame/lamp_shadow.ts); `scene()` leaves it empty, the compiler fills it.
 */
export type Lighting = { readonly sun: Sun; readonly amount: SunAmount; readonly lights: readonly Light[]; readonly sampling: Sampling; readonly shadows: ReadonlyMap<Light, ShadowTest> };

/**
 * The full sun's amount: this literal, so a style that multiplies by it adds no node (`mul` returns its other operand
 * for a constant 1) and every scene written before lights keeps its shader text byte for byte.
 */
const FULL: V3 = [1, 1, 1];

/** How a fold combines the lights' terms: their sum, or the largest (componentwise). */
export type Combine = "sum" | "largest";
/** Which light a term is for: the sun (the key, at infinity) or a local light (`Light`). A fact of the build. */
export type Which = "sun" | "local";

/**
 * Every light at a point, folded: `term` says what one light does here, `combine` how the lights' terms join. The sun's
 * term comes first, then each of the scene's lights in order. With no lights it is the sun's term itself.
 */
export type LightFold = <T extends NumTy>(combine: Combine, term: (lit: Arrival, which: Which) => Expr<T>) => Expr<T>;

/** The sun's arrival. */
export function sunArrival(l: Lighting): Arrival {
  const toward = lift<"vec3">(sunDir(l.sun));
  switch (l.amount.kind) {
    case "full": return { toward, amount: FULL };
    case "given": return { toward, amount: l.amount.amount };
  }
}

/**
 * The fold at a receiver. Each light is evaluated only inside a call, so a style that never asks creates no
 * expression (an expression created and not used still renumbers every later one's name in the shader text). A light
 * with a shadow map arrives multiplied by its test here, before the style sees it.
 */
export function foldAt(l: Lighting, r: Receiver): LightFold {
  return <T extends NumTy>(combine: Combine, term: (lit: Arrival, which: Which) => Expr<T>): Expr<T> => {
    let acc = term(sunArrival(l), "sun");
    for (const [i, light] of l.lights.entries()) {
      const t = term(shadowed(l.shadows.get(light), r, arriving(light, r.at, l.sampling, i)), "local");
      acc = (combine === "sum" ? add(acc as Expr<NumTy>, t as Expr<NumTy>) : max(acc as Expr<NumTy>, t as Expr<NumTy>)) as Expr<T>;
    }
    return acc;
  };
}

/** An arrival past its shadow map's test, when the light has one. */
function shadowed(test: ShadowTest | undefined, r: Receiver, a: Arrival): Arrival {
  return test === undefined ? a : { ...a, amount: mul(a.amount, test(r, a)) };
}

/** Light `i` at p: at its point, or at the still's sample (see `Sampling`). */
function arriving(light: Light, p: Expr<"vec3">, s: Sampling, i: number): Arrival {
  switch (s.kind) {
    case "point": return light(p);
    case "sampled": {
      // A hash of the point at about 2 mm (a pixel's rotation, fixed across the samples of a still), and the light's
      // own golden-ratio offset, so two lights do not share their samples.
      const cell = hash22(vec2(dot(p, [512, 0, 911]), dot(p, [0, 512, 0])));
      return light(p, fract(add(add(input(LIGHT_SAMPLE_INPUT, "vec2", [0.5, 0.5]), cell), [0.618034 * i, 0.754878 * i])));
    }
  }
}

/** The same lighting with the sun alone: what the sky sees (a local light has no meaning at infinity). */
export const sunOnly = (l: Lighting): Lighting => ({ ...l, lights: [] });

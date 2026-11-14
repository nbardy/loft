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

import { add, div, dot, input, lift, max, mul, normalize, saturate, sqrt, sub, type Expr, type In, type NumTy } from "./expr.ts";
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
};

/**
 * A point light: `colour` x reach^2 / (reach^2 + d^2) x (1 - x^2)^2, x = d / (4 reach), nothing past 4 reaches. Half of
 * `colour` arrives at `reach`. `colour` 1 is the sun at noon; it may exceed 1.
 *
 * Parameters: `at` and `colour` take expressions (it moves; it flickers: mul(colour, flicker)); `reach` is a positive
 * number of world units. It carries `bound` (a sphere of 4 reaches, when `at` is a constant) and `source` (the point).
 * Cannot: cast shadows (the floor under a body standing in its pool is lit; light passes through every thing); light
 * the outside of a thing it is inside (the paper of a lantern: give the paper `emits`); have size (its highlights are
 * points).
 * Relatives: a spot is this times smoothstep over dot(-toward, axis); a gobo is this times a pattern of -toward; a tube
 * is this from the closest point on a segment; a pool from a table is `fetch` of a baked grid at p; a row of lamps is
 * one function that picks the nearest by `floor` (a seam where it changes: fade each pool before the midpoint).
 */
export function lamp(at: In<"vec3">, o: { readonly colour: In<"vec3">; readonly reach: number }): Light {
  if (!(o.reach > 0 && Number.isFinite(o.reach))) throw new Error(`lamp: reach must be a positive number of world units, got ${o.reach}`);
  const centre = lift<"vec3">(at);
  const r2 = o.reach * o.reach, edge = 4 * o.reach;
  const light = (p: Expr<"vec3">): Arrival => {
    const v = sub(centre, p);
    const d2 = dot(v, v);
    const d = sqrt(d2);
    const x2 = saturate(mul(d2, 1 / (edge * edge)));
    const window = mul(sub(1, x2), sub(1, x2));
    return { toward: normalize(v), amount: mul(o.colour, mul(window, div(r2, add(d2, r2)))), distance: d };
  };
  const bound: TraceBound | undefined = Array.isArray(at) ? { kind: "sphere", centre: [at[0]!, at[1]!, at[2]!], radius: edge } : undefined;
  return Object.assign(light, { source: { kind: "point", at: centre } as const, ...(bound === undefined ? {} : { bound }) });
}

/** The sun's amount: full unless the scene states one. */
export type SunAmount = { readonly kind: "full" } | { readonly kind: "given"; readonly amount: In<"vec3"> };

/** Everything that lights a scene, as `scene()` classified it. */
export type Lighting = { readonly sun: Sun; readonly amount: SunAmount; readonly lights: readonly Light[] };

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
 * The fold at world point `p`. Each light is evaluated only inside a call, so a style that never asks creates no
 * expression (an expression created and not used still renumbers every later one's name in the shader text).
 */
export function foldAt(l: Lighting, p: Expr<"vec3">): LightFold {
  return <T extends NumTy>(combine: Combine, term: (lit: Arrival, which: Which) => Expr<T>): Expr<T> => {
    let acc = term(sunArrival(l), "sun");
    for (const light of l.lights) {
      const t = term(light(p), "local");
      acc = (combine === "sum" ? add(acc as Expr<NumTy>, t as Expr<NumTy>) : max(acc as Expr<NumTy>, t as Expr<NumTy>)) as Expr<T>;
    }
    return acc;
  };
}

/** The same lighting with the sun alone: what the sky sees (a local light has no meaning at infinity). */
export const sunOnly = (l: Lighting): Lighting => ({ ...l, lights: [] });

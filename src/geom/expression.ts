// Expressions as values (docs/animation_principles.md, Appeal).
//
// A face is drawn from a few named channels (how far a lid is down, how tall a stalk stands, where it looks). An
// expression is one number per channel. A score lays expressions over one another by weights that are closed forms of
// a clock (an attack's beats, a window of seconds), so the posed channels are closed forms of the clock too: no state
// between frames, readable at any other time (a part that lags reads `score.at(sub(time, lag))`).
//
// Nothing here knows what a channel means: the face that draws them does.

import { add, lift, mix, mul, smoothstep, sub, type In } from "../expr.ts";
import type { F } from "../map/types.ts";

/** An expression: one number per channel. */
export type Expression<K extends string> = Readonly<Record<K, number>>;
/** Channels as closed forms of the clock: what a face is drawn from. */
export type Posed<K extends string> = Readonly<Record<K, F>>;

/** A cast: the rest expression and the named ones, each complete (one number for every channel). */
export type Cast<K extends string, N extends string> = {
  readonly channels: readonly K[];
  readonly rest: Expression<K>;
  readonly named: Readonly<Record<N, Expression<K>>>;
};

export class ExpressionError extends Error {}

/**
 * A cast from its rest expression and each named expression's departures from rest: a channel a named expression does
 * not mention is the rest's. A named expression that mentions a channel the rest lacks, or a number that is not finite,
 * is an `ExpressionError`.
 */
export function cast<K extends string, N extends string>(rest: Expression<K>, named: Readonly<Record<N, Partial<Expression<K>>>>): Cast<K, N> {
  const channels = Object.keys(rest) as K[];
  const finite = (who: string, k: string, x: number) => { if (!Number.isFinite(x)) throw new ExpressionError(`expression "${who}": channel "${k}" is ${x}, not a finite number`); };
  for (const k of channels) finite("rest", k, rest[k]);
  const out = {} as Record<N, Expression<K>>;
  for (const name of Object.keys(named) as N[]) {
    if (name === "rest") throw new ExpressionError(`an expression cannot be named "rest": that is the cast's own`);
    const departs = named[name] as Readonly<Record<string, number>>;
    for (const k of Object.keys(departs)) {
      if (!(k in rest)) throw new ExpressionError(`expression "${name}": no channel "${k}" (channels: ${channels.join(", ")})`);
      finite(name, k, departs[k]!);
    }
    out[name] = { ...rest, ...departs } as Expression<K>;
  }
  return { channels, rest, named: out };
}

/** One layer of a score. Layers apply in order, later over earlier. */
export type Layer<K extends string, N extends string> =
  /** The whole face goes toward a named expression by `weight` (0 not at all, 1 entirely). */
  | { readonly kind: "show"; readonly show: N; readonly weight: In<"f32"> }
  /** Only the listed channels go toward it: a blink moves the lids and leaves the rest of the face alone. */
  | { readonly kind: "only"; readonly show: N; readonly channels: readonly K[]; readonly weight: In<"f32"> }
  /** Added on top of whatever is showing, `by` per channel times `weight`: a tremble, a breath. */
  | { readonly kind: "add"; readonly by: Partial<Readonly<Record<K, In<"f32">>>>; readonly weight: In<"f32"> };

export const show = <N extends string>(name: N, weight: In<"f32">): Layer<never, N> => ({ kind: "show", show: name, weight });
export const only = <K extends string, N extends string>(name: N, channels: readonly K[], weight: In<"f32">): Layer<K, N> => ({ kind: "only", show: name, channels, weight });
export const added = <K extends string>(by: Partial<Readonly<Record<K, In<"f32">>>>, weight: In<"f32"> = 1): Layer<K, never> => ({ kind: "add", by, weight });

/**
 * A weight from a clock: 0 before `from`, rising over `rise` seconds, 1 until `to`, falling over `fall` seconds after
 * it. The clock is any expression of seconds (an attack's `tau`, a beat's `after`); `to` may be omitted for a weight
 * that stays.
 */
export function during(clock: In<"f32">, o: { readonly from: number; readonly to?: number; readonly rise: number; readonly fall?: number }): F {
  if (!(o.rise > 0)) throw new ExpressionError(`during: rise must be a positive number of seconds, got ${o.rise}`);
  const up = smoothstep(o.from, o.from + o.rise, lift<"f32">(clock));
  if (o.to === undefined) return up;
  const fall = o.fall ?? o.rise;
  if (!(fall > 0)) throw new ExpressionError(`during: fall must be a positive number of seconds, got ${fall}`);
  if (!(o.to >= o.from)) throw new ExpressionError(`during: the window ends (${o.to}) before it starts (${o.from})`);
  return mul(up, sub(1, smoothstep(o.to, o.to + fall, lift<"f32">(clock))));
}

export type Score<K extends string, N extends string> = {
  readonly cast: Cast<K, N>;
  /** The channels at clock t. */
  readonly at: (t: In<"f32">) => Posed<K>;
  /**
   * How much of the face each named expression is at clock t, after every later `show` layer has taken its share; the
   * shares and `rest` sum to 1. `only` and `add` layers do not count: they never make the face a named expression.
   */
  readonly shares: (t: In<"f32">) => Readonly<Record<N | "rest", F>>;
};

/**
 * A score: the cast's rest expression under the layers `layersAt(t)` gives for a clock, applied in order. The layers'
 * weights are the caller's closed forms of that clock, so the same score read at `t - lag` is the same performance late.
 */
export function score<K extends string, N extends string>(cast_: Cast<K, N>, layersAt: (t: F) => readonly Layer<NoInfer<K>, NoInfer<N>>[]): Score<K, N> {
  const known = (name: N): Expression<K> => {
    const e = cast_.named[name];
    if (e === undefined) throw new ExpressionError(`score: no expression "${name}" in the cast (named: ${Object.keys(cast_.named).join(", ")})`);
    return e;
  };
  const channel = (k: K): K => {
    if (!cast_.channels.includes(k)) throw new ExpressionError(`score: no channel "${k}" (channels: ${cast_.channels.join(", ")})`);
    return k;
  };
  const at = (t: In<"f32">): Posed<K> => {
    const posed = Object.fromEntries(cast_.channels.map((k) => [k, lift<"f32">(cast_.rest[k])])) as Record<K, F>;
    for (const layer of layersAt(lift<"f32">(t))) {
      // The one place a layer's kind is read.
      switch (layer.kind) {
        case "show": { const e = known(layer.show); for (const k of cast_.channels) posed[k] = mix(posed[k], e[k], layer.weight); break; }
        case "only": { const e = known(layer.show); for (const k of layer.channels) posed[channel(k)] = mix(posed[k], e[k], layer.weight); break; }
        case "add": {
          for (const k of Object.keys(layer.by) as K[]) posed[channel(k)] = add(posed[k], mul(layer.by[k] as In<"f32">, layer.weight));
          break;
        }
      }
    }
    return posed;
  };
  const shares = (t: In<"f32">): Readonly<Record<N | "rest", F>> => {
    const names = Object.keys(cast_.named) as N[];
    const share = Object.fromEntries([["rest", lift<"f32">(1)], ...names.map((n) => [n, lift<"f32">(0)])]) as Record<N | "rest", F>;
    for (const layer of layersAt(lift<"f32">(t))) {
      switch (layer.kind) {
        case "show": {
          known(layer.show);
          for (const n of Object.keys(share) as (N | "rest")[]) share[n] = mul(share[n], sub(1, layer.weight));
          share[layer.show] = add(share[layer.show], layer.weight);
          break;
        }
        case "only": case "add": break;
      }
    }
    return share;
  };
  return { cast: cast_, at, shares };
}

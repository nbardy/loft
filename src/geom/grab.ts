// A grab: a gripper reaches for a thing, closes on it by its size, carries it, squeezes it and lets it go.
// Everything is a closed formula of one clock (seconds since the grab began) and of the thing's place and size, which
// may be host inputs. Nothing is kept between frames, so it runs wherever pose runs.
//
// Three facts the formulas are built on:
//   1. The grip opens before it arrives, and by the thing's size. In reach-to-grasp the aperture peaks about two
//      thirds of the way through the reach and grows linearly with the object (Jeannerod 1984). So the opening is a
//      function of the thing's radius, not a fixed pose, and a small ship and a large one are taken differently.
//   2. A squeezed thing keeps its volume: narrowed to k across the grip, it grows by 1 / sqrt(k) along the other two.
//   3. A body of fixed length wrapped round a thing makes length / (length of one turn) turns. A bigger thing gets
//      fewer turns from the same body, and tightening the coil adds turns. `wrap` derives the turn count; nobody sets it.
//
// One gripper differs from another only in how it turns the gap into a pose: `fingerAngle` for claws and jaws,
// `wrap` for coils and tentacles.

import { add, atan2, clamp, cos, cross, div, lift, lt, max, mix, mul, normalize, select, sin, sqrt, sub, vec3, type Expr, type In } from "../expr.ts";
import type { Size } from "../map/types.ts";
import { anticipated, arc, slowIn, slowInOut, slowOut } from "./lively.ts";

type F = Expr<"f32">;
type P3 = Expr<"vec3">;
/** A size along each axis: what a squeeze always is, so a caller can read its lengths without asking its kind. */
export type AxesSize = Extract<Size, { kind: "axes" }>;
const TAU = Math.PI * 2;

/** Seconds in each part of a grab. `hold` is the carry and the squeeze together. */
export type GrabBeats = { readonly reach: number; readonly close: number; readonly hold: number; readonly release: number };

export type GrabSpec = {
  /** Where the gripper waits, and returns to. */
  readonly rest: In<"vec3">;
  /** Where the thing is when it is taken. */
  readonly from: In<"vec3">;
  /** Where it is let go. Leave it out to hold the thing where it was taken. */
  readonly to?: In<"vec3">;
  /** Half the thing's width across the grip. The opening and the closed gap both follow from it. */
  readonly radius: In<"f32">;
  /** How far the reach (and the return) arches off the straight line, along `up`. Default 0. */
  readonly reachLift?: In<"f32">;
  /** How far the carry arches. With no `to`, this is how high the thing is raised before it is set back down. Default 0. */
  readonly carryLift?: In<"f32">;
  readonly up?: In<"vec3">;
  /** How much wider than the thing the grip opens, as a fraction of the thing's width. Default 0.5. */
  readonly margin?: number;
  /** The gap at rest. Default: the open gap (the gripper waits open). */
  readonly restGap?: In<"f32">;
  /** The fraction of the thing's width crushed at the peak of a squeeze: 0 (default) holds without squeezing. */
  readonly squeeze?: number;
  /** How many times it squeezes during the hold. Default 1. */
  readonly pulses?: number;
  /**
   * Anticipation: how far the gripper first draws back from the thing before it reaches, as `anticipated`'s `back`
   * (1.7 draws back a tenth of the way). Default 0: it leaves gently and arrives gently. With a wind-up it leaves
   * the wrong way and arrives at speed.
   */
  readonly windup?: number;
};

export type Grab = {
  /** The middle of the grip. */
  readonly palm: P3;
  /** The distance between the gripping surfaces: wide on the way in, the thing's width when closed, less when squeezing. */
  readonly gap: F;
  /** The thing's place: where it was until taken, with the palm while held, where it was let go after. */
  readonly thing: P3;
  /**
   * The thing's size, `base` times: narrowed along `across` (the axis of the thing the grip closes on; default x)
   * while squeezed, grown along the other two, volume kept.
   */
  squeezed(base?: In<"f32">, across?: "x" | "y" | "z"): AxesSize;
  /** The thing's width across the grip as a fraction of its own: 1 unsqueezed, 1 - squeeze at the peak. */
  readonly crush: F;
  /** 1 while the thing is in the grip, else 0. The game's rule for "this ship is held" reads the same number. */
  readonly held: F;
};

/** A grab on the clock `since` (seconds since it began; before 0 the gripper is at rest). */
export function grab(since: In<"f32">, spec: GrabSpec, b: GrabBeats): Grab {
  for (const [k, v] of Object.entries(b)) if (!(v > 0 && Number.isFinite(v))) throw new Error(`grab: ${k} must be a positive, finite number of seconds; got ${v}`);
  const margin = spec.margin ?? 0.5, depth = spec.squeeze ?? 0, pulses = spec.pulses ?? 1;
  if (!(margin > 0)) throw new Error(`grab: margin must be positive (the grip has to open wider than the thing); got ${margin}`);
  if (!(depth >= 0 && depth < 1)) throw new Error(`grab: squeeze must be in [0, 1) (a fraction of the thing's width); got ${depth}`);
  if (!(Number.isInteger(pulses) && pulses >= 1)) throw new Error(`grab: pulses must be a positive integer (each squeeze ends released); got ${pulses}`);

  const t = lift<"f32">(since), from = lift<"vec3">(spec.from), to = lift<"vec3">(spec.to ?? spec.from), rest = lift<"vec3">(spec.rest);
  const up = spec.up ?? [0, 1, 0], reachLift = spec.reachLift ?? 0, carryLift = spec.carryLift ?? 0;
  const tClosed = b.reach + b.close, tLetGo = tClosed + b.hold;
  const uReach = clamp(div(t, b.reach), 0, 1), uClose = clamp(div(sub(t, b.reach), b.close), 0, 1);
  const uHold = clamp(div(sub(t, tClosed), b.hold), 0, 1), uRelease = clamp(div(sub(t, tLetGo), b.release), 0, 1);
  // Release is two halves: the grip opens where it is, then the gripper withdraws. Withdrawing while still closed
  // would drag the thing it has just let go.
  const uOpen = clamp(mul(uRelease, 2), 0, 1), uWithdraw = clamp(sub(mul(uRelease, 2), 1), 0, 1);

  const carry = arc(from, to, slowInOut(uHold), carryLift, up);
  // With a wind-up the reach parameter dips below 0 first; `arc` clamps at 0, so the part below is added back as a
  // straight draw away from the thing.
  const windup = spec.windup ?? 0;
  if (!(windup >= 0)) throw new Error(`grab: windup must be at least 0, got ${windup}`);
  const drive = windup > 0 ? anticipated(uReach, windup) : slowInOut(uReach);
  const reaching = add(arc(rest, from, drive, reachLift, up), mul(sub(rest, from), max(mul(drive, -1), 0)));
  const palm = select(lt(t, tClosed), reaching, select(lt(t, tLetGo), carry, arc(to, rest, slowInOut(uWithdraw), reachLift, up)));

  const width = mul(spec.radius, 2), open = mul(width, 1 + margin), restGap = lift<"f32">(spec.restGap ?? open);
  const pulse = sin(mul(uHold, Math.PI * pulses)), crush = sub(1, mul(depth, mul(pulse, pulse)));
  // The opening is complete at 0.65 of the reach (fact 1), and the grip closes at speed: it lands, it does not settle.
  const arriving = mix(mix(restGap, open, slowInOut(clamp(div(uReach, 0.65), 0, 1))), width, slowIn(uClose));
  const leaving = mix(mix(width, open, slowOut(uOpen)), restGap, slowInOut(uWithdraw));
  const gap = select(lt(t, tClosed), arriving, select(lt(t, tLetGo), mul(width, crush), leaving));

  const across = div(1, sqrt(max(crush, 1e-3)));
  return {
    palm, gap, crush,
    thing: select(lt(t, tClosed), from, select(lt(t, tLetGo), carry, to)),
    squeezed: (base = 1, axis = "x") => ({ kind: "axes", k: mul(vec3(axis === "x" ? crush : across, axis === "y" ? crush : across, axis === "z" ? crush : across), base) }),
    held: select(lt(t, tClosed), 0, select(lt(t, tLetGo), 1, 0)),
  };
}

/**
 * The angle each of two fingers (or the two halves of a jaw) stands off the grip's forward axis so that their tips are
 * `gap` apart: fingers of `length`, hinged `hinge` apart at the palm. A gap the fingers cannot span gives a right angle.
 * For a curved finger, `length` is its chord (hinge to tip, straight) and the result is the chord's angle: subtract the
 * angle the chord makes with the finger's root to get the hinge's.
 */
export function fingerAngle(gap: In<"f32">, length: In<"f32">, hinge: In<"f32"> = 0): F {
  const s = clamp(div(sub(gap, hinge), mul(length, 2)), -1, 1);
  return atan2(s, sqrt(max(sub(1, mul(s, s)), 0)));
}

/**
 * A pincer: one finger fixed, one moving, both of `length` (hinge to tip, straight) from one hinge. The angle between
 * them for their tips to be `gap` apart: gap = 2 length sin(angle / 2). A crab's claw, a beak.
 */
export function pincerAngle(gap: In<"f32">, length: In<"f32">): F {
  const s = clamp(div(gap, mul(length, 2)), 0, 1);
  return mul(2, atan2(s, sqrt(max(sub(1, mul(s, s)), 0))));
}

export type WrapSpec = {
  /** The middle of the thing wrapped. */
  readonly centre: In<"vec3">;
  /** The direction the coil advances along (unit): the thing's long axis. */
  readonly axis: In<"vec3">;
  /** The side the coil starts on. Must not lie along `axis`. */
  readonly toward: In<"vec3">;
  /** The coil's own radius: the thing's radius plus the body's. Multiply the thing's part by `crush` to tighten. */
  readonly radius: In<"f32">;
  /** How much of the body's length is spent on the coil. */
  readonly length: In<"f32">;
  /** How far the coil advances per turn. At least the body's width, or the turns overlap. */
  readonly pitch: In<"f32">;
};

/**
 * A coil round a thing, by its size. The number of turns is derived from the body's length (fact 3), so the same body
 * goes round a small ship more times than round a large one, and takes up more turns as it tightens. `spine(m)` is the
 * coil by arc length, m in [0, 1], centred on `centre`: blend a strand's free spine into it as the grip closes.
 */
export function wrap(spec: WrapSpec): { readonly turns: F; spine(m: F): P3 } {
  const axis = lift<"vec3">(spec.axis), centre = lift<"vec3">(spec.centre), r = lift<"f32">(spec.radius), pitch = lift<"f32">(spec.pitch);
  const e2 = normalize(cross(axis, lift<"vec3">(spec.toward))), e1 = cross(e2, axis);
  const around = mul(r, TAU), turns = div(spec.length, sqrt(add(mul(around, around), mul(pitch, pitch))));
  return {
    turns,
    spine(m) {
      const angle = mul(mul(turns, m), TAU), along = mul(mul(pitch, turns), sub(m, 0.5));
      return add(add(centre, mul(axis, along)), mul(r, add(mul(e1, cos(angle)), mul(e2, sin(angle)))));
    },
  };
}

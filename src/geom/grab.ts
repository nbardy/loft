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
//
// How a hold ends is a sum type (`Ending`): set down where the carry ends, or thrown (docs/throw_and_shove.md). A throw
// is built on one rule: the thing's velocity is the same on both sides of every hand-off. The carry ends at rest at the
// wind-back place; the stroke accelerates uniformly from there to the release velocity; the flight is ballistic; the
// water takes the thing at its touchdown velocity (a drag along, a damped heave down). The gripper follows through
// along the throw, decelerating, and goes home.

import { add, atan2, clamp, cos, cross, div, dot, exp, lift, lt, max, mix, mul, normalize, select, sin, sqrt, sub, vec3, type Expr, type In } from "../expr.ts";
import type { Frame, Size } from "../map/types.ts";
import { anticipated, arc, slowIn, slowInOut, slowOut } from "./lively.ts";
import { placedBy, type Placer } from "./placing.ts";

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
  /** How the hold ends. Default: set down where the carry ends (`SET_DOWN`). */
  readonly end?: Ending;
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
  const from = lift<"vec3">(spec.from), to = lift<"vec3">(spec.to ?? spec.from), up = lift<"vec3">(spec.up ?? [0, 1, 0]);
  const carryLift = spec.carryLift ?? 0;
  const t = lift<"f32">(since);
  const h = heldThing(sub(t, b.reach + b.close), b.hold, from, (u) => arc(from, to, slowInOut(u), carryLift, up), spec.squeeze ?? 0, spec.pulses ?? 1, spec.end ?? SET_DOWN);
  return gripper(t, spec, b, h);
}

// ---- how a hold ends ----------------------------------------------------------------------------------------------

/** Set down: the carry ends, at rest, where the thing is let go; the grip opens there and then withdraws. */
export type SetDown = { readonly kind: "setDown" };
/**
 * Thrown: the carry ends at rest at the wind-back place; over the hold's last `stroke` seconds the thing is accelerated
 * uniformly to the release velocity, which is whatever lands it at `land` after `flight` seconds under `gravity`
 * (`throwOf`). On the water it skids on under a linear drag (time constant `drag`) and heaves as a damped spring
 * (`heave`), both started with its touchdown velocity. The gripper follows through for `follow` seconds.
 *
 * That flight and landing are Loft's stand-in. `placer` says who places the thing once it is let go: the stand-in, or
 * the host, whose own physics takes it from the release point at the release velocity (`throwOf`) and whose pose is
 * then where the thing is. Up to the release the two are the same throw.
 */
export type Thrown = {
  readonly kind: "thrown";
  /** Where the thing's middle touches down, at the height it floats or rests at. */
  readonly land: P3;
  /** Seconds in the air. */
  readonly flight: number;
  /** Gravity, world units per second squared, along -up. */
  readonly gravity: number;
  /** Seconds of the hold's end spent accelerating the thing to its release velocity. Shorter than the hold. */
  readonly stroke: number;
  /** Seconds the gripper follows through after letting go, decelerating to rest along the throw. Shorter than its release. */
  readonly follow: number;
  /** Seconds for the water to take off the thing's speed along (1 / e): it skids `drag` times that speed and stops. */
  readonly drag: number;
  /** The heave after touchdown: its frequency (Hz) and damping ratio (below 1). The plunge's depth follows from the touchdown speed. */
  readonly heave: { readonly hz: number; readonly damping: number };
  /** Up (unit). */
  readonly up: P3;
  /** Who places the thing after the release. A host's pose is of the thing's middle (the point `hold.at` is). */
  readonly placer: Placer;
};
/** How a hold ends. One handler per kind (`courseOf`); nothing else asks which. */
export type Ending = SetDown | Thrown;

export const SET_DOWN: SetDown = { kind: "setDown" };

export type ThrownSpec = Omit<Thrown, "kind" | "land" | "up"> & { readonly land: In<"vec3">; /** Default +y. */ readonly up?: In<"vec3"> };
/** A hold that ends in a throw (see `Thrown`). Every number is checked here, once. */
export function thrown(o: ThrownSpec): Thrown {
  const positive = { flight: o.flight, gravity: o.gravity, stroke: o.stroke, follow: o.follow, drag: o.drag, "heave.hz": o.heave.hz };
  for (const [k, v] of Object.entries(positive)) if (!(v > 0 && Number.isFinite(v))) throw new Error(`thrown: ${k} must be a positive, finite number; got ${v}`);
  if (!(o.heave.damping > 0 && o.heave.damping < 1)) throw new Error(`thrown: heave.damping must be in (0, 1) (a heave that swings at least once); got ${o.heave.damping}`);
  return { kind: "thrown", land: lift<"vec3">(o.land), flight: o.flight, gravity: o.gravity, stroke: o.stroke, follow: o.follow, drag: o.drag, heave: o.heave, up: lift<"vec3">(o.up ?? [0, 1, 0]), placer: o.placer };
}

/** A throw's derived places and velocities: what a game reads, and what the checks measure the motion against. */
export type Throw = {
  /** Where the carry ends at rest and the stroke begins: behind the release along the throw. */
  readonly windback: P3;
  /** Where the thing is let go, and its velocity there (the gripper's too). */
  readonly release: P3;
  readonly velocity: P3;
  /** Where it touches down, and where it comes to rest after its skid. */
  readonly land: P3;
  readonly rest: P3;
  /** Its velocity along the water (constant through the flight) and its speed down at touchdown. */
  readonly along: P3;
  readonly down: F;
};

/**
 * The throw from wind-back place W. The flight `P + v0 s - g s^2 / 2` must reach `land` at s = flight, and the release
 * point is itself `P = W + v0 stroke / 2` (uniform acceleration from rest), so
 *   v0 = ((land - W) / T + up g T / 2) / (1 + stroke / (2 T)).
 */
export function throwOf(windback: P3, e: Thrown): Throw {
  const T = e.flight;
  const v0 = mul(add(mul(sub(e.land, windback), 1 / T), mul(e.up, (e.gravity * T) / 2)), 1 / (1 + e.stroke / (2 * T)));
  const rise = dot(v0, e.up), along = sub(v0, mul(e.up, rise));
  return { windback, release: add(windback, mul(v0, e.stroke / 2)), velocity: v0, land: e.land, rest: add(e.land, mul(along, e.drag)), along, down: sub(e.gravity * T, rise) };
}

/** A gripper after it lets go: its palm, how far its grip has opened (0..1) and how far it has gone home (0..1). */
export type Leaving = { readonly palm: P3; readonly open: F; readonly home: F };

/** What an ending decides, as closed forms on the hold's clock. Built by one handler per `Ending`; read without asking the kind. */
type Course = {
  /** The thing's place t seconds after it is taken (t >= 0): through the hold and ever after. */
  readonly place: (t: F) => P3;
  /** The squeezes' clock, 0..1 over the part of the hold they are spread across. */
  readonly squeezing: (t: F) => F;
  /** 1 while the thing is in the air, else 0. */
  readonly flying: (t: F) => F;
  /** Where the gripper lets go. */
  readonly letGo: P3;
  /** The gripper s seconds after letting go, with `over` seconds to open and go home; `home(from, u)` is its way home from a place. */
  readonly leave: (s: F, over: number, home: (from: P3, u: F) => P3) => Leaving;
};

function courseOf(end: Ending, seconds: number, carry: (u: F) => P3): Course {
  switch (end.kind) {
    case "setDown": return setDownCourse(seconds, carry);
    case "thrown": return thrownCourse(end, seconds, carry);
  }
}

function setDownCourse(seconds: number, carry: (u: F) => P3): Course {
  const to = carry(lift<"f32">(1)), u = (t: F): F => clamp(div(t, seconds), 0, 1);
  return {
    place: (t) => select(lt(t, seconds), carry(u(t)), to),
    squeezing: u,
    flying: () => lift<"f32">(0),
    letGo: to,
    // Two halves: the grip opens where it is, then the gripper withdraws. Withdrawing while still closed would drag the
    // thing it has just let go.
    leave: (s, over, home) => {
      const r = clamp(div(s, over), 0, 1), open = clamp(mul(r, 2), 0, 1), back = clamp(sub(mul(r, 2), 1), 0, 1);
      return { palm: home(to, back), open, home: back };
    },
  };
}

function thrownCourse(e: Thrown, seconds: number, carry: (u: F) => P3): Course {
  if (!(e.stroke < seconds)) throw new Error(`thrown: the stroke (${e.stroke} s) must be shorter than the hold (${seconds} s): the carry before it needs time`);
  const carrying = seconds - e.stroke, T = e.flight;
  const th = throwOf(carry(lift<"f32">(1)), e);
  const w = 2 * Math.PI * e.heave.hz, wd = w * Math.sqrt(1 - e.heave.damping ** 2);
  const u = (t: F): F => clamp(div(t, carrying), 0, 1);
  // Uniform acceleration from rest at the wind-back place: v0 at the release, `stroke` seconds later.
  const stroke = (t: F): P3 => { const tau = sub(t, carrying); return add(th.windback, mul(th.velocity, mul(mul(tau, tau), 1 / (2 * e.stroke)))); };
  const flight = (s: F): P3 => add(add(th.release, mul(th.velocity, s)), mul(e.up, mul(mul(s, s), -e.gravity / 2)));
  // On the water, q seconds after touchdown, with the touchdown's own velocity: along, a linear drag (it skids
  // `along * drag` and stops); down, a damped spring about the height it floats at (it plunges, comes back up past it,
  // and settles). q is held at 0 before touchdown: exp(-q / drag) would overflow there, in the branch not taken.
  const landed = (t: F): P3 => {
    const q = max(sub(t, seconds + T), 0);
    const skid = mul(th.along, mul(sub(1, exp(mul(q, -1 / e.drag))), e.drag));
    const plunge = mul(mul(div(th.down, -wd), exp(mul(q, -e.heave.damping * w))), sin(mul(q, wd)));
    return add(add(e.land, skid), mul(e.up, plunge));
  };
  // Once let go, the thing is where its placer has it: the stand-in's flight and landing, or the host's own pose.
  const free = (t: F): P3 => placedBy(e.placer, {
    standIn: () => select(lt(t, seconds + T), flight(sub(t, seconds)), landed(t)),
    host: (pose) => pose.at,
  });
  return {
    place: (t) => select(lt(t, carrying), carry(u(t)), select(lt(t, seconds), stroke(t), free(t))),
    squeezing: u,
    flying: (t) => select(lt(t, seconds), 0, select(lt(t, seconds + T), 1, 0)),
    letGo: th.release,
    // The gripper leaves the release point at the release velocity and decelerates uniformly to rest over `follow`
    // seconds (v0 follow / 2 further along the throw), its grip opening as it goes; then it goes home from there.
    leave: (s, over, home) => {
      if (!(e.follow < over)) throw new Error(`thrown: the follow-through (${e.follow} s) must be shorter than the gripper's release (${over} s): it still has to go home`);
      const k = clamp(div(s, e.follow), 0, 1), back = clamp(div(sub(s, e.follow), over - e.follow), 0, 1);
      const through = add(th.release, mul(th.velocity, mul(sub(k, mul(mul(k, k), 0.5)), e.follow)));
      return { palm: select(lt(s, e.follow), through, home(add(th.release, mul(th.velocity, e.follow / 2)), back)), open: k, home: back };
    },
  };
}

/**
 * A thing's axes in the air: `g` turned about `axis` (unit, fixed in space, through g's origin) by `turns` whole turns
 * spread evenly over the flight, whose share gone is `u` (0 before, 1 after: `Attack.flight().u`). A free body turns at
 * a constant rate, and whole turns bring it down the way up it left.
 */
export function tumbled(g: Frame, axis: In<"vec3">, turns: number, u: In<"f32">): Frame {
  if (!Number.isInteger(turns)) throw new Error(`tumbled: turns must be a whole number (the thing lands the way up it left); got ${turns}`);
  const a = lift<"vec3">(axis), angle = mul(clamp(u, 0, 1), turns * TAU), c = cos(angle), s = sin(angle);
  // Rodrigues' rotation of each axis.
  const turn = (v: P3): P3 => add(add(mul(v, c), mul(cross(a, v), s)), mul(a, mul(sub(1, c), dot(a, v))));
  return { o: g.o, x: turn(g.x), y: turn(g.y), z: turn(g.z) };
}

/** A held thing on the hold's own clock: where it is and how it is squeezed. Shared by `grab` and an attack's hold. */
export type Held = {
  /** Where it is taken, and where it is let go (set down: the carry's end; thrown: the release point). */
  readonly from: P3;
  readonly to: P3;
  /** Its place: `from` until taken, carried during the hold, then as its ending says (where it was set down; or flying, then where it landed). */
  readonly thing: P3;
  /** 1 while it is in the air after a throw, by the stand-in's flight time; else 0 (always 0 when set down). */
  readonly flying: F;
  /** The gripper s seconds after letting go, with `over` seconds to open and go home by `home(from, u)`. */
  readonly leave: (s: F, over: number, home: (from: P3, u: F) => P3) => Leaving;
  /** Its width across the grip as a fraction of its own: 1 unsqueezed, 1 - squeeze at the peak. */
  readonly crush: F;
  /** 1 while held, else 0. */
  readonly held: F;
  /** Its size, `base` times: narrowed along `across` while squeezed, grown along the other two, volume kept. */
  squeezed(base?: In<"f32">, across?: "x" | "y" | "z"): AxesSize;
};

/**
 * The thing a hold owns, on `since` (seconds since it was taken; negative before) for a hold of `seconds`: `carry(u)`
 * is its place at u in [0, 1] of the carry (u = 0 must be `from`), squeezed `pulses` times to `1 - squeeze` of its width
 * while carried; `end` is how the hold ends (set down: the carry is the whole hold).
 */
export function heldThing(since: In<"f32">, seconds: number, from: P3, carry: (u: F) => P3, squeeze: number, pulses: number, end: Ending): Held {
  if (!(seconds > 0 && Number.isFinite(seconds))) throw new Error(`hold: seconds must be a positive, finite number; got ${seconds}`);
  if (!(squeeze >= 0 && squeeze < 1)) throw new Error(`hold: squeeze must be in [0, 1) (a fraction of the thing's width); got ${squeeze}`);
  if (!(Number.isInteger(pulses) && pulses >= 1)) throw new Error(`hold: pulses must be a positive integer (each squeeze ends released); got ${pulses}`);
  const t = lift<"f32">(since), c = courseOf(end, seconds, carry);
  const pulse = sin(mul(c.squeezing(t), Math.PI * pulses)), crush = sub(1, mul(squeeze, mul(pulse, pulse)));
  const across = div(1, sqrt(max(crush, 1e-3)));
  return {
    from, to: c.letGo, crush, leave: c.leave,
    thing: select(lt(t, 0), from, c.place(t)),
    flying: select(lt(t, 0), 0, c.flying(t)),
    held: select(lt(t, 0), 0, select(lt(t, seconds), 1, 0)),
    squeezed: (base = 1, axis = "x") => ({ kind: "axes", k: mul(vec3(axis === "x" ? crush : across, axis === "y" ? crush : across, axis === "z" ? crush : across), base) }),
  };
}

/** How a gripper moves, apart from the thing it takes: `GrabSpec` without the thing's place and carry. */
export type GripperSpec = Pick<GrabSpec, "rest" | "radius" | "reachLift" | "up" | "margin" | "restGap" | "windup">;

/**
 * The gripper round a held thing `h`, on the grab's clock `since` (seconds since the reach began) and its beats: it
 * reaches for `h.from`, closes, follows `h.thing` through the hold (b.hold must be the hold's seconds), lets go at `h.to`
 * as the hold's ending says (`h.leave`: it opens where it is and withdraws, or follows a throw through) and goes home to `rest`.
 */
export function gripper(since: In<"f32">, spec: GripperSpec, b: GrabBeats, h: Held): Grab {
  for (const [k, v] of Object.entries(b)) if (!(v > 0 && Number.isFinite(v))) throw new Error(`grab: ${k} must be a positive, finite number of seconds; got ${v}`);
  const margin = spec.margin ?? 0.5;
  if (!(margin > 0)) throw new Error(`grab: margin must be positive (the grip has to open wider than the thing); got ${margin}`);

  const t = lift<"f32">(since), rest = lift<"vec3">(spec.rest), from = h.from;
  const up = spec.up ?? [0, 1, 0], reachLift = spec.reachLift ?? 0;
  const tClosed = b.reach + b.close, tLetGo = tClosed + b.hold;
  const uReach = clamp(div(t, b.reach), 0, 1), uClose = clamp(div(sub(t, b.reach), b.close), 0, 1);
  // After letting go: the hold's ending says where the palm is, how far the grip has opened and how far home it is.
  const leaving = h.leave(sub(t, tLetGo), b.release, (at, u) => arc(at, rest, slowInOut(u), reachLift, up));

  // With a wind-up the reach parameter dips below 0 first; `arc` clamps at 0, so the part below is added back as a
  // straight draw away from the thing.
  const windup = spec.windup ?? 0;
  if (!(windup >= 0)) throw new Error(`grab: windup must be at least 0, got ${windup}`);
  const drive = windup > 0 ? anticipated(uReach, windup) : slowInOut(uReach);
  const reaching = add(arc(rest, from, drive, reachLift, up), mul(sub(rest, from), max(mul(drive, -1), 0)));
  const palm = select(lt(t, tClosed), reaching, select(lt(t, tLetGo), h.thing, leaving.palm));

  const width = mul(spec.radius, 2), open = mul(width, 1 + margin), restGap = lift<"f32">(spec.restGap ?? open);
  // The opening is complete at 0.65 of the reach (fact 1), and the grip closes at speed: it lands, it does not settle.
  const arriving = mix(mix(restGap, open, slowInOut(clamp(div(uReach, 0.65), 0, 1))), width, slowIn(uClose));
  const opening = mix(mix(width, open, slowOut(leaving.open)), restGap, slowInOut(leaving.home));
  const gap = select(lt(t, tClosed), arriving, select(lt(t, tLetGo), mul(width, h.crush), opening));

  return { palm, gap, crush: h.crush, thing: h.thing, squeezed: h.squeezed, held: h.held };
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

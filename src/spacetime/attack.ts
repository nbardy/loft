// An attack defined once, everything derived from it (DESIGN.md 8). The simulation supplies a hit shape, an arming
// time and a warning duration (host inputs); the attack poses the creature's spine through time in closed form, so
// the body at any other time is the same expressions with `time` replaced (`body.at(t)`), which is what the
// telegraph, the smear, the wake and the checks read. Nothing is stored between frames.
//
// The motion: the striking end of the spine is mixed from its rest posture toward a striking posture by
//   spine(m) = mix(rest(m), pose(m, q), w(m) p)
// at the part's own clock (a retarded time: the motion runs along the body from or toward the striking end). `p` is
// how far the body is in the attack's posture (it coils in during the wind-up and lets go in the recovery), `q` is the
// strike's progress (0 coiled, 1 landed, below 0 the anticipation before the strike). `q` is a second-order step
// response normalised by its own first peak, so it is exactly 1 at the arming time and never above 1: the striking end
// lands where the hit shape says at that instant and never travels past it (it stops at the target's hull).

import {
  add, clamp, compileJs, div, length, lift, mix, mul, normalize, smoothstep, sub, time, variable, vec2, vec3, type Expr, type In,
} from "../expr.ts";
import { response } from "../curve.ts";
import type { Drawable } from "../compile/context.ts";
import type { Body } from "../map/sweep.ts";
import type { Strand } from "../geom/strand.ts";
import { hitInradius, laneOf, polar, type Arc, type Capsule, type Disc, type Hit } from "./hit.ts";

type F = Expr<"f32">;
type P3 = Expr<"vec3">;
type V2 = Expr<"vec2">;
export type Spine = (m: F) => P3;

/** What an attack must not pass into: a disc on the water (a vertical cylinder), or a body's skeleton. */
export type Target =
  | { readonly kind: "disc"; readonly centre: V2; readonly radius: F }
  | { readonly kind: "body"; readonly body: Body };

/** A stretch of a body that strikes: m from `from` to `to`, its striking tip at m = `tip`. */
export type Part = { readonly name: string; readonly body: Strand; readonly from: number; readonly to: number; readonly tip: number };

export function part(name: string, body: Strand, range: { readonly from?: number; readonly to?: number; readonly tip: number }): Part {
  const from = range.from ?? 0, to = range.to ?? 1;
  if (!(from >= 0 && to <= 1 && from < to)) throw new Error(`part "${name}": need 0 <= from < to <= 1, got ${from}..${to}`);
  if (!(range.tip >= from && range.tip <= to)) throw new Error(`part "${name}": the tip ${range.tip} is outside ${from}..${to}`);
  return { name, body, from, to, tip: range.tip };
}

/** Seconds and rates of an attack, all relative to its arming time T. Numbers: they select the closed forms. */
export type Timing = {
  /** Seconds the body takes to coil into the attack's posture, ending when the strike starts. */
  readonly windup: number;
  /** Seconds from the start of the strike to landing (the response's first peak). */
  readonly strike: number;
  /** Damping of the strike: below 1 it recoils after landing. */
  readonly zeta: number;
  /** Initial response of the strike, below 0 for anticipation (a flinch backward before it goes). */
  readonly anticipation: number;
  /** Seconds after landing before the body starts to let go. */
  readonly hold: number;
  /** Natural frequency (Hz) of the recovery back to the rest posture. */
  readonly recover: number;
  /** Body m per second at which the motion runs along the body. */
  readonly travel: number;
};
const DEFAULT_TIMING: Timing = { windup: 0.85, strike: 0.3, zeta: 0.5, anticipation: -0.9, hold: 0.12, recover: 0.55, travel: 2.2 };

/** A lunge: the m = 0 end of the spine is laid along the lane and lands with the part's tip on the shape's far end. */
export type LungeSpec = {
  readonly kind: "lunge";
  readonly hit: Capsule | Disc;
  /** Spine m posed by the lunge (the neck); beyond it the rest posture is untouched. */
  readonly reach: number;
  /** World distance along the lane from the spine's m = 0 to the striking tip (a head reaching ahead of its neck). */
  readonly lead: number;
  /** Height of the spine's m = 0 above the still water at landing. */
  readonly height: number;
  /** How far back along the lane, and how high, the end coils during the wind-up. */
  readonly coil: number;
  readonly rise: number;
  /**
   * Radians the neck climbs into the end at landing (from below: it comes out of the water behind the head). A head
   * hung forward of its neck at a pitch levels out when the neck meets it at that pitch.
   */
  readonly approach: number;
};
/** A sweep: the m = 1 end of the spine swings round the arc's centre from its `from` angle to its `to` angle. */
export type SweepSpec = {
  readonly kind: "sweep";
  readonly hit: Arc;
  /** Spine m where the swept stretch starts (it runs to m = 1). */
  readonly root: number;
  readonly height: number;
  /** World distance kept inside the arc's edges at landing (half the swept part's width and some). */
  readonly inset: number;
};

export type AttackSpec = {
  readonly name: string;
  /** Absolute time (seconds) at which the hit arms: the striking part lands then. */
  readonly arm: In<"f32">;
  /** Seconds of warning before the arming time. */
  readonly warn: In<"f32">;
  readonly target: Target;
  /** The name of the creature's part that strikes. */
  readonly part: string;
  readonly motion: LungeSpec | SweepSpec;
  readonly timing?: Partial<Timing>;
};

/** The attack's two envelopes at a clock: posture p (0 rest .. 1) and strike progress q (exactly 1 at arming). */
export type Envelope = { readonly p: F; readonly q: F; readonly tau: F };

/** A second-order step response's first peak, found by sampling the closed form once (it scales as 1 / f). */
function firstPeak(zeta: number, r: number): { readonly at: number; readonly value: number } {
  const x = variable("loft_peak_t", "f32");
  const run = compileJs([response(x, { f: 1, zeta, r })]);
  let best = { at: 0, value: -Infinity };
  for (let i = 1; i <= 4000; i++) {
    const t = i * 0.0005, v = run({ vars: { loft_peak_t: t }, inputs: {}, params: {} })[0] as number;
    if (v > best.value) best = { at: t, value: v };
    else if (v < best.value - 1e-3 && best.value > 1) break; // past the first peak
  }
  if (!(best.value > 0)) throw new Error(`attack: the strike response (zeta ${zeta}, anticipation ${r}) never rises`);
  return best;
}

export class Attack {
  readonly name: string;
  readonly hit: Hit;
  readonly arm: F;
  readonly warn: F;
  readonly target: Target;
  readonly part: string;
  readonly motion: LungeSpec | SweepSpec;
  readonly timing: Timing;
  /** Strike response frequency (Hz) and its value at the first peak, chosen so the peak comes `timing.strike` after the start. */
  private readonly f: number;
  private readonly peak: number;

  constructor(spec: AttackSpec) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(spec.name)) throw new Error(`attack "${spec.name}": names must be identifiers`);
    this.name = spec.name;
    this.hit = spec.motion.hit;
    this.arm = lift<"f32">(spec.arm);
    this.warn = lift<"f32">(spec.warn);
    this.target = spec.target;
    this.part = spec.part;
    this.motion = spec.motion;
    this.timing = { ...DEFAULT_TIMING, ...spec.timing };
    for (const [k, v] of Object.entries(this.timing)) if (!Number.isFinite(v)) throw new Error(`attack "${spec.name}": timing.${k} is ${v}`);
    if (!(this.timing.strike > 0 && this.timing.windup > 0 && this.timing.travel > 0 && this.timing.recover > 0 && this.timing.hold >= 0)) {
      throw new Error(`attack "${spec.name}": strike, windup, travel and recover must be positive and hold >= 0 (${JSON.stringify(this.timing)})`);
    }
    const unit = firstPeak(this.timing.zeta, this.timing.anticipation);
    this.f = unit.at / this.timing.strike;
    this.peak = unit.value;
  }

  /** The envelopes at clock t (the striking end's clock; `time` for now). */
  envelope(t: In<"f32">): Envelope {
    const T = this.timing;
    const tau = sub(t, this.arm);
    const coil = smoothstep(-(T.strike + T.windup), -T.strike, tau);
    const release = sub(1, response(sub(tau, T.hold), { f: T.recover, zeta: 1 }));
    const q = mul(response(add(tau, T.strike), { f: this.f, zeta: T.zeta, r: T.anticipation }), 1 / this.peak);
    return { p: mul(coil, release), q, tau };
  }

  /** Seconds from the first movement to the end of the recovery (to within 1%): the attack's span around T. */
  get span(): { readonly before: number; readonly after: number } {
    const T = this.timing;
    // A critically damped step reaches 99% after about 6.6 / (2 pi f) seconds.
    return { before: T.strike + T.windup + 1 / T.travel, after: T.hold + 6.6 / (2 * Math.PI * T.recover) + 1 / T.travel };
  }

  /** The rest spine with this attack applied. Exact at the arming time at the striking end. */
  pose(rest: Spine): Spine {
    const m = this.motion;
    switch (m.kind) {
      case "lunge": return lungeSpine(this, m, rest);
      case "sweep": return sweepSpine(this, m, rest);
    }
  }

  /** Where the striking end of the spine is at the arming time, as the specification says (for checks and tests). */
  landing(rest: Spine): P3 {
    const m = this.motion;
    switch (m.kind) {
      case "lunge": return lungeLane(m, rest).spineTip;
      case "sweep": return sweepPoint(m, 1, lift<"f32">(1));
    }
  }
}

export const attack = (spec: AttackSpec): Attack => new Attack(spec);

// ---- lunge --------------------------------------------------------------------------------------------------------

type Lane = { readonly dir: V2; readonly tip: V2; readonly spineTip: P3 };
/** The lane: its direction, the striking tip's landing point on the water, and where the spine's m = 0 lands. */
function lungeLane(m: LungeSpec, rest: Spine): Lane {
  const lane = laneEnd(m.hit, rest);
  const at = sub(lane.tip, mul(lane.dir, m.lead));
  return { ...lane, spineTip: vec3(at.x, m.height, at.y) };
}
function laneEnd(h: Capsule | Disc, rest: Spine): { readonly dir: V2; readonly tip: V2 } {
  switch (h.kind) {
    // The tip lands inside the far cap, half a radius short of the end (a capsule's end is round).
    case "capsule": { const dir = laneOf(h); return { dir, tip: sub(h.b, mul(dir, mul(hitInradius(h), 0.5))) }; }
    // A slam on a disc comes from wherever the end rests now, and lands at the centre.
    case "disc": { const from = rest(lift<"f32">(0)); return { dir: normalize(sub(h.centre, vec2(from.x, from.z))), tip: h.centre }; }
  }
}

function lungeSpine(a: Attack, m: LungeSpec, rest: Spine): Spine {
  const lane = lungeLane(m, rest);
  const dir3 = vec3(lane.dir.x, 0, lane.dir.y), up = vec3(0, 1, 0);
  return (s) => {
    // The motion runs down the body after the head: each m reads the clock of its distance from the end.
    const env = a.envelope(sub(time, mul(s, 1 / a.timing.travel)));
    const coiled = add(sub(lane.spineTip, mul(dir3, m.coil)), mul(up, m.rise));
    const head = mix(coiled, lane.spineTip, env.q);
    // A quadratic Bezier from the head back along the lane (leaving it at the approach angle) to where the rest
    // posture continues.
    const root = rest(lift<"f32">(m.reach));
    const half = mul(length(sub(vec2(head.x, head.z), vec2(root.x, root.z))), 0.5);
    const ctrl = sub(sub(head, mul(dir3, half)), mul(up, mul(half, Math.tan(m.approach))));
    const u = clamp(mul(s, 1 / m.reach), 0, 1), v = sub(1, u);
    const posed = add(add(mul(head, mul(v, v)), mul(ctrl, mul(mul(u, v), 2))), mul(root, mul(u, u)));
    const weight = sub(1, smoothstep(0.55 * m.reach, m.reach, s));
    return mix(rest(s), posed, mul(weight, env.p));
  };
}

// ---- sweep --------------------------------------------------------------------------------------------------------

/** The swept posture at spine m and progress q: radially out from the centre, at an angle from `from` toward `to`. */
function sweepPoint(m: SweepSpec, s: In<"f32">, q: F): P3 {
  const h = m.hit;
  const u = clamp(mul(sub(s, m.root), 1 / (1 - m.root)), 0, 1);
  const rho = mix(add(h.inner, m.inset), sub(h.outer, m.inset), u);
  // Each point keeps `inset` inside the end edges, so the swept part lands inside the arc along its whole length.
  const margin = div(m.inset, rho);
  const angle = mix(add(h.from, margin), sub(h.to, margin), q);
  const xz = add(h.centre, mul(polar(angle), rho));
  return vec3(xz.x, m.height, xz.y);
}

function sweepSpine(a: Attack, m: SweepSpec, rest: Spine): Spine {
  return (s) => {
    // The motion runs toward the tip, like a whip: parts nearer the root move first (their clock runs ahead).
    const env = a.envelope(add(time, mul(sub(1, s), 1 / a.timing.travel)));
    const weight = smoothstep(m.root, m.root + 0.08, s);
    return mix(rest(s), sweepPoint(m, s, env.q), mul(weight, env.p));
  };
}

// ---- a creature with attacks --------------------------------------------------------------------------------------

/** What a creature builder returns: its drawables, and the parts an attack may name. */
export type Creature = { readonly things: readonly Drawable[]; readonly parts: Readonly<Record<string, Part>> };

/** An attack bound to the part of the posed creature that strikes with it; `before` is the spine the attack was applied to. */
export type Strike = { readonly attack: Attack; readonly part: Part; readonly before: Spine };

export type Animated = Creature & {
  /** The spine with every attack applied, in order. */
  readonly spine: Spine;
  readonly strikes: Readonly<Record<string, Strike>>;
};

/**
 * Build a creature on its rest spine with attacks applied (each to its own stretch of the spine), and bind each
 * attack to the part it names. An attack naming a part the creature does not have is an error.
 */
export function animate(rest: Spine, attacks: readonly Attack[], build: (spine: Spine) => Creature): Animated {
  const before: Spine[] = [];
  const spine = attacks.reduce<Spine>((s, a) => { before.push(s); return a.pose(s); }, rest);
  const c = build(spine);
  const strikes: Record<string, Strike> = {};
  for (const [i, a] of attacks.entries()) {
    const p = c.parts[a.part];
    if (p === undefined) throw new Error(`attack "${a.name}" strikes with "${a.part}", which the creature does not have (parts: ${Object.keys(c.parts).join(", ")})`);
    if (a.name in strikes) throw new Error(`two attacks are named "${a.name}"`);
    strikes[a.name] = { attack: a, part: p, before: before[i] };
  }
  return { ...c, spine, strikes };
}

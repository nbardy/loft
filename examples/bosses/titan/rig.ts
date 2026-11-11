// The Reefback Titan's rig: everything the game reads and the checks measure, for any variant. The walk in place, Claw
// Crush (whose ring wave shoves the second boat) and the Grab (which throws the first), each defined once, the sea and
// the two ships, the body and shell frames that every slot is drawn in, the claws' shoulders, the warnings, the crash
// and the splash. It reads the numbers and what the claws offer (slots.ts, `ClawsOffer`), never which options were
// chosen. The throw and the wave: docs/throw_and_shove.md.
//
// Crab coordinates: x forward, y up, z to its right; the body frame rides `ride` above the still water.

import {
  add, alive, atan2, attack, blendFrames, clamp, course, cos, cross, div, dragged, emote, every, exp, gait, ge, gt, handTurn, hit, hold, hosted, impact, input, lift,
  lt, max, mix, mul, slowOut,
  normalize, placedBy, poseFrame, rigid, ring, rotateAbout, sea, select, sin, size, slowIn, slowInOut, smoothstep, sqrt, STAND_IN, sub, swing, telegraph, thrown,
  time, tumbled, tumbleRate, turnedAbout, vec2, vec3, type Attack, type BodyAttack, type Drawable, type Expr, type F, type Frame, type Gait, type GripSpec, type Hold, type P3, type Placer,
  type Pose, type Ring, type RingWave, type Sea, type SeaDeform, type SeaSpec, type Target, type Thrown,
} from "../../../src/index.ts";
import { rotate } from "../../../src/map/chart.ts";
import { impactFoam } from "../lib/impact.ts";
import { scaleShip, scaleShipNamed } from "../lib/scale_ship.ts";
import { PRINT_WARNING } from "../lib/warning.ts";
import { perFrame, sides, type V3 } from "./ink.ts";
import type { Numbers } from "./numbers.ts";
import type { BoatsOption, ClawsOffer } from "./slots.ts";

type V2 = Expr<"vec2">;

export type Rig = {
  readonly n: Numbers;
  readonly walk: Gait;
  /** Each foot's phase in the walk (a share of the cycle, as given to `gait`): foot k plants when
   * `time / n.period + footPhase[k]` crosses a whole number. Options that react to a step (foam, a swinging lantern) read it. */
  readonly footPhase: readonly number[];
  /** Claw Crush. Its aftermath is a ring wave: `crush.pushed(place)` is how far a thing afloat there has been shoved. */
  readonly crush: BodyAttack<Ring>;
  /** Seconds since this cycle's slam (0 before it): the body's thud, the shell's follow-through, the eyes' whip. */
  readonly crushAfter: F;
  /** The Grab: a hold that ends thrown (`grab.toss()` says where and when the ship lands, `grab.flight()` whether it is in the air). */
  readonly grab: Attack<GripSpec, Hold<Thrown>>;
  /** How far the cutter is given over to the grab: the attack's posture, 0 idle, 1 from the end of its wind-up until it lets go. */
  readonly grabOn: F;
  readonly grabEnv: ReturnType<Attack["envelope"]>;
  /** The grab's beats; `settle` is the eye's 1.5 s to look away, `after` counts from the end of the release. */
  readonly grabBeat: ReturnType<Attack["beats"]>;
  /** The cutter's own axis in the Grab (unit, world): the first ship's keel as the socket has it, level along the `boat1.heading` input until the throw's stroke, then turning with the hand. */
  readonly keel: P3;
  /** The held ship's own up as the socket has it (world up until the throw's stroke, then turning with the hand): the gripping palm lies flat on it. */
  readonly gripUp: P3;
  /** Where the cutter is raised to, in the grab's posture (world). */
  readonly gripRest: P3;
  /** The first ship's middle (world), wherever the Grab has put it: afloat, held, in the air, or where it landed. */
  readonly shipNow: P3;
  /** The host inputs both placers read, whose defaults are the show (`grab.arm`, `crush.arm`, `crush.centre`, `boat1.at`, `boat1.heading`, `boat1.beam`, `throw.land`). */
  readonly inputs: Inputs;
  /** Where the second boat is before the ring reaches it (x, z): the stand-in's `boat2.at` input, or the host's pose. */
  readonly boatTwoAt: V2;
  /** A point in the boss's own axes (x forward, y up, z to its right; the boss at rest, no sway) in the world: `place` in space, `place2` on the water (x, z). At home they are the identity. */
  readonly place: (p: V3 | P3) => P3;
  readonly place2: (p: V2 | readonly [number, number]) => V2;
  /** Where the boss's audience is (world): what a face that plays to the camera looks at. */
  readonly audience: P3;
  /** The taunt's dance, each readout once per frame (all 0, and `raised` the identity, for a boss that does not taunt). */
  readonly dance: Dance;
  /** The centre of Claw Crush's disc on the water (world x, z): the `crush.centre` input, carried by the boss's pose. What a face looks at when it eyes the slam. */
  readonly crushCentre: V2;
  /** The first ship's hull as it is drawn (afloat, in the socket, in the air), as a target: what the claw's fingers must stay out of once they let go. */
  readonly shipTarget: Target;
  /** Who places each boat when the crab does not hold it (the `boats` slot's choice). */
  readonly placer: { readonly boat1: Placer; readonly boat2: Placer };
  /** What a game that runs its own boats reads. */
  readonly handoff: Handoff;
  readonly body: Frame;
  /** The atoll's frame: the body's, with the lean read late and a follow-through pitch. */
  readonly shell: Frame;
  /** A crab-coordinate point (or offset from the body's origin) in the world. */
  readonly local: (p: V3 | P3) => P3;
  /** The claw shoulder on a side (crab coordinates): -1 the cutter's (left), 1 the crusher's. */
  readonly shoulder: (side: 1 | -1) => V3;
  /** Drawn by the scene, not the crab: the sea, the two ships, the two warnings, the slam's crash and the thrown ship's splash (the stand-in's). */
  readonly world: { readonly sea: Sea; readonly warnings: readonly Drawable[]; readonly crash: Drawable; readonly splash: readonly Drawable[]; readonly boat: readonly Drawable[] };
};

/** The host inputs (docs/throw_and_shove.md lists their names, units and defaults). */
export type Inputs = {
  readonly grabArm: F; readonly crushArm: F; readonly crushCentre: V2;
  readonly boat1At: V2; readonly boat1Heading: F; readonly boat1Beam: F; readonly throwLand: V2;
};

/**
 * The hand-off: closed forms a game evaluates each frame. Places are of a ship's origin (its waterline amidships),
 * world units; velocities units per second; times seconds on the scene's clock, of the attack's cycle now.
 */
export type Handoff = {
  /** While `held` is 1 the first ship is the crab's: its frame is `socket` (origin, bow x, mast y, starboard z) and its size `squeeze` (along its own x, y, z). */
  readonly socket: Frame; readonly held: F; readonly squeeze: P3;
  /** The release: its instant, the ship's place and velocity then, and the spin the stand-in gives it (radians per second about the vector; the claw itself gives none). */
  readonly release: { readonly time: F; readonly at: P3; readonly velocity: P3; readonly spin: P3 };
  /** Where and when the stand-in's flight lands and comes to rest: a prediction under this scene's gravity for a host that flies its own. */
  readonly landing: { readonly time: F; readonly at: P3; readonly rest: P3 };
  /** Claw Crush's ring as a field: `arrival(place)` seconds after `crush.arm`, `lifts(place, t)` units, `impulse(place)` units per second at arrival. */
  readonly wave: RingWave;
};

// ---- staging: where a boss stands, how its show ends, whether it taunts (docs/lineup.md) ---------------------------

/** How the stand-in boats get back to their places for the next cycle. */
export type Show =
  /** They are back at their places when an attack's cycle turns: a jump, at the loop's seam if the scene arranges it. */
  | { readonly kind: "cut" }
  /**
   * No jump, so the scene can loop. The thrown boat goes down where it landed (`sink.after` seconds after touchdown,
   * over `sink.seconds`, to `sink.depth` under: an opaque sea hides it) and a new boat sails in, straight from
   * `sailIn.from` (an offset from its place, in the boss's axes: somewhere out of frame) over the last `sailIn.seconds`
   * of the Grab's cycle, rounding up to its own heading over the last `sailIn.round` share of the way, and is home as
   * the next warning starts. The shoved boat sails back to its place, starting `sailBack.after` seconds after the
   * slam, over `sailBack.seconds`.
   */
  | {
    readonly kind: "loop"; readonly sink: { readonly after: number; readonly seconds: number; readonly depth: number };
    readonly sailIn: { readonly from: readonly [number, number]; readonly seconds: number; readonly round: number };
    readonly sailBack: { readonly after: number; readonly seconds: number };
  };

/** Whether the boss taunts: not at all, or a dance whose moves land `accents` seconds after `first` (every `crushPeriod`). */
export type Taunt = { readonly kind: "none" } | { readonly kind: "dance"; readonly first: number; readonly accents: readonly number[] };

/** Where the boss's audience is (what a face that plays to it looks at): its own, a point in its axes that moves with it; or a place in the world (a camera's eye). */
export type Audience = { readonly kind: "own" } | { readonly kind: "at"; readonly eye: V3 };

export type Staging = {
  /** The defaults of the `boss.at` (world x, z) and `boss.heading` (radians from +x toward +z) inputs. */
  readonly at: readonly [number, number]; readonly heading: number;
  /** How far ahead of the scene's clock the walk treads, as a share of its cycle. */
  readonly walkAhead: number;
  readonly show: Show; readonly taunt: Taunt; readonly audience: Audience;
};
/** The crab on its own: at the origin, facing +x, its boats jumping home, no taunt, its own audience. */
export const AT_HOME: Staging = { at: [0, 0], heading: 0, walkAhead: 0, show: { kind: "cut" }, taunt: { kind: "none" }, audience: { kind: "own" } };

/** The first stage's product: the boss's deformations of the sea, the sea it would have alone, and the second stage. */
export type StagedRig = { readonly deforms: readonly SeaDeform[]; readonly sea: SeaSpec; readonly finish: (water: Sea) => Rig };

/** A dance's readouts, each once per frame: all 0 for a boss that does not taunt. */
export type Dance = {
  /** In the taunt (0..1), the move (0..1..0 on each accent), its sideways swing (out one way rising, back the other falling), the side stepped to (-1..1). */
  readonly on: F; readonly wave: F; readonly swing: F; readonly step: F;
  /** The same move half a beat late: what the small claw answers with. */
  readonly answer: F;
  /** What a great claw that has a display of its own should show: its own when there is no taunt, the dance's move when there is. */
  readonly display: (own: F) => F;
  /** x with k times one of the readouts above added: x itself for a boss that does not taunt. */
  readonly plus: (x: F, k: number, of: F) => F;
  /**
   * A claw's rest wrist (crab coordinates) lifted by the dance: the great claw (side 1) up, out and forward on each move,
   * swinging out one way and back the other; the small claw (side -1) the same, smaller, half a beat late. The wrist
   * itself for a boss that does not taunt.
   */
  readonly raised: (side: 1 | -1, wrist: P3) => P3;
};
function tauntOf(t: Taunt, period: number, T: Numbers["temper"]): Dance {
  const zero = lift<"f32">(0);
  switch (t.kind) {
    case "none": return { on: zero, wave: zero, swing: zero, step: zero, answer: zero, display: (own) => own, plus: (x) => x, raised: (_side, wrist) => wrist };
    case "dance": {
      // The accents are the scene's (on its beats); how the crab arrives on them is its temperament: how long before the
      // accent it starts (so a slow mover still lands on the beat), how sharply, how long it holds the pose, how long it
      // takes to leave. The side changes a half beat (0.25 s at 120 a minute) after each accent.
      const e = emote({ name: "taunt", arm: every(period, t.first), accents: t.accents, lead: T.arrive, hold: T.hold, fall: T.leave, snap: T.snap, offbeat: 0.25, shuffle: 0.22, settle: 0.4 });
      const b = e.beats(time), late = e.beats(sub(time, T.lag));
      const tab = (name: string, x: F): F => perFrame(`titan_s_taunt_${name}`, x);
      const wave = tab("wave", b.wave), on = tab("on", b.on), swing = tab("swing", mul(sin(mul(b.turn, Math.PI)), b.wave));
      // The small claw: late after the great one, or (together = 1) up with it.
      const answer = tab("answer", mix(late.wave, b.wave, T.together));
      // A twitch on the half-beats of a move (8 a second), for a crab that has any.
      const twitch = mul(mul(sin(mul(time, 2 * Math.PI * 8)), T.jitter), add(mul(wave, 0.6), mul(on, 0.25)));
      const R = T.reach;
      const lifts = {
        great: perFrame("titan_taunt_great", vec3(mul(wave, 0.3 * R), add(mul(wave, 2.4 * R), mul(on, 0.8)), add(add(mul(wave, 1.0 * R), mul(swing, 0.9 * R)), mul(twitch, 0.6)))),
        small: perFrame("titan_taunt_small", vec3(mul(answer, 0.2 * R), add(mul(answer, (1.7 + 0.7 * T.together) * R), mul(on, 0.5)), sub(mul(answer, -0.8 * R), mul(twitch, 0.4)))),
      };
      const raised = (side: 1 | -1, wrist: P3): P3 => { switch (side) { case 1: return add(wrist, lifts.great); case -1: return add(wrist, lifts.small); } };
      return { on, wave, swing, step: tab("step", add(b.step, mul(twitch, 0.5))), answer, display: () => wave, plus: (x, k, of) => add(x, mul(of, k)), raised };
    }
  }
}

/** The audience's place in the world, given the boss's own carrying. */
function audienceOf(a: Audience, place: (p: V3) => P3): P3 {
  switch (a.kind) {
    case "own": return place(OWN_AUDIENCE);
    case "at": return vec3(...a.eye);
  }
}
/** The audience of a boss on its own, in its axes: where the game camera stands when the boss is at home. */
const OWN_AUDIENCE: V3 = [23, 27, 21];

/** The crab on its own, at home: its attacks, then its own sea with its ring on it, then the rest. */
export function rig(n: Numbers, offer: ClawsOffer, boats: BoatsOption): Rig {
  const s = stage(n, offer, boats, AT_HOME);
  return s.finish(sea({ ...s.sea, deform: s.deforms }));
}

/**
 * The crab in two stages (docs/lineup.md): first its attacks, which is where its ring wave comes from; then, on a sea
 * made with every ring that will run on it (its own, and any other boss's in the scene), everything else. `how` is where
 * the boss stands, how its show ends and whether it taunts.
 */
export function stage(n: Numbers, offer: ClawsOffer, boats: BoatsOption, how: Staging): StagedRig {
  const I = n.ink;

  // ==== the boss's pose: where it stands and which way it faces, inputs whose defaults are `how.at`, `how.heading` ====
  // Everything below is authored in the boss's own axes (x forward, z to its right) and carried to the world by this
  // pose. At the default pose (0, 0; 0) the turn is by cos 0 = 1 and sin 0 = 0 and the shift by 0: the same numbers.
  const bossAt = input("boss.at", "vec2", [...how.at]), bossHeading = input("boss.heading", "f32", [how.heading]);
  const hc = perFrame("titan_s_boss_cos", cos(bossHeading)), hs = perFrame("titan_s_boss_sin", sin(bossHeading));
  /** A direction in the boss's axes, in the world's (a turn about up). */
  const turn3 = (v: P3): P3 => vec3(sub(mul(v.x, hc), mul(v.z, hs)), v.y, add(mul(v.x, hs), mul(v.z, hc)));
  /** A point in the boss's axes, in the world: on the water (x, z), or in space. */
  const place2 = (p: V2 | readonly [number, number]): V2 => { const q = lift<"vec2">(p); return add(bossAt, vec2(sub(mul(q.x, hc), mul(q.y, hs)), add(mul(q.x, hs), mul(q.y, hc)))); };
  const place = (p: V3 | P3): P3 => { const q = turn3(lift<"vec3">(p)); return vec3(add(q.x, bossAt.x), q.y, add(q.z, bossAt.y)); };
  const placeFrame = (g: Frame): Frame => ({ o: place(g.o), x: turn3(g.x), y: turn3(g.y), z: turn3(g.z) });

  // ==== the walk: in place, the body swaying over feet that tread ====
  const feetHome = sides.flatMap((s) => n.legFeet.map(([x, z]) => vec3(x, n.floor, s * z)));
  // Alternating tetrapods, each side a small wave toward the front.
  const phase = sides.flatMap((s) => n.legFeet.map((_, i) => ((i % 2 === 0) === (s < 0) ? 0 : 0.5) + 0.06 * i));
  // The walk's own clock: the scene's, ahead by a share of a cycle (`how.walkAhead`: each crab of a line treads on its
  // own beat). At home it is `time` itself.
  const walkClock = how.walkAhead === 0 ? time : add(time, how.walkAhead * n.period);
  const home = gait({ feet: feetHome, phase, course: course.line([0, 0, 0], 0), stride: 0, period: n.period, duty: n.duty, lift: n.lift, t: walkClock });
  const walk: Gait = { travelled: home.travelled, body: placeFrame(home.body), foot: (i) => place(home.foot(i)), planted: (i) => home.planted(i) };
  const w = (2 * Math.PI) / n.period;
  const g = walk.body;

  // ---- what a game sets: host inputs, whose defaults are this show (docs/throw_and_shove.md, "The contract") ----
  //
  // Where the first ship floats by default, the cutter can reach it. DERIVATION (the grab check measures it at the
  // closing instant): the cutter's shoulder is at crab (3.9, -0.1, -2.26), 1.25 above the still water; its arm segments
  // are 2.5 and 2.4, so the wrist reaches 4.9 at most. At 4.2 out and 1.0 down it is 4.32 away: in reach, the elbow
  // still bent. The grip's middle closed on the beam lies `closedDepth(beam)` beyond the wrist along the palm: the
  // claws' offer (claws/base.ts derives it for the base claw, 4.81). So the ship's default place is 4.2 + 4.81 = 9.01
  // from the shoulder along its keel bearing, and the claw arrives along the keel with a finger down each side of the
  // hull. A game that puts its ship elsewhere is trusted to arm the Grab only where the claw reaches (`gripReaches`
  // measures the default).
  const shoulder = (side: 1 | -1): V3 => [n.carapace.to - 1.1, -0.1, side * n.carapace.width * 0.55];
  const shoulderL = shoulder(-1);
  const shipFar = n.grabWristReach + offer.cutter.closedDepth(scaleShip.hull.lines.beam);
  const B2 = n.boatTwo, TH = n.throw;
  const start = {
    boat1: { at: [shoulderL[0] + Math.cos(n.grabBearing) * shipFar, shoulderL[2] + Math.sin(n.grabBearing) * shipFar] as const, heading: n.grabBearing },
    boat2: { at: [n.crushCentre[0] + Math.cos(B2.bearing) * (n.crushRadius + B2.beyond), n.crushCentre[1] + Math.sin(B2.bearing) * (n.crushRadius + B2.beyond)] as const, heading: B2.heading },
  };
  // Places and headings are given in the boss's own axes (their defaults are numbers, and a number cannot follow a
  // pose that is itself an input); `IN` holds them carried to the world, which is what everything below reads.
  const boat1HeadingOwn = input("boat1.heading", "f32", [start.boat1.heading]);
  const IN = {
    /** When each attack arms (seconds; it repeats every `crushPeriod`): the Grab's grip closes, Claw Crush lands. */
    grabArm: input("grab.arm", "f32", [n.grabClose]), crushArm: input("crush.arm", "f32", [n.crushFirst]),
    /** The slam's disc on the water (x, z). */
    crushCentre: place2(input("crush.centre", "vec2", [...n.crushCentre])),
    /** The first ship when the Grab arms: where it floats (x, z), its heading (radians from +x toward +z), its beam. */
    boat1At: place2(input("boat1.at", "vec2", [...start.boat1.at])), boat1Heading: add(boat1HeadingOwn, bossHeading),
    boat1Beam: input("boat1.beam", "f32", [scaleShip.hull.lines.beam]),
    /** Where the throw is aimed to land (x, z). */
    throwLand: place2(input("throw.land", "vec2", [...TH.land])),
  };
  /** Who places each boat when the crab does not hold it: classified once, by the `boats` slot. */
  const placer = { boat1: boats.placer("boat1", start.boat1), boat2: boats.placer("boat2", start.boat2) };

  // ---- Claw Crush: the attack, defined once (here, since the body's lean reads its arming time) ----
  // The lunge law poses a two-point spine from the wrist (m = 0) back to the shoulder: its m = 0 end is the wrist's
  // target. During the wind-up it pulls back by `coil` and rises by `rise` (the claw lifts high), then lands `lead` short
  // of the disc's centre exactly at the arming time (so the palm, reaching on, covers the centre), and recovers.
  const crushDisc = hit.disc({ centre: IN.crushCentre, radius: n.crushRadius });
  const crush = attack({
    name: "crush", arm: every(n.crushPeriod, IN.crushArm), warn: n.crushWarn,
    target: { kind: "disc", centre: crushDisc.centre, radius: crushDisc.radius },
    part: "crusherPalm",
    motion: { kind: "lunge", hit: crushDisc, reach: 1, lead: offer.crushLead, height: 0.7, coil: n.crushCoil, rise: n.crushRise, approach: 0.3, neck: 1 },
    timing: { windup: n.crushSeconds.windup, strike: n.crushSeconds.strike, zeta: 0.55, hold: 0.45, recover: 0.5, travel: 50 },
    // The slam raises a ring wave from the disc's edge. Its speed is not chosen: the crab stands on the bottom, so this
    // is shallow water and the wave runs at sqrt(g depth) under the same gravity as the thrown ship.
    // The front is a bore, faster the taller it is: one speed is kept, the slowest it has while it carries anything,
    // sqrt(g (depth + holding height)).
    aftermath: ring({ speed: Math.sqrt(n.gravity * (n.seaDepth + n.wave.hold)), ...n.wave }),
  });

  // ---- the taunt: an emote, if this staging has one (docs/lineup.md) ----
  const taunt = tauntOf(how.taunt, n.crushPeriod, n.temper);

  // ---- the sea is the scene's: this boss's own, with its ring on it, or one shared with other bosses and their rings.
  // Everything from here on floats on it or reads it, so it is the second stage.
  const ownSea: SeaSpec = {
    swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 0.9 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 0.7, phase: 1.3 }],
    foam: 0.85,
  };
  return { deforms: crush.deforms(), sea: ownSea, finish: (water: Sea): Rig => {
  const WIND: readonly [number, number] = [0.35, 1.0];
  const UP = vec3(0, 1, 0);
  const scaleShipTwo = scaleShipNamed("scaleShipTwo", B2.sail);

  // ---- the Grab: the first ship in its own axes, where it is taken, drawn back to and thrown ----
  /** The height of the hull's middle, where the grip takes it, above its own origin (the waterline amidships). */
  const GRIP_Y = 0.25;
  /** The ship's keel (unit, level, toward the bow) and its starboard side, from the heading it has when taken. */
  const keel = vec3(cos(IN.boat1Heading), 0, sin(IN.boat1Heading)), stbd = vec3(mul(sin(IN.boat1Heading), -1), 0, cos(IN.boat1Heading));
  /** The keel in the boss's own axes: what the cutter's raised rest place is set along (crab coordinates). */
  const keelOwn = vec3(cos(boat1HeadingOwn), 0, sin(boat1HeadingOwn));
  const taken = vec3(IN.boat1At.x, GRIP_Y, IN.boat1At.y);
  // The throw. The carry ends at the wind-back place: drawn back along the keel toward the crab, a little to starboard
  // (toward the face) and up, in the ship's own axes. The release point and velocity are not stated: they follow from
  // the wind-back, the landing place and the flight time (`grab.toss()`).
  const windback = add(add(sub(taken, mul(keel, TH.back)), mul(stbd, TH.side)), mul(UP, TH.up));
  const land = vec3(IN.throwLand.x, GRIP_Y, IN.throwLand.y);
  /** A host's pose is of the ship's origin; the attack's thing is the hull's middle, `GRIP_Y` up the ship's own mast. */
  const middleOf = (p: Pose): Pose => ({ ...p, at: add(p.at, mul(poseFrame(p).y, GRIP_Y)) });

  // The Grab as an attack that holds. Its arming is the instant the grip closes; the warning is the same print-palette
  // marker as Claw Crush's, on the water where the ship is taken. Beats (seconds): wind-up 0.9 (the claw rises 3.3 over
  // its shoulder: the posture), reach 1.3 (it first draws back, the wrong way, then down and out on an arc, opening to 1.5
  // beams by two thirds of the way), close 0.22 (fast: it lands), hold 3.1 (2.7 lifted on an arc and squeezed twice to
  // half the beam while it is drawn back and cocked, then 0.4 of whip: uniform acceleration to the release velocity),
  // release 1.3 (0.25 following the throw through, opening, then home); then the posture lets go and the arm swings
  // once. The stand-in's ship flies 3.2 s, lands, skids and bobs; a host's ship is the host's from the release.
  const GS = n.grabSeconds;
  const grab = attack({
    name: "grab", arm: every(n.crushPeriod, IN.grabArm), warn: n.grabWarn,
    // What the claw must not pass into: the hull where it floats when taken.
    target: scaleShip.target(scaleShip.afloatChart(water, { at: IN.boat1At, heading: IN.boat1Heading, wind: WIND })),
    motion: {
      kind: "grip", hit: hit.disc({ centre: IN.boat1At, radius: scaleShip.hull.lines.length * 0.85 }),
      reachLift: 1.2, restGap: offer.cutter.restGap, windup: n.grabDrawBack, close: GS.close, release: GS.release,
    },
    effect: hold({
      seconds: GS.hold, at: taken, radius: div(IN.boat1Beam, 2), carry: { kind: "arc", to: windback, lift: n.grabLift }, squeeze: n.grabSqueeze, pulses: 2,
      end: thrown({
        land, flight: TH.flight, gravity: n.gravity, stroke: TH.stroke, follow: TH.follow, drag: TH.drag, heave: TH.heave,
        placer: placedBy<Placer>(placer.boat1, { standIn: () => STAND_IN, host: (p) => hosted(middleOf(p)) }),
      }),
    }),
    // The posture lets go at once after the release, over about a second (99% by 1.17 s).
    timing: { windup: GS.windup, strike: GS.reach + GS.close, hold: 0, recover: 0.9 },
  });
  // Scalars read by many tables (the body and shell frames, the claws) are themselves evaluated once per frame: inline,
  // every frame table re-evaluated the grab's and the crush's clocks.
  const tabS = (name: string, e: F): F => perFrame(`titan_s_${name}`, e);
  const grabEnv = grab.envelope(time);
  const grabOn = tabS("grabOn", grabEnv.p);
  const grabBeat = grab.beats(time, 1.5);

  // ---- the body: Claw Crush in three beats ----
  //
  // Wind-up (1.15 s): the whole crab rears back and up, away from the target (anticipation). Act (0.3 s): it drives
  // down and forward into the slam. Contact: it drops 0.45 and springs back once and a half (impact, a heavy 2 Hz).
  // Settle (1.8 s): it eases back to the walk. The atoll is a separate frame on the same clock read 0.25 s late (drag),
  // and pitches on after the body stops (follow-through), swinging twice.
  const crushBeat = (t: F) => crush.beats(t, n.crushSettle);
  /** The lean: -0.6 reared back at the end of the wind-up, 1 driven into the slam, 0 at rest. */
  const leanAt = (t: F): F => {
    const b = crushBeat(t);
    // How sharply it rears into the wind-up is its temperament (`snap`: 2 is the gentle ease it always had).
    return mul(mix(mix(0, -0.6, slowInOut(b.anticipate, n.temper.snap)), 1, slowIn(b.act)), sub(1, slowInOut(b.settle)));
  };
  const lean = tabS("lean", leanAt(time)), crushAfter = tabS("crushAfter", crushBeat(time).after);
  /** The drop at contact: 0 before, -0.45 at the slam, springing back past 0. (0.3 did not read from the game camera.) */
  const thud = mul(sub(impact(crushAfter, 1, 2.0, 0.3), 1), 0.45);
  // The taunt, if there is one (each term is 0 without): the body rises on its legs with each wave of the claw (a little
  // crouched between), sidesteps on the off-beat and rolls into the step.
  const danced = taunt.plus;
  /** The idle's size: doubled for a crab that is all twitch. */
  const idle = 1 + n.temper.jitter;
  const bob = danced(danced(add(add(add(mul(cos(mul(walkClock, 2 * w)), n.bob), alive(time, 0.06 * idle, 11)), mul(lean, -0.25)), thud), n.temper.rise, taunt.wave), -0.15, taunt.on);
  const roll = danced(add(add(mul(sin(mul(walkClock, w)), n.roll), alive(time, 0.015 * idle, 12)), mul(lean, 0.07)), 0.07, taunt.step);
  // The crab turns toward whichever claw is working: right into the slam, left (positive yaw) toward the grab.
  const yaw = danced(add(add(add(mul(sin(mul(walkClock, w * 0.5)), n.yaw), mul(lean, -0.1)), mul(grabOn, 0.14)), alive(time, 0.02 * idle, 13)), 0.06, taunt.swing);
  const sway = danced(mul(sin(mul(walkClock, w)), n.sway), 0.9, taunt.step);
  const yx = rotateAbout(g.x, g.y, yaw), yz = rotateAbout(g.z, g.y, yaw);
  // Roll about the body's x, then pitch (the lean) about the rolled z: an orthonormal frame, as draw.copies' carry needs.
  const ry = rotateAbout(g.y, yx, roll), rz = rotateAbout(yz, yx, roll);
  const pitchOf = (l: F): F => mul(l, -0.12);
  const origin = add(add(g.o, mul(g.y, add(n.ride, bob))), mul(g.z, sway));
  const pitched = (p: F): Frame => ({ o: origin, x: rotateAbout(yx, rz, p), y: rotateAbout(ry, rz, p), z: rz });
  const bodyNow: Frame = pitched(pitchOf(lean));
  // The atoll: the lean read a quarter second late, then a pitch that keeps going forward after the slam and swings back.
  // (How late, and how far it swings on, is the crab's temperament: `lag`, 0.25 s as first drawn.)
  const shellNow: Frame = pitched(tabS("shellPitch", add(pitchOf(leanAt(dragged(time, 1, n.temper.lag))), swing(sub(crushAfter, 0.2), (-0.09 * n.temper.lag) / 0.25, 1.0, 0.25))));
  const tab = (name: string, e: P3) => perFrame(`titan_frame_${name}`, e);
  const body: Frame = { o: tab("o", bodyNow.o), x: tab("x", bodyNow.x), y: tab("y", bodyNow.y), z: tab("z", bodyNow.z) };
  const shell: Frame = { o: tab("shell_o", shellNow.o), x: tab("shell_x", shellNow.x), y: tab("shell_y", shellNow.y), z: tab("shell_z", shellNow.z) };
  const local = (p: V3 | P3): P3 => add(body.o, rotate(body, lift<"vec3">(p)));

  // ---- the ships, the warnings, the crash and the splash ----
  const gripRest = local(add(vec3(shoulderL[0], shoulderL[1] + 3.3, shoulderL[2]), mul(keelOwn, 4.5)));
  const tabV = (name: string, e: P3) => perFrame(`titan_grab_${name}`, e);
  const tabFrame = (name: string, g: Frame): Frame => ({ o: tabV(`${name}_o`, g.o), x: tabV(`${name}_x`, g.x), y: tabV(`${name}_y`, g.y), z: tabV(`${name}_z`, g.z) });
  const flight = grab.flight(), toss = grab.toss(), grabTau = grabEnv.tau;
  // How the show ends for the two stand-in boats (`Show`): one record per kind, read below without asking which.
  const show = ((): { readonly ship: (thing: P3) => P3; readonly heading: (landed: F) => F; readonly kept: (tau: F) => F } => {
    const h = how.show, released = lt(grabTau, GS.hold);
    switch (h.kind) {
      // Back at their places when an attack's cycle turns; the shoved boat is kept where it ended for `keep` seconds.
      case "cut": return { ship: (thing) => thing, heading: (landed) => select(released, IN.boat1Heading, landed), kept: (tau) => select(lt(tau, B2.keep), 1, 0) };
      case "loop": {
        // The Grab's cycle ends when its next warning starts: `crushPeriod` after this one's did.
        const cycleEnd = n.crushPeriod - n.grabWarn, sails = cycleEnd - h.sailIn.seconds, last = GS.hold + TH.flight + h.sink.after + h.sink.seconds;
        if (!(sails > last)) throw new Error(`show: the new boat would sail in (${sails.toFixed(2)} s after the grip closes) before the thrown one has gone down (${last.toFixed(2)} s)`);
        const sailing = ge(grabTau, sails), u = clamp(div(sub(grabTau, sails), h.sailIn.seconds), 0, 1);
        const sunk = mul(slowIn(clamp(div(sub(flight.landed, h.sink.after), h.sink.seconds), 0, 1)), -h.sink.depth);
        // It comes straight in, slowing to a stop at its place, pointing the way it travels; then rounds up to its own
        // heading (the shorter way round).
        const from = turn3(vec3(h.sailIn.from[0], 0, h.sailIn.from[1]));
        const course = Math.atan2(-h.sailIn.from[1], -h.sailIn.from[0]);
        const round = ((start.boat1.heading - course + 3 * Math.PI) % (2 * Math.PI)) - Math.PI;
        const sailed = add(add(course, bossHeading), mul(round, slowInOut(clamp(div(sub(u, 1 - h.sailIn.round), h.sailIn.round), 0, 1))));
        return {
          ship: (thing) => select(sailing, add(taken, mul(from, sub(1, slowOut(u)))), add(thing, mul(UP, sunk))),
          heading: (landed) => select(sailing, sailed, select(released, IN.boat1Heading, landed)),
          kept: (tau) => sub(1, slowInOut(clamp(div(sub(tau, h.sailBack.after), h.sailBack.seconds), 0, 1))),
        };
      }
    }
  })();
  const shipNow = tabV("ship", show.ship(grab.thing()));

  // The socket: the first ship's frame while the claw has it, and what a game attaches its boat to. The ship keeps the
  // heading it was taken at, level, until the stroke; over the stroke the hand turns it toward the release velocity,
  // about one axis (keel x release velocity) with uniform angular acceleration, up to exactly the rate of the tumble
  // that carries on through the flight: angular velocity is continuous at the release, as velocity is. For `turns`
  // whole turns in the air that turn is pi turns stroke / flight (45 degrees of the 71 between keel and velocity for
  // two turns: all of it would put the wrist 6.2 from a shoulder whose arm spans 4.9). It turns about its middle (the
  // point that flies), not about its origin a quarter unit below it.
  const spinAxis = normalize(cross(keel, toss.velocity));
  const turned = tumbled({ o: shipNow, x: keel, y: UP, z: stbd }, spinAxis, TH, TH.turns, flight);
  const socket: Frame = { o: sub(shipNow, mul(turned.y, GRIP_Y)), x: turned.x, y: turned.y, z: turned.z };
  /** The claw's own axis: along the ship's keel as the socket has it (the taken heading until the stroke, then turning with the hand). */
  const inHand = tumbled({ o: shipNow, x: keel, y: UP, z: stbd }, spinAxis, TH, TH.turns, { whip: flight.whip, u: 0 });
  const gripAxis = tabV("axis", inHand.x), gripUp = tabV("up", inHand.y);
  /** Where the ship points when it comes down: as it left the hand (whole turns later), laid level. */
  const landedHeading = (() => { const x = turnedAbout({ o: shipNow, x: keel, y: UP, z: stbd }, spinAxis, handTurn(TH, TH.turns)).x; return atan2(x.z, x.x); })();

  // The first ship when the crab does not hold it, by who places it. One record per handler: its frame, how much its
  // hull gives on touchdown, and the splash.
  const free1 = placedBy(placer.boat1, {
    // The stand-in. Afloat, it rides the water under its middle (so it floats again where it lands, and plunges through
    // the surface on touchdown), lifted by its middle's height above where that floats. In the air it is the socket's
    // frame carried along the flight, still turning at the rate the hand gave it, so it leaves the grip exactly as it
    // was held and comes down as it left; over 0.4 s from touchdown it is given back to the water, on that heading.
    standIn: () => {
      const under = scaleShip.afloatChart(water, { at: vec2(shipNow.x, shipNow.z), heading: show.heading(landedHeading), wind: WIND }).frame(vec3(0, 0, 0));
      const floating: Frame = { o: add(under.o, mul(UP, sub(shipNow.y, GRIP_Y))), x: under.x, y: under.y, z: under.z };
      const afloatAgain = select(lt(grabTau, GS.hold), 1, smoothstep(0, 0.4, flight.landed));
      // The hull squashes on touchdown: a swing from 1, so it gives as it goes under and springs back; volume kept.
      const give = sub(1, swing(flight.landed, n.splash.squash, 1.6, 0.35));
      /** The splash: the crash's clawed foam where the thrown ship touches down, from that instant. */
      const splash = impactFoam("splash", water, {
        area: hit.disc({ centre: IN.throwLand, radius: n.splash.reach * 0.5 }), pad: n.splash.reach * 0.5 + 1,
        splashes: [{ at: IN.throwLand, since: flight.landed }],
        reach: n.splash.reach, seconds: n.splash.seconds, foam: I.cream, line: I.black,
      });
      return { frame: blendFrames(socket, floating, afloatAgain), give, splash: [splash] as readonly Drawable[] };
    },
    // The host: drawn where the game says, as it is.
    host: (p) => ({ frame: poseFrame(p), give: lift<"f32">(1), splash: [] as readonly Drawable[] }),
  });
  // Whose the ship is: the crab's from the closing of the grip (taken off the water over the close, not in one frame)
  // until the release, then at once its placer's again (the stand-in's flight starts as the socket was; a host's own
  // physics starts from the release point at the release velocity).
  const owned = select(lt(grabTau, GS.hold), smoothstep(-GS.close, 0, grabTau), 0);
  const shipFrame = tabFrame("ship", blendFrames(free1.frame, socket, owned));
  const wide = div(1, sqrt(free1.give));
  const squeeze = grab.squeezed(1, "z").k;
  const boatOne = scaleShip.placed(rigid(shipFrame), WIND, size.axes(tabV("squeeze", mul(squeeze, vec3(wide, free1.give, wide)))));

  // The second boat floats outside the crush disc, its bow toward the slam. Its stand-in is the ring's own ride
  // (`crush.pushed`: the rule a game integrates, solved): lifted as the front's edge passes under it, taken by the face,
  // carried stern first at the wave's speed until the crest can no longer hold it, then dropped to coast. It floats on
  // the sea at its forced place, so its rise and its lie on the wave's face come from the sea's own height, ring
  // included. What the stand-in adds as art: the hull's own answer to being hit (a pitch that throws the bow up and
  // rings down, `kick`), a slew as it coasts to a stop (`slew`), and a burst of spray where the face takes it. It keeps
  // the boat where it ended for `keep` seconds after the slam, then the boat is back at its place (in this show that is
  // the loop's seam, while the Grab plays).
  const free2 = placedBy(placer.boat2, {
    // The stand-in's own inputs: where the second boat floats when Claw Crush arms (x, z), and its heading.
    standIn: () => {
      const at = place2(input("boat2.at", "vec2", [...start.boat2.at])), heading = add(input("boat2.heading", "f32", [start.boat2.heading]), bossHeading);
      const tau = sub(time, crush.arm), kept = perFrame("titan_s_boat2_kept", show.kept(tau)), ride = crush.wave().ride(at);
      const forced = crush.pushed(at), shove = tabV("boat2_shove", mul(vec3(forced.x, 0, forced.y), kept));
      const now = vec2(add(at.x, shove.x), add(at.y, shove.z));
      /** Seconds since the face took it, and since it was dropped (each negative before, and once the boat is back at its place). */
      const taken = perFrame("titan_s_boat2_taken", select(gt(kept, 0), sub(tau, ride.taken), -1)), dropped = sub(taken, ride.seconds);
      const slewed = add(heading, mul(mul(B2.slew, kept), sub(1, exp(mul(max(dropped, 0), -1 / n.wave.drag)))));
      const afloat2 = scaleShipTwo.afloatChart(water, { at: now, heading: slewed, wind: WIND }).frame(vec3(0, 0, 0));
      // Bow up about the ship's own beam (z = bow x mast), by a swing from 0 that rings down.
      const frame = turnedAbout(afloat2, afloat2.z, swing(taken, B2.kick, 0.8, 0.3));
      const spray = impactFoam("spray", water, {
        area: hit.disc({ centre: at, radius: 4 }), pad: 4,
        splashes: [{ at: now, since: taken }],
        reach: 3.2, seconds: 1.3, foam: I.cream, line: I.black,
      });
      return { at, frame, spray: [spray] as readonly Drawable[] };
    },
    host: (p) => ({ at: vec2(p.at.x, p.at.z), frame: poseFrame(p), spray: [] as readonly Drawable[] }),
  });
  const boatTwo = scaleShipTwo.placed(rigid(tabFrame("boat2", free2.frame)), WIND);
  const boat = [...boatOne, ...boatTwo, ...free2.spray];

  const warnStyle = { ...PRINT_WARNING, spacing: 1.2, edge: 0.22, line: 0.09 };
  /** The crash: clawed cream foam bursting from the disc's centre at the slam and running out past its edge (lib/impact.ts). */
  const crashOuter = n.crushRadius * n.crashReach;
  const crash = impactFoam("crash", water, {
    area: crushDisc, pad: crashOuter - n.crushRadius + 1,
    splashes: [{ at: IN.crushCentre, since: sub(time, crush.arm) }],
    reach: crashOuter, seconds: n.crashSeconds, foam: I.cream, line: I.black,
  });
  /** What a game that runs its own boats reads (docs/throw_and_shove.md, "The contract"); the stand-ins above read the same. */
  const handoff: Handoff = {
    socket, held: grab.held(), squeeze,
    release: { time: toss.releasedAt, at: sub(toss.release, mul(UP, GRIP_Y)), velocity: toss.velocity, spin: mul(spinAxis, tumbleRate(TH, TH.turns)) },
    landing: { time: toss.landsAt, at: sub(toss.land, mul(UP, GRIP_Y)), rest: sub(toss.rest, mul(UP, GRIP_Y)) },
    wave: crush.wave(),
  };

  return {
    n, walk, footPhase: phase, crush, crushAfter, grab, grabOn, grabEnv, grabBeat, keel: gripAxis, gripUp, gripRest, shipNow, body, shell, local, shoulder,
    inputs: IN, placer, handoff, boatTwoAt: free2.at, crushCentre: IN.crushCentre, shipTarget: scaleShip.target(rigid(shipFrame)),
    place, place2, audience: audienceOf(how.audience, place), dance: taunt,
    world: { sea: water, warnings: [telegraph(crush, { style: warnStyle }), telegraph(grab, { style: warnStyle })], crash, splash: free1.splash, boat },
  };
  } };
}

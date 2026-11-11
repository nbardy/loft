// The Reefback Titan's rig: everything the game reads and the checks measure, for any variant. The walk in place, Claw
// Crush and the Grab (each defined once), the sea and the ship the Grab takes, the body and shell frames that every slot
// is drawn in, the claws' shoulders, the warnings and the crash. It reads the numbers and what the claws offer
// (slots.ts, `ClawsOffer`), never which options were chosen.
//
// Crab coordinates: x forward, y up, z to its right; the body frame rides `ride` above the still water.

import {
  add, alive, attack, course, cos, div, dragged, every, gait, hit, hold, impact, lift, mix, mul, rotateAbout, sea, shifted, sin, size, slowIn,
  slowInOut, sub, swing, telegraph, time, vec2, vec3, type Attack, type BodyAttack, type Drawable, type F, type Frame, type Gait, type GripSpec, type Hold,
  type P3, type Sea,
} from "../../../src/index.ts";
import { rotate } from "../../../src/map/chart.ts";
import { impactFoam } from "../lib/impact.ts";
import { scaleShip } from "../lib/scale_ship.ts";
import { PRINT_WARNING } from "../lib/warning.ts";
import { perFrame, sides, type V3 } from "./ink.ts";
import type { Numbers } from "./numbers.ts";
import type { ClawsOffer } from "./slots.ts";

export type Rig = {
  readonly n: Numbers;
  readonly walk: Gait;
  readonly crush: BodyAttack;
  /** Seconds since this cycle's slam (0 before it): the body's thud, the shell's follow-through, the eyes' whip. */
  readonly crushAfter: F;
  readonly grab: Attack<GripSpec, Hold>;
  /** How far the cutter is given over to the grab: the attack's posture, 0 idle, 1 from the end of its wind-up until it lets go. */
  readonly grabOn: F;
  readonly grabEnv: ReturnType<Attack["envelope"]>;
  /** The grab's beats; `settle` is the eye's 1.5 s to look away, `after` counts from the end of the release. */
  readonly grabBeat: ReturnType<Attack["beats"]>;
  /** The keel's bearing as a unit (x, z): the cutter reaches along it. */
  readonly keel: readonly [number, number];
  /** Where the cutter is raised to, in the grab's posture (world). */
  readonly gripRest: P3;
  /** The ship where the hold has it (world); after the release it slips back to where it was taken. */
  readonly shipNow: P3;
  readonly body: Frame;
  /** The atoll's frame: the body's, with the lean read late and a follow-through pitch. */
  readonly shell: Frame;
  /** A crab-coordinate point (or offset from the body's origin) in the world. */
  readonly local: (p: V3 | P3) => P3;
  /** The claw shoulder on a side (crab coordinates): -1 the cutter's (left), 1 the crusher's. */
  readonly shoulder: (side: 1 | -1) => V3;
  /** Drawn by the scene, not the crab: the sea, the ship, the two warnings, the crash. */
  readonly world: { readonly sea: Sea; readonly warnings: readonly Drawable[]; readonly crash: Drawable; readonly boat: readonly Drawable[] };
};

export function rig(n: Numbers, offer: ClawsOffer): Rig {
  const I = n.ink;

  // ==== the walk: in place, the body swaying over feet that tread ====
  const feetHome = sides.flatMap((s) => n.legFeet.map(([x, z]) => vec3(x, n.floor, s * z)));
  // Alternating tetrapods, each side a small wave toward the front.
  const phase = sides.flatMap((s) => n.legFeet.map((_, i) => ((i % 2 === 0) === (s < 0) ? 0 : 0.5) + 0.06 * i));
  const walk = gait({ feet: feetHome, phase, course: course.line([0, 0, 0], 0), stride: 0, period: n.period, duty: n.duty, lift: n.lift });
  const w = (2 * Math.PI) / n.period;
  const g = walk.body;

  // ---- Claw Crush: the attack, defined once (here, since the body's lean reads its arming time) ----
  // The lunge law poses a two-point spine from the wrist (m = 0) back to the shoulder: its m = 0 end is the wrist's
  // target. During the wind-up it pulls back by `coil` and rises by `rise` (the claw lifts high), then lands `lead` short
  // of the disc's centre exactly at the arming time (so the palm, reaching on, covers the centre), and recovers.
  const crush = attack({
    name: "crush", arm: every(n.crushPeriod, n.crushFirst), warn: n.crushWarn,
    target: { kind: "disc", centre: vec2(...n.crushCentre), radius: lift<"f32">(n.crushRadius) },
    part: "crusherPalm",
    motion: { kind: "lunge", hit: hit.disc({ centre: n.crushCentre, radius: n.crushRadius }), reach: 1, lead: offer.crushLead, height: 0.7, coil: n.crushCoil, rise: n.crushRise, approach: 0.3, neck: 1 },
    timing: { windup: 1.15, strike: 0.3, zeta: 0.55, hold: 0.45, recover: 0.5, travel: 50 },
  });

  // ---- the sea and the ship (the Grab's prize), here because the Grab's warning and the body's turn read them ----
  const water = sea({
    swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 0.9 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 0.7, phase: 1.3 }],
    foam: 0.85,
  });
  const WIND: readonly [number, number] = [0.35, 1.0];

  // ---- the Grab: where the ship must sit for the cutter to reach it ----
  //
  // DERIVATION (the grab check measures it at the closing instant): the cutter's shoulder is at crab
  // (3.9, -0.1, -2.26), 1.25 above the still water; its arm segments are 2.5 and 2.4, so the wrist reaches 4.9 at most.
  // At 4.2 out and 1.0 down it is 4.32 away: in reach, the elbow still bent. The grip's middle closed on the beam lies
  // `closedDepth(beam)` beyond the wrist along the palm: the claws' offer (claws/base.ts derives it for the base claw,
  // 4.81). So the ship sits 4.2 + 4.81 = 9.01 from the shoulder along its keel bearing, and the claw arrives along the
  // keel with a finger down each side of the hull.
  const shoulder = (side: 1 | -1): V3 => [n.carapace.to - 1.1, -0.1, side * n.carapace.width * 0.55];
  const shoulderL = shoulder(-1);
  const beam = scaleShip.hull.lines.beam;
  const keel: readonly [number, number] = [Math.cos(n.grabBearing), Math.sin(n.grabBearing)];
  const shipFar = n.grabWristReach + offer.cutter.closedDepth(beam);
  const SHIP_AT: readonly [number, number] = [shoulderL[0] + keel[0] * shipFar, shoulderL[2] + keel[1] * shipFar];
  /** Where the grip takes the hull: its middle, a little above the waterline. And where it sets it down: nearer the face. */
  const GRIP_FROM: V3 = [SHIP_AT[0], 0.25, SHIP_AT[1]];
  const GRIP_TO: V3 = [SHIP_AT[0] - keel[0] * 1.4, 0.25, SHIP_AT[1] - keel[1] * 1.4];
  const shipChart = scaleShip.afloatChart(water, { at: SHIP_AT, heading: n.grabBearing, wind: WIND });

  // The Grab as an attack that holds. Its arming is the instant the grip closes; the warning is the same print-palette
  // marker as Claw Crush's, on the water where the ship is taken. Beats (seconds): wind-up 0.9 (the claw rises 3.3 over
  // its shoulder: the posture), reach 1.3 (it first draws back, the wrong way, then down and out on an arc, opening to 1.5
  // beams by two thirds of the way), close 0.22 (fast: it lands), hold 3.2 (lifted 3.5 clear on an arc toward the face,
  // squeezed twice to half the beam, set down 1.4 nearer), release 1.3 (opens where it is, then withdraws); then the
  // posture lets go and the arm swings once. The ship then slips back to where it was taken.
  const GS = n.grabSeconds;
  const grab = attack({
    name: "grab", arm: every(n.crushPeriod, n.grabClose), warn: n.grabWarn, target: scaleShip.target(shipChart),
    motion: {
      kind: "grip", hit: hit.disc({ centre: SHIP_AT, radius: scaleShip.hull.lines.length * 0.85 }),
      reachLift: 1.2, restGap: offer.cutter.restGap, windup: n.grabDrawBack, close: GS.close, release: GS.release,
    },
    effect: hold({ seconds: GS.hold, at: GRIP_FROM, radius: beam / 2, carry: { kind: "arc", to: GRIP_TO, lift: n.grabLift }, squeeze: n.grabSqueeze, pulses: 2 }),
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
    return mul(mix(mix(0, -0.6, slowInOut(b.anticipate)), 1, slowIn(b.act)), sub(1, slowInOut(b.settle)));
  };
  const lean = tabS("lean", leanAt(time)), crushAfter = tabS("crushAfter", crushBeat(time).after);
  /** The drop at contact: 0 before, -0.45 at the slam, springing back past 0. (0.3 did not read from the game camera.) */
  const thud = mul(sub(impact(crushAfter, 1, 2.0, 0.3), 1), 0.45);
  const bob = add(add(add(mul(cos(mul(time, 2 * w)), n.bob), alive(time, 0.06, 11)), mul(lean, -0.25)), thud);
  const roll = add(add(mul(sin(mul(time, w)), n.roll), alive(time, 0.015, 12)), mul(lean, 0.07));
  // The crab turns toward whichever claw is working: right into the slam, left (positive yaw) toward the grab.
  const yaw = add(add(add(mul(sin(mul(time, w * 0.5)), n.yaw), mul(lean, -0.1)), mul(grabOn, 0.14)), alive(time, 0.02, 13));
  const sway = mul(sin(mul(time, w)), n.sway);
  const yx = rotateAbout(g.x, g.y, yaw), yz = rotateAbout(g.z, g.y, yaw);
  // Roll about the body's x, then pitch (the lean) about the rolled z: an orthonormal frame, as draw.copies' carry needs.
  const ry = rotateAbout(g.y, yx, roll), rz = rotateAbout(yz, yx, roll);
  const pitchOf = (l: F): F => mul(l, -0.12);
  const origin = add(add(g.o, mul(g.y, add(n.ride, bob))), mul(g.z, sway));
  const pitched = (p: F): Frame => ({ o: origin, x: rotateAbout(yx, rz, p), y: rotateAbout(ry, rz, p), z: rz });
  const bodyNow: Frame = pitched(pitchOf(lean));
  // The atoll: the lean read a quarter second late, then a pitch that keeps going forward after the slam and swings back.
  const shellNow: Frame = pitched(tabS("shellPitch", add(pitchOf(leanAt(dragged(time, 1, 0.25))), swing(sub(crushAfter, 0.2), -0.09, 1.0, 0.25))));
  const tab = (name: string, e: P3) => perFrame(`titan_frame_${name}`, e);
  const body: Frame = { o: tab("o", bodyNow.o), x: tab("x", bodyNow.x), y: tab("y", bodyNow.y), z: tab("z", bodyNow.z) };
  const shell: Frame = { o: tab("shell_o", shellNow.o), x: tab("shell_x", shellNow.x), y: tab("shell_y", shellNow.y), z: tab("shell_z", shellNow.z) };
  const local = (p: V3 | P3): P3 => add(body.o, rotate(body, lift<"vec3">(p)));

  // ---- the ship where the hold has it (what the game reads too), the warnings, the crash ----
  const gripRest = local([shoulderL[0] + keel[0] * 4.5, shoulderL[1] + 3.3, shoulderL[2] + keel[1] * 4.5]);
  const tabV = (name: string, e: P3) => perFrame(`titan_grab_${name}`, e);
  const shipNow = tabV("ship", add(grab.thing(), mul(sub(vec3(...GRIP_FROM), vec3(...GRIP_TO)), slowInOut(div(grabBeat.after, n.grabFlee)))));
  const boat = scaleShip.placed(shifted(shipChart, sub(shipNow, vec3(...GRIP_FROM))), WIND, size.axes(tabV("squeeze", grab.squeezed(1, "z").k)));

  const warnStyle = { ...PRINT_WARNING, spacing: 1.2, edge: 0.22, line: 0.09 };
  /** The crash: clawed cream foam bursting from the disc's centre at the slam and running out past its edge (lib/impact.ts). */
  const crashOuter = n.crushRadius * n.crashReach;
  const crash = impactFoam("crash", water, {
    area: hit.disc({ centre: n.crushCentre, radius: n.crushRadius }), pad: crashOuter - n.crushRadius + 1,
    splashes: [{ at: n.crushCentre, since: sub(time, crush.arm) }],
    reach: crashOuter, seconds: n.crashSeconds, foam: I.cream, line: I.black,
  });

  return {
    n, walk, crush, crushAfter, grab, grabOn, grabEnv, grabBeat, keel, gripRest, shipNow, body, shell, local, shoulder,
    world: { sea: water, warnings: [telegraph(crush, { style: warnStyle }), telegraph(grab, { style: warnStyle })], crash, boat },
  };
}

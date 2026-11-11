// The Reefback Titan's rig: everything the game reads and the checks measure, for any variant. The walk in place, Claw
// Crush (whose ring wave shoves the second boat) and the Grab (which throws the first), each defined once, the sea and
// the two ships, the body and shell frames that every slot is drawn in, the claws' shoulders, the warnings, the crash
// and the splash. It reads the numbers and what the claws offer (slots.ts, `ClawsOffer`), never which options were
// chosen. The throw and the wave: docs/throw_and_shove.md.
//
// Crab coordinates: x forward, y up, z to its right; the body frame rides `ride` above the still water.

import {
  add, alive, attack, blendFrames, course, cos, cross, div, dragged, every, gait, hit, hold, hosted, impact, input, lift, lt, mix, mul, normalize, placedBy,
  poseFrame, rigid, ring, rotateAbout, sea, select, sin, size, slowIn, slowInOut, smoothstep, sqrt, STAND_IN, sub, swing, telegraph, thrown, time, tumbled,
  vec2, vec3, type Attack, type BodyAttack, type Drawable, type Expr, type F, type Frame, type Gait, type GripSpec, type Hold, type P3, type Placer,
  type Pose, type Ring, type RingWave, type Sea, type Thrown,
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
  /** The first ship's keel when it is taken (unit, level, world; from the `boat1.heading` input): the cutter reaches along it. */
  readonly keel: P3;
  /** Where the cutter is raised to, in the grab's posture (world). */
  readonly gripRest: P3;
  /** The first ship's middle (world), wherever the Grab has put it: afloat, held, in the air, or where it landed. */
  readonly shipNow: P3;
  /** The host inputs both placers read, whose defaults are the show (`grab.arm`, `crush.arm`, `crush.centre`, `boat1.at`, `boat1.heading`, `boat1.beam`, `throw.land`). */
  readonly inputs: Inputs;
  /** Where the second boat is before the ring reaches it (x, z): the stand-in's `boat2.at` input, or the host's pose. */
  readonly boatTwoAt: V2;
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

export function rig(n: Numbers, offer: ClawsOffer, boats: BoatsOption): Rig {
  const I = n.ink;

  // ==== the walk: in place, the body swaying over feet that tread ====
  const feetHome = sides.flatMap((s) => n.legFeet.map(([x, z]) => vec3(x, n.floor, s * z)));
  // Alternating tetrapods, each side a small wave toward the front.
  const phase = sides.flatMap((s) => n.legFeet.map((_, i) => ((i % 2 === 0) === (s < 0) ? 0 : 0.5) + 0.06 * i));
  const walk = gait({ feet: feetHome, phase, course: course.line([0, 0, 0], 0), stride: 0, period: n.period, duty: n.duty, lift: n.lift });
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
    boat2: { at: [n.crushCentre[0] + Math.cos(B2.bearing) * B2.distance, n.crushCentre[1] + Math.sin(B2.bearing) * B2.distance] as const, heading: B2.heading },
  };
  const IN = {
    /** When each attack arms (seconds; it repeats every `crushPeriod`): the Grab's grip closes, Claw Crush lands. */
    grabArm: input("grab.arm", "f32", [n.grabClose]), crushArm: input("crush.arm", "f32", [n.crushFirst]),
    /** The slam's disc on the water (x, z). */
    crushCentre: input("crush.centre", "vec2", [...n.crushCentre]),
    /** The first ship when the Grab arms: where it floats (x, z), its heading (radians from +x toward +z), its beam. */
    boat1At: input("boat1.at", "vec2", [...start.boat1.at]), boat1Heading: input("boat1.heading", "f32", [start.boat1.heading]),
    boat1Beam: input("boat1.beam", "f32", [scaleShip.hull.lines.beam]),
    /** Where the throw is aimed to land (x, z). */
    throwLand: input("throw.land", "vec2", [...TH.land]),
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
    timing: { windup: 1.15, strike: 0.3, zeta: 0.55, hold: 0.45, recover: 0.5, travel: 50 },
    // The slam raises a ring wave from the disc's edge. Its speed is not chosen: the crab stands on the bottom, so this
    // is shallow water and the wave runs at sqrt(g depth) under the same gravity as the thrown ship.
    aftermath: ring({ speed: Math.sqrt(n.gravity * n.seaDepth), ...n.wave }),
  });

  // ---- the sea, here because the ships float on it and the Grab's warning and the body's turn read them ----
  const water = sea({
    swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 0.9 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 0.7, phase: 1.3 }],
    foam: 0.85, deform: crush.deforms(),
  });
  const WIND: readonly [number, number] = [0.35, 1.0];
  const UP = vec3(0, 1, 0);
  const scaleShipTwo = scaleShipNamed("scaleShipTwo", B2.sail);

  // ---- the Grab: the first ship in its own axes, where it is taken, drawn back to and thrown ----
  /** The height of the hull's middle, where the grip takes it, above its own origin (the waterline amidships). */
  const GRIP_Y = 0.25;
  /** The ship's keel (unit, level, toward the bow) and its starboard side, from the heading it has when taken. */
  const keel = vec3(cos(IN.boat1Heading), 0, sin(IN.boat1Heading)), stbd = vec3(mul(sin(IN.boat1Heading), -1), 0, cos(IN.boat1Heading));
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
  // beams by two thirds of the way), close 0.22 (fast: it lands), hold 3.1 (2.6 lifted on an arc and squeezed twice to
  // half the beam while it is drawn back and cocked, then 0.5 of whip: uniform acceleration to the release velocity),
  // release 1.3 (0.3 following the throw through, opening, then home); then the posture lets go and the arm swings
  // once. The stand-in's ship flies 2.6 s, lands, skids and bobs; a host's ship is the host's from the release.
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

  // ---- the ships, the warnings, the crash and the splash ----
  const gripRest = local(add(vec3(shoulderL[0], shoulderL[1] + 3.3, shoulderL[2]), mul(keel, 4.5)));
  const tabV = (name: string, e: P3) => perFrame(`titan_grab_${name}`, e);
  const tabFrame = (name: string, g: Frame): Frame => ({ o: tabV(`${name}_o`, g.o), x: tabV(`${name}_x`, g.x), y: tabV(`${name}_y`, g.y), z: tabV(`${name}_z`, g.z) });
  const shipNow = tabV("ship", grab.thing());
  const flight = grab.flight(), toss = grab.toss(), grabTau = grabEnv.tau;

  // The socket: the first ship's frame while the claw has it. The palm does not turn, so the ship keeps the heading it
  // was taken at, level, its middle at the grip's (`grab.thing()`). This is what a game attaches its boat to.
  const socket: Frame = { o: sub(shipNow, mul(UP, GRIP_Y)), x: keel, y: UP, z: stbd };
  /** The tumble the stand-in gives the thrown ship: whole turns about the level axis across the throw, top first. */
  const spinAxis = normalize(cross(UP, toss.along));

  // The first ship when the crab does not hold it, by who places it. One record per handler: its frame, how much its
  // hull gives on touchdown, and the splash.
  const free1 = placedBy(placer.boat1, {
    // The stand-in. Afloat, it rides the water under its middle (so it floats again where it lands, and plunges through
    // the surface on touchdown), lifted by its middle's height above where that floats. In the air it is the socket's
    // level frame carried along the flight and tumbled, so it leaves the grip exactly as it was held and comes down the
    // way up it left; over 0.4 s from touchdown it is given back to the water.
    standIn: () => {
      const under = scaleShip.afloatChart(water, { at: vec2(shipNow.x, shipNow.z), heading: IN.boat1Heading, wind: WIND }).frame(vec3(0, 0, 0));
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
      // It turns about its middle (the point that flies), not about its origin a quarter unit below it.
      const turned = tumbled(socket, spinAxis, TH.turns, flight.u);
      const flying: Frame = { o: sub(shipNow, mul(turned.y, GRIP_Y)), x: turned.x, y: turned.y, z: turned.z };
      return { frame: blendFrames(flying, floating, afloatAgain), give, splash: [splash] as readonly Drawable[] };
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

  // The second boat floats outside the crush disc. Its stand-in takes the ring's impulse under a linear drag
  // (`crush.pushed`: built from the same field a game samples) and floats on the sea at its shoved place, so its rise
  // over the crest and its heel on the wave's faces come from the sea's own height, ring included; nothing animates
  // them. The stand-in keeps the boat where the ring left it for `keep` seconds after the slam, then it is back at its
  // place (in this show that is the loop's seam, while the Grab plays).
  const free2 = placedBy(placer.boat2, {
    // The stand-in's own inputs: where the second boat floats when Claw Crush arms (x, z), and its heading.
    standIn: () => {
      const at = input("boat2.at", "vec2", [...start.boat2.at]), heading = input("boat2.heading", "f32", [start.boat2.heading]);
      const shoved = crush.pushed(at), kept = select(lt(sub(time, crush.arm), B2.keep), 1, 0);
      const shove = tabV("boat2_shove", mul(vec3(shoved.x, 0, shoved.y), kept));
      return { at, frame: scaleShipTwo.afloatChart(water, { at: vec2(add(at.x, shove.x), add(at.y, shove.z)), heading, wind: WIND }).frame(vec3(0, 0, 0)) };
    },
    host: (p) => ({ at: vec2(p.at.x, p.at.z), frame: poseFrame(p) }),
  });
  const boatTwo = scaleShipTwo.placed(rigid(tabFrame("boat2", free2.frame)), WIND);
  const boat = [...boatOne, ...boatTwo];

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
    release: { time: toss.releasedAt, at: sub(toss.release, mul(UP, GRIP_Y)), velocity: toss.velocity, spin: mul(spinAxis, (TH.turns * 2 * Math.PI) / TH.flight) },
    landing: { time: toss.landsAt, at: sub(toss.land, mul(UP, GRIP_Y)), rest: sub(toss.rest, mul(UP, GRIP_Y)) },
    wave: crush.wave(),
  };

  return {
    n, walk, footPhase: phase, crush, crushAfter, grab, grabOn, grabEnv, grabBeat, keel, gripRest, shipNow, body, shell, local, shoulder,
    inputs: IN, placer, handoff, boatTwoAt: free2.at,
    world: { sea: water, warnings: [telegraph(crush, { style: warnStyle }), telegraph(grab, { style: warnStyle })], crash, splash: free1.splash, boat },
  };
}

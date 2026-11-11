// The Reefback Titan's numbers: every number an art director changes, in one record. A variant or a slot option
// overrides any of them (src/design.ts, `Overrides`); the rig and every slot read them from the build's record, never
// from this module, so an override reaches everything that reads that number.

import type { Widen } from "../../../src/design.ts";

type V3 = readonly [number, number, number];

export const S = {
  // ---- inks (woodblock): few, flat; shadow is the same ink overprinted with indigo ----
  ink: {
    black: [0.075, 0.07, 0.09], indigo: [0.13, 0.2, 0.37], cream: [0.93, 0.88, 0.75],
    brick: [0.66, 0.22, 0.13], brickDark: [0.42, 0.12, 0.09], moss: [0.47, 0.56, 0.24], mossDeep: [0.2, 0.32, 0.17],
    stone: [0.7, 0.63, 0.5], bark: [0.3, 0.2, 0.14], coral: [0.86, 0.56, 0.46],
  } satisfies Record<string, V3>,
  /** How much indigo the shadow side of every ink takes (0 none, 1 indigo). */
  shadowIndigo: 0.42,
  /** Shadow side brightness after the indigo. */
  shadowValue: 0.78,

  // ---- size and stance ----
  ride: 1.35, floor: -1.7,
  carapace: { from: -1.6, to: 5.0, width: 4.1, height: 1.25 },
  legRoots: [3.2, 2.0, 0.8, -0.4], legFeet: [[6.8, 8.2], [4.0, 9.6], [0.8, 9.9], [-2.6, 9.0]] as const, legLengths: [4.0, 3.8, 1.6] as const,
  // ---- the walk in place ----
  period: 3.2, duty: 0.66, lift: 1.5, sway: 0.25, bob: 0.08, roll: 0.03, yaw: 0.035,

  // ---- the atoll ----
  atoll: { centre: [-2.0, 0] as const, rx: 4.3, rz: 4.5, top: 2.9, base: -1.75, hills: 0.35, lumps: 0.08 },
  coralHeads: 18, trees: 6,
  fort: { at: [-2.6, -1.1] as const, height: 2.4, girth: 0.85, lean: 0.12 },

  // ---- claws: the right (crusher) 1.4 times the left ----
  crusher: 1.4, cutter: 1.0, clawPalm: 2.9, clawFat: 1.1, armLengths: [2.5, 2.4] as const,

  // ---- eyes ----
  stalk: 2.4, eye: 0.3,

  // ---- Claw Crush: the hit disc on the water (world xz), its warning, its loop ----
  // The show loops every 16 s: Claw Crush arms at 3.4, the Grab closes at 11.2, so one idea reads at a time.
  crushCentre: [9.6, 5.6] as const, crushRadius: 3.4, crushWarn: 1.5, crushFirst: 3.4, crushPeriod: 16,
  // Pushed 1.2x and 1.5x from 6.5 and 2.0 (exaggeration): the claw climbs higher and pulls further back the wrong way.
  // At 8.5 (1.3x) the raised claw left the top of the game camera's frame.
  crushRise: 7.8, crushCoil: 3.0,
  /** Seconds the body takes to settle after Claw Crush lands: its wind-up and strike are the attack's own. */
  crushSettle: 1.8,
  /** The foam crash after the slam: how far the ring runs out (times the disc's radius), and for how long. */
  crashReach: 1.6, crashSeconds: 1.4,

  // ---- the Grab: the cutter takes the scale ship (lib/scale_ship.ts), lifts it, squeezes it twice, sets it down ----
  /** Seconds into the loop the grip closes (the grab's arming), and the warning before it. */
  grabClose: 11.2, grabWarn: 3.0,
  /**
   * The grab's seconds, each stated once: the claw rises into posture (windup), reaches and closes (the attack's strike
   * is reach + close), holds, lets go. `drawBack` is how far the reach first leaves the wrong way (grab's `windup`).
   */
  grabSeconds: { windup: 0.9, reach: 1.3, close: 0.22, hold: 3.2, release: 1.3 }, grabDrawBack: 1.7,
  /** The ship's keel bearing, radians from +x toward +z: ahead-left of the crab, on the cutter's side, and from the game
   * camera clear of the resting crusher (at -0.44 the crusher covered it). */
  grabBearing: -0.75,
  /** Horizontal distance from the cutter's shoulder to its wrist when the grip closes: the arm spans 4.9, so the elbow stays bent. */
  grabWristReach: 4.2,
  // A 2.2 lift barely read from the high game camera; 3.5 does (wrist still 4.3 from the shoulder at the top).
  grabLift: 3.5, grabSqueeze: 0.5,
  /** Seconds after the release for the ship to slip back to where it was taken (it was set down nearer the crab). */
  grabFlee: 2.5,
} as const;

/** The numbers as a build reads them: the same shape with every literal widened, so overrides may change them. */
export type Numbers = Widen<typeof S>;

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
    // The one accent a variant may give a single part when it repaints the body (DIRECTION.md: one accent per monster):
    // the fiddler's great claw. No base option reads it; it is the base brick, so that claw on the base crab stays brick.
    accent: [0.66, 0.22, 0.13],
    // The mask direction's dark lacquer (armour, helmet): lighter than the line black so ink lines still read on it.
    // The base reads none of it.
    lacquer: [0.14, 0.1, 0.12],
    // The stilt direction's inks (read only by its options; the base never paints with them): bone-and-straw legs, a
    // dark olive thorny body, and shrine vermilion as its one accent.
    straw: [0.86, 0.79, 0.58], strawDark: [0.52, 0.43, 0.27], thorn: [0.34, 0.31, 0.2], thornDark: [0.15, 0.13, 0.09],
    shrine: [0.8, 0.16, 0.1], shrineDark: [0.42, 0.07, 0.06], paper: [0.97, 0.86, 0.6],
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
  // The show loops every 22 s: the Grab closes at 3.0 and throws its ship (down at 9.3), Claw Crush arms at 12.0 and
  // its ring shoves the second boat, which coasts for the rest of the loop, so one idea reads at a time. Both boats are
  // back at their places at the loop's seam.
  crushCentre: [9.6, 5.6] as const, crushRadius: 3.4, crushWarn: 1.5, crushFirst: 12.0, crushPeriod: 22,
  // Pushed 1.2x and 1.5x from 6.5 and 2.0 (exaggeration): the claw climbs higher and pulls further back the wrong way.
  // At 8.5 (1.3x) the raised claw left the top of the game camera's frame.
  crushRise: 7.8, crushCoil: 3.0,
  /** Seconds the body takes to settle after Claw Crush lands: its wind-up and strike are the attack's own. */
  crushSettle: 1.8,
  /** The foam crash after the slam: how far the ring runs out (times the disc's radius), and for how long. */
  crashReach: 1.6, crashSeconds: 1.4,

  // ---- the Grab: the cutter takes the scale ship (lib/scale_ship.ts), lifts it, squeezes it twice, throws it ----
  /** Seconds into the loop the grip closes (the grab's arming), and the warning before it. They are equal, so the grab's
   * cycle (and with it the thrown ship's return to its place) turns over exactly at the loop's seam. */
  grabClose: 3.0, grabWarn: 3.0,
  /**
   * The grab's seconds, each stated once: the claw rises into posture (windup), reaches and closes (the attack's strike
   * is reach + close), holds, lets go. `drawBack` is how far the reach first leaves the wrong way (grab's `windup`).
   */
  grabSeconds: { windup: 0.9, reach: 1.3, close: 0.22, hold: 3.1, release: 1.3 }, grabDrawBack: 1.7,
  /** The ship's keel bearing, radians from +x toward +z: ahead-left of the crab, on the cutter's side, and from the game
   * camera clear of the resting crusher (at -0.44 the crusher covered it). */
  grabBearing: -0.75,
  /** Horizontal distance from the cutter's shoulder to its wrist when the grip closes: the arm spans 4.9, so the elbow stays bent. */
  grabWristReach: 4.2,
  // The carry's arch over the straight way from where the ship is taken to the wind-back place (2.15 up): with 2.0 the
  // ship tops out 3.5 above the water two thirds of the way, as the old lift did, then sinks back as the claw cocks.
  grabLift: 2.0, grabSqueeze: 0.5,

  // ---- the throw and the wave (docs/throw_and_shove.md) ----
  /** The world's gravity, units/s^2: the thrown ship's flight and (with the sea's depth) the ring wave's speed. */
  gravity: 5,
  /** The sea's depth here: the base crab stands on the bottom (`-floor`). The world's, not a variant's: the stilt crab's
   * feet plant higher (floor -1.0), and a wave taken from that ran at 2.24, slower than the shove it was asked to give. */
  seaDepth: 1.7,
  /**
   * The throw that ends the Grab. The carry draws the ship back to the wind-back place, given in the ship's own axes
   * from where it is taken (`back` along the keel toward the crab, `side` to starboard, `up`), so every variant's claw
   * cocks relative to its own ship. `stroke` seconds accelerate it, it flies `flight` seconds to `land` (world x, z:
   * the default of the `throw.land` input; the farthest open water the game camera shows whole, its far right corner:
   * at (5.7, -21.0) the bowsprit and the sail's peak were cut by the frame), skids on under `drag` and heaves; the claw
   * follows through for `follow`. `turns` whole turns in the air, about the level axis across the throw.
   * Since the owner's second note the landing is far (29 flown, 37 from the crab's middle): the game camera loses the
   * ship through its top right corner and the `arena` and `throw` cameras show it land. The landing the game camera
   * can show whole is (4.3, -19.6), with flight 2.6, stroke 0.5, follow 0.3 and one turn: 13.3 flown.
   */
  throw: { back: 2.7, side: 0.9, up: 2.15, stroke: 0.4, follow: 0.25, flight: 3.2, land: [-1.0, -35.0] as const, drag: 0.2, heave: { hz: 0.9, damping: 0.45 }, turns: 2 },
  /** The splash where the thrown ship lands: how far its foam runs, for how long, and the squash of the hull at touchdown. */
  splash: { reach: 3.6, seconds: 1.5, squash: 0.3 },
  /** Claw Crush's ring wave: crest height at the disc's edge (the depth is 1.7; a crest breaks near 1.3), pulse length,
   * where it is gone, and how far a thing afloat at the edge ends up shoved under the stand-in's drag (`drag` s: a heavy
   * boat coasts). The front gives it shove / drag = 2.9 units/s there, the most the ring allows (the wave's own speed is
   * 2.92). The first version (0.8 high, 4.6 over 1.6 s, then 5.6 over 2.0 s) moved the second boat one of its lengths:
   * a nudge. */
  wave: { height: 1.0, length: 6, reach: 12, shove: 8.7, drag: 3.0 },
  /** The second boat's default place: `beyond` the crush disc's edge (whatever the disc's radius) along `bearing` from
   * its centre, lying at `heading` (quartering the ring, and side-on to the game camera; bow-on it read as a sail with no
   * hull). On bearing -1.35 the game frame has 10 units of open water for it to be shoved across, where the first boat
   * floated before it was thrown (at -0.88 and -1.1 it reached the frame's right edge after 4). Its stand-in keeps its
   * shove for `keep` s after the slam, then the boat is back at its place: 10 is the loop's seam (crushPeriod - crushFirst). */
  boatTwo: { bearing: -1.35, beyond: 2.0, heading: -0.5, keep: 10.0, sail: [0.86, 0.79, 0.58] as const },
} as const;

/** The numbers as a build reads them: the same shape with every literal widened, so overrides may change them. */
export type Numbers = Widen<typeof S>;

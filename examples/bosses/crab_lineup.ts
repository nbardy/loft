// Four crabs in one scene, on a beat, as one loop (docs/lineup.md). Each is a variant of the Reefback Titan's design
// (examples/bosses/reefback_titan.ts) standing in its own place, built in its own namespace, on one sea that carries
// all four ring waves. One clock in beats: 120 a minute, 4/4, twelve bars, 24 s. Three sections: the slam (a cascade
// along the line), the throw (in unison), the taunt (call and answer, a cascade, unison). Every accent is on a beat,
// and each crab arrives on it in its own manner (its `temper`, in the design's numbers).
//
//   bun src/cli/main.ts clip examples/bosses/crab_lineup.ts --view lineup --seconds 24 --size 1920x1080 --out out/lineup/lineup_1080p.mp4

import type { Camera } from "../../src/artefact.ts";
import { lineup, scene } from "../../src/index.ts";
import { design, styleFor, SUN, WATER } from "./reefback_titan.ts";
import { S } from "./titan/numbers.ts";
import type { Staging } from "./titan/rig.ts";

// ---- the tempo ----
export const BPM = 120;
/** Seconds in a beat and in a bar (4/4), and the loop: twelve bars. */
export const BEAT = 60 / BPM, BAR = 4 * BEAT, BARS = 12, PERIOD = BARS * BAR;
/** Beat b (0 is the loop's first downbeat), in seconds. */
const beat = (b: number): number => b * BEAT;

// ---- the line, left to right as the camera sees it: swap names here ----
export const LINE = ["islander", "fiddler", "mask", "stilt"] as const;

/** The camera: in front of the line, low enough to feel their size, high enough to see the water. 16:9. */
const EYE = [106, 16, 0] as const;
export const cameras: Readonly<Record<string, Camera>> = {
  lineup: { kind: "perspective", eye: EYE, target: [0, 8, 0], fovY: 0.6 },
};

/** The crabs stand 22 apart along z (the camera's left is +z), facing the camera (+x), the outer ones toed in a little. */
const SPACING = 22;
const standing = (k: number): { readonly at: readonly [number, number]; readonly heading: number } =>
  ({ at: [0, (1.5 - k) * SPACING], heading: (k - 1.5) * 0.08 });

// ---- the score (beats; every accent below is a whole or a half number of beats) ----
export const SCORE = {
  /** The slam, a cascade: crab k's claw lands on beat 4 + k (its wind-up starts three beats before). */
  slam: (k: number): number => beat(4 + k),
  /** The throw, in unison: every grip closes on beat 19; the two squeezes are beats 20 and 22; the whip starts on beat 23 and every claw lets go on beat 24, bar 7's downbeat; the boats come down on beat 30. */
  grip: beat(19), release: beat(24), lands: beat(30),
  /**
   * The taunt (bars 9 to 12). Bars 9 and 10, call and answer: crabs 0 and 2 on beats 1 and 3, crabs 1 and 3 on beats
   * 2 and 4. Bar 11, a cascade: 0, 1, 2, 3 on successive beats. Bar 12, unison on beats 1 and 3.
   */
  taunt: (k: number): readonly number[] => [
    ...[32, 34, 36, 38].map((b) => beat(b + (k % 2))),
    beat(40 + k),
    beat(44), beat(46),
  ],
} as const;
/** The taunt arms a bar and a half before its first accent, so the slowest mover's first move has room to start early. */
const TAUNT_ARMS = beat(29);

/** What the scene changes of a crab's numbers so that its attacks' own seconds fall on beats (the crab alone keeps its own). */
const onTheBeat = (k: number) => ({
  crushPeriod: PERIOD, period: BAR,
  // Three beats from the claw's first movement to its landing; the warning a beat before that.
  crushSeconds: { windup: 3 * BEAT - 0.3, strike: 0.3 }, crushWarn: 4 * BEAT, crushFirst: SCORE.slam(k),
  // Five beats from the cutter's first movement to its closing; a carry of four beats (the squeezes on the second and
  // the fourth half), a whip of one, a flight of six.
  grabClose: SCORE.grip, grabWarn: 6 * BEAT,
  grabSeconds: { windup: 0.9, reach: 5 * BEAT - 0.9 - 0.22, close: 0.22, hold: 5 * BEAT, release: 1.3 },
  // Thrown back over its left shoulder and away, to come down between it and its neighbour, far behind the line.
  // (Drawn straight back before the whip, not toward the face: `side` 0. With 0.9 the islander's ship left 0.50 from its
  // carapace; landing at z = -13 took the spider crab's over the edge of its own island.)
  throw: { ...S.throw, side: 0, stroke: BEAT, follow: 0.25, flight: 6 * BEAT, land: [-24, -15] as const, turns: 2 },
  // The shoved boat starts to the camera's side of the disc and is carried along the line and toward the camera.
  boatTwo: { ...S.boatTwo, bearing: -0.9, heading: 1.9, keep: PERIOD },
});

const how = (k: number): Staging => ({
  ...standing(k),
  walkAhead: k / 4,
  // The thrown boat goes down where it lands; a new one sails in from below the frame, arriving as the next warning
  // starts; the shoved boat sails back to its place during the taunt.
  show: { kind: "loop", sink: { after: 0.8, seconds: 2.2, depth: 5.5 }, sailIn: { from: [64, -20], seconds: 12, round: 0.3 }, sailBack: { after: beat(28) - SCORE.slam(k), seconds: beat(16) } },
  taunt: { kind: "dance", first: TAUNT_ARMS, accents: SCORE.taunt(k).map((t) => t - TAUNT_ARMS) },
  audience: { kind: "at", eye: EYE },
});

const line = lineup({
  members: LINE.map((variant, k) => ({ name: `crab${k}`, design, variant, numbers: onTheBeat(k), how: how(k) })),
  // The swells' periods are 12 s and 8 s: whole turns in the loop.
  water: { swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 13 / 12 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 6 / 8, phase: 1.3 }], foam: 0.85 },
  loop: { kind: "every", seconds: PERIOD },
});

export default scene({ sea: line.sea, things: line.things, style: styleFor(S, WATER.base), sun: SUN, cameras, period: PERIOD });
export const checks = line.checks;
/** Each crab's product, in line order: what a game reads of it (`holds`, `waves`, `handoff`, `rig.dance`). */
export const members = line.members;

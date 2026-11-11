// Four crabs in one scene, on a beat, as one loop (docs/lineup.md). Each is a variant of the Reefback Titan's design
// (examples/bosses/reefback_titan.ts) standing in its own place, built in its own namespace, on one sea that carries
// all four ring waves. One clock in beats: 120 a minute, 4/4, twelve bars, 24 s. Three sections: the slam (a cascade
// along the line), the throw (in unison), the taunt (call and answer, a cascade, unison). Every accent is on a beat,
// and each crab arrives on it in its own manner (its `temper`, in the design's numbers).
//
// Staged for a video (docs/lineup.md, "Staging"): a low long-lens row at sunset, the middle pair a little forward,
// each crab's disc and boats in a lane in front of it, and shots cut on downbeats (examples/bosses/lineup_shots.ts,
// which the deliverables read: `bun tools/lineup_deliver.ts`). The scene's cameras are those shots' first frames.

import type { Camera } from "../../src/artefact.ts";
import { lineup, scene } from "../../src/index.ts";
import { design, styleFor, SUN, WATER } from "./reefback_titan.ts";
import { AUDIENCE, BAR, BEAT, beat, FORMATS, PERIOD, shotAt, standing } from "./lineup_shots.ts";
import { S } from "./titan/numbers.ts";
import type { Staging } from "./titan/rig.ts";

export { BAR, BARS, BEAT, BPM, PERIOD } from "./lineup_shots.ts";

// ---- the line, left to right as the camera sees it: the fourth slot is a word (`bruiser` for the islander's twin) ----
export const LINE = ["islander", "fiddler", "mask", "stilt"] as const;

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

/**
 * What the scene changes of a crab's numbers: its attacks' seconds on beats, and its props in a lane in front of it
 * (docs/lineup.md, "The formation and the props"). The crab alone keeps its own.
 */
/** Where crab k's thrown boat lands, in its axes: a fan, the outer two outward. */
const FAN = [[38, 6], [38, 0], [38, -4], [38, -8]] as const;
const onTheBeat = (k: number) => ({
  crushPeriod: PERIOD, period: BAR,
  // Three beats from the claw's first movement to its landing. The warning starts at the loop's seam for every crab
  // (2.0 + 0.5 k before its slam): a cycle turns where its warning starts, and the shoved boat goes home when its cycle
  // turns, so all four go home on the cut at the seam.
  crushSeconds: { windup: 3 * BEAT - 0.3, strike: 0.3 }, crushWarn: SCORE.slam(k), crushFirst: SCORE.slam(k),
  // The disc in the crab's lane: in front, a little to its right (the camera's left), clear of its neighbour's boat.
  crushCentre: [10, 1.5] as const,
  // Five beats from the cutter's first movement to its closing; a carry of four beats (the squeezes on the second and
  // the fourth half), a whip of one, a flight of six. The warning is seven beats, so the Grab's cycle turns at 6.0 (bar
  // 4's downbeat, a cut): the thrown boat is home again there.
  grabClose: SCORE.grip, grabWarn: 7 * BEAT,
  grabSeconds: { windup: 0.9, reach: 5 * BEAT - 0.9 - 0.22, close: 0.22, hold: 5 * BEAT, release: 1.3 },
  // The boat taken nearly straight ahead of the left shoulder, in the crab's lane.
  grabBearing: -0.26,
  // Thrown forward, toward the camera, landing in the foreground sea in frame, the four fanned out (with the toe-in,
  // about 18 either side of the middle in the world). 38 forward: a throw must carry 25 from where the boat was taken
  // (`throwLands`; 26 forward carried 15). Drawn straight back before the whip (`side` 0). One turn in the air, not two:
  // thrown forward, the release velocity lies only about 48 degrees off the keel, and the hand's turn over the stroke
  // for two turns (pi 2 0.5 / 3.0, 60 degrees) carried the hull past it and out of the fingers (middle 0.35 to 0.62 off).
  throw: { ...S.throw, side: 0, stroke: BEAT, follow: 0.25, flight: 6 * BEAT, land: FAN[k]!, turns: 1 },
  // The shoved boat starts at the front-left of the disc (the camera's left), side on, stern toward the slam, and
  // rides forward and to the left; it stays where the ride leaves it until its cycle turns at the seam.
  boatTwo: { ...S.boatTwo, bearing: 0.6, heading: -2.2, keep: PERIOD },
});

const how = (k: number): Staging => ({
  ...standing(k),
  walkAhead: k / 4,
  // Every boat goes home on a cut (the seam, 6.0): no sinking, no sailing in.
  show: { kind: "cut" },
  taunt: { kind: "dance", first: TAUNT_ARMS, accents: SCORE.taunt(k).map((t) => t - TAUNT_ARMS) },
  audience: { kind: "at", eye: AUDIENCE },
});

/**
 * The sunset (docs/lineup.md, "The sky"): a bokashi from orange-gold at the horizon through vermilion and violet to
 * indigo, a red sun half set behind the gap between the middle pair, three cream cloud bands, and the far sea's strip
 * at the horizon printed orange-gold (the sky's `below`). Heights are the view direction's y: the wide frames reach
 * about 0.2 above the horizon.
 */
const SUNSET = {
  grade: { kind: "stops", stops: [{ at: 0, ink: [0.97, 0.74, 0.36] }, { at: 0.03, ink: [0.95, 0.52, 0.22] }, { at: 0.07, ink: [0.82, 0.28, 0.2] }, { at: 0.12, ink: [0.42, 0.24, 0.42] }, { at: 0.2, ink: [0.13, 0.15, 0.33] }] },
  sun: { kind: "disc", toward: [-1, 0.012, 0], radius: 0.075, ink: [0.84, 0.2, 0.13] },
  clouds: { kind: "bands", ink: [0.93, 0.87, 0.74], line: 0.0012, bands: [
    // Thin and short: the first strip's (0.018 to 0.03 thick, 0.26 to 0.37 long) were slabs across the frame.
    { at: 0.112, thick: 0.013, from: 2.88, to: 3.1, lumps: 4 }, { at: 0.145, thick: 0.011, from: 3.2, to: 3.38, lumps: 3 }, { at: 0.085, thick: 0.009, from: 3.3, to: 3.45, lumps: 3 },
  ] },
  below: [0.93, 0.6, 0.3],
} as const;

/**
 * The look the deliverables ask for by name. Today one: the print with the sunset. The lighting recipes being built
 * (a hatched light, a reference palette, a sunset recipe) arrive as named options of the style; each becomes a case
 * here, and an unknown name is refused (not drawn as the default).
 */
export const LOOKS = ["default"] as const;
export type Look = (typeof LOOKS)[number];
function lookOf(name: string): Look {
  const l = LOOKS.find((x) => x === name);
  if (l === undefined) throw new Error(`crab lineup: no look "${name}"; the looks are [${LOOKS.join(", ")}]`);
  return l;
}
function styleOf(look: Look) {
  switch (look) {
    case "default": return styleFor(S, WATER.base, SUNSET);
  }
}

/** The scene's cameras: each shot's first frame, by format and shot (`wide_slam`, `square_taunt`, ...); `lineup` is the main framing. */
export const cameras: Readonly<Record<string, Camera>> = Object.fromEntries([
  ["lineup", shotAt(FORMATS.wide, 0).camera],
  ...Object.values(FORMATS).flatMap((f) => f.cuts.map((c) => [`${f.name}_${c.shot.name}`, shotAt(f, c.at).camera] as const)),
]);

/** The lineup with any four variants (left to right) in a look. */
export function lineupScene(o: { readonly line: readonly string[]; readonly look: string }) {
  if (o.line.length !== 4) throw new Error(`crab lineup: four variants, left to right; got ${o.line.length}`);
  const line = lineup({
    members: o.line.map((variant, k) => ({ name: `crab${k}`, design, variant, numbers: onTheBeat(k), how: how(k) })),
    // The swells' periods are 12 s and 8 s: whole turns in the loop.
    water: { swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 13 / 12 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 6 / 8, phase: 1.3 }], foam: 0.85 },
    loop: { kind: "every", seconds: PERIOD },
  });
  return { scene: scene({ sea: line.sea, things: line.things, style: styleOf(lookOf(o.look)), sun: SUN, cameras, period: PERIOD }), checks: line.checks, members: line.members };
}

const built = lineupScene({ line: LINE, look: "default" });
export default built.scene;
export const checks = built.checks;
/** Each crab's product, in line order: what a game reads of it (`holds`, `waves`, `handoff`, `rig.dance`). */
export const members = built.members;

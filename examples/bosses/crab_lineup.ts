// Five crabs in one scene, dancing as one group, as one loop (docs/lineup.md, "A drilled group"). Each is a variant of
// the Reefback Titan's design (examples/bosses/reefback_titan.ts) built in its own namespace, on one sea that carries
// every ring wave. One clock in counts: 120 a minute, 4/4, sixteen bars, 32 s. One score (`DANCE`) for the group: key
// poses on counts, in the group's manner, unison unless a key says canon or mirror; the slam and the throw are the
// group's point moves, in unison, in a line; each crab's own manner (`temper`) is kept for its solo.
//
// Staged for a video: shots in examples/bosses/lineup_shots.ts, which the deliverables read (`bun tools/lineup_deliver.ts`).
// The scene's cameras are those shots' first frames.

import type { Camera } from "../../src/artefact.ts";
import { atan2, div, lineup, movable, parseVariantAsk, routine, scene, sub } from "../../src/index.ts";
import { design, LOOK, styleFor, SUN, WATER } from "./reefback_titan.ts";
import { AUDIENCE, BAR, BEAT, beat, formats, PERIOD, PLACES, shotAt } from "./lineup_shots.ts";
import { S, type Numbers } from "./titan/numbers.ts";
import type { Staging } from "./titan/rig.ts";

export { BAR, BARS, BEAT, BPM, PERIOD } from "./lineup_shots.ts";

// ---- the group, left to right as the camera sees it; the islander (the hermit, its spiral shell) is the centre ----
export const LINE = ["fiddler", "mask", "islander", "bruiser", "stilt"] as const;

// ---- the score ----
/** Where a claw's wrist goes from its shoulder (forward, up, out from its own side), in arm spans: straight up, out to the side, across in front. */
const UP = [0.2, 0.76, 0.12] as const, OUT = [0.2, 0.28, 0.76] as const, ACROSS = [0.6, 0.28, -0.44] as const;
/** A quarter turn to the diagonal: 45 degrees to the crab's right (the camera's left). */
const DIAGONAL = Math.PI / 4;

/**
 * The dance, in counts (a count is a beat; 64 in the loop). Each key lands on its count and holds until the next key
 * of the same channel; everyone together unless `who` says otherwise. The slam (claws down on 12) and the throw
 * (grips close on 21, every boat let go on 26) are the attacks' own, on the same counts for every member (`onTheBeat`).
 */
export const DANCE = routine({
  beat: BEAT, length: 64, centre: 2, places: PLACES,
  manner: { arrive: 0.15, snap: 4 },
  size: { bounce: 0.22, lean: 0.4, roll: 0.08, level: 0.55 },
  score: [
    // 1 to 8: out of the button, a lean each way, sidestep into the line.
    { at: 1, level: 0, claw: "rest", small: "rest" },
    { at: 2, lean: 1 }, { at: 3, lean: -1 }, { at: 4, lean: 0 },
    { at: 8, go: "line", over: 4 },
    // 8 to 16: crouch to load the slam (9 to 12 is the slam's own wind-up and strike); up again.
    { at: 8, level: -0.6 }, { at: 13, level: 0 },
    // 16 to 26: the throw (the attack has the body). 27 to 32: a claw punched up as the boats fly; a lean each way; they land on 32.
    { at: 27, claw: UP, level: 1 }, { at: 29, claw: "rest", level: 0 },
    { at: 30, lean: -1 }, { at: 31, lean: 1 }, { at: 32, lean: 0 },
    // 34 to 38: sidestep into the diagonal (the fiddler in front: the centre changes). 38 to 43: up; down and lean;
    // a quarter turn with the claw swept out; lean; turn back, both claws across; a deep crouch.
    { at: 38, go: "diagonal", over: 4 },
    { at: 38, claw: UP, level: 1 }, { at: 39, claw: "rest", level: 0, lean: -1 },
    { at: 40, face: DIAGONAL, over: 1, lean: 0 }, { at: 40, claw: OUT }, { at: 41, lean: 1 },
    { at: 42, face: 0, over: 1, claw: ACROSS, small: ACROSS, lean: 0 }, { at: 43, level: -1, claw: "rest", small: "rest" },
    // 44 to 46: a canon down the line, half a count apart: up and rise. 47: a mirror: lean out, both claws out.
    { at: 44, who: { canon: 0.5 }, claw: UP, level: 1 },
    { at: 47, who: "mirror", lean: -1, claw: OUT, small: OUT, level: 0 },
    // 48 to 58: the solos, two counts each, while the rest hold a low frame and keep the groove.
    { at: 48, lean: 0, level: -0.3, claw: ACROSS, small: "rest" },
    { solo: 0, on: [48, 49] }, { solo: 1, on: [50, 51] }, { solo: 2, on: [52, 53] }, { solo: 3, on: [54, 55] }, { solo: 4, on: [56, 57] },
    // 58 to 62: sidestep back into the V; the button on 62, struck and held dead still across the loop's seam.
    { at: 58, claw: "rest", level: 0 },
    { at: 62, go: "V", over: 4 },
    { at: 62, hit: 2.5, claw: UP, small: OUT, level: 1 },
  ],
});

/** The group's manner: every member's `temper` in the lineup, so the attacks' rear and the shell's lag are in unison too. A crab's own is kept for its solo. */
const GROUP: Numbers["temper"] = { snap: 4, arrive: 0.15, hold: 0, leave: 0.25, jitter: 0, lag: 0.2, rise: 0.5, reach: 1, together: 0, lift: 1 };

/**
 * Where crab k's thrown boat lands, in its axes: forward, in the line's foreground (a throw must carry 25 from where the
 * boat was taken: `throwLands`). A host parks the landed boat out of every frame from the cut on 33 to the cut on 16
 * (`hostInputs`, lineup_shots.ts).
 */
export const FAN = [[42, 0], [40, 0], [38, -2], [40, -4], [42, -4]] as const;
/** What the scene changes of a crab's numbers: its attacks' seconds on counts, its props in its lane, the group's manner. The crab alone keeps its own. */
const onTheBeat = (k: number) => ({
  crushPeriod: PERIOD, period: BAR, temper: GROUP,
  // Three counts from the claw's first movement to its landing on 12; warned four counts ahead, so its cycle turns on 8.
  crushSeconds: { windup: 3 * BEAT - 0.3, strike: 0.3 }, crushWarn: 4 * BEAT, crushFirst: beat(12),
  // The disc in the crab's lane: in front, a little to its right (the camera's left).
  crushCentre: [10, 1.5] as const,
  // Five counts from the cutter's first movement to its closing on 21; a hold of five (squeezes on 22 and 24, the whip on
  // 25), let go on 26, a flight of six. Warned five counts ahead, so its cycle turns (the boat home) on the cut on 16.
  grabClose: beat(21), grabWarn: 5 * BEAT,
  grabSeconds: { windup: 0.9, reach: 5 * BEAT - 0.9 - 0.22, close: 0.22, hold: 5 * BEAT, release: 1.3 },
  // The boat taken nearly straight ahead of the left shoulder, in the crab's lane.
  grabBearing: -0.26,
  // Thrown forward, toward the camera, drawn straight back first (`side` 0), one turn in the air (docs/lineup.md, "Staging").
  throw: { ...S.throw, side: 0, stroke: BEAT, follow: 0.25, flight: 6 * BEAT, land: FAN[k]!, turns: 1 },
  boatTwo: { ...S.boatTwo, keep: PERIOD },
});

/** A crab's own manner (its variant's `temper`), for its solo. */
const ownManner = (variant: string): Numbers["temper"] => design.resolve(parseVariantAsk(variant)).numbers.temper;
const how = (k: number, variant: string): Staging => ({
  at: DANCE.member(k).home, heading: 0, walkAhead: 0,
  // Every boat goes home on a cut (16, the seam): no sinking, no sailing in.
  show: { kind: "cut" },
  taunt: { kind: "routine", dancer: DANCE.member(k), size: DANCE.spec.size, beat: BEAT, own: ownManner(variant) },
  audience: { kind: "at", eye: AUDIENCE },
});

/**
 * The sunset (docs/lineup.md, "The sky"): a bokashi from orange-gold at the horizon through vermilion and violet to
 * indigo, a red sun half set behind the group, three cream cloud bands, and the far sea's strip at the horizon printed
 * orange-gold (the sky's `below`). Heights are the view direction's y.
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

/** The looks the deliverables ask for by name; an unknown one is refused (not drawn as the default). */
export const LOOKS = ["default", "hatched", "cel", "engraving", "engravingSunset"] as const;
export type Look = (typeof LOOKS)[number];
function lookOf(name: string): Look {
  const l = LOOKS.find((x) => x === name);
  if (l === undefined) throw new Error(`crab lineup: no look "${name}"; the looks are [${LOOKS.join(", ")}]`);
  return l;
}
function styleOf(look: Look) {
  switch (look) {
    case "default": return styleFor(S, WATER.base, SUNSET);
    case "hatched": return styleFor(S, WATER.base, SUNSET, LOOK.hatched);
    case "cel": return styleFor(S, WATER.base, SUNSET, LOOK.cel);
    case "engraving": return styleFor(S, WATER.base, SUNSET, LOOK.engraving);
    case "engravingSunset": return styleFor(S, WATER.base, SUNSET, LOOK.engravingSunset);
  }
}
/** The sun of a look: the sunset recipe is lit from low behind the group (docs/lighting.md); the others by the crab's own sun. */
function sunOf(look: Look) {
  switch (look) {
    case "default": case "hatched": case "cel": case "engraving": return SUN;
    case "engravingSunset": return movable([-0.95, 0.3, 0.05]);
  }
}

/** The scene's cameras: each shot's first frame, by format and shot (`wide_open`, `square_solo2`, ...); `lineup` is the opening framing. */
const camerasOf = (line: readonly string[]): Readonly<Record<string, Camera>> => {
  const F = formats(line);
  return Object.fromEntries([["lineup", shotAt(F.wide, 0).camera], ...Object.values(F).flatMap((f) => f.cuts.map((c) => [`${f.name}_${c.shot.name}`, shotAt(f, c.at).camera] as const))]);
};

/** The group, left to right (as many variants as `DANCE` has members), in a look. */
export function lineupScene(o: { readonly line: readonly string[]; readonly look: string }) {
  if (o.line.length !== DANCE.members) throw new Error(`crab lineup: the dance has ${DANCE.members} members, left to right; got ${o.line.length} variants`);
  const line = lineup({
    members: o.line.map((variant, k) => ({ name: `crab${k}`, design, variant, numbers: onTheBeat(k), how: how(k, variant) })),
    // The swells' periods are 16 s and 8 s: whole turns in the loop.
    water: { swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 13 / 16 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 6 / 8, phase: 1.3 }], foam: 0.85 },
    loop: { kind: "every", seconds: PERIOD },
  });
  // The instrument (docs/lineup.md, "The instrument"): from each member's own rig, when each key arrives and how far it
  // goes, member against member: arrivals within a frame at 60 a second, sizes within 5% of the channel's largest.
  const sync = DANCE.inSync((k) => {
    const { rig: r, great } = line.members[k]!.made, shoulder = r.local(r.shoulder(1));
    return { lift: sub(r.body.o.y, r.n.ride), lean: r.body.z.y, face: atan2(r.body.x.z, r.body.x.x), claw: div(sub(great.wrist.y, shoulder.y), great.span) };
  }, { arrival: 1 / 60, size: 0.05 });
  const cameras = camerasOf(o.line);
  return { scene: scene({ sea: line.sea, things: line.things, style: styleOf(lookOf(o.look)), sun: sunOf(lookOf(o.look)), cameras, period: PERIOD }), checks: [...line.checks, sync], sync, members: line.members };
}

const built = lineupScene({ line: LINE, look: "default" });
export default built.scene;
export const checks = built.checks;
/** Each crab's product, in line order: what a game reads of it (`holds`, `waves`, `handoff`, `rig.dance`). */
export const members = built.members;
/** The sync instrument (also the last of `checks`): `sync.table(probe)` gives every key's arrivals and sizes. */
export const sync = built.sync;

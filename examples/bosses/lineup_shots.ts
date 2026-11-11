// The crab lineup's tempo, formation and shots (docs/lineup.md, "Staging"). Plain numbers, no scene: the scene
// (crab_lineup.ts), the deliverables (tools/lineup_deliver.ts) and the framing stand-in (tools/lineup_framing.ts) all
// read them here, so a camera is stated once.

import type { Camera } from "../../src/artefact.ts";

// ---- the tempo ----
export const BPM = 120;
/** Seconds in a beat and in a bar (4/4), and the loop: twelve bars. */
export const BEAT = 60 / BPM, BAR = 4 * BEAT, BARS = 12, PERIOD = BARS * BAR;
/** Beat b (0 is the loop's first downbeat), in seconds. */
export const beat = (b: number): number => b * BEAT;

// ---- the formation: an open row, 20 apart (16 left the fiddler's great claw within reach of the islander's face), nearly facing the camera, so that each crab can be framed alone, large, with
// no neighbour across its face (docs/lineup.md, "Third staging": the edit gives each crab its moment; the chevron of the
// second staging hid the outer pair behind the middle pair's claws and stalks).
/** Lateral spacing (world z). */
export const SPACING = 20;
/** Where the faces look: far in front of the row's middle, at face height (each single's camera is nearly in that line). */
export const AUDIENCE = [100, 3, 0] as const;
/** Crab k (0 .. 3, left to right as the camera sees it; the camera's left is +z): its place and heading (radians from +x toward +z). */
export function standing(k: number): { readonly at: readonly [number, number]; readonly heading: number } {
  const at = [0, (1.5 - k) * SPACING] as const;
  return { at, heading: Math.atan2(AUDIENCE[2] - at[1], AUDIENCE[0] - at[0]) };
}

// ---- the boats a host moves: the thrown boat, once landed, is parked out of every frame at the cut into the taunt ----
/**
 * Each crab's `throw.land` (in its own axes) at loop time t: where its thrown boat comes down from the throw until the
 * cut into the taunt (16.0), and from there to the Grab's next cycle (6.0, another cut) a place far behind the row, out
 * of every frame. The boat lies wherever `throw.land` says once it has landed, so moving the input moves it; both moves
 * fall on cuts, and the throw itself (12.0 to 15.0) reads the true landing. Checks read the defaults (the true landing).
 */
export const PARKED = [-70, 0] as const;
export function hostInputs(names: readonly string[], land: (k: number) => readonly [number, number], t: number): Record<string, readonly number[]> {
  const u = ((t % PERIOD) + PERIOD) % PERIOD, parked = u >= beat(32) || u < beat(12);
  return Object.fromEntries(names.map((n, k) => [`${n}.throw.land`, parked ? PARKED : land(k)]));
}

// ---- the shots ----
type V3 = readonly [number, number, number];
/** A camera: low (just above the water), level, perspective. */
export type Lens = { readonly eye: V3; readonly target: V3; readonly fovY: number };
/** What a shot frames: one crab large, a neighbouring pair, or all four. What `lineup_framing` counts screen time by. */
export type Subject = { readonly kind: "single"; readonly k: number } | { readonly kind: "pair"; readonly k: number } | { readonly kind: "wide"; readonly distance: number };
/** A shot holds one camera, or moves from one to another between two instants (seconds of the loop), eased in and out. */
export type Shot =
  | { readonly kind: "still"; readonly name: string; readonly subject: Subject; readonly lens: Lens }
  | { readonly kind: "move"; readonly name: string; readonly subject: Subject; readonly from: Lens; readonly to: Lens; readonly start: number; readonly end: number };
/** A format: its frame, and its shots, each from `at` seconds to the next one's (the last to the period). Every `at` is on a beat. */
export type Format = { readonly name: string; readonly width: number; readonly height: number; readonly cuts: readonly { readonly at: number; readonly shot: Shot }[] };

const lens = (eye: V3, target: V3, fovY: number): Lens => ({ eye, target, fovY });
/** fovY 0.4 rad, about 59 mm on a full frame, for every shot. */
const FOV = 0.4;
/**
 * The camera of a subject. A single stands straight in front of its crab, the eye 3 above the water (2 showed the
 * foam collar at the feet as a line), close enough that the body (shell top to waterline, measured on the strips:
 * 3.4 to 4.5 for the low crabs) is at least 35% of the height: 26 to 32 away; the spider crab, whose body stands 6.8 up
 * on its stilts, from 40, aimed higher. A pair from 82, between its two (from 58 the spider crab, tall on its stilts, was large in every pair and had twice the others' large time). The wides from in front of the middle.
 */
function framing(s: Subject): Lens {
  switch (s.kind) {
    case "single": {
      const z = standing(s.k).at[1], d = SINGLE[s.k]!;
      return lens([d.distance, 3, z], [0, d.aim, z], FOV);
    }
    case "pair": { const z = (standing(s.k).at[1] + standing(s.k + 1).at[1]) / 2; return lens([82, 3, z], [0, 6, z], FOV); }
    case "wide": return lens([s.distance, 4, 0], [0, 6.5, 0], FOV);
  }
}
/** Per crab (left to right: islander, fiddler, samurai, spider crab), a single's distance and aim (target height). */
const SINGLE = [{ distance: 30, aim: 4.4 }, { distance: 26, aim: 4.2 }, { distance: 31, aim: 4.4 }, { distance: 40, aim: 7 }] as const;

type Cue = readonly [number, Subject];
const one = (k: number): Subject => ({ kind: "single", k }), two = (k: number): Subject => ({ kind: "pair", k });
const wide = (distance: number): Subject => ({ kind: "wide", distance });
/**
 * The edit, in beats (0 is the loop's first downbeat; 48 in the loop). Slam: all four for a bar (the cascade's wind-ups),
 * each crab alone on its own slam's beat, then the pairs as the rings carry the boats out. Throw: the pairs as the claws
 * rise, each alone for a beat as the grips close and squeeze, all four for the whip, the release and the flight, each
 * alone watching, all four for the four splashes. Taunt: each alone on its own accents (call and answer: islander,
 * fiddler, samurai, spider crab, twice), the cascade as four singles a beat each, all four for the unison's bar. The
 * last bar's wide is tighter than the first's, so the seam is a cut (the shoved boats go home there, unseen).
 */
const EDIT: readonly Cue[] = [
  [0, wide(114)], [4, one(0)], [5, one(1)], [6, one(2)], [7, one(3)], [8, two(0)], [10, two(2)],
  [12, two(0)], [15, two(2)], [18, one(0)], [19, one(1)], [20, one(2)], [21, one(3)], [22, wide(114)],
  [26, one(0)], [27, one(1)], [28, one(2)], [29, one(3)], [30, wide(114)],
  [32, one(0)], [33, one(1)], [34, one(2)], [35, one(3)], [36, one(0)], [37, one(1)], [38, one(2)], [39, one(3)],
  [40, one(0)], [41, one(1)], [42, one(2)], [43, one(3)], [44, wide(100)],
];
/** The square: the same edit with its middle wides as pairs (four across a square is small): the whip and release on the left pair then the right, the splashes on the middle. */
const SQUARE_EDIT: readonly Cue[] = EDIT.flatMap(([b, s]): Cue[] => {
  switch (s.kind) {
    case "single": case "pair": return [[b, s]];
    case "wide": return b === 22 ? [[22, two(0)], [24, two(2)]] : b === 30 ? [[30, two(1)]] : [[b, s]];
  }
});
const SECTION = (b: number): string => (b < 12 ? "slam" : b < 32 ? "throw" : "taunt");
const cutsOf = (edit: readonly Cue[]) => edit.map(([b, subject]) => ({ at: beat(b), shot: { kind: "still" as const, name: `${SECTION(b)}_${b}`, subject, lens: framing(subject) } }));
export const WIDE: Format = { name: "wide", width: 1920, height: 1080, cuts: cutsOf(EDIT) };
export const SQUARE: Format = { name: "square", width: 1080, height: 1080, cuts: cutsOf(SQUARE_EDIT) };
export const FORMATS = { wide: WIDE, square: SQUARE } as const;
export type FormatName = keyof typeof FORMATS;

/** Ease in and out (the shape of `slowInOut` in src/geom/lively.ts, power 2), on the CPU: a camera is a number per frame. */
const ease = (u: number): number => (u < 0.5 ? 2 * u * u : 1 - 2 * (1 - u) * (1 - u));
const mixLens = (a: Lens, b: Lens, u: number): Lens => {
  const m = (x: number, y: number) => x + (y - x) * u;
  return { eye: [m(a.eye[0], b.eye[0]), m(a.eye[1], b.eye[1]), m(a.eye[2], b.eye[2])], target: [m(a.target[0], b.target[0]), m(a.target[1], b.target[1]), m(a.target[2], b.target[2])], fovY: m(a.fovY, b.fovY) };
};
/** The shot's camera at loop time t. */
function lensOf(s: Shot, t: number): Lens {
  switch (s.kind) {
    case "still": return s.lens;
    case "move": return mixLens(s.from, s.to, ease(Math.min(Math.max((t - s.start) / (s.end - s.start), 0), 1)));
  }
}
/** The shot in force at time t (wrapped into the loop), and its camera. */
export function shotAt(f: Format, t: number): { readonly shot: Shot; readonly camera: Camera } {
  const u = ((t % PERIOD) + PERIOD) % PERIOD;
  let cut = f.cuts[0]!;
  for (const c of f.cuts) if (c.at <= u) cut = c;
  const l = lensOf(cut.shot, u);
  return { shot: cut.shot, camera: { kind: "perspective", eye: [...l.eye], target: [...l.target], fovY: l.fovY } };
}

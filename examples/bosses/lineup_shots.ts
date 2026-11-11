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

// ---- the formation: a chevron. The middle pair (the fiddler and the samurai, the low crabs) 8 forward and 11 apart, the
// outer pair 8 back and 16 out (12 in the strip hid them wholly behind the middle pair, whose bodies are wider than
// the stand-in's boxes), so that the camera can come close enough for the middle pair's bodies to be a third of
// the frame and the outer pair still stands in it, smaller and further, faces clear (docs/lineup.md, "Second staging").
/** The middle pair's spacing (world z), the outer pair's offset from the middle, and how far forward / back each pair stands. */
export const MIDDLE = 11, OUTER = 16, ADVANCE = 8;
/** Where the faces look: the main framing's eye. */
export const AUDIENCE = [44, 2.5, 0] as const;
/** Crab k (0 .. 3, left to right as the camera sees it; the camera's left is +z): its place and heading (radians from +x toward +z). */
export function standing(k: number): { readonly at: readonly [number, number]; readonly heading: number } {
  const middle = k === 1 || k === 2;
  const at = [middle ? ADVANCE : -ADVANCE, middle ? (1.5 - k) * MIDDLE : (k === 0 ? OUTER : -OUTER)] as const;
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
/** A camera: low (just above the water, below the faces), level, perspective. */
export type Lens = { readonly eye: V3; readonly target: V3; readonly fovY: number };
/** A shot holds one camera, or moves from one to another between two instants (seconds of the loop), eased in and out. */
export type Shot =
  | { readonly kind: "still"; readonly name: string; readonly lens: Lens }
  | { readonly kind: "move"; readonly name: string; readonly from: Lens; readonly to: Lens; readonly start: number; readonly end: number };
/** A format: its frame, and its shots, each from `at` seconds to the next one's (the last to the period). Every `at` is a downbeat. */
export type Format = { readonly name: string; readonly width: number; readonly height: number; readonly cuts: readonly { readonly at: number; readonly shot: Shot }[] };

const lens = (eye: V3, target: V3, fovY: number): Lens => ({ eye, target, fovY });

/**
 * 16:9. fovY 0.4 rad (about 59 mm on a full frame): closer and wider than the first staging's 85 mm, which could not
 * make the bodies large (a long lens from in front cannot shorten a row of four). The eye 2 above the water, below the
 * faces. Three shots cut on downbeats: the slam wide (all four, the boats riding out across the foreground), the throw
 * in the main framing (the middle pair's bodies a third of the height, the outer pair in frame behind; the thrown boats
 * go over the camera), the taunt pushing in from the main framing to the middle pair's two faces. The cut at 6.0 hides the
 * thrown boats going home (the grab's cycle turns there), the seam the shoved ones (the crushes' cycles turn at 0).
 */
export const WIDE: Format = {
  name: "wide", width: 1920, height: 1080, cuts: [
    { at: 0, shot: { kind: "still", name: "slam", lens: lens([58, 3, 0], [0, 6.5, 0], 0.4) } },
    { at: beat(12), shot: { kind: "still", name: "throw", lens: lens([44, 2, 0], [0, 4.8, 0], 0.4) } },
    { at: beat(32), shot: { kind: "move", name: "taunt", from: lens([44, 2, 0], [0, 4.8, 0], 0.4), to: lens([31, 2, 0], [0, 4.5, 0], 0.4), start: beat(32), end: PERIOD } },
  ],
};
/**
 * Square. Four across a square cannot be large, so: the slam wide (all four, small), the throw on the middle pair (the
 * outer pair at the edges), the taunt pushing in to the middle pair's faces. fovY 0.4 as the 16:9.
 */
export const SQUARE: Format = {
  name: "square", width: 1080, height: 1080, cuts: [
    { at: 0, shot: { kind: "still", name: "slam", lens: lens([62, 3, 0], [0, 6.5, 0], 0.4) } },
    { at: beat(12), shot: { kind: "still", name: "throw", lens: lens([48, 2, 0], [0, 5, 0], 0.4) } },
    { at: beat(32), shot: { kind: "move", name: "taunt", from: lens([48, 2, 0], [0, 5, 0], 0.4), to: lens([33, 2, 0], [0, 4.5, 0], 0.4), start: beat(32), end: PERIOD } },
  ],
};
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

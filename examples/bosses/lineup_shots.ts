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

// ---- the formation: one row, the middle pair a little forward (a shallow chevron), each crab facing the audience ----
/** Lateral spacing (world z) and how far the middle pair stands forward (world x, toward the camera). */
export const SPACING = 12.5, ADVANCE = 3;
/** Where the faces look: in front of the line at the taunt's middle distance, a little above the water. */
export const AUDIENCE = [104, 3.5, 0] as const;
/** Crab k (0 .. 3, left to right as the camera sees it; the camera's left is +z): its place and heading (radians from +x toward +z). */
export function standing(k: number): { readonly at: readonly [number, number]; readonly heading: number } {
  const at = [k === 1 || k === 2 ? ADVANCE : 0, (1.5 - k) * SPACING] as const;
  return { at, heading: Math.atan2(AUDIENCE[2] - at[1], AUDIENCE[0] - at[0]) };
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
/** Looking along -x at world z `z`: the square's truck along the line. */
const along = (d: number, h: number, ty: number, z: number, fovY: number): Lens => lens([d, h, z], [0, ty, z], fovY);

/**
 * 16:9. fovY 0.28 rad is about an 85 mm lens on a full frame (24 / (2 tan 0.14)): the long end of the references'
 * 75 to 85 mm, where the four fill the width with their faces at 90 px or more. Three shots cut on downbeats: the slam
 * (the main framing, where the targets are measured), the throw (pulled back, so the boats thrown forward land in
 * frame), the taunt (a slow push in on the faces from the slam's framing to 112; to 100, the second strip cut the
 * islander in half at the left edge). The cut at 6.0 hides the thrown boats going home (the grab's cycle
 * turns there) and the cut at the seam the shoved ones (the crushes' cycles turn at 0).
 */
export const WIDE: Format = {
  name: "wide", width: 1920, height: 1080, cuts: [
    { at: 0, shot: { kind: "still", name: "slam", lens: lens([120, 3, 0], [0, 10.5, 0], 0.28) } },
    { at: beat(12), shot: { kind: "still", name: "throw", lens: lens([138, 3, 0], [0, 11, 0], 0.28) } },
    { at: beat(32), shot: { kind: "move", name: "taunt", from: lens([122, 3, 0], [0, 10.5, 0], 0.28), to: lens([112, 3.5, 0], [0, 10, 0], 0.28), start: beat(32), end: PERIOD } },
  ],
};
/**
 * Square. Four across a square frame cannot be large (each crab a quarter of the width is a quarter of the height), so
 * the square trucks along the row two crabs at a time for the slam (following the cascade) and the taunt (following
 * the call and answer and the cascade), and holds a wider frame on the middle pair for the throw in unison. fovY 0.25
 * (about 96 mm) from 96 to 135 away: a frame 24 to 34 units wide at the row. From 60 with fovY 0.4 (the second strip)
 * the boats thrown 38 forward landed 22 from the lens and their sails filled the square's foreground.
 */
export const SQUARE: Format = {
  name: "square", width: 1080, height: 1080, cuts: [
    { at: 0, shot: { kind: "move", name: "slam", from: along(100, 3.5, 8, SPACING, 0.25), to: along(100, 3.5, 8, -SPACING, 0.25), start: 1.75, end: 3.75 } },
    { at: beat(12), shot: { kind: "still", name: "throw", lens: along(135, 3.5, 8.5, 0, 0.25) } },
    { at: beat(32), shot: { kind: "move", name: "taunt", from: along(96, 3.5, 8, SPACING, 0.25), to: along(96, 3.5, 8, -SPACING, 0.25), start: beat(33), end: beat(47) } },
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

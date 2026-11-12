// The crab lineup's tempo, formations and shots (docs/lineup.md, "A drilled group"). Plain numbers, no scene: the scene
// (crab_lineup.ts), the deliverables (tools/lineup_deliver.ts) and the framing stand-in (tools/lineup_framing.ts) all
// read them here, so a camera is stated once.

import type { Camera } from "../../src/artefact.ts";

// ---- the tempo ----
export const BPM = 120;
/** Seconds in a beat (a count) and in a bar (4/4), and the loop: sixteen bars, 64 counts. */
export const BEAT = 60 / BPM, BAR = 4 * BEAT, BARS = 16, PERIOD = BARS * BAR;
/** Count c (0 is the loop's first downbeat), in seconds. */
export const beat = (c: number): number => c * BEAT;

// ---- the group: left to right as the camera sees it; the centre is the middle member ----
/** Where the faces look: far in front of the group (each crab faces +x; the camera's left is +z). */
export const AUDIENCE = [100, 3, 6] as const;
type XZ = readonly [number, number];
/**
 * The formations: every member's place (world x toward the camera, z the camera's left), left to right. Every crab
 * carries its great claw on its own right (the camera's left), so in the V the left wing stands behind a claw: 13 apart
 * there, 7 on the right. The diagonal (front-left to back-right) has every claw on the side away from those behind it,
 * so it is tight (4 deep, 7 across). The line is for the attacks: each crab's disc and boat in its own lane (13 apart).
 */
export const PLACES: Readonly<Record<"V" | "line" | "diagonal", readonly XZ[]>> = {
  V: [[-6, 26], [-3, 13], [0, 0], [-3, -7], [-6, -14]],
  line: [[0, 32], [0, 19], [0, 6], [0, -7], [0, -20]],
  diagonal: [[8, 20], [4, 13], [0, 6], [-4, -1], [-8, -8]],
};

// ---- the boats a host moves ----
/**
 * The host's inputs at loop time t. The shoved boats (`boat2.at`) are parked out of every frame all through: the slam
 * is about the water. Each thrown boat (`throw.land`, in its crab's axes) comes down where it is thrown, in the line's
 * foreground, on count 32; from the cut on 33 to the Grab's next cycle (the cut on 16, where it is home again) the
 * landing is a place far behind the group. Every move of a boat is on a cut. Checks read the defaults (the true places).
 */
export const PARKED = [-70, 0] as const;
export function hostInputs(names: readonly string[], land: (k: number) => XZ, t: number): Record<string, readonly number[]> {
  const u = ((t % PERIOD) + PERIOD) % PERIOD, thrownParked = u >= beat(33) || u < beat(16);
  return Object.fromEntries(names.flatMap((n, k) => [[`${n}.throw.land`, thrownParked ? PARKED : land(k)], [`${n}.boat2.at`, PARKED]]));
}

// ---- the shots ----
type V3 = readonly [number, number, number];
/** A camera: low (just above the water), level, perspective. */
export type Lens = { readonly eye: V3; readonly target: V3; readonly fovY: number };
/** A shot holds one camera, or moves from one to another between two instants (seconds of the loop), eased in and out. */
export type Shot =
  | { readonly kind: "still"; readonly name: string; readonly lens: Lens }
  | { readonly kind: "move"; readonly name: string; readonly from: Lens; readonly to: Lens; readonly start: number; readonly end: number };
/** A format: its frame, and its shots, each from `at` seconds to the next one's (the last to the period). Every `at` is on a count. */
export type Format = { readonly name: string; readonly width: number; readonly height: number; readonly cuts: readonly { readonly at: number; readonly shot: Shot }[] };

const lens = (eye: V3, target: V3, fovY: number): Lens => ({ eye, target, fovY });
/** One member alone, straight in front of its place (a solo, a face): close enough that its body is a third of the height or more (docs/lineup.md, "Third staging"). */
const SINGLE: Readonly<Record<string, { readonly distance: number; readonly aim: number }>> = {
  fiddler: { distance: 26, aim: 3.2 }, mask: { distance: 31, aim: 3.4 }, islander: { distance: 30, aim: 3.4 }, bruiser: { distance: 30, aim: 3.4 }, stilt: { distance: 40, aim: 5.8 },
};
const single = (variant: string, at: XZ): Lens => { const d = SINGLE[variant.split("+")[0]!] ?? SINGLE.islander!; return lens([at[0] + d.distance, 3, at[1]], [at[0], d.aim, at[1]], 0.4); };

/**
 * The group's framings (predicted in docs/lineup.md), by format. Each is aimed so the group's waterline sits about 80%
 * down the frame and the sunset fills the top (aimed at the group's own height, 4.5 to 6 up, the first strip put the
 * group in a band across the middle over 40% of empty sea): the target's height is the eye's plus the distance times
 * the angle from the waterline (eye height over distance) to 30% of the field of view above it.
 */
const aimed = (eye: V3, at: readonly [number, number], fovY: number): Lens => {
  const d = Math.hypot(eye[0] - at[0], eye[2] - at[1]), a = 0.3 * fovY - eye[1] / d;
  return lens(eye, [at[0], eye[1] + d * Math.tan(a), at[1]], fovY);
};
const GROUP = {
  wide: {
    V: aimed([55, 3, 6], [0, 6], 0.5), line: aimed([95, 3, 6], [0, 6], 0.4), lineLow: aimed([86, 2.5, 6], [0, 6], 0.4),
    lineClose: aimed([78, 2.5, 6], [0, 6], 0.4), diagonal: aimed([63, 3, 6], [8, 6], 0.4), diagonalIn: aimed([55, 3, 6], [8, 6], 0.4),
  },
  square: {
    V: aimed([62, 3, 4], [0, 4], 0.6), line: aimed([80, 3, 6], [0, 6], 0.6), lineLow: aimed([74, 2.5, 6], [0, 6], 0.6),
    lineClose: aimed([68, 2.5, 6], [0, 6], 0.6), diagonal: aimed([66, 3, 6], [8, 6], 0.5), diagonalIn: aimed([58, 3, 6], [8, 6], 0.5),
  },
} as const;

/**
 * The edit, in counts: the group in frame for 52 of 64, cut to a member only for its solo and one face. The V opens and
 * pulls back as the group sidesteps into the line (4 to 8); the slam on the line; a cut on 16 to a lower framing that
 * pushes in through the throw; the islander's face on 33 (the boats have landed); the group sidesteps into the diagonal
 * as the camera pushes in (34 to 48); each soloist alone for its two counts; back to the V (58 to 62), held still
 * across the seam on the struck pose, so the loop's seam is inside one shot.
 */
function edit(f: keyof typeof GROUP, line: readonly string[]): Format["cuts"] {
  const G = GROUP[f], at = (c: number) => beat(c);
  const cuts: { at: number; shot: Shot }[] = [
    { at: at(0), shot: { kind: "move", name: "open", from: G.V, to: G.line, start: at(4), end: at(8) } },
    { at: at(16), shot: { kind: "move", name: "throw", from: G.lineLow, to: G.lineClose, start: at(16), end: at(26) } },
    { at: at(33), shot: { kind: "still", name: "face", lens: single(line[2]!, PLACES.line[2]!) } },
    { at: at(34), shot: { kind: "move", name: "chorus", from: G.line, to: G.diagonalIn, start: at(34), end: at(46) } },
    ...line.map((v, k) => ({ at: at(48 + 2 * k), shot: { kind: "still" as const, name: `solo${k}`, lens: single(v, PLACES.diagonal[k]!) } })),
    { at: at(58), shot: { kind: "move", name: "button", from: G.diagonal, to: G.V, start: at(58), end: at(62) } },
  ];
  return cuts;
}
export function formats(line: readonly string[]): { readonly wide: Format; readonly square: Format } {
  return { wide: { name: "wide", width: 1920, height: 1080, cuts: edit("wide", line) }, square: { name: "square", width: 1080, height: 1080, cuts: edit("square", line) } };
}
export type FormatName = "wide" | "square";

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

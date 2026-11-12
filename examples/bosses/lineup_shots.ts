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
 * The formations: every member's place (world x toward the camera, z the camera's left), left to right, seen from above
 * the heads (docs/lineup.md, "Framing from above"): depth in a formation is height on screen, so each has real depth.
 * The V is three rows 8 deep: the centre in front, a pair behind it 12 to the right and 15 to the left (every great claw
 * is on the camera's left, so the left wing stands 3 wider to keep the samurai's low face out from behind the
 * centre's claw), the outer pair behind them. The line is a shallow arc, 13 apart (the least width that keeps five
 * slam discs and five boats in their own lanes), its ends 8 back. The diagonal runs away from the camera, the fiddler
 * in front, each next member 6 back and 9 to the right.
 */
const DIAGONAL: readonly XZ[] = [[0, 18], [-6, 9], [-12, 0], [-18, -9], [-24, -18]];
/** The diagonal with member k stepped 8 forward, out of the group, for its solo (in the group a soloist's own low single
 * had its front-left neighbour's eyestalks, legs and gauntlet across its face). */
const soloing = (k: number): readonly XZ[] => DIAGONAL.map(([x, z], j) => [j === k ? x + 8 : x, z] as const);
export const PLACES: Readonly<Record<string, readonly XZ[]>> = {
  V: [[-16, 30], [-8, 15], [0, 0], [-8, -12], [-16, -24]],
  line: [[-8, 26], [-2, 13], [0, 0], [-2, -13], [-8, -26]],
  diagonal: DIAGONAL,
  solo0: soloing(0), solo1: soloing(1), solo2: soloing(2), solo3: soloing(3), solo4: soloing(4),
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
 * The group's framings (docs/lineup.md, "Framing from above"), by format: raised, wide, close to the front member and
 * pitched down, so that depth in the formation becomes height in the frame, the front member is large, the others
 * step back up the frame, rear faces show above the front members' heads, and the horizon and the sunset are a band at
 * the top. `raised(front, z, height, distance, pitch, fovY)`: the eye `distance` in front of the front member (world x)
 * at `z`, `height` above the water, pitched down `pitch` degrees.
 */
const raised = (front: number, z: number, height: number, distance: number, pitch: number, fovY: number): Lens => {
  const p = (pitch * Math.PI) / 180, x = front + distance;
  return lens([x, height, z], [x - 10 * Math.cos(p), height - 10 * Math.sin(p), z], fovY);
};
const GROUP = {
  wide: {
    V: raised(0, 3, 20, 34, 15, 0.8), line: raised(0, 0, 24, 40, 17, 0.8), lineLow: raised(0, 0, 24, 40, 17, 0.8),
    lineClose: raised(0, 0, 22, 36, 18, 0.8), diagonal: raised(0, 9, 20, 30, 22, 0.8), diagonalIn: raised(0, 9, 20, 28, 22, 0.8),
  },
  square: {
    V: raised(0, 3, 36, 40, 20, 1.0), line: raised(0, 0, 36, 60, 15, 1.0), lineLow: raised(0, 0, 36, 60, 15, 1.0),
    lineClose: raised(0, 0, 34, 54, 16, 1.0), diagonal: raised(0, 9, 24, 40, 12, 0.9), diagonalIn: raised(0, 9, 24, 38, 13, 0.9),
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
    { at: at(33), shot: { kind: "still", name: "face", lens: single(line[2]!, PLACES.line![2]!) } },
    { at: at(34), shot: { kind: "move", name: "chorus", from: G.line, to: G.diagonalIn, start: at(34), end: at(46) } },
    ...line.map((v, k) => ({ at: at(48 + 2 * k), shot: { kind: "still" as const, name: `solo${k}`, lens: single(v, PLACES[`solo${k}`]![k]!) } })),
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

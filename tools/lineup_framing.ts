// The lineup's framing on paper (docs/lineup.md, "A drilled group", "Framing from above"): each crab as boxes in its own
// axes (everything from feet to the tops of its buildings, its body from shell top to waterline, its face with the eyes
// on their stalks, its great claw at rest or raised, its buildings), placed where the score has it, projected through
// the shot in force on the CPU. A second to run: `bun tools/lineup_framing.ts`. At each unison key of the score
// (counts 2, 12, 26, 27, 38, 40, 43, 62) and at each group shot's first frame: the group's box (leftmost to rightmost
// leg, feet to the tops of the buildings) as a share of the frame's width and height, the front member's body as a
// share of the height, each face's window (the share of its box no nearer member's body, claw or buildings covers),
// and where the horizon is (a share of the height from the top; outside 0..1 it is out of frame).
import { compileJs, variable } from "../src/expr.ts";
import { DANCE, LINE } from "../examples/bosses/crab_lineup.ts";
import { BEAT, formats, shotAt, type Lens } from "../examples/bosses/lineup_shots.ts";
type V3 = readonly [number, number, number];
type Box = { x: readonly [number, number]; y: readonly [number, number]; z: readonly [number, number] };
type Crab = { all: Box; body: Box; face: Box; claw: Box; up: Box; built: Box };
// Crab axes (x forward, y up, z its right). Feet drawn in to 0.85 of the spread (the routine's stance). The claw at rest:
// the fiddler's banner beside the face, the samurai's gauntlet low in front, the tongs in front of the stilts; raised:
// the wrist about 0.76 of the arm span over the shoulder and the hand above it.
const BANNER: Box = { x: [1, 8], y: [0.5, 9.5], z: [2.5, 8.5] }, BANNER_UP: Box = { x: [2, 7], y: [4, 13], z: [0.5, 5] };
export const CRABS: Record<string, Crab> = {
  islander: { all: { x: [-3, 7], y: [-0.3, 11], z: [-8.5, 9] }, body: { x: [-1.6, 5], y: [-0.3, 4.5], z: [-4.1, 4.1] }, face: { x: [4, 6], y: [2, 7], z: [-2.5, 2.5] }, claw: BANNER, up: BANNER_UP, built: { x: [-3, 1.5], y: [1.5, 9.5], z: [-2.5, 2.5] } },
  fiddler: { all: { x: [-3, 7], y: [-0.3, 12.5], z: [-8.5, 9] }, body: { x: [-1.6, 5], y: [-0.3, 3.4], z: [-4.1, 4.1] }, face: { x: [4, 6], y: [2, 7.5], z: [-2.5, 2.5] }, claw: BANNER, up: BANNER_UP, built: { x: [-1.5, 1.5], y: [3, 12.5], z: [-1.5, 1.5] } },
  bruiser: { all: { x: [-3, 7], y: [-0.3, 10], z: [-8.5, 10] }, body: { x: [-1.6, 5], y: [-0.3, 4.5], z: [-4.1, 4.1] }, face: { x: [4, 6.5], y: [1.5, 5], z: [-2.5, 2.5] }, claw: { x: [0, 7], y: [0.5, 9.5], z: [3.6, 9.6] }, up: BANNER_UP, built: { x: [-3, 2], y: [3, 7.5], z: [-3, 3] } },
  mask: { all: { x: [-3, 10], y: [-0.3, 10.5], z: [-8.5, 8.5] }, body: { x: [-1.6, 5], y: [-0.3, 4.4], z: [-4.1, 4.1] }, face: { x: [4.5, 5.5], y: [0.6, 3.6], z: [-3.5, 3.5] }, claw: { x: [4, 10], y: [-0.5, 2.5], z: [1.5, 5] }, up: { x: [3, 7], y: [3, 9], z: [1, 5] }, built: { x: [-2, 2], y: [3, 10.5], z: [-2, 2] } },
  stilt: { all: { x: [-8, 6], y: [-0.3, 15.5], z: [-9.5, 9.5] }, body: { x: [-2.5, 3], y: [3.5, 9], z: [-3.5, 3.5] }, face: { x: [2, 3.5], y: [5.6, 8.2], z: [-2.2, 2.2] }, claw: { x: [2, 8], y: [0, 6], z: [-4, 4] }, up: { x: [2, 7], y: [8, 16], z: [0.5, 5] }, built: { x: [-1.5, 1.5], y: [9, 15.5], z: [-1.5, 1.5] } },
};
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: V3): V3 => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };

export type Pose = { readonly at: readonly [number, number]; readonly heading: number };
type R = { x0: number; x1: number; y0: number; y1: number; d: number };
export type Framing = { readonly groupW: number; readonly groupH: number; readonly groupX: readonly [number, number]; readonly groupY: readonly [number, number]; readonly front: number; readonly frontBody: number; readonly windows: readonly number[]; readonly faces: readonly number[]; readonly horizon: number; readonly worstWindow: number };
/** The group through a lens: `line` the variants, `poses` their places, `raised` whether the great claws are up. */
export function frameGroup(line: readonly string[], poses: readonly Pose[], l: Lens, W: number, H: number, raised: boolean): Framing {
  const fw = unit(sub(l.target, l.eye)), r = unit(cross(fw, [0, 1, 0])), u = cross(r, fw), kk = H / 2 / Math.tan(l.fovY / 2);
  const px = (p: V3) => { const d = sub(p, l.eye), z = dot(d, fw); return [W / 2 + (kk * dot(d, r)) / z, H / 2 - (kk * dot(d, u)) / z, z] as const; };
  const rect = (k: number, b: Box): R => {
    const p = poses[k]!, c = Math.cos(p.heading), n = Math.sin(p.heading), ps: (readonly [number, number, number])[] = [];
    for (const x of b.x) for (const y of b.y) for (const z of b.z) ps.push(px([p.at[0] + x * c - z * n, y, p.at[1] + x * n + z * c]));
    return { x0: Math.min(...ps.map((q) => q[0])), x1: Math.max(...ps.map((q) => q[0])), y0: Math.min(...ps.map((q) => q[1])), y1: Math.max(...ps.map((q) => q[1])), d: Math.min(...ps.map((q) => q[2])) };
  };
  const per = line.map((v, k) => { const C = CRABS[v.split("+")[0]!]!; return { all: rect(k, C.all), body: rect(k, C.body), face: rect(k, C.face), claw: rect(k, raised ? C.up : C.claw), built: rect(k, C.built) }; });
  const windows = per.map((m, k) => {
    const f = m.face, occ = per.flatMap((p, j) => (j === k ? [] : [p.body, p.claw, p.built].filter((o) => o.d < f.d)));
    let hid = 0, inside = 0; const N = 12;
    for (let i = 0; i < N; i++) for (let j = 0; j < N; j++) {
      const x = f.x0 + ((i + 0.5) / N) * (f.x1 - f.x0), y = f.y0 + ((j + 0.5) / N) * (f.y1 - f.y0);
      if (x < 0 || x > W || y < 0 || y > H) continue;
      inside++; if (occ.some((o) => x > o.x0 && x < o.x1 && y > o.y0 && y < o.y1)) hid++;
    }
    return inside < (N * N) / 2 ? 0 : 1 - hid / (N * N);
  });
  const gx0 = Math.min(...per.map((p) => p.all.x0)), gx1 = Math.max(...per.map((p) => p.all.x1)), gy0 = Math.min(...per.map((p) => p.all.y0)), gy1 = Math.max(...per.map((p) => p.all.y1));
  const front = per.reduce((b, p, k) => (p.body.d < per[b]!.body.d ? k : b), 0);
  // The horizon: a far point straight ahead, level.
  const far = px([l.eye[0] + fw[0] * 1e5, l.eye[1], l.eye[2] + fw[2] * 1e5]);
  return {
    groupW: (Math.min(gx1, W) - Math.max(gx0, 0)) / W, groupH: (Math.min(gy1, H) - Math.max(gy0, 0)) / H, groupX: [gx0 / W, gx1 / W], groupY: [gy0 / H, gy1 / H],
    front, frontBody: (per[front]!.body.y1 - per[front]!.body.y0) / H, windows, faces: per.map((p) => p.face.y1 - p.face.y0), horizon: far[1] / H,
    worstWindow: Math.min(...windows),
  };
}

// Each member's place and facing at a loop time, from the score itself.
const tv = variable("framing_t", "f32");
const placeOf = Array.from({ length: DANCE.members }, (_, k) => { const d = DANCE.member(k); return compileJs([d.place(tv), d.face(tv)]); });
export const posesAt = (t: number): Pose[] => placeOf.map((f, k) => { const [p, h] = f({ vars: { framing_t: t }, inputs: {}, params: {} }) as [number[], number]; const home = DANCE.member(k).home; return { at: [home[0] + p[0]!, home[1] + p[1]!] as const, heading: h }; });
/** Counts at which the great claws are up (the score's `UP` keys and the held button). */
const RAISED = [0, 27, 28, 38, 44, 45, 46, 62, 63];
/** The unison keys the targets are judged at. */
export const KEYS = [2, 12, 26, 27, 38, 40, 43, 62];

if (import.meta.main) {
  const pc = (x: number) => (100 * x).toFixed(0);
  console.log(`left to right: ${LINE.join(", ")}`);
  for (const f of Object.values(formats(LINE))) {
    console.log(`${f.name} ${f.width}x${f.height}  (group box w x h; front member's body; face windows; horizon from the top)`);
    const times = [...new Set([...f.cuts.map((c) => c.at + 0.05 * BEAT), ...KEYS.map((c) => (c + 0.1) * BEAT)])].sort((a, b) => a - b);
    for (const t of times) {
      const s = shotAt(f, t), cam = s.camera;
      if (cam.kind !== "perspective") throw new Error("lineup_framing: perspective cameras only");
      const c = t / BEAT, key = KEYS.some((k) => Math.abs(k + 0.1 - c) < 1e-6);
      if (s.shot.name.startsWith("solo") || s.shot.name === "face") { console.log(`  count ${c.toFixed(1).padStart(4)} ${s.shot.name.padEnd(7)} (a single)`); continue; }
      const r = frameGroup(LINE, posesAt(t), { eye: cam.eye as unknown as V3, target: cam.target as unknown as V3, fovY: cam.fovY }, f.width, f.height, RAISED.includes(Math.floor(c)));
      console.log(`  count ${c.toFixed(1).padStart(4)}${key ? "*" : " "}${s.shot.name.padEnd(7)} group ${pc(r.groupW)} x ${pc(r.groupH)}%  front ${LINE[r.front]} body ${pc(r.frontBody)}%  windows ${r.windows.map(pc).join("/")}%  faces ${r.faces.map((x) => x.toFixed(0)).join("/")} px  horizon ${pc(r.horizon)}%`);
    }
  }
}

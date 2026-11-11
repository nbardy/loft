// The lineup's framing on paper (docs/lineup.md, "Second staging"): each crab as boxes in its own axes (its largest
// moment, its body from shell top to waterline without claws or buildings, its face, its feet), projected through each
// shot's camera on the CPU, with the props' places. A second to run: `bun tools/lineup_framing.ts`. The boxes are read off
// the strips (16 px a unit at 120 away, 960 x 540); the first staging's stand-in read about 10% large on heights.
import { BEAT, FORMATS, PERIOD, shotAt, standing } from "../examples/bosses/lineup_shots.ts";
type V3 = readonly [number, number, number];
type Box = { x: readonly [number, number]; y: readonly [number, number]; z: readonly [number, number] };
// Crab axes. `body`: shell top to waterline, no claws, no buildings (measured on the strips: 16 px a unit at 120 away).
const C: Record<string, { all: Box; body: Box; face: Box; feet: Box }> = {
  islander: { all: { x: [-3, 7], y: [-0.3, 11.5], z: [-10, 15.5] }, body: { x: [-1.6, 5], y: [-0.3, 4.5], z: [-4.1, 4.1] }, face: { x: [4, 6], y: [1.8, 5.2], z: [-2.5, 2.5] }, feet: { x: [-2.6, 6.8], y: [-0.4, -0.4], z: [-9.9, 9.9] } },
  fiddler: { all: { x: [-3, 7], y: [-0.3, 12.5], z: [-10, 15] }, body: { x: [-1.6, 5], y: [-0.3, 3.4], z: [-4.1, 4.1] }, face: { x: [4, 6], y: [1.8, 5.5], z: [-2.5, 2.5] }, feet: { x: [-2.6, 6.8], y: [-0.4, -0.4], z: [-9.9, 9.9] } },
  mask: { all: { x: [-3, 7], y: [-0.3, 10.5], z: [-9.6, 10] }, body: { x: [-1.6, 5], y: [-0.3, 4.4], z: [-4.1, 4.1] }, face: { x: [4.5, 5.5], y: [0.6, 3.6], z: [-3.5, 3.5] }, feet: { x: [-2.6, 6.8], y: [-0.4, -0.4], z: [-9.9, 9.9] } },
  stilt: { all: { x: [-9.5, 6], y: [-0.3, 15.5], z: [-11.3, 11.8] }, body: { x: [-2.5, 3], y: [-0.3, 9], z: [-3.5, 3.5] }, face: { x: [2, 3.5], y: [5.6, 8.2], z: [-2.2, 2.2] }, feet: { x: [-9.3, 5.8], y: [-0.4, -0.4], z: [-11, 11] } },
};
const L = ["islander", "fiddler", "mask", "stilt"];
const PROPS: Record<string, V3> = { disc: [10, 0, 1.5], boat1: [12.6, 1.5, -4.6], land: [38, 1.5, -4], apex: [24, 8, -5] };
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: V3): V3 => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };
function evaluate(eye: V3, target: V3, fovY: number, W: number, H: number) {
  const world = (k: number, q: V3): V3 => { const p = standing(k), c = Math.cos(p.heading), n = Math.sin(p.heading); return [p.at[0] + q[0] * c - q[2] * n, q[1], p.at[1] + q[0] * n + q[2] * c]; };
  const fw = unit(sub(target, eye)), r = unit(cross(fw, [0, 1, 0])), u = cross(r, fw), kk = H / 2 / Math.tan(fovY / 2);
  const px = (p: V3) => { const d = sub(p, eye), z = dot(d, fw); return [W / 2 + (kk * dot(d, r)) / z, H / 2 - (kk * dot(d, u)) / z, z] as const; };
  const rect = (k: number, b: Box) => { const ps: (readonly [number, number, number])[] = []; for (const x of b.x) for (const y of b.y) for (const z of b.z) ps.push(px(world(k, [x, y, z]))); return { x0: Math.min(...ps.map((p) => p[0])) / W, x1: Math.max(...ps.map((p) => p[0])) / W, y0: Math.min(...ps.map((p) => p[1])) / H, y1: Math.max(...ps.map((p) => p[1])) / H }; };
  const horizon = px([eye[0] + fw[0] * 1e5, eye[1], eye[2] + fw[2] * 1e5])[1] / H;
  const per = L.map((n, k) => ({ all: rect(k, C[n]!.all), body: rect(k, C[n]!.body), face: rect(k, C[n]!.face), feet: rect(k, C[n]!.feet) }));
  const props = Object.fromEntries(Object.entries(PROPS).map(([p, q]) => [p, L.map((_, k) => { const [x, y, z] = px(world(k, q)); return z < 1 ? "behind" : `(${(100 * x / W).toFixed(0)},${(100 * y / H).toFixed(0)})`; })]));
  return { per, horizon, props };
}
const pc = (x: number) => (100 * x).toFixed(0);
// The strips read bodies about 14% larger than these boxes (the second staging: the fiddler 40% where 35 was predicted),
// so a body counts as large (at least 35% in the picture) from 31% here.
const LARGE = 0.31;
for (const f of Object.values(FORMATS)) {
  const large = [0, 0, 0, 0];
  console.log(`${f.name} ${f.width}x${f.height}`);
  f.cuts.forEach((c, i) => {
    const end = i + 1 < f.cuts.length ? f.cuts[i + 1]!.at : PERIOD, beats = (end - c.at) / BEAT;
    const cam = shotAt(f, c.at).camera;
    if (cam.kind !== "perspective") throw new Error("lineup_framing: perspective cameras only");
    const r = evaluate(cam.eye, cam.target, cam.fovY, f.width, f.height);
    const bodies = r.per.map((p) => p.body.y1 - p.body.y0), inFrame = r.per.map((p) => p.face.x1 > 0 && p.face.x0 < 1);
    // A neighbour across a single's face: its largest box (claws up) overlapping the subject's face.
    const crossing = (k: number) => r.per.some((p, j) => j !== k && p.all.x1 > r.per[k]!.face.x0 && p.all.x0 < r.per[k]!.face.x1 && Math.abs(j - k) === 1 && p.all.y0 < r.per[k]!.face.y1);
    bodies.forEach((b, k) => { if (b >= LARGE && inFrame[k]) large[k]! += beats; });
    const sub = c.shot.subject, who = sub.kind === "wide" ? "all four" : sub.kind === "pair" ? `${L[sub.k]} + ${L[sub.k + 1]}` : L[sub.k];
    const detail = sub.kind === "single" ? `body ${pc(bodies[sub.k]!)}%, face ${((r.per[sub.k]!.face.y1 - r.per[sub.k]!.face.y0) * f.height).toFixed(0)} px, feet to ${pc(r.per[sub.k]!.feet.y1)}%, a neighbour's box near the face: ${crossing(sub.k) ? "yes" : "no"}`
      : `bodies ${bodies.map((b, k) => (inFrame[k] ? pc(b) : "-")).join("/")}%`;
    console.log(`  beat ${String(c.at / BEAT).padStart(2)} +${beats}  ${c.shot.name.padEnd(9)} ${who!.padEnd(18)} ${detail}  horizon ${pc(r.horizon)}%`);
  });
  console.log(`  large (body >= 35% in the picture, ${LARGE * 100}% here), beats: ${L.map((n, k) => `${n} ${large[k]}`).join(", ")}`);
}

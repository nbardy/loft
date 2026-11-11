// The lineup's framing on paper (docs/lineup.md, "Staging"): each crab as boxes in its own axes, projected through each
// shot's camera on the CPU. No GPU, no scene: a second to run. `bun tools/lineup_framing.ts`
//
// The boxes are calibrated on the first cut's frame at 22.0 s (960 x 540, camera [106, 16, 0]): the stand-in gave the
// spider crab 139 px where the picture shows about 125, and the line 85% where it shows 80%. So the stand-in reads about
// 10% large; its numbers times 0.9 are the prediction.
import { FORMATS, shotAt, standing, type Format } from "../examples/bosses/lineup_shots.ts";

type V3 = readonly [number, number, number];
type Box = { readonly x: readonly [number, number]; readonly y: readonly [number, number]; readonly z: readonly [number, number] };
/** Crab axes: x forward, y up, z the crab's right (the camera's left). `all` at its largest moment (claw up), `core` the body and back (what hides), `face` what a viewer reads. */
const CRAB: Record<string, { readonly all: Box; readonly core: Box; readonly face: Box }> = {
  islander: { all: { x: [-3, 7], y: [-0.3, 11.5], z: [-10, 15.5] }, core: { x: [-1.6, 5], y: [0, 8.5], z: [-4.5, 4.5] }, face: { x: [4, 6], y: [1.8, 5.2], z: [-2.5, 2.5] } },
  fiddler: { all: { x: [-3, 7], y: [-0.3, 12.5], z: [-10, 15] }, core: { x: [-1.6, 5], y: [0, 12], z: [-4.5, 4.5] }, face: { x: [4, 6], y: [1.8, 5.5], z: [-2.5, 2.5] } },
  mask: { all: { x: [-3, 7], y: [-0.3, 10.5], z: [-9.6, 10] }, core: { x: [-1.6, 5], y: [0, 9], z: [-4.5, 4.5] }, face: { x: [4.5, 5.5], y: [0.6, 3.6], z: [-3.5, 3.5] } },
  stilt: { all: { x: [-9.5, 6], y: [-0.3, 15.5], z: [-11.3, 11.8] }, core: { x: [-2.5, 3], y: [5, 14], z: [-3.5, 3.5] }, face: { x: [2, 3.5], y: [5.6, 8.2], z: [-2.2, 2.2] } },
};
CRAB.bruiser = CRAB.islander!;
/** Props in crab axes: the slam's disc, the boat the claw takes, where the thrown one lands, where the shoved one ends its ride. */
const PROPS: Record<string, V3> = { disc: [10, 0, 1.5], boat1: [12.6, 1.5, -4.6], land: [26, 1.5, -8], rideEnd: [27, 2, 14] };

const LINE = (process.argv[2] ?? "islander,fiddler,mask,stilt").split(",");
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: V3): V3 => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };
const world = (k: number, q: V3): V3 => { const s = standing(k), c = Math.cos(s.heading), n = Math.sin(s.heading); return [s.at[0] + q[0] * c - q[2] * n, q[1], s.at[1] + q[0] * n + q[2] * c]; };
type Rect = { x0: number; x1: number; y0: number; y1: number; depth: number };

function frame(f: Format, t: number) {
  const { camera } = shotAt(f, t), eye = camera.eye as V3, fw = unit(sub(camera.target as V3, eye)), r = unit(cross(fw, [0, 1, 0])), u = cross(r, fw);
  const k = f.height / 2 / Math.tan((camera.kind === "perspective" ? camera.fovY : 0.5) / 2);
  const px = (p: V3): [number, number, number] => { const d = sub(p, eye), z = dot(d, fw); return [f.width / 2 + (k * dot(d, r)) / z, f.height / 2 - (k * dot(d, u)) / z, z]; };
  const rect = (i: number, b: Box): Rect => {
    const ps: [number, number, number][] = [];
    for (const x of b.x) for (const y of b.y) for (const z of b.z) ps.push(px(world(i, [x, y, z])));
    return { x0: Math.min(...ps.map((p) => p[0])), x1: Math.max(...ps.map((p) => p[0])), y0: Math.min(...ps.map((p) => p[1])), y1: Math.max(...ps.map((p) => p[1])), depth: ps.reduce((s, p) => s + p[2], 0) / ps.length };
  };
  const all = LINE.map((n, i) => rect(i, CRAB[n]!.all)), core = LINE.map((n, i) => rect(i, CRAB[n]!.core)), face = LINE.map((n, i) => rect(i, CRAB[n]!.face));
  const over = (a: Rect, b: Rect) => Math.max(0, Math.min(a.x1, b.x1) - Math.max(a.x0, b.x0)) * Math.max(0, Math.min(a.y1, b.y1) - Math.max(a.y0, b.y0)) / ((a.x1 - a.x0) * (a.y1 - a.y0));
  const hidden = face.map((fc, i) => Math.max(0, ...core.map((c, j) => (j !== i && c.depth < fc.depth ? over(fc, c) : 0))));
  const horizon = px([eye[0] + fw[0] * 1e5, eye[1], eye[2] + fw[2] * 1e5])[1] / f.height;
  const props = Object.fromEntries(Object.entries(PROPS).map(([p, q]) => [p, LINE.map((_, i) => px(world(i, q))).map(([x, y]) => [x / f.width, y / f.height] as const)]));
  return { all, face, hidden, horizon, props, W: f.width, H: f.height };
}

const pc = (x: number) => `${(100 * x).toFixed(0)}%`;
for (const f of Object.values(FORMATS)) {
  console.log(`${f.name} ${f.width}x${f.height}   (stand-in numbers: times about 0.9 for the picture)`);
  for (const c of f.cuts) {
    // A move ending on the period is read just before it (the period wraps to the first shot).
    const ts = c.shot.kind === "move" ? [c.shot.start, c.shot.end - 1e-3] : [c.at];
    for (const t of ts) {
      const m = frame(f, t), x0 = Math.min(...m.all.map((r) => r.x0)), x1 = Math.max(...m.all.map((r) => r.x1));
      const heights = m.all.map((r) => (r.y1 - r.y0) / m.H);
      console.log(`  ${c.shot.name.padEnd(5)} t ${t.toFixed(2).padStart(5)}  line ${pc((x1 - x0) / m.W)} [${pc(x0 / m.W)}..${pc(x1 / m.W)}]  heights ${heights.map(pc).join(" ")} (${m.all.map((r) => (r.y1 - r.y0).toFixed(0)).join(" ")} px)  faces ${m.face.map((r) => (r.y1 - r.y0).toFixed(0)).join(" ")} px, hidden ${m.hidden.map(pc).join(" ")}  horizon ${pc(m.horizon)}  crabs ${pc(Math.min(...m.all.map((r) => r.y0)) / m.H)}..${pc(Math.max(...m.all.map((r) => r.y1)) / m.H)}`);
      if (process.env.PROPS) for (const [p, ps] of Object.entries(m.props)) console.log(`        ${p.padEnd(8)}${ps.map(([x, y]) => `(${pc(x)}, ${pc(y)})`).join(" ")}`);
    }
  }
}

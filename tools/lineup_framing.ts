// The lineup's framing on paper (docs/lineup.md, "A drilled group"): each crab as boxes in its own axes (its body from
// shell top to waterline without claws or buildings, its face, its great claw at rest), placed where the score has it
// at each shot's first frame (and, for a moving shot, its last), projected through that shot's camera on the CPU. A
// second to run: `bun tools/lineup_framing.ts`. For every group shot: each body's share of the height, each face's
// window (the share of its box no nearer member's body or great claw covers), and its height in pixels.
import { compileJs, variable } from "../src/expr.ts";
import { DANCE, LINE } from "../examples/bosses/crab_lineup.ts";
import { AUDIENCE, BEAT, formats, PERIOD, shotAt, type Lens } from "../examples/bosses/lineup_shots.ts";
type V3 = readonly [number, number, number];
type Box = { x: readonly [number, number]; y: readonly [number, number]; z: readonly [number, number] };
// Crab axes (x forward, y up, z its right). Bodies measured on the strips (docs/lineup.md, second staging); faces include
// the eyes on their stalks; the claw is the great claw at rest (the fiddler's banner beside the face, the samurai's
// gauntlet low in front, the spider crab's tongs hanging in front of its stilts).
const BANNER: Box = { x: [1, 8], y: [0.5, 9.5], z: [2.5, 8.5] };
const C: Record<string, { body: Box; face: Box; claw: Box }> = {
  islander: { body: { x: [-1.6, 5], y: [-0.3, 4.5], z: [-4.1, 4.1] }, face: { x: [4, 6], y: [2, 7], z: [-2.5, 2.5] }, claw: BANNER },
  fiddler: { body: { x: [-1.6, 5], y: [-0.3, 3.4], z: [-4.1, 4.1] }, face: { x: [4, 6], y: [2, 7.5], z: [-2.5, 2.5] }, claw: BANNER },
  bruiser: { body: { x: [-1.6, 5], y: [-0.3, 4.5], z: [-4.1, 4.1] }, face: { x: [4, 6.5], y: [1.5, 5], z: [-2.5, 2.5] }, claw: { x: [0, 7], y: [0.5, 9.5], z: [3.6, 9.6] } },
  mask: { body: { x: [-1.6, 5], y: [-0.3, 4.4], z: [-4.1, 4.1] }, face: { x: [4.5, 5.5], y: [0.6, 3.6], z: [-3.5, 3.5] }, claw: { x: [4, 10], y: [-0.5, 2.5], z: [1.5, 5] } },
  stilt: { body: { x: [-2.5, 3], y: [3.5, 9], z: [-3.5, 3.5] }, face: { x: [2, 3.5], y: [5.6, 8.2], z: [-2.2, 2.2] }, claw: { x: [2, 8], y: [0, 6], z: [-4, 4] } },
};
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: V3): V3 => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };

// Each member's place and facing at a loop time, from the score itself.
const tv = variable("framing_t", "f32");
const placeOf = DANCE.members > 0 ? Array.from({ length: DANCE.members }, (_, k) => { const d = DANCE.member(k); return compileJs([d.place(tv), d.face(tv)]); }) : [];
const pose = (k: number, t: number) => { const [p, h] = placeOf[k]!({ vars: { framing_t: t }, inputs: {}, params: {} }) as [number[], number]; const home = DANCE.member(k).home; return { at: [home[0] + p[0]!, home[1] + p[1]!] as const, heading: h }; };

type R = { x0: number; x1: number; y0: number; y1: number; d: number };
function evaluate(l: Lens, t: number, W: number, H: number) {
  const fw = unit(sub(l.target, l.eye)), r = unit(cross(fw, [0, 1, 0])), u = cross(r, fw), kk = H / 2 / Math.tan(l.fovY / 2);
  const px = (p: V3) => { const d = sub(p, l.eye), z = dot(d, fw); return [W / 2 + (kk * dot(d, r)) / z, H / 2 - (kk * dot(d, u)) / z, z] as const; };
  const rect = (k: number, b: Box): R => {
    const p = pose(k, t), c = Math.cos(p.heading), n = Math.sin(p.heading), ps: (readonly [number, number, number])[] = [];
    for (const x of b.x) for (const y of b.y) for (const z of b.z) ps.push(px([p.at[0] + x * c - z * n, y, p.at[1] + x * n + z * c]));
    return { x0: Math.min(...ps.map((q) => q[0])), x1: Math.max(...ps.map((q) => q[0])), y0: Math.min(...ps.map((q) => q[1])), y1: Math.max(...ps.map((q) => q[1])), d: Math.min(...ps.map((q) => q[2])) };
  };
  const per = LINE.map((n, k) => ({ body: rect(k, C[n]!.body), face: rect(k, C[n]!.face), claw: rect(k, C[n]!.claw) }));
  return LINE.map((n, k) => {
    const f = per[k]!.face, occ = per.flatMap((p, j) => (j === k ? [] : [p.body, p.claw].filter((o) => o.d < f.d)));
    let hid = 0, inside = 0; const N = 12;
    for (let i = 0; i < N; i++) for (let j = 0; j < N; j++) {
      const x = f.x0 + ((i + 0.5) / N) * (f.x1 - f.x0), y = f.y0 + ((j + 0.5) / N) * (f.y1 - f.y0);
      if (x < 0 || x > W || y < 0 || y > H) continue;
      inside++; if (occ.some((o) => x > o.x0 && x < o.x1 && y > o.y0 && y < o.y1)) hid++;
    }
    const b = per[k]!.body;
    return { body: (b.y1 - b.y0) / H, window: inside === 0 ? 0 : 1 - hid / (N * N), face: f.y1 - f.y0, inFrame: b.x1 > 0 && b.x0 < W };
  });
}
const pc = (x: number) => (100 * x).toFixed(0);
console.log(`left to right: ${LINE.join(", ")}; audience ${AUDIENCE.join(", ")}`);
for (const f of Object.values(formats(LINE))) {
  console.log(`${f.name} ${f.width}x${f.height}`);
  f.cuts.forEach((c, i) => {
    const end = i + 1 < f.cuts.length ? f.cuts[i + 1]!.at : PERIOD;
    const ends = c.shot.kind === "move" ? [c.at + 0.05, Math.min(c.shot.end, end - 0.05)] : [c.at + 0.05];
    for (const t of ends) {
      const cam = shotAt(f, t).camera;
      if (cam.kind !== "perspective") throw new Error("lineup_framing: perspective cameras only");
      const r = evaluate({ eye: cam.eye as unknown as V3, target: cam.target as unknown as V3, fovY: cam.fovY }, t, f.width, f.height);
      const row = (g: (m: (typeof r)[number]) => string) => r.map((m) => (m.inFrame ? g(m) : "-")).join("/");
      console.log(`  count ${(t / BEAT).toFixed(1).padStart(4)} ${c.shot.name.padEnd(7)} bodies ${row((m) => pc(m.body))}%  windows ${row((m) => pc(m.window))}%  faces ${row((m) => m.face.toFixed(0))} px`);
    }
  });
}

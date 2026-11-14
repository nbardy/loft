// A pane of glass breaks at t = 0.4 and falls as 36 shards: triangles from the author's own algorithm (`draw.mesh`).
// The shards are Voronoi cells of the pane, clipped in plain TypeScript at build, each made a thin prism. They fall
// and spin by an expression of time, so any frame draws alone. The algorithm (topology) runs once; the motion (`at`)
// answers for every frame, and the normals are carried through it by its Jacobian.
import { add, cos, distance, draw, f, max, min, mix, mul, normalize, plane, rotateAbout, scene, sin, sqrt, sub, time, toon, vec2, vec3, type F, type P3 } from "../src/index.ts";

type Pt = [number, number];
const H = 0.015; // half the glass's thickness
const seeds: Pt[] = Array.from({ length: 36 }, (_, k) => [Math.sin(k * 12.99) * 0.95, 1 + Math.sin(k * 78.23) * 0.95]);

/** Seed i's cell: the pane's rectangle clipped to the half-plane nearer seed i than each other seed. */
function cell(i: number): Pt[] {
  let poly: Pt[] = [[-1, 0.05], [1, 0.05], [1, 2], [-1, 2]];
  const [ax, ay] = seeds[i]!;
  seeds.forEach(([bx, by], j) => {
    if (j === i) return;
    const nx = bx - ax, ny = by - ay, c = (nx * (ax + bx) + ny * (ay + by)) / 2;
    const side = (p: Pt) => nx * p[0] + ny * p[1] - c;
    poly = poly.flatMap((p, k): Pt[] => {
      const q = poly[(k + 1) % poly.length]!, sp = side(p), sq = side(q);
      const cut: Pt[] = sp * sq < 0 ? [[p[0] + (q[0] - p[0]) * sp / (sp - sq), p[1] + (q[1] - p[1]) * sp / (sp - sq)]] : [];
      return sp <= 0 ? [p, ...cut] : cut;
    });
  });
  return poly;
}

// One row per corner: rest position x y z, the shard's centre cx cy, its index k, and 1 on the rim (0 on the faces).
const points: number[][] = [];
const triangles: [number, number, number][] = [];
const corner = (row: number[]) => points.push(row) - 1;
const centres: Pt[] = [];
seeds.forEach((_, k) => {
  const poly = cell(k);
  const cx = poly.reduce((s, p) => s + p[0], 0) / poly.length, cy = poly.reduce((s, p) => s + p[1], 0) / poly.length;
  centres.push([cx, cy]);
  for (const z of [H, -H]) {
    const fan = poly.map((p) => corner([p[0], p[1], z, cx, cy, k, 0]));
    for (let j = 1; j + 1 < fan.length; j++) triangles.push(z > 0 ? [fan[0]!, fan[j]!, fan[j + 1]!] : [fan[0]!, fan[j + 1]!, fan[j]!]);
  }
  poly.forEach((p, j) => { // the rim: its own corners, so it shades flat against the faces
    const q = poly[(j + 1) % poly.length]!;
    const a = corner([p[0], p[1], H, cx, cy, k, 1]), b = corner([q[0], q[1], H, cx, cy, k, 1]);
    const c = corner([q[0], q[1], -H, cx, cy, k, 1]), d = corner([p[0], p[1], -H, cx, cy, k, 1]);
    triangles.push([a, d, c], [a, c, b]);
  });
});

/** Where a corner is at `time`: each shard comes loose later the further it is from the blow at (0, 1.2), then spins
 *  about its centre and falls until its centre is 5 cm above the ground (it stops there; shards do not stack). */
const at = (r: readonly F[]): P3 => {
  const [x, y, z, cx, cy, k] = r as [F, F, F, F, F, F];
  const loose = add(0.4, mul(0.15, distance(vec2(cx, cy), vec2(0, 1.2))));
  const s = min(max(sub(time, loose), 0), sqrt(mul(sub(cy, 0.05), 1 / 4.9))); // seconds falling, up to landing
  const centre = vec3(cx, cy, 0);
  const turned = rotateAbout(sub(vec3(x, y, z), centre), normalize(vec3(sin(k), 0.4, cos(k))), mul(s, 3));
  return add(add(centre, turned), vec3(mul(cx, mul(s, 0.6)), mul(mul(s, s), -4.9), mul(s, 1.2)));
};

const pane = draw.mesh("pane", { points, triangles }, {
  at,
  paint: (row) => mix(vec3(0.55, 0.75, 0.8), vec3(0.92, 0.97, 1), row[6]!), // the rim reads lighter
  sides: "one",
  outline: "hull",
  // One short capsule per shard, carried by the same `at`, so each shard casts its own shadow.
  skeletons: centres.map(([cx, cy], k) => ({ joints: 2, joint: (j: number) => ({ p: at([cx + (j - 0.5) * 0.1, cy, 0, cx, cy, k].map(f)), r: f(0.08) }) })),
});

export default scene({
  ground: plane, things: [pane], style: toon(), sun: [-0.4, 0.8, 0.45], period: 3,
  cameras: {
    whole: { kind: "perspective", eye: [3.5, 2.2, 4.5], target: [0, 0.9, 0], fovY: 0.7 },
    blow: { kind: "perspective", eye: [0.8, 1.4, 1.6], target: [0, 1.2, 0], fovY: 0.5 },
  },
});

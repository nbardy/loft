// The terraced back's theory, before the build (docs/art_direction/DIRECTION.md, Round 5): where its planned points fall
// on screen from the design's cameras at 1280 x 800, the body at rest (crab coordinates are the world's, raised by the
// ride). Plain arithmetic on the plan's numbers.
//
//   bun tools/terraces_theory.ts

import { cameraMatrices, project } from "../src/host/camera.ts";
import { S } from "../examples/bosses/titan/numbers.ts";

const W = 1280, H = 800, A = S.atoll, WATER = -S.ride;
const cams = {
  game: { kind: "perspective", eye: [23, 27, 21], target: [3, 0.5, 2.5], fovY: 0.6 },
  far: { kind: "perspective", eye: [33, 21, 21], target: [3, 1.5, -1], fovY: 0.6 },
  back: { kind: "perspective", eye: [-15, 19, 9], target: [-1.5, 2.5, 0], fovY: 0.6 },
} as const;
const deg = (d: number) => (d * Math.PI) / 180;
/** A point r out from the island's centre at a bearing (degrees from the bow toward the right flank), y above the water. */
const at = (r: number, bearing: number, above: number): readonly [number, number, number] =>
  [A.centre[0] + r * Math.cos(deg(bearing)), WATER + above, A.centre[1] + r * Math.sin(deg(bearing))];

// The plan (crab coordinates; see DIRECTION.md).
const POINT = 200, LIGHT_R = 7.2, MOUND = 0.8, TOWER = 3.0, LANTERN = 0.8, LEAN = 0.34;
const plan = {
  "light: water at its foot": at(LIGHT_R, POINT, 0),
  "light: mound top": at(LIGHT_R, POINT, MOUND),
  "light: finial": at(LIGHT_R + LEAN * (TOWER + LANTERN), POINT, MOUND + TOWER + LANTERN),
  "point: tip": at(8.4, POINT, 0.3),
  "point: root at the rim": at(4.6, POINT, 0.9),
  "beach: bow end": at(5.3, 152, 0.1), "beach: middle": at(5.6, 172, 0.1), "beach: point end": at(5.8, 190, 0.1),
  "stack 1 foot": at(6.1, 126, 0), "stack 1 top": at(6.1, 126, 2.3), "stack 2 top": at(7.0, 121, 1.5), "stack 3 top": at(7.7, 117, 0.9),
  "crown": at(0, 0, 4.6), "shoulder toward game": at(3.4, 43, 3.35), "rim toward game": at(4.3, 43, 1.16),
  "rim, right flank": at(4.6, 90, 1.16), "rim, tail": at(4.8, 180, 1.16),
};
for (const [name, c] of Object.entries(cams)) {
  const cam = cameraMatrices(c as never, W / H);
  console.log(name);
  for (const [what, p] of Object.entries(plan)) { const q = project(cam, p, W, H); console.log(`  ${what.padEnd(26)} (${q.x.toFixed(0)}, ${q.y.toFixed(0)})`); }
  const px = (p: readonly [number, number, number]) => { const q = project(cam, p, W, H); return [q.x, q.y] as const; };
  const d = (a: readonly [number, number, number], b: readonly [number, number, number]) => Math.hypot(px(a)[0] - px(b)[0], px(a)[1] - px(b)[1]);
  console.log(`  light, mound top to finial ${d(plan["light: mound top"], plan["light: finial"]).toFixed(0)} px; water to finial ${d(plan["light: water at its foot"], plan["light: finial"]).toFixed(0)} px; tower foot across ${d(at(LIGHT_R, POINT, MOUND), [at(LIGHT_R, POINT, MOUND)[0] + 1.06, at(LIGHT_R, POINT, MOUND)[1], at(LIGHT_R, POINT, MOUND)[2] - 1.06]).toFixed(0)} px`);
  console.log(`  beach end to end ${d(plan["beach: bow end"], plan["beach: point end"]).toFixed(0)} px; a major plate, rim to shoulder toward game ${d(plan["rim toward game"], plan["shoulder toward game"]).toFixed(0)} px; rim arc of 60 degrees toward game ${d(at(4.3, 13, 1.16), at(4.3, 73, 1.16)).toFixed(0)} px`);
}
// What a thrown ship passes (rig.ts: released about (8.5, 3.8, -7.6) with velocity (-3.0, 6.9, -8.6), gravity 5).
let least = Infinity;
for (let t = 0; t <= 3.2; t += 0.02) {
  const s = [8.5 - 3.0 * t, 3.8 + 6.9 * t - 2.5 * t * t, -7.6 - 8.6 * t] as const;
  for (const p of [at(8.4, POINT, 0.3), at(LIGHT_R, POINT, 2), at(LIGHT_R + 1.3, POINT, 4.6), at(4.9, 225, 1)]) least = Math.min(least, Math.hypot(s[0] - p[0], s[1] - p[1], s[2] - p[2]));
}
console.log(`thrown ship: nearest to the point, the light or the left-tail rim ${least.toFixed(1)}`);
// The last legs' feet (numbers.ts, legFeet) against the stacks and the point at rest.
const foot = (side: number) => [S.legFeet[3][0], S.legFeet[3][1] * side] as const;
const near = (p: readonly [number, number, number], f: readonly [number, number]) => Math.hypot(p[0] - f[0], p[2] - f[1]);
console.log(`last right foot to stack 1 ${near(at(6.1, 126, 0), foot(1)).toFixed(1)}, stack 3 ${near(at(7.7, 117, 0), foot(1)).toFixed(1)}; last left foot to the point's tip ${near(at(8.4, POINT, 0), foot(-1)).toFixed(1)}, to the light ${near(at(LIGHT_R, POINT, 0), foot(-1)).toFixed(1)}`);

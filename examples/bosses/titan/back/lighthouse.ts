// A lighthouse: the fiddler's back. One ruined lighthouse leaning back off the pebble, on a low dark rock, its lantern
// broken open; gulls wheel round its top. It rides the shell frame (the body read late, pitching on after the slam),
// and the tower rocks on its rock a beat later still, so from the slam the body thuds, the shell follows, the tower
// sways, and the gulls scatter out and drift back.

import {
  abs, add, alive, cos, floor, fract, max, mix, mul, normalize, place, rigid, sin, smoothstep, strand, sub, swing, through, time, vec3, type F, type P3,
} from "../../../../src/index.ts";
import { cut, TAU } from "../ink.ts";
import type { BackOption } from "../slots.ts";

/** The tower: its foot on the rock (crab coordinates), its height, its girth at the foot and the top, its lean. */
const TOWER = { foot: [-0.9, 1.55, -0.7] as const, height: 6.4, girth: [1.05, 0.72] as const, lean: [-0.26, 1, -0.2] as const };
const GULLS = 5;

export const lighthouse: BackOption = {
  about: "a leaning ruined lighthouse on a dark rock, gulls wheeling round it",
  draw: ({ n, shell, crushAfter }) => {
    const I = n.ink;
    const rock = place(strand({
      name: "rock", spine: (m) => mix(vec3(-2.6, 1.15, -0.5), vec3(0.9, 1.35, -0.3), m), stations: 5, girth: (m) => mul(1.9, add(0.75, mul(sin(mul(m, Math.PI)), 0.25))),
      section: { aspect: 0.42, squareness: 1.7 }, tips: [{ round: 0.5 }, { round: 0.5 }], up: [0, 1, 0],
      // Dark stone with pale strata.
      paint: (m, v) => mix(vec3(...I.bark), vec3(...I.stone), sub(1, cut(abs(sin(add(mul(v, 21), mul(m, 5)))), 0.12))),
      detail: { along: 20, around: 24 },
    }), rigid(shell));

    // The tower rocks on its rock after the slam (a beat after the shell), and never stands quite still.
    const rockBy = add(swing(sub(crushAfter, 0.15), 0.14, 0.75, 0.2), alive(time, 0.015, 51));
    const [lx, ly, lz] = TOWER.lean;
    const dir = normalize(vec3(add(lx, rockBy), ly, add(lz, mul(rockBy, 0.4))));
    const foot = vec3(...TOWER.foot), H = TOWER.height, [g0, g1] = TOWER.girth;
    const at = (m: number | F): P3 => add(foot, mul(dir, mul(H, m)));
    const tower = place(strand({
      name: "lighthouse", spine: (m) => at(m), stations: 6, girth: (m) => mix(g0, g1, m), tips: [{ round: 0.05 }, "open"], up: [1, 0, 0],
      paint: (m, v) => {
        // Salmon brick in courses, offset joints; a cream band below the gallery; three black windows up one side.
        const course = sub(1, cut(abs(sub(fract(mul(m, 16)), 0.5)), 0.06));
        const joint = mul(sub(1, cut(abs(sub(fract(add(mul(v, 10), mul(floor(mul(m, 16)), 0.5))), 0.5)), 0.06)), 0.7);
        const brick = mix(vec3(...I.coral), vec3(...I.accent), mul(max(course, joint), 0.75));
        const band = mul(cut(m, 0.74), sub(1, cut(m, 0.8)));
        const win = mul(sub(1, cut(abs(sub(v, 0.3)), 0.025)), cut(abs(sub(fract(mul(m, 4)), 0.5)), 0.38));
        return mix(mix(brick, vec3(...I.cream), band), vec3(...I.black), mul(win, sub(1, cut(m, 0.86))));
      },
      detail: { along: 40, around: 28 },
    }), rigid(shell));
    // The gallery round its top: a cream ring with black rails.
    const top = at(0.9);
    const gallery = place(strand({
      name: "gallery", spine: (m) => add(top, vec3(mul(cos(mul(m, TAU)), g1 + 0.32), 0, mul(sin(mul(m, TAU)), g1 + 0.32))), stations: 12, girth: 0.13,
      section: { aspect: 1.6 }, tips: ["round", "round"], up: [0, 1, 0],
      paint: (m) => mix(vec3(...I.cream), vec3(...I.black), cut(abs(sub(fract(mul(m, 18)), 0.5)), 0.38)),
      detail: { along: 48, around: 8 },
    }), rigid(shell));
    // The lantern, broken: a black cage stub, open, its cream glass mostly gone.
    const lantern = place(strand({
      name: "lantern", spine: (m) => add(at(1), mul(dir, mul(m, 1.0))), stations: 4, girth: (m) => mix(0.56, 0.5, m), tips: [{ round: 0.05 }, "open"], up: [1, 0, 0],
      paint: (m, v) => mix(vec3(...I.black), vec3(...I.cream), mul(cut(abs(sub(fract(mul(v, 6)), 0.5)), 0.32), mul(cut(m, 0.2), sub(1, cut(m, 0.75))))),
      detail: { along: 8, around: 24 },
    }), rigid(shell));

    // Gulls: cream Vs with black tips, wheeling round the top at their own heights and rates; they flap. At the slam they
    // scatter outward and drift back over four seconds.
    const scatter = mul(smoothstep(0, 0.5, crushAfter), sub(1, smoothstep(1.5, 4.5, crushAfter)));
    const gulls = Array.from({ length: GULLS }, (_, i) => {
      const turn = add(mul(time, i % 2 === 0 ? 0.42 : -0.33), i * 1.3);
      const rho = add(2.2 + 0.55 * i, mul(scatter, 2.5 + 0.4 * i));
      const centre = add(at(1), vec3(0, 0.9 + 0.45 * i, 0));
      const body = add(add(centre, vec3(mul(cos(turn), rho), mul(sin(add(mul(time, 0.7), i)), 0.35), mul(sin(turn), rho))), vec3(0, mul(scatter, 1.2), 0));
      const radial = vec3(cos(turn), 0, sin(turn));
      const flap = sin(add(mul(time, 6.5 + 0.7 * i), i * 2.1));
      const tipUp = vec3(0, add(0.18, mul(flap, 0.26)), 0);
      return place(strand({
        name: `gull${i}`, spine: (m) => through(m, [add(add(body, mul(radial, -0.55)), tipUp), add(body, mul(radial, -0.2)), body, add(body, mul(radial, 0.2)), add(add(body, mul(radial, 0.55)), tipUp)]),
        stations: 5, girth: (m) => mul(0.085, sub(1, mul(abs(sub(mul(m, 2), 1)), 0.55))), tips: [{ point: 0.2 }, { point: 0.2 }], up: [0, 1, 0],
        paint: (m) => mix(vec3(...I.cream), vec3(...I.black), cut(abs(sub(mul(m, 2), 1)), 0.72)),
        detail: { along: 12, around: 6 },
      }), rigid(shell));
    });
    return [rock, tower, gallery, lantern, ...gulls];
  },
};

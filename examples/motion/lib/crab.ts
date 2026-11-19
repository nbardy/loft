// A small walking crab for the motion comparison (agent_notes/2026-11-19_motion.md): islandback.ts's carapace, eight
// three-segment legs by IK, two claws and stalked eyes, without the shell, the slam or the params. A body frame and
// the gait's feet in, drawables out: the keyed and the solved walks differ only in where the body frame comes from.
// Faces +x, walks sideways toward +z on a left-hand arc (toward its front), y up, floor at y = 0.
import {
  abs, add, chain, course, cos, gait, keys, lift, max, min, mix, mul, palette, place, rotateAbout, sin, smoothstep, spiralTube, strand, sub, through,
  unit, vec3, type F, type Frame, type P3,
} from "../../../src/index.ts";
import { rotate } from "../../../src/map/chart.ts";
import { rigid } from "../../../src/geom/rig.ts";

export const S = {
  ride: 2.5, width: 3.2, length: 2.4, height: 1.05,
  stride: 2.1, period: 2.0, duty: 0.62, lift: 1.4, radius: 6, seconds: 4,
  legRoots: [1.35, 0.5, -0.4, -1.3], legFeet: [[2.8, 6.6], [1.1, 7.4], [-0.9, 7.3], [-2.7, 6.4]] as const, legLengths: [3.0, 2.9, 1.2] as const,
  claw: [1.45, 0.65], stalk: 2.6, eye: 0.48,
  back: [0.92, 0.3, 0.16], belly: [1.0, 0.85, 0.66], tip: [0.16, 0.07, 0.07], iris: [1.0, 0.92, 0.2], bone: [1.0, 0.94, 0.8],
} as const;
export const SPAN = S.legLengths.reduce((a, b) => a + b, 0);

const sides = [-1, 1] as const;
/** The course: an arc of radius 6 turning toward the crab's front, sideways walking (heading +z, facing +x). */
export const path = course.arc([0, 0, 0], -Math.PI / 2, S.radius, Math.PI / 2);
/** The course in numbers at time t (the gait's body at a constant pace): position, yaw and their rates. */
export function onCourse(t: number) {
  const v = S.stride / S.period, phi = (v * t) / S.radius;
  return { x: S.radius * (1 - Math.cos(phi)), z: S.radius * Math.sin(phi), yaw: phi, vx: v * Math.sin(phi), vz: v * Math.cos(phi), vyaw: v / S.radius };
}
/** Feet: left 0..3 then right 0..3, alternating tetrapods with a small wave toward the front (islandback's). */
export const walk = gait({
  feet: sides.flatMap((s) => S.legFeet.map(([x, z]) => vec3(x, 0, s * z))),
  phase: sides.flatMap((s) => S.legFeet.map((_, i) => ((i % 2 === 0) === (s < 0) ? 0 : 0.5) + 0.07 * i)),
  course: path, stride: S.stride, pace: { kind: "time", period: S.period }, duty: S.duty, lift: S.lift,
});

/** The body frame at o, turned `yaw` about up (the gait's convention: x = (cos, 0, -sin)), rolled `roll` about x. */
export function bodyFrame(o: P3, yaw: F, roll: F | number = 0): Frame {
  const x = vec3(cos(yaw), 0, mul(sin(yaw), -1)), y = lift<"vec3">([0, 1, 0]), z = vec3(sin(yaw), 0, cos(yaw));
  return { o, x, y: rotateAbout(y, x, roll), z: rotateAbout(z, x, roll) };
}
const inBody = (body: Frame, p: readonly [number, number, number] | P3): P3 => add(body.o, rotate(body, lift<"vec3">(p)));
/** Where leg k leaves the body (body coordinates). */
export const legRoot = (k: number): readonly [number, number, number] => [S.legRoots[k % 4]!, 0.1, (k < 4 ? -1 : 1) * S.width * 0.78];

export function crab(body: Frame) {
  const local = (p: readonly [number, number, number] | P3) => inBody(body, p);
  const carapace = place(strand({
    name: "carapace", spine: (m) => through(m, [[-S.length, 0, 0], [0, 0.1, 0], [S.length, -0.05, 0]]), stations: 8,
    girth: S.width, section: { aspect: S.height / S.width, squareness: 2.4 }, tips: [{ round: 0.5 }, { round: 0.5 }],
    paint: (_m, v) => mix(palette.countershade(v, { back: S.back, belly: S.belly }), vec3(...S.tip), smoothstep(0.06, 0.02, abs(sub(sin(mul(v, Math.PI * 2)), -0.2)))),
    detail: { along: 40, around: 40 }, up: [0, 1, 0],
  }), rigid(body));
  const segs = keys([[0, 1], [0.4, 0.85], [0.8, 0.5], [1, 0.18]]);
  const legs = sides.flatMap((s, si) => S.legRoots.map((_, i) => {
    const k = si * 4 + i;
    const leg = chain({ root: local(legRoot(k)), target: walk.foot(k), lengths: S.legLengths, bend: add(body.y, mul(body.z, s * 0.45)), foot: mul(body.y, -1) });
    return strand({
      name: `leg${s < 0 ? "L" : "R"}${i}`, spine: leg.spine, stations: 24, girth: (m) => mul(segs(m).x, 0.36), tips: ["round", { point: 0.1 }],
      section: { aspect: 0.8 }, up: body.y,
      paint: (m, v) => mix(palette.countershade(v, { back: S.back, belly: S.belly }), vec3(...S.tip), max(smoothstep(0.8, 0.86, m), sub(1, smoothstep(0.012, 0.03, leg.knots.slice(1, 3).reduce((d: F, kn) => min(d, abs(sub(m, kn))), lift<"f32">(1)))))),
      detail: { along: 48, around: 10 },
    });
  }));
  const claws = sides.flatMap((side, ci) => {
    const size = S.claw[ci === 1 ? 0 : 1]!, z = side * S.width * 0.5;
    const arm = chain({ root: local([S.length * 0.75, 0.2, z]), target: local([S.length + 1.9 * size, 0.7, z + side * 1.2 * size]), lengths: [1.9 * size + 0.4, 1.7 * size + 0.4], bend: add(mul(body.z, side), mul(body.y, -0.4)) });
    const [, elbow, wrist] = arm.joints;
    const along = unit(add(unit(sub(wrist!, elbow!)), mul(body.z, -side * 1.1)));
    const len = 2.5 * size, fat = 0.95 * size;
    const armStrand = strand({ name: `arm${ci}`, spine: arm.spine, stations: 16, girth: (m) => mul(fat, sub(0.45, mul(m, 0.12))), tips: ["round", "round"], up: body.y,
      paint: (_m, v) => palette.countershade(v, { back: S.back, belly: S.belly }), detail: { along: 32, around: 12 } });
    const palm = strand({
      name: `palm${ci}`, spine: (m) => add(wrist!, mul(along, mul(len, m))), stations: 6, girth: (m) => mul(fat, add(0.5, mul(sin(mul(m, Math.PI)), 0.28))),
      section: { aspect: 0.6 }, tips: [{ round: 0.3 }, { round: 0.3 }], up: body.y,
      paint: (m, v) => mix(palette.countershade(v, { back: S.back, belly: S.belly }), vec3(1, 0.62, 0.3), mul(smoothstep(0.4, 0.9, m), 0.4)), detail: { along: 20, around: 16 },
    });
    const finger = (v: number, a: number) => {
      const f = palm.frame(vec3(0.9, v, 0.35));
      const hinged: Frame = { o: f.o, x: rotateAbout(f.x, f.z, a), y: rotateAbout(f.y, f.z, a), z: f.z };
      return place(spiralTube({ name: `finger${ci}${v > 0 ? "a" : "b"}`, length: len * 0.9, curl: 0.75, taper: 0.18, girth: fat * 0.42, out: [1, -0.15, 0], toward: [0, -1, 0], paint: (m) => mix(vec3(...S.back), vec3(...S.bone), smoothstep(0.55, 0.75, m)) }), rigid(hinged));
    };
    return [armStrand, palm, finger(side > 0 ? 0.5 : 0, 0.08), finger(side > 0 ? 0 : 0.5, 0.25)];
  });
  const eyes = sides.flatMap((s) => {
    const base = local([S.length * 0.95, S.height * 0.4, s * 0.8]);
    const tip = local([S.length * 0.95 + S.stalk * 0.45, S.height * 0.4 + S.stalk, s * 1.1]);
    const tag = s < 0 ? "L" : "R";
    const stalk = strand({ name: `stalk${tag}`, spine: (m) => through(m, [base, add(base, mul(body.y, S.stalk * 0.6)), tip]), stations: 6, girth: 0.2, tips: ["open", "round"], paint: S.back, detail: { along: 12, around: 8 } });
    const ball = strand({ name: `eye${tag}`, spine: (m) => add(tip, mul(body.x, mul(sub(m, 0.5), S.eye * 2))), stations: 4, girth: S.eye, tips: [{ round: 0.5 }, { round: 0.5 }], up: body.y,
      paint: (m) => mix(mix(vec3(0.1, 0.05, 0.05), vec3(...S.iris), smoothstep(0.35, 0.45, m)), vec3(0.03, 0.02, 0.02), smoothstep(0.9, 0.93, m)), detail: { along: 12, around: 12 } });
    return [stalk, ball];
  });
  return [carapace, ...legs, ...claws, ...eyes];
}

/** The comparison's camera, ground and light: identical for both panels. */
export const CAMERAS = {
  square: { kind: "perspective" as const, eye: [12, 17, 20] as [number, number, number], target: [0.9, 1.4, 2.0] as [number, number, number], fovY: 0.74 },
};

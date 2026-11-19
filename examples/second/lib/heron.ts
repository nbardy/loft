// The heron of examples/second/heron.ts as parts, for any motion: a body frame (the hips' place and heading) and two
// foot targets in, the drawables and the legs' inverse-kinematics specs out. Walking (heron.ts) and leaping
// (heron_leap.ts) differ only in where the frame and the feet come from.
import { add, chain, mix, mul, place, rigid, rotateAbout, smoothstep, strand, sub, through, vec3, lift, type F, type Frame, type In, type P3 } from "../../../src/index.ts";
import type { ChainSpec } from "../../../src/geom/limb.ts";
import { rotate } from "../../../src/map/chart.ts";

/** Upper and lower leg (hip to ankle, ankle to foot), metres. */
export const LEGS = [1.05, 1.1] as const;
/** Where each foot sits across the body (its z in the ground frame). */
export const HOMES = [[0, 0, -0.17], [0, 0, 0.17]] as const;
const grey = [0.55, 0.6, 0.66] as const, white = [0.95, 0.94, 0.9] as const, dark = [0.12, 0.12, 0.14] as const, bill = [0.98, 0.78, 0.2] as const;

/** A point given in the body frame's coordinates, in the world. */
export const inBody = (body: Frame, p: In<"vec3">): P3 => add(body.o, rotate(body, lift<"vec3">(p)));
/** Where leg i leaves the body, in the body frame. */
export const hipOf = (i: number): readonly [number, number, number] => [0, -0.15, HOMES[i]![2] * 0.8];

/** Where the neck leaves the body (body frame): the pivot of `headFrame`. */
export const NECK_BASE = [0.5, 0.2, 0] as const;
/**
 * The frame the neck, bill and plume are drawn in: the body's, turned by `angle` (radians, positive lifts the bill, as
 * a pose's pitch) about the body's z through `NECK_BASE`. With angle = -pitch the head stays level in the world.
 */
export function headFrame(body: Frame, angle: F): Frame {
  const x = rotateAbout(body.x, body.z, angle), y = rotateAbout(body.y, body.z, angle);
  const turned: Frame = { o: body.o, x, y, z: body.z };
  // Keep the pivot where the body has it: o' = pivot - turned(NECK_BASE).
  return { ...turned, o: sub(inBody(body, NECK_BASE), rotate(turned, lift<"vec3">(NECK_BASE))) };
}

/**
 * The heron drawn at `body` (x ahead, y up through the back) with feet at `foot(i)`, the toes splayed along `ahead`
 * (the ground's forward direction). The neck, bill and plume ride `head` (default the body; `headFrame` turns them); `neckSpine`, a world curve from the shoulder to the head, replaces the rigid neck. The legs reach from the hips to the feet by `chain`, the joint bent backward.
 */
export function heron(body: Frame, foot: (i: number) => P3, ahead: { readonly x: P3; readonly z: P3 }, head: Frame = body, neckSpine?: (m: F) => P3) {
  const torso = place(strand({
    name: "torso", spine: (m) => through(m, [[0.55, 0.15, 0], [0, 0, 0], [-0.7, -0.05, 0], [-1.05, -0.2, 0]]), stations: 8,
    girth: (m) => mul(0.34, sub(1, mul(m, 0.55))), section: { aspect: 1.15 }, tips: ["round", { point: 0.3 }],
    paint: (m) => mix(vec3(...grey), vec3(...dark), smoothstep(0.75, 0.95, m)), detail: { along: 30, around: 18 }, up: [0, 1, 0],
  }), rigid(body));
  const neckPts: readonly (readonly [number, number, number])[] = [[0.5, 0.2, 0], [0.75, 0.55, 0], [0.55, 0.95, 0], [0.75, 1.3, 0], [1.0, 1.38, 0]];
  const neck = place(strand({
    name: "neck", spine: (m) => through(m, neckPts), stations: 16, girth: (m) => mix(0.14, 0.08, m), tips: ["open", "round"],
    paint: white, detail: { along: 40, around: 12 }, up: [0, 1, 0],
  }), rigid(head));
  // A neck given as a world curve (shoulder to head, e.g. per-frame IK) replaces the rigid one.
  const neckDrawn = neckSpine === undefined ? neck : strand({
    name: "neck", spine: neckSpine, stations: 16, girth: (m) => mix(0.14, 0.08, m), tips: ["open", "round"], paint: white, detail: { along: 40, around: 12 },
  });
  const beak = place(strand({
    name: "bill", spine: (m) => vec3(add(1.0, mul(m, 0.75)), sub(1.38, mul(m, 0.1)), 0), stations: 4,
    girth: (m) => mix(0.075, 0.01, m), section: { aspect: 0.7 }, tips: ["round", "point"], paint: bill, detail: { along: 12, around: 10 }, up: [0, 1, 0],
  }), rigid(head));
  const crest = place(strand({
    name: "plume", spine: (m) => vec3(sub(0.95, mul(m, 0.55)), add(1.45, mul(m, 0.08)), 0), stations: 4,
    girth: (m) => mix(0.04, 0.008, m), tips: ["round", "point"], paint: dark, detail: { along: 10, around: 8 }, up: [0, 1, 0],
  }), rigid(head));
  // Legs: hip to foot by IK, the joint bent backward (a heron's visible "knee" is its ankle).
  const legs: ChainSpec[] = HOMES.map((_, i) => ({ root: inBody(body, hipOf(i)), target: foot(i), lengths: LEGS, bend: mul(body.x, -1) }));
  const legDrawn = legs.map((spec, i) => strand({
    name: `leg${i}`, spine: chain(spec).spine, stations: 12, girth: 0.045, tips: ["round", "round"], paint: bill, detail: { along: 24, around: 8 },
  }));
  // Toes: three per foot, splayed forward on the floor, drawn from the foot.
  const toes = HOMES.flatMap((_, i) => [-0.5, 0, 0.5].map((a, k) => strand({
    name: `toe${i}${k}`, spine: (m) => add(foot(i), mul(add(mul(ahead.x, Math.cos(a)), mul(ahead.z, Math.sin(a))), mul(m, 0.28))),
    stations: 4, girth: 0.025, tips: ["round", "point"], paint: dark, detail: { along: 4, around: 6 },
  })));
  return { things: [torso, neckDrawn, beak, crest, ...legDrawn, ...toes], legs };
}

// The Reefback Titan (game design notes 2026-11-01, section 6.2): a hermit crab the size of an island, wearing a dead
// atoll for its shell with a ruined fort on the crown. Woodblock print (docs/art_direction/DIRECTION.md): a brick-red
// crab, a moss-green atoll with stone cliffs, a cream ruin; indigo sea, cream foam; one small blue-sailed sloop for
// scale.
//
// What it is made of:
// - the crab: a carapace (a short fat strand), eight three-segment legs on a gait (it treads in place: the game moves
//   the boss, the art only has to look alive), two unequal claws whose arms reach their wrists by inverse kinematics
//   and whose pincer fingers are horns, stalked eyes that look about, a row of mouthpart fangs (`draw.points`);
// - the atoll: one closed-form surface (a lumpy mesa: moss plateau, a lip, stone cliffs with vertical strokes, a foot
//   tucked under into the water), carried by the crab's frame;
// - coral heads round the cliff top, trees on the plateau and the fort's rubble and merlons: one `draw.copies` list of
//   rounded cones riding the crab's frame (the `carry` option of draw.copies), plus the roofless tower as a strand;
// - Claw Crush, the signature attack: a space-time `attack` on a disc ahead-right of the crab. Its lunge law poses the
//   great claw's wrist target (wind up high and open, slam down onto the disc exactly at the arming time); its
//   `telegraph` is the warning on the water. It loops on time: armed every `S.crushPeriod` seconds.
//
// Crab coordinates: x forward, y up, z to its right; the body frame rides `S.ride` above the still water.

import {
  abs, add, attack, course, cos, cross, div, draw, dot, floor, framed, fract, gait, hit, horn, keys, length, lift, limb, lookAt, max, min, mix, mul,
  place, points, rigid, rotateAbout, scene, sea, ship, sin, smoothstep, spacing, sqrt, step, strand, sub, surface, tabulate, telegraph, through, time,
  unit, vec2, vec3, vec4, woodblock, type F, type Frame, type P3, type Style,
} from "../../src/index.ts";
import { rotate } from "../../src/map/chart.ts";
import { hash3, luminance, noise2, type Branch } from "../../src/lib/index.ts";
import { edgeOf, rgb } from "../../src/style/shared.ts";
import { brig, PLAYER } from "../scenes/lib/fleet.ts";

type V3 = readonly [number, number, number];

// ==== the numbers an art director changes ============================================================================

export const S = {
  // ---- inks (woodblock): few, flat; shadow is the same ink overprinted with indigo ----
  ink: {
    black: [0.075, 0.07, 0.09], indigo: [0.13, 0.2, 0.37], cream: [0.93, 0.88, 0.75],
    brick: [0.66, 0.22, 0.13], brickDark: [0.42, 0.12, 0.09], moss: [0.47, 0.56, 0.24], mossDeep: [0.2, 0.32, 0.17],
    stone: [0.7, 0.63, 0.5], bark: [0.3, 0.2, 0.14], coral: [0.86, 0.56, 0.46],
  } satisfies Record<string, V3>,
  /** How much indigo the shadow side of every ink takes (0 none, 1 indigo). */
  shadowIndigo: 0.42,
  /** Shadow side brightness after the indigo. */
  shadowValue: 0.78,

  // ---- size and stance ----
  ride: 1.35, floor: -1.7,
  carapace: { from: -1.6, to: 5.0, width: 4.1, height: 1.25 },
  legRoots: [3.2, 2.0, 0.8, -0.4], legFeet: [[6.8, 8.2], [4.0, 9.6], [0.8, 9.9], [-2.6, 9.0]] as const, legLengths: [4.0, 3.8, 1.6] as const,
  // ---- the walk in place ----
  period: 3.2, duty: 0.66, lift: 1.5, sway: 0.25, bob: 0.08, roll: 0.03, yaw: 0.035,

  // ---- the atoll ----
  atoll: { centre: [-2.0, 0] as const, rx: 4.3, rz: 4.5, top: 2.9, base: -1.75, hills: 0.35, lumps: 0.08 },
  coralHeads: 18, trees: 6,
  fort: { at: [-2.6, -1.1] as const, height: 2.4, girth: 0.85, lean: 0.12 },

  // ---- claws: the right (crusher) 1.4 times the left ----
  crusher: 1.4, cutter: 1.0, clawPalm: 2.9, clawFat: 1.1, armLengths: [2.5, 2.4] as const,

  // ---- eyes ----
  stalk: 2.4, eye: 0.3,

  // ---- Claw Crush: the hit disc on the water (world xz), its warning, its loop ----
  crushCentre: [9.6, 5.6] as const, crushRadius: 3.4, crushWarn: 1.5, crushFirst: 3.4, crushPeriod: 8,
  crushRise: 6.5, crushCoil: 2.0,
  /** The foam crash after the slam: how far the ring runs out (times the disc's radius), and for how long. */
  crashReach: 1.6, crashSeconds: 1.4,

  // ---- the ship for scale (world xz, radians) ----
  shipAt: [5, 15.5] as const, shipHeading: -1.15, shipLength: 4.6,
} as const;

const I = S.ink;

/** A crisp ink edge: 1 where x is above `at`, about one pixel wide (from the screen-space derivative). A wide
 * smoothstep is a soft airbrushed fade, which the woodblock's flat inks never have. */
const cut = (x: F, at: number | F): F => edgeOf(x, at, 1e-5);

// ==== the style: the woodblock's key line, paper and sea, with this monster's inks ===================================

/**
 * The woodblock's post passes, sea and sky, with bodies shaded in this monster's inks: every paint is already a flat
 * ink, lit side as painted, shadow side overprinted with indigo (two tones, a hard terminator).
 */
function titanWoodblock(): Style {
  const wb = woodblock();
  return {
    ...wb,
    shade: (ctx) => {
      const lit = edgeOf(dot(ctx.normal, ctx.sun), 0.0, 1e-5);
      const shadow = mul(mix(ctx.base, rgb(I.indigo), S.shadowIndigo), S.shadowValue);
      // Paint near black prints as the key block's black on both sides.
      return mix(rgb(I.black), mix(shadow, ctx.base, lit), edgeOf(luminance(ctx.base), 0.1, 1e-5));
    },
  };
}

// ==== the walk: in place, the body swaying over feet that tread ======================================================

const sides = [-1, 1] as const;
const feetHome = sides.flatMap((s) => S.legFeet.map(([x, z]) => vec3(x, S.floor, s * z)));
// Alternating tetrapods, each side a small wave toward the front.
const phase = sides.flatMap((s) => S.legFeet.map((_, i) => ((i % 2 === 0) === (s < 0) ? 0 : 0.5) + 0.06 * i));
const walk = gait({ feet: feetHome, phase, course: course.line([0, 0, 0], 0), stride: 0, period: S.period, duty: S.duty, lift: S.lift });
const w = (2 * Math.PI) / S.period;
const g = walk.body;

// ---- Claw Crush: armed every crushPeriod seconds (looping on time), first at crushFirst ----
// The arming time steps to the next cycle 3 s after each slam (the recovery is over by then).
const crushArm = add(mul(floor(mul(add(sub(time, S.crushFirst), 3), 1 / S.crushPeriod)), S.crushPeriod), S.crushFirst);
const crushDisc = hit.disc({ centre: S.crushCentre, radius: S.crushRadius });

// The body leans into the slam, on its own smooth envelope around the same arming time (longer and softer than the
// attack's posture envelope, so the whole crab settles into the blow and back out of it).
const lean0 = smoothstep(-1.5, -0.3, sub(time, crushArm)), lean1 = sub(1, smoothstep(0.4, 2.2, sub(time, crushArm)));
const lean = mul(lean0, lean1);
const bob = add(mul(cos(mul(time, 2 * w)), S.bob), mul(lean, -0.25));
const roll = add(mul(sin(mul(time, w)), S.roll), mul(lean, 0.06));
const yaw = add(mul(sin(mul(time, w * 0.5)), S.yaw), mul(lean, -0.08));
const sway = mul(sin(mul(time, w)), S.sway);
const yx = rotateAbout(g.x, g.y, yaw), yz = rotateAbout(g.z, g.y, yaw);
// Roll about the body's x, then pitch (the lean) about the rolled z: an orthonormal frame, as draw.copies' carry needs.
const ry = rotateAbout(g.y, yx, roll), rz = rotateAbout(yz, yx, roll), pitch = mul(lean, -0.05);
const bodyNow: Frame = { o: add(add(g.o, mul(g.y, add(S.ride, bob))), mul(g.z, sway)), x: rotateAbout(yx, rz, pitch), y: rotateAbout(ry, rz, pitch), z: rz };
// The frame is evaluated once per frame on the CPU into a small table, and every vertex fetches it. Written inline it
// was re-evaluated per vertex by the atoll and the carried copies (gait, sway, lean and the looping arming time):
// profiled at 1.3 ms for the copies and 0.63 ms for the atoll from the game camera, against 0.49 and 0.21 with the
// copies not carried (2026-11-11).
const tab = (name: string, e: P3) => tabulate(`titan_frame_${name}`, () => e, 2, "frame")(0);
const body: Frame = { o: tab("o", bodyNow.o), x: tab("x", bodyNow.x), y: tab("y", bodyNow.y), z: tab("z", bodyNow.z) };
const local = (p: V3 | P3): P3 => add(body.o, rotate(body, lift<"vec3">(p)));

// ==== the carapace: brick with dark mottling, a pale rim line ========================================================

const C = S.carapace;
const mottle = (m: F, v: F): F => {
  const a = sin(add(mul(m, 23), mul(sin(mul(v, 31)), 1.7))), b = sin(add(mul(v, 41), mul(sin(mul(m, 17)), 1.3)));
  return cut(mul(a, b), 0.575);
};
const carapace = place(strand({
  name: "titan", spine: (m) => through(m, [[C.from, -0.1, 0], [(C.from + C.to) / 2, 0.15, 0], [C.to, 0, 0]]), stations: 8,
  girth: C.width, section: { aspect: C.height / C.width, squareness: 2.3 }, tips: [{ round: 0.45 }, { round: 0.4 }],
  paint: (m, v) => {
    const rim = sub(1, cut(abs(sub(sin(mul(v, Math.PI * 2)), -0.15)), 0.035));
    const back = mix(vec3(...I.brick), vec3(...I.brickDark), mottle(m, v));
    const belly = vec3(...I.cream);
    return mix(mix(belly, back, cut(sin(mul(v, Math.PI * 2)), -0.2)), vec3(...I.black), rim);
  },
  detail: { along: 40, around: 40 }, up: [0, 1, 0],
}), rigid(body));

// ==== legs: three segments by inverse kinematics, banded at the joints, cream dactyls ================================

const segs = keys([[0, 1], [0.4, 0.85], [0.8, 0.5], [1, 0.18]]);
const legs = sides.flatMap((s, si) => S.legRoots.map((x, i) => {
  const k = si * 4 + i;
  const root = local([x, -0.1, s * C.width * 0.85]);
  const leg = limb({ root, target: walk.foot(k), lengths: S.legLengths, bend: add(body.y, mul(body.z, s * 0.4)), foot: mul(body.y, -1) });
  const nearKnot = (m: F): F => leg.knots.slice(1, 3).reduce((d: F, kn) => min(d, abs(sub(m, kn))), lift<"f32">(1));
  const pinch = (m: F) => sub(1, mul(smoothstep(0.05, 0, nearKnot(m)), 0.3));
  return strand({
    name: `leg${s < 0 ? "L" : "R"}${i}`, spine: leg.spine, stations: 24, girth: (m) => mul(mul(segs(m).x, 0.5), pinch(m)), tips: ["round", { point: 0.1 }],
    section: { aspect: 0.8 }, up: body.y,
    paint: (m, v) => {
      const top = mix(vec3(...I.brick), vec3(...I.brickDark), mottle(mul(m, 0.7), v));
      const shell = mix(vec3(...I.cream), top, cut(sin(mul(v, Math.PI * 2)), -0.2));
      // A cream band before each joint and a black ring at it; the dactyl cream to a dark point.
      const band = sub(1, cut(nearKnot(m), 0.028));
      const ring = sub(1, cut(nearKnot(m), 0.012));
      const tip = cut(m, 0.84);
      return mix(mix(mix(shell, vec3(...I.cream), max(band, tip)), vec3(...I.black), ring), vec3(...I.black), cut(m, 0.98));
    },
    detail: { along: 64, around: 12 },
  });
}));

// ==== Claw Crush: the attack, defined once ===========================================================================

/** Size of a claw: palm length, palm half-width, arm segment lengths. */
const clawDims = (size: number) => ({ len: S.clawPalm * size, fat: S.clawFat * size, arm: S.armLengths.map((l) => l * (0.55 + 0.45 * size)) as unknown as readonly [number, number] });
const bigDims = clawDims(S.crusher);
const shoulder = (side: 1 | -1): V3 => [C.to - 1.1, -0.1, side * C.width * 0.55];
const wristRest = (side: 1 | -1, size: number): V3 => [C.to + 1.0 * size, 0.4, side * (C.width * 0.55 + 0.6 * size)];

// The lunge law poses a two-point spine from the wrist (m = 0) back to the shoulder: its m = 0 end is the wrist's
// target. During the wind-up it pulls back by `coil` and rises by `rise` (the claw lifts high), then lands `lead` short
// of the disc's centre exactly at the arming time (so the palm, reaching on, covers the centre), and recovers.
const crush = attack({
  name: "crush", arm: crushArm, warn: S.crushWarn,
  target: { kind: "disc", centre: vec2(...S.crushCentre), radius: lift<"f32">(S.crushRadius) },
  part: "crusherPalm",
  motion: { kind: "lunge", hit: crushDisc, reach: 1, lead: bigDims.len * 0.5, height: 0.7, coil: S.crushCoil, rise: S.crushRise, approach: 0.3, neck: 1 },
  timing: { windup: 1.15, strike: 0.3, zeta: 0.55, hold: 0.45, recover: 0.5, travel: 50 },
});
const env = crush.envelope(time);
const crushWrist = crush.pose((m) => mix(local(wristRest(1, S.crusher)), local(shoulder(1)), m))(lift<"f32">(0));

function claw(name: string, side: 1 | -1, size: number, wrist: P3, gape: F, fold: F) {
  const d = clawDims(size);
  const arm = limb({ root: local(shoulder(side)), target: wrist, lengths: d.arm, bend: add(mul(body.z, side), mul(body.y, 0.5)) });
  const [, elbow, w] = arm.joints;
  // The palm continues the forearm, folded in toward the face at rest as a crab carries it.
  const along = unit(add(unit(sub(w, elbow)), mul(body.z, mul(fold, -side * 0.55))));
  const armStrand = strand({
    name: `${name}Arm`, spine: arm.spine, stations: 16, girth: (m) => mul(d.fat, sub(0.42, mul(m, 0.1))), tips: ["round", "round"], up: body.y,
    paint: (m, v) => {
      const top = mix(vec3(...I.brick), vec3(...I.brickDark), mottle(mul(m, 0.6), v));
      const ring = sub(1, cut(abs(sub(m, arm.knots[1]!)), 0.022));
      return mix(mix(vec3(...I.cream), top, cut(sin(mul(v, Math.PI * 2)), -0.25)), vec3(...I.black), ring);
    },
    detail: { along: 40, around: 14 },
  });
  const palm = strand({
    name: `${name}Palm`, spine: (m) => add(w, mul(along, mul(d.len, m))), stations: 6, girth: (m) => mul(d.fat, add(0.55, mul(sin(mul(m, Math.PI)), 0.3))),
    section: { aspect: 0.62 }, tips: [{ round: 0.3 }, { round: 0.3 }], up: body.y,
    paint: (m, v) => mix(mix(vec3(...I.brick), vec3(...I.brickDark), mottle(mul(m, 0.8), mul(v, 1.3))), vec3(...I.cream), cut(mul(sin(mul(v, Math.PI * 2)), -1), -0.475)),
    detail: { along: 24, around: 20 },
  });
  // The pincer opens sideways, in the palm's flat plane, so it reads as a V from above.
  const finger = (fname: string, v: number, opens: number) => {
    const f = palm.frame(vec3(0.88, v, 0.35));
    const a = mul(gape, opens);
    const hinged: Frame = { o: f.o, x: rotateAbout(f.x, f.z, a), y: rotateAbout(f.y, f.z, a), z: f.z };
    return place(horn({
      name: fname, length: d.len * 0.95, curl: 0.8, taper: 0.16, girth: mul(d.fat, 0.44), out: [1, -0.15, 0], toward: [0, -1, 0],
      paint: (m) => mix(mix(vec3(...I.brick), vec3(...I.cream), cut(m, 0.56)), vec3(...I.black), cut(m, 0.965)),
    }), rigid(hinged));
  };
  return [armStrand, palm, finger(`${name}Fixed`, side > 0 ? 0.5 : 0, 0.35), finger(`${name}Dactyl`, side > 0 ? 0 : 0.5, 1)];
}
// The crusher gapes wide in the wind-up and shuts on the hit; the cutter works slowly at rest.
const crushGape = add(0.28, mul(mul(env.p, sub(1, smoothstep(0.55, 1, env.q))), 0.85));
const cutterWrist = local(add(vec3(...wristRest(-1, S.cutter)), vec3(mul(sin(mul(time, 0.7)), 0.4), mul(sin(mul(time, 0.9)), 0.35), 0)));
const claws = [
  // In the attack the crusher's palm unfolds to drive straight on along its forearm.
  ...claw("crusher", 1, S.crusher, crushWrist, crushGape, sub(1, env.p)),
  ...claw("cutter", -1, S.cutter, cutterWrist, add(0.3, mul(max(sin(mul(time, 1.3)), 0), 0.35)), lift<"f32">(1)),
];

// ==== the face: a grudging old giant ==================================================================================
//
// Temperament: grudging. It is older than the fort on its back, half asleep, chewing; it has outlived whatever took its
// left eye and does not care about you until you come into its water. Every feature serves that:
// - a heavy brow (the carapace's front edge) in a shallow V, low over the eyes: a scowl it wears even asleep;
// - eyes under heavy lids that sit across the top of a horizontal goat's pupil; the lids slant down toward the middle;
// - the left eye is the old injury: a short, healed stalk, a clouded eye with a scar through it, a lid that droops,
//   barnacles round its socket and more weed on that side. The two eyes never move together;
// - the mouth is layered plates (the third maxillipeds) that close like doors and work slowly, with a beard of weed;
// - in Claw Crush the good eye snaps wide with a pinpoint pupil, the stalks stand up toward the target, the plates
//   flare open on the black mouth; at the slam it screws the eye shut and clamps its mouth; then it stares, and
//   settles back into the heavy-lidded chew.

/** Seconds from this cycle's slam (crushArm), and the face's two attack envelopes, both 0 before and after a cycle. */
const tA = sub(time, crushArm);
/** Roused: from the start of the wind-up, through the stare after the slam, settling back by 2.4 s. */
const rouse = mul(smoothstep(-1.3, -0.95, tA), sub(1, smoothstep(0.9, 2.4, tA)));
/** Braced: the eye screwed shut and the mouth clamped, at the slam. */
const brace = mul(smoothstep(-0.14, 0, tA), sub(1, smoothstep(0.25, 0.8, tA)));
/** A blink every `period` seconds: shut for an instant, opening over 0.13 s. None while roused. */
const blink = (period: number, offset: number): F => {
  const k = abs(sub(mul(fract(mul(add(time, offset), 1 / period)), period), 0.3));
  return mul(sub(1, smoothstep(0.03, 0.16, k)), sub(1, rouse));
};

const tabF = (name: string, e: F): F => tabulate(`titan_face_${name}`, () => e, 2, "frame")(0);
/** An offset in the crab's frame. */
const inBody = (x: F | number, y: F | number, z: F | number): P3 => add(add(mul(body.x, x), mul(body.y, y)), mul(body.z, z));
const TAU = 2 * Math.PI;

// ---- the brow: one heavy ridge across the front of the carapace, a shallow V low over the eyes ----

/** The brow's line in crab coordinates, outer left to outer right; the middle is the V's low point. */
const BROW: readonly V3[] = [[3.2, 0.98, -3.05], [4.1, 1.06, -1.9], [4.6, 0.86, -0.85], [4.93, 0.5, 0], [4.6, 0.86, 0.85], [4.1, 1.06, 1.9], [3.2, 0.98, 3.05]];
const brow = place(strand({
  name: "brow", spine: (m) => through(m, BROW), stations: 12,
  // Heaviest over the eyes, thinner at the V and at the ends.
  girth: (m) => mul(0.5, add(0.6, mul(abs(sin(mul(m, 2 * TAU))), 0.4))), section: { aspect: 0.5, squareness: 2.6 },
  tips: [{ round: 0.08 }, { round: 0.08 }], up: [0.7, 1, 0],
  paint: (m, v) => {
    // Plain dark brick (mottle made it texture, not a shape) with a cream underside: the line the face hangs from.
    return mix(vec3(...I.brickDark), vec3(...I.cream), cut(mul(sin(mul(v, TAU)), -1), 0.35));
  },
  detail: { along: 48, around: 12 },
}), rigid(body));

// ---- the eyes ----

type Eye = {
  readonly tag: "L" | "R";
  /** The stalk's root and its tip, world. */
  readonly base: P3; readonly tip: P3;
  /** The eye's facing, a unit vector, world. */
  readonly look: P3;
  readonly radius: number;
  /** Where the upper lid's edge is, radians from straight up over the front (pi / 2 halves the eye; pi shuts it). */
  readonly lid: F;
  /** The lid edge's slant, radians: positive drops it toward the middle (a scowl), negative toward the outside (a droop). */
  readonly slant: F;
  /** The eyeball's paint over its front cap: x across, y up, both in [-1, 1]; and its back (m below 0.5). */
  readonly iris: (x: F, y: F) => P3;
  readonly stalkPaint: (m: F) => P3;
  readonly lidPaint: (edge: F) => P3;
};

/** The ball, its stalk and its lid (a spherical lune hinged on the eye's side axis, swinging over the front). */
function eyeParts(e: Eye) {
  const tip = tab(`eye_${e.tag}_tip`, e.tip), F0 = tab(`eye_${e.tag}_look`, e.look);
  const lid = tabF(`eye_${e.tag}_lid`, e.lid), slant = tabF(`eye_${e.tag}_slant`, e.slant);
  const side0 = unit(cross(F0, body.y)), up0 = cross(side0, F0);
  // The hinge axis tipped by the slant (outer pole raised for a scowl). `side` points right of the gaze, so the right
  // eye's outer pole is +side and the left eye's -side: the caller signs the slant.
  const side = add(mul(side0, cos(slant)), mul(up0, sin(slant))), up = sub(mul(up0, cos(slant)), mul(side0, sin(slant)));
  const mid = add(e.base, mul(sub(tip, e.base), 0.55));
  const stalk = strand({
    name: `stalk${e.tag}`, spine: (m) => mix(mix(e.base, add(mid, mul(body.y, 0.4)), m), mix(add(mid, mul(body.y, 0.4)), tip, m), m), stations: 6,
    girth: (m) => mix(0.24, 0.17, m), tips: ["open", "round"], paint: (m) => e.stalkPaint(m), detail: { along: 16, around: 10 },
  });
  const ball = strand({
    name: `eye${e.tag}`, spine: (m) => add(tip, mul(F0, mul(sub(m, 0.5), e.radius * 2))), stations: 4, girth: e.radius,
    tips: [{ round: 0.5 }, { round: 0.5 }], up: body.y,
    paint: (m, v) => {
      // The front cap's own coordinates: r from the axis, then across and up.
      const z = sub(mul(m, 2), 1), r = sqrt(max(sub(1, mul(z, z)), 0));
      // All cream, the pupil on the front. (A brick back made the eyes read as dark balls from every side but the front.)
      return mix(vec3(...I.cream), e.iris(mul(r, cos(mul(v, TAU))), mul(r, sin(mul(v, TAU)))), cut(m, 0.5));
    },
    detail: { along: 14, around: 18 },
  });
  // The lid hoods only the top front: from the high strategy camera a lid over the whole top hid the eye.
  const R = e.radius * 1.1, BACK = -0.4;
  const lidShape = surface((u, v) => {
    const a = mix(BACK, lid, u), b = mix(0.1, Math.PI - 0.1, v);
    const ring = add(mul(up, cos(a)), mul(F0, sin(a)));
    return add(tip, mul(add(mul(side, cos(b)), mul(ring, sin(b))), R));
  }, { u: ["free", "free"], v: "free" });
  const lidDraw = draw.surface(`lid${e.tag}`, lidShape, {
    // `edge` is radians back from the lid's edge.
    paint: (u) => e.lidPaint(mul(sub(1, u), sub(lid, BACK))),
    grid: { u: spacing.even(10), v: spacing.even(14) }, sides: "two", outline: "border",
  });
  return [stalk, ball, lidDraw];
}

/** The lid's paint: dark brick with a heavy black rim at its edge and one age crease behind it. (Barnacle specks on the
 * dead eye's lid made it read as a toadstool; more creases made a knitted cap.) */
const lidInk = (edge: F): P3 => {
  const rim = sub(1, cut(edge, 0.2));
  const crease = sub(1, cut(abs(sub(edge, 0.55)), 0.035));
  const base = vec3(...I.brickDark);
  return mix(mix(base, vec3(...I.black), crease), vec3(...I.black), rim);
};

// The good right eye: wanders slowly; when roused it fixes on the target.
const baseR = local([4.05, 0.98, 0.95]);
// It gazes a little upward, over the horizon: from the high cameras that turns the pupil toward the viewer.
const wander = local(vec3(14, add(mul(sin(mul(time, 0.29)), 2), 5), add(mul(sin(add(mul(time, 0.37), 1)), 8), 1)));
const lookR = lookAt({ from: { o: baseR, x: body.x, y: body.y, z: body.z }, target: mix(wander, vec3(S.crushCentre[0], 0, S.crushCentre[1]), rouse), yaw: 1.2, pitch: 0.7 }).dir;
const goodEye: Eye = {
  tag: "R", base: baseR,
  // At rest the stalk leans out and forward, a little tired; roused it stands up and leans toward the target.
  tip: add(baseR, add(mix(inBody(0.75, 1.75, 0.65), inBody(0.5, 2.35, 0.2), rouse), mul(lookR, 0.35))),
  look: lookR, radius: 0.64,
  lid: add(mix(mix(mix(1.45, 3.0, blink(5.3, 3.9)), 0.5, rouse), 2.55, brace), mul(sin(mul(time, 0.6)), 0.1)),
  slant: mix(0.38, 0.55, max(rouse, brace)),
  // A goat's bar pupil, wide and sleepy at rest; a pinpoint when roused.
  iris: (x, y) => {
    const w = mix(0.62, 0.16, rouse), h = mix(0.24, 0.16, rouse);
    const qx = div(x, w), qy = div(y, h);
    return mix(vec3(...I.cream), vec3(...I.black), cut(sub(1, add(mul(mul(qx, qx), mul(qx, qx)), mul(mul(qy, qy), mul(qy, qy)))), 0));
  },
  stalkPaint: (m) => mix(vec3(...I.brick), vec3(...I.black), cut(abs(sub(fract(mul(m, 4)), 0.5)), 0.47)),
  lidPaint: lidInk,
};

// The dead left eye: a short healed stalk splayed out, gazing at nothing; it only flinches.
const baseL = local([4.05, 0.98, -0.95]);
// It sags down and forward, so the clouded iris and the scar face the front cameras.
const lookL = unit(inBody(0.85, add(-0.4, mul(sin(mul(time, 0.21)), 0.08)), add(-0.3, mul(sin(mul(time, 0.17)), 0.1))));
const deadEye: Eye = {
  tag: "L", base: baseL,
  tip: add(baseL, add(inBody(0.55, 1.15, -0.75), mul(inBody(0, 0.2, 0), rouse))),
  look: lookL, radius: 0.5,
  lid: add(mix(1.75, 2.6, brace), mul(sin(mul(time, 0.45)), 0.06)),
  // Drooping toward the outside: the left eye's outer pole is -side, so a positive slant drops it there.
  slant: lift<"f32">(0.32),
  // Clouded: an indigo blot for a pupil, and the black line of the scar across it.
  iris: (x, y) => {
    const blot = cut(sub(0.32, length(vec2(x, mul(y, 1.3)))), 0);
    const scar = sub(1, cut(abs(sub(y, mul(x, 0.8))), 0.06));
    return mix(mix(vec3(...I.cream), mix(vec3(...I.cream), vec3(...I.indigo), 0.55), blot), vec3(...I.black), scar);
  },
  // Barnacle specks on the old stalk.
  stalkPaint: (m) => mix(vec3(...I.brick), vec3(...I.cream), cut(mul(sin(mul(m, 31)), sin(add(mul(m, 13), 2))), 0.6)),
  lidPaint: lidInk,
};

// ---- the mouth: two plates that close like doors, working slowly; flared in the wind-up, clamped at the slam ----

function plate(s: 1 | -1) {
  const chew = add(0.06, mul(max(sin(add(mul(time, 2.1), s * 1.3)), 0), 0.2));
  const open = tabF(`mouth_${s < 0 ? "L" : "R"}`, mul(mix(chew, 1, rouse), sub(1, brace)));
  const hinge = local([4.75, -0.3, s * 0.48]);
  const tipNow = local(add(vec3(5.3, -1.15, s * 0.22), vec3(mul(open, 0.25), mul(open, 0.2), mul(open, s * 0.85))));
  return strand({
    name: `maxilliped${s < 0 ? "L" : "R"}`, spine: (m) => mix(hinge, tipNow, m), stations: 4,
    girth: (m) => mul(0.4, sub(1, mul(m, 0.3))), section: { aspect: 0.32, squareness: 3 }, up: body.x,
    tips: [{ round: 0.1 }, { round: 0.15 }],
    paint: (m, v) => {
      // Cream plates, brick at the hinge, a black joint line, a row of black bristles down the inner edge.
      const segs = mix(vec3(...I.brick), vec3(...I.cream), cut(m, 0.3));
      const joint = sub(1, cut(abs(sub(m, 0.33)), 0.025));
      const bristle = mul(cut(mul(cos(mul(v, TAU)), -s), 0.8), cut(sin(mul(m, 40)), 0.3));
      return mix(segs, vec3(...I.black), max(joint, bristle));
    },
    detail: { along: 10, around: 14 },
  });
}
// The black of the mouth behind the plates: seen only when they open.
const gullet = place(strand({
  name: "gullet", spine: (m) => mix(vec3(4.7, -0.2, 0), vec3(4.85, -1.05, 0), m), stations: 4, girth: 0.6,
  section: { aspect: 0.35 }, up: [1, 0, 0], tips: [{ round: 0.2 }, { round: 0.3 }], paint: () => vec3(...I.black), detail: { along: 6, around: 10 },
}), rigid(body));

// ---- the weed beard: hanging under the face on both sides of the mouth, heavier on the dead eye's side ----

const weed = horn({
  name: "weed", length: 1.6, curl: 1.4, taper: 0.6, girth: 0.11, out: [0.4, 1, 0], toward: [1, 0, 0],
  paint: (m) => mix(vec3(...I.mossDeep), vec3(...I.moss), cut(sin(add(mul(m, 7), 1)), 0.6)),
});
const BEARD: readonly (readonly [number, number, number])[] = [
  [0.9, 0.53, 0.9], [0.93, 0.57, 1.2], [0.95, 0.61, 1.0], [0.91, 0.65, 0.8], [0.96, 0.66, 1.3], // left (v toward 0.5)
  [0.94, 0.88, 0.9], [0.92, 0.93, 0.7], // right
];
const beard = draw.points("beard", place(points(BEARD.length, (i) => {
  const pick = (k: 0 | 1 | 2) => BEARD.reduce((acc: F, t, j) => add(acc, mul(t[k], sub(1, min(abs(sub(i, j)), 1)))), lift<"f32">(0));
  return { at: vec3(pick(0), pick(1), 0.92), scale: add(pick(2), mul(sin(add(mul(time, 0.8), i)), 0.05)) };
}), carapace), weed);

const face = [brow, ...eyeParts(goodEye), ...eyeParts(deadEye), plate(1), plate(-1), gullet, beard];

// ==== the atoll: a lumpy mesa carried on the crab ====================================================================

const A = S.atoll;
/** Radius scale of the atoll's outline at angle a (radians): an ellipse with lumps. JavaScript twin below. */
const lumpsOf = (a: F): F => add(1, mul(add(add(sin(add(mul(a, 3), 1.1)), mul(sin(add(mul(a, 5), 2.3)), 0.7)), mul(sin(add(mul(a, 9), 0.4)), 0.4)), A.lumps));
const lumpsJs = (a: number) => 1 + (Math.sin(3 * a + 1.1) + 0.7 * Math.sin(5 * a + 2.3) + 0.4 * Math.sin(9 * a + 0.4)) * A.lumps;
/** Plateau height above `top` at local (x, z): low hills, highest at the fort's knoll. JavaScript twin below. */
const hillsOf = (x: F, z: F): F => {
  const k = sub(1, smoothstep(0, 2.6, add(abs(sub(x, S.fort.at[0])), abs(sub(z, S.fort.at[1])))));
  return add(mul(add(mul(sin(add(mul(x, 0.9), 0.3)), sin(mul(z, 0.8))), 0.5), A.hills), mul(k, 0.6));
};
const hillsJs = (x: number, z: number) => {
  const d = Math.abs(x - S.fort.at[0]) + Math.abs(z - S.fort.at[1]);
  const t = Math.min(Math.max(d / 2.6, 0), 1), k = 1 - t * t * (3 - 2 * t);
  return (Math.sin(0.9 * x + 0.3) * Math.sin(0.8 * z)) * 0.5 * A.hills + 0.6 * k;
};
// Profile from the crown (u = 0) out over the plateau, over the lip, down the cliff and tucked under (u = 1).
// The cliff (u 0.56 to 0.88) is even in u, so paint over u is even down its face.
const rhoOf = keys([[0, 0], [0.45, 0.82], [0.52, 0.93], [0.56, 0.97], [0.88, 1.03], [1, 0.0]]);
const dropOf = keys([[0, 0], [0.45, 0], [0.52, 0.05], [0.56, 0.14], [0.88, 1.0], [1, 1.05]]); // share of top - base
const atollPoint = (u: F, v: F): P3 => {
  const a = mul(v, -2 * Math.PI);
  const r = mul(rhoOf(u).x, lumpsOf(a));
  const x = add(A.centre[0], mul(mul(cos(a), r), A.rx)), z = add(A.centre[1], mul(mul(sin(a), r), A.rz));
  // Hills only on the plateau: they fade out over the lip so the cliff top stays a clean line.
  const y = add(sub(A.top, mul(dropOf(u).x, A.top - A.base)), mul(hillsOf(x, z), sub(1, smoothstep(0.4, 0.55, u))));
  return vec3(x, y, z);
};
const atollPaint = (u: F, v: F): P3 => {
  // Moss on the plateau with deep-moss tufts (lines of a noise field); it spills unevenly over the lip.
  // (Patterns here and on the copies are sums of sines, not noise: noise per pixel was most of these draws' cost.)
  const turn = mul(v, 2 * Math.PI);
  const spill = add(0.56, add(mul(sin(mul(turn, 7)), 0.03), mul(sin(add(mul(turn, 17), 1)), 0.02)));
  const qx = mul(mul(u, cos(turn)), 7), qz = mul(mul(u, sin(turn)), 7);
  const contour = add(sin(add(qx, mul(sin(mul(qz, 0.8)), 1.7))), sin(add(mul(qz, 1.2), mul(sin(mul(qx, 0.7)), 1.3))));
  const tuft = sub(1, cut(abs(contour), 0.105));
  const moss = mix(vec3(...I.moss), vec3(...I.mossDeep), tuft);
  // Stone cliffs cut by vertical strokes of varying weight; a dark band where the cliff meets its foot.
  const strokeAt = abs(sub(fract(add(mul(v, 70), mul(sin(add(mul(turn, 5), mul(u, 7))), 0.3))), 0.5));
  const stroke = mul(sub(1, cut(strokeAt, 0.075)), cut(u, 0.58));
  const cliff = mix(vec3(...I.stone), vec3(...I.bark), stroke);
  const foot = mix(cliff, vec3(...I.bark), cut(add(u, mul(sin(add(mul(turn, 11), 0.5)), 0.025)), 0.77));
  return mix(moss, foot, cut(u, spill));
};
const atoll = draw.surface("atoll", framed(surface(atollPoint, { u: ["cap", "cap"], v: "periodic" }), { frame: body, scale: lift<"f32">(1) }), {
  paint: atollPaint, grid: { u: spacing.even(48), v: spacing.even(96) }, sides: "one", outline: "hull",
});

/** A point on the atoll's rim at angle a (radians), just inside the cliff top: where coral grows. */
const rimAt = (a: number, inset: number): V3 => {
  const r = 0.93 * lumpsJs(a) * inset;
  return [A.centre[0] + Math.cos(a) * r * A.rx, A.top - 0.12 * (A.top - A.base) + 0.05, A.centre[1] + Math.sin(a) * r * A.rz];
};
/** A point at the cliff's foot at angle a, a little above the waterline. */
const footAt = (a: number): V3 => {
  const r = 1.0 * lumpsJs(a);
  return [A.centre[0] + Math.cos(a) * r * A.rx, A.base + 0.18 * (A.top - A.base), A.centre[1] + Math.sin(a) * r * A.rz];
};
/** A point of the plateau at local (x, z). */
const plateauAt = (x: number, z: number): V3 => [x, A.top + hillsJs(x, z), z];

// ==== coral, trees and the fort's rubble: one list of rounded cones on the crab's frame ==============================

type Kind = 0 | 1 | 2 | 3; // coral, trunk, crown, stone: the id's first lane, read by the paint
const hash = (n: number) => { const x = Math.sin(n * 127.1 + 311.7) * 43758.5453; return x - Math.floor(x); };
const cone = (kind: Kind, a: V3, b: V3, ra: number, rb: number, level: number, salt: number): Branch =>
  ({ a, b, ra, rb, level, id: [kind, salt, 0] });

/** A coral head: a clump of short thick fingers leaning out from one point, pink with cream tips. */
function coralHead(at: V3, size: number, turn: number, salt: number): Branch[] {
  const n = 5 + Math.floor(3 * hash(salt + 11));
  return Array.from({ length: n }, (_, j) => {
    const a = turn + (j / n) * 2 * Math.PI, tilt = j === 0 ? 0 : 0.5 + 0.35 * hash(salt * 13 + j);
    const len = size * (0.45 + 0.35 * hash(salt * 7 + j)) * (j === 0 ? 1.25 : 1);
    const dir: V3 = [Math.sin(tilt) * Math.cos(a), Math.cos(tilt), Math.sin(tilt) * Math.sin(a)];
    const b: V3 = [at[0] + dir[0] * len, at[1] + dir[1] * len, at[2] + dir[2] * len];
    return cone(0, at, b, 0.2 * size, 0.13 * size, 1, salt);
  });
}
// Half the heads on the lip of the cliff, half clinging at its foot by the water.
const corals = Array.from({ length: S.coralHeads }, (_, k) => {
  const a = (k / S.coralHeads) * 2 * Math.PI + 0.3 * hash(k);
  const atFoot = k % 2 === 1;
  const at = atFoot ? footAt(a) : rimAt(a, 0.97);
  return coralHead(at, (atFoot ? 0.9 : 0.6) + 0.4 * hash(k + 7), 6 * hash(k + 3), k);
}).flat();

/** A broadleaf tree: a leaning trunk and three overlapping crown blobs. */
function tree(x: number, z: number, h: number, salt: number): Branch[] {
  const foot = plateauAt(x, z), lean = [0.25 * (hash(salt) - 0.5), 0, 0.25 * (hash(salt + 1) - 0.5)];
  const top: V3 = [foot[0] + lean[0]! * h, foot[1] + h, foot[2] + lean[2]! * h];
  const blob = (dx: number, dy: number, dz: number, r: number, j: number) => cone(2, [top[0] + dx, top[1] + dy, top[2] + dz], [top[0] + dx, top[1] + dy + 0.01, top[2] + dz], r, r, 0, salt * 3 + j);
  return [
    cone(1, [foot[0], foot[1] - 0.3, foot[2]], top, 0.16, 0.1, 0, salt),
    blob(0, 0.35, 0, 0.75 * h * 0.45, 0), blob(0.45, 0.05, 0.3, 0.55 * h * 0.45, 1), blob(-0.4, 0.1, -0.35, 0.6 * h * 0.45, 2),
  ];
}
const TREES: readonly (readonly [number, number, number])[] = [[0.6, 2.2, 1.9], [1.4, -1.6, 1.6], [-0.4, 3.6, 1.5], [-3.6, 2.4, 2.0], [-4.3, -2.6, 1.7], [-0.6, -3.4, 1.8]];
const trees = TREES.slice(0, S.trees).flatMap(([x, z, h], k) => tree(x, z, h, 40 + k));

// The fort: rubble round the tower's foot, a broken curtain wall to one side, merlons of unequal height on the tower.
const F0 = S.fort, fortFoot = plateauAt(F0.at[0], F0.at[1]);
const fortTop: V3 = [fortFoot[0] + F0.lean * F0.height, fortFoot[1] + F0.height, fortFoot[2] - F0.lean * 0.5 * F0.height];
const stones: Branch[] = [
  ...Array.from({ length: 5 }, (_, k) => {
    const a = (k / 5) * 2 * Math.PI + 0.4, r = F0.girth * 0.82, hgt = 0.25 + 0.5 * hash(k + 90);
    const p: V3 = [fortTop[0] + Math.cos(a) * r, fortTop[1] - 0.15, fortTop[2] + Math.sin(a) * r];
    return k === 3 ? null : cone(3, p, [p[0], p[1] + hgt, p[2]], 0.2, 0.18, 0, 90 + k);
  }).filter((b): b is Branch => b !== null),
  // The curtain wall, broken off: two low courses running out to a corner stub.
  cone(3, [fortFoot[0] + 0.6, fortFoot[1] + 0.3, fortFoot[2] + 0.4], [fortFoot[0] + 2.1, plateauAt(fortFoot[0] + 2.1, fortFoot[2] + 1.0)[1] + 0.35, fortFoot[2] + 1.0], 0.22, 0.2, 0, 100),
  cone(3, [fortFoot[0] + 2.1, plateauAt(fortFoot[0] + 2.1, fortFoot[2] + 1.0)[1], fortFoot[2] + 1.0], [fortFoot[0] + 2.15, plateauAt(fortFoot[0] + 2.1, fortFoot[2] + 1.0)[1] + 0.9, fortFoot[2] + 1.0], 0.3, 0.26, 0, 101),
  cone(3, [fortFoot[0] - 0.4, fortFoot[1] + 0.3, fortFoot[2] + 0.75], [fortFoot[0] - 1.6, plateauAt(fortFoot[0] - 1.6, fortFoot[2] + 1.4)[1] + 0.25, fortFoot[2] + 1.4], 0.2, 0.18, 0, 102),
  // Rubble.
  ...Array.from({ length: 7 }, (_, k) => {
    const a = 2 * Math.PI * hash(k + 110), r = F0.girth + 0.3 + 0.8 * hash(k + 120);
    const x = fortFoot[0] + Math.cos(a) * r, z = fortFoot[2] + Math.sin(a) * r, y = plateauAt(x, z)[1] + 0.05;
    const s = 0.14 + 0.12 * hash(k + 130);
    return cone(3, [x, y, z], [x + s, y + 0.05, z], s, s * 0.8, 0, 110 + k);
  }),
];

const reefList = draw.copies("reef", {
  kind: "cone",
  copies: [...corals, ...trees, ...stones],
  carry: body,
  // Flat inks with pattern lines: coral cream with brick tips; bark; crowns deep moss with curling lines; stone cream
  // with courses. p is in the crab's coordinates, so the lines ride it.
  paint: ({ p, level, id }) => {
    const specks = mul(mul(sin(mul(p.x, 9)), sin(add(mul(p.y, 9), 1))), sin(add(mul(p.z, 9), 2)));
    const coral = mix(vec3(...I.coral), vec3(...I.cream), cut(specks, 0.33));
    const swirl = add(sin(add(mul(p.x, 2.5), mul(sin(mul(p.y, 3)), 1.5))), sin(add(mul(p.z, 2.5), mul(sin(mul(p.x, 2.2)), 1.5))));
    const curl = sub(1, cut(abs(sin(mul(swirl, 2.2))), 0.12));
    const crownLined = mix(vec3(...I.mossDeep), vec3(...I.black), mul(curl, step(0.5, hash3(id))));
    const course = sub(1, cut(abs(sub(fract(mul(p.y, 3.2)), 0.5)), 0.045));
    const stone = mix(vec3(...I.cream), vec3(...I.bark), course);
    const k = id.x;
    return mix(mix(mix(coral, vec3(...I.bark), step(0.5, k)), crownLined, step(1.5, k)), stone, step(2.5, k));
  },
  outline: "silhouette", minPixels: 1, keepLevels: 2, inkBack: 6,
});

// The tower: a squarish, leaning, roofless stump (an open top shows its dark inside), stone courses.
const tower = place(strand({
  name: "fortTower", spine: (m) => mix(vec3(fortFoot[0], fortFoot[1] - 0.4, fortFoot[2]), vec3(...fortTop), m), stations: 4,
  girth: (m) => mix(F0.girth, F0.girth * 0.85, m), section: { squareness: 4 }, tips: [{ round: 0.05 }, "open"], up: [1, 0, 0],
  paint: (m, v) => {
    const course = sub(1, cut(abs(sub(fract(mul(m, 9)), 0.5)), 0.05));
    const joint = mul(sub(1, cut(abs(sub(fract(add(mul(v, 12), mul(floor(mul(m, 9)), 0.5))), 0.5)), 0.045)), 0.8);
    const slit = mul(sub(1, cut(abs(sub(fract(mul(v, 4)), 0.125)), 0.015)), mul(cut(m, 0.575), sub(1, cut(m, 0.795))));
    return mix(mix(vec3(...I.cream), vec3(...I.stone), max(course, joint)), vec3(...I.black), slit);
  },
  detail: { along: 12, around: 24 },
}), rigid(body));

// ==== the sea, the ship, the warning =================================================================================

const water = sea({
  swell: [{ direction: [1, 0.3], length: 13, height: 0.2, speed: 0.9 }, { direction: [-0.3, 1], length: 6, height: 0.07, speed: 0.7, phase: 1.3 }],
  foam: 0.85,
});
// A small brig coming on toward the cameras with the wind behind it, so its square sails face them.
const WIND: readonly [number, number] = [0.35, 1.0];
const boat = ship(brig("brig", S.shipLength, PLAYER)).afloat(water, { at: S.shipAt, heading: S.shipHeading, wind: WIND });
const warning = telegraph(crush, { style: { fill: I.brick, bone: I.cream, ink: I.black, flash: I.cream, spacing: 1.2, edge: 0.22, line: 0.09, fillTo: 0.88 } });

/**
 * The crash: a ring of clawed cream foam running out from the disc's centre after the slam and breaking up as it
 * goes (fingers along the zero lines of a noise field, as the woodblock's contact foam), with an indigo line inside it.
 */
const crashSince = sub(time, crushArm);
const crashK = smoothstep(0, S.crashSeconds, crashSince);
const crashOuter = S.crushRadius * S.crashReach;
const crash = draw.glazed("crash", surface((u: F, v: F) => {
  const a = mul(v, 2 * Math.PI), r = mul(u, crashOuter + 0.5);
  const x = add(S.crushCentre[0], mul(cos(a), r)), z = add(S.crushCentre[1], mul(sin(a), r));
  return vec3(x, water.height(vec2(x, z)), z);
}, { u: ["cap", "free"], v: "periodic" }), {
  colour: (fr) => {
    const d = vec2(sub(fr.world.x, S.crushCentre[0]), sub(fr.world.z, S.crushCentre[1]));
    // The front runs out fast and slows (a square root of time), thinning as it goes.
    const front = mul(mix(0.25, 1, sqrt(crashK)), crashOuter);
    const band = sub(1, smoothstep(0.0, mix(1.1, 0.35, crashK), abs(sub(length(d), front))));
    const fingers = abs(noise2(mul(vec2(fr.world.x, fr.world.z), 1.3)));
    const foam = edgeOf(sub(band, mul(fingers, 1.6)), 0.15, 0.004);
    const line = mul(edgeOf(sub(band, mul(fingers, 1.6)), -0.05, 0.004), sub(1, foam));
    const live = mul(step(0, crashSince), sub(1, smoothstep(0.7, 1, crashK)));
    return vec4(mix(rgb(I.indigo), rgb(I.cream), foam), mul(max(foam, mul(line, 0.8)), live));
  },
  grid: { u: spacing.even(12), v: spacing.even(48) }, phase: "on-water", sceneDepth: false,
});

export default scene({
  sea: water,
  things: [carapace, atoll, reefList, tower, ...face, ...legs, ...claws, warning, crash, ...boat],
  style: titanWoodblock(),
  sun: [0.15, 0.8, 0.6],
  cameras: {
    game: { kind: "perspective", eye: [23, 27, 21], target: [3, 0.5, 2.5], fovY: 0.6 },
    three: { kind: "perspective", eye: [27, 12, 9], target: [1.5, 2, 5.5], fovY: 0.6 },
    // The face, from the front and above (the claws, folded at rest, cover the mouth from lower down), the fort behind.
    close: { kind: "perspective", eye: [15.5, 9, 3], target: [4.2, 1.4, 0.2], fovY: 0.55 },
    top: { kind: "orthographic", eye: [1, 50, 0.001], target: [1, 0, 1], height: 34 },
  },
});

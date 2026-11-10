// A sea serpent as data: named numbers, curves, and closed-form motion. Every part (head, jaw, horns, eyes, fins) is
// written in terms of the body's spine, so it moves with the body; foam collars and shadows are derived by the
// compiler. Head toward +x. Ported from keel's preview_probe `serpent.rs`.

import {
  add, crest, cross, dot, frameOf, keys, mul, normalize, palette, param, retarded, rotate, scene, sea, sin, skinPoint,
  smoothstep, strand, sub, TAU, through, time, toon, vec3, wave, abs, cos, fract, mix, type Expr, type In,
} from "../src/index.ts";

const S = {
  // body
  length: 15, girth: 0.64, neckGirth: 0.66, tailGirth: 0.4, depth: 1.12, squareness: 2.3,
  // posture
  ride: -0.85, neckRise: 3.7, neckReach: 1.7, neckArch: 0.95, humps: 2.5, humpHeight: 1.65, tailLift: 1.2,
  // swimming
  coils: 1.6, coilWidth: 1.5, swimRate: 0.42, headSteady: 0.12, humpRate: 0.3, followSpeed: 1.5, headScan: 0.7,
  // head
  headLength: 2.2, headWidth: 1.3, headHeight: 0.95, snout: 0.45, headPitch: 0.24, jawLength: 1.5, jawGape: 0.22, teeth: 6, toothLength: 0.2,
  eyeSize: 0.13, hornLength: 1.7, hornCurl: 1.9, hornSplay: 0.7, hornGirth: 0.17,
  // crest
  crestHeight: 1.25, crestSpan: [0.05, 0.6] as const, crestRake: 0.55, crestRays: 13, crestSag: 0.36,
  // colour: saturated, so the creature reads against the water
  back: [0.34, 0.17, 0.62], belly: [1.0, 0.88, 0.6], band: [0.6, 0.48, 1.0], membrane: [1.0, 0.33, 0.4],
  bone: [1.0, 0.95, 0.82], iris: [1.0, 0.78, 0.12], mouth: [0.86, 0.2, 0.36],
} as const;

const NECK = 6, BODY = 16, POINTS = NECK + BODY - 1;
/** Shift so the body sits around the origin. */
const CENTRE = S.length * 0.42;
const neckEnd = NECK / POINTS;
const ramp01 = (x: number, a: number, b: number) => { const t = Math.min(1, Math.max(0, (x - a) / (b - a))); return t * t * (3 - 2 * t); };

const rise = param("serpent.neckRise", S.neckRise, { min: 0.5, max: 7 });
const humpHeight = param("serpent.humpHeight", S.humpHeight, { min: 0, max: 3 });

/** The posture at time t: an S-neck rising from the sea, then a body whose coils and humps travel to the tail. */
function controlPoint(i: number): Expr<"vec3"> {
  // Head motion (scan side to side, nod) runs down the neck on a retarded clock: the head leads, the neck follows.
  const along = i / POINTS;
  const neck = 1 - ramp01(along, 0.02, 0.34);
  const clock = retarded(time, along, S.followSpeed);
  const scan = mul(add(sin(mul(clock, 0.55)), mul(sin(add(mul(clock, 1.27), 1.3)), 0.45)), S.headScan * neck);
  const nod = mul(sin(mul(clock, 0.8)), mul(rise, 0.04 * neck));
  if (i < NECK) {
    const q = 1 - i / NECK;
    const sway0 = wave(0, { axis: [0, 0, 1], waves: S.coils, rate: S.swimRate, amplitude: S.coilWidth * S.headSteady * (1 - q) * (1 - q) });
    const y = add(add(S.ride, mul(rise, Math.sin((q * Math.PI) / 2))), nod);
    return add(add(vec3(S.neckReach * q - S.neckArch * Math.sin(Math.PI * q) + CENTRE, y, 0), sway0), vec3(0, 0, scan));
  }
  const p = (i - NECK) / (BODY - 1);
  const hump = sin(mul(sub(S.humps * p, mul(time, S.humpRate)), Math.PI));
  const lift = mul(mul(hump, hump), mul(humpHeight, ramp01(p, 0, 0.14) * (1 - 0.3 * p)));
  const y = add(S.ride + S.tailLift * ramp01(p, 0.78, 1), lift);
  const sway = wave(p, { axis: [0, 0, 1], waves: S.coils, rate: S.swimRate, amplitude: S.coilWidth, grow: S.headSteady + (1 - S.headSteady) * p ** 1.3 });
  return add(add(vec3(-S.length * p + CENTRE, y, 0), sway), vec3(0, 0, scan));
}
const points = Array.from({ length: NECK + BODY }, (_, i) => controlPoint(i));

const g = S.girth;
const girth = keys([[0, S.neckGirth * 0.9], [neckEnd * 0.75, S.neckGirth], [neckEnd + 0.07, 1], [0.55, 0.9], [0.86, S.tailGirth], [1, S.tailGirth * 0.45]]);
const girthScale = param("serpent.girth", g, { min: 0.2, max: 1.6 });

const body = strand({
  name: "serpent",
  spine: (m) => through(m, points),
  stations: 48,
  girth: (m) => mul(girth(m).x, girthScale),
  section: { aspect: S.depth, squareness: S.squareness },
  tips: [{ round: 0.012 }, { point: 0.045 }],
  level: 1.2,
  paint: (m, v) => palette.bands(m, v, palette.countershade(v, { back: S.back, belly: S.belly }), { colour: S.band, count: 9, width: 0.5, from: neckEnd * 0.5, to: 0.97 }),
  detail: { along: 160, around: 28 },
});

// ---- head: a skull riding the neck's first frame, a hinged jaw, horns, eyes ---------------------------------------

const neckTop = frameOf(body, 0);
const forward = rotate(mul(neckTop.t, -1), neckTop.s, S.headPitch);
const crown = rotate(neckTop.d, neckTop.s, S.headPitch);
const hw = S.headWidth / 2, hh = S.headHeight / 2, hl = S.headLength;
const backOfHead = add(neckTop.p, mul(neckTop.t, 0.12 * hl));
const skullPoints = [0, 0.35, 0.7, 1].map((u) => add(backOfHead, sub(mul(forward, hl * u), mul(crown, 0.06 * hl * u * u))));

/** Width keys and height keys at the same positions become a girth curve and an aspect curve. */
function section(at: readonly number[], width: readonly number[], height: readonly number[]) {
  return { girth: keys(at.map((x, i) => [x, width[i]] as const)), aspect: keys(at.map((x, i) => [x, height[i] / width[i]] as const)) };
}
const skull = section([0, 0.3, 0.62, 1], [hw * 0.8, hw, hw * 0.78, hw * S.snout], [hh * 0.85, hh, hh * 0.72, hh * S.snout * 0.8]);
const head = strand({
  name: "head",
  spine: (m) => through(m, skullPoints),
  stations: 12,
  girth: (m) => skull.girth(m).x,
  section: { aspect: (m) => skull.aspect(m).x, squareness: 2.8 },
  tips: [{ round: 0.22 }, { round: 0.1 }],
  up: crown,
  // The palate (underside, forward of the throat) is mouth-coloured: it shows when the jaw opens.
  paint: (m, v) => mix(
    palette.countershade(v, { back: mix(vec3(...S.back), vec3(...S.band), smoothstep(0.55, 1.0, m)), belly: S.belly }),
    vec3(...S.mouth), mul(smoothstep(0.8, 0.9, mul(sin(mul(v, TAU)), -1)), smoothstep(0.3, 0.4, m))),
  detail: { along: 48, around: 28 },
});

const gape = mul(add(1, mul(sin(mul(time, 0.9)), 0.4)), S.jawGape);
const hinge = sub(through(0.25, skullPoints), mul(crown, hh * 0.9));
const jawForward = rotate(forward, neckTop.s, gape);
const jawCrown = rotate(crown, neckTop.s, gape);
const chin = section([0, 0.45, 1], [hw * 0.7, hw * 0.72, hw * S.snout * 0.95], [hh * 0.3, hh * 0.26, hh * 0.16]);
const jaw = strand({
  name: "jaw",
  spine: (m) => through(m, [sub(hinge, mul(jawForward, 0.15 * S.jawLength)), add(hinge, mul(jawForward, 0.5 * S.jawLength)), add(hinge, mul(jawForward, S.jawLength))]),
  stations: 8,
  girth: (m) => chin.girth(m).x,
  section: { aspect: (m) => chin.aspect(m).x, squareness: 2.6 },
  tips: [{ round: 0.15 }, { round: 0.12 }],
  up: jawCrown,
  // The top of the jaw is the floor of the mouth.
  paint: (_m, v) => mix(palette.countershade(v, { back: S.back, belly: S.belly }), vec3(...S.mouth), smoothstep(0.75, 0.85, sin(mul(v, TAU)))),
  detail: { along: 32, around: 24 },
});

/** A horn: a strand on a logarithmic-spiral spine leaving `base` along `out` and curling toward `back`. */
function horn(name: string, base: Expr<"vec3">, out: Expr<"vec3">, backward: Expr<"vec3">) {
  const TIGHTEN = 0.45, N = 7, curl = S.hornCurl;
  const o = normalize(out);
  const b = normalize(sub(backward, mul(o, dot(backward, o))));
  const norm = Math.hypot(1, TIGHTEN);
  const e1 = [-TIGHTEN / norm, 1 / norm], e2 = [-1 / norm, -TIGHTEN / norm];
  const scale = S.hornLength / ((norm / TIGHTEN) * (1 - Math.exp(-TIGHTEN * curl)));
  const pts = Array.from({ length: N }, (_, i) => {
    const th = (curl * i) / (N - 1), r = Math.exp(-TIGHTEN * th);
    const d = [r * Math.cos(th) - 1, r * Math.sin(th)];
    const alongOut = d[0] * e1[0] + d[1] * e1[1], toward = d[0] * e2[0] + d[1] * e2[1];
    return i === 0 ? sub(base, mul(o, S.hornGirth * 0.6)) : add(base, add(mul(o, alongOut * scale), mul(b, toward * scale)));
  });
  const profile = keys([[0, S.hornGirth], [0.45, S.hornGirth * 0.62], [1, S.hornGirth * 0.08]]);
  return strand({
    name, spine: (m) => through(m, pts), stations: 12, girth: (m) => profile(m).x, tips: ["open", { point: 0.2 }],
    up: cross(o, b),
    paint: (m) => mix(vec3(...S.bone), mul(vec3(...S.bone), 0.62), smoothstep(0.2, 1.0, m)),
    detail: { along: 24, around: 12 },
  });
}

/** An eye: a two-point strand with round tips is a ball. Amber, with a slit pupil facing out. */
function eye(name: string, socket: { pos: Expr<"vec3">; normal: Expr<"vec3"> }) {
  const r = S.eyeSize, c = add(socket.pos, mul(socket.normal, r * 0.25));
  return strand({
    name, spine: (m) => through(m, [sub(c, mul(socket.normal, r)), add(c, mul(socket.normal, r))]), stations: 4,
    girth: r, tips: [{ round: 0.5 }, { round: 0.5 }], up: crown,
    paint: (m, v) => {
      const iris = mix(mul(vec3(...S.iris), 0.7), vec3(...S.iris), smoothstep(0.55, 0.8, m));
      const slit = mul(smoothstep(0.8, 0.86, m), sub(1, smoothstep(0.25, 0.4, abs(cos(mul(v, TAU))))));
      return mix(iris, vec3(0.04, 0.02, 0.06), slit);
    },
    detail: { along: 16, around: 16 },
  });
}

const parts = [-1, 1].flatMap((side) => {
  const tag = side < 0 ? "L" : "R";
  const root = skinPoint(head, 0.14, 0.25 + (side * S.hornSplay) / TAU);
  const socket = skinPoint(head, 0.47, 0.25 + side * (0.25 - 0.8 / TAU));
  return [
    horn(`horn${tag}`, root.pos, add(root.normal, mul(crown, 0.3)), sub(mul(crown, 0.2), forward)),
    eye(`eye${tag}`, socket),
  ];
});

// ---- fins ---------------------------------------------------------------------------------------------------------

const crestHeight = param("serpent.crestHeight", S.crestHeight, { min: 0, max: 3 });
const crestProfile = keys([[0, 0], [0.07, 1], [0.35, 0.85], [0.75, 0.6], [1, 0]]);
const dorsal = crest({
  on: body, from: S.crestSpan[0], to: S.crestSpan[1], name: "dorsal",
  height: (x: In<"f32">) => mul(crestProfile(x), crestHeight),
  paint: S.membrane, rays: S.crestRays, sag: S.crestSag, rake: S.crestRake,
  // The crest sways on a wave running down the body, twice the swimming rate.
  flutter: (m) => mul(sin(mul(sub(mul(m, 2.5), mul(time, S.swimRate * 2)), TAU)), 0.3),
  rows: 120,
});
const tailFin = crest({
  on: body, from: 0.84, to: 0.995, name: "tailfin",
  height: keys([[0, 0], [0.45, 0.7], [0.85, 1.5], [1, 0]]),
  paint: S.membrane, rays: 6, sag: 0.25, rake: 0.9, rows: 48,
});

/** Teeth: a sawtooth membrane along the mouth's edge, one row per side per jaw. */
const tooth = (x: Expr<"f32">, grow: number) => mul(sub(1, abs(sub(mul(fract(mul(x, S.teeth)), 2), 1))), mul(add(0.75, mul(x, grow)), S.toothLength));
const teeth = [-1, 1].flatMap((side) => [
  crest({ on: head, name: `teethUp${side < 0 ? "L" : "R"}`, from: 0.42, to: 0.9, v: 0.75 + side * 0.1, height: (x: Expr<"f32">) => tooth(x, 0.5), paint: () => vec3(...S.bone), sag: 0, rays: 1, rake: 0, rows: 48 }),
  crest({ on: jaw, name: `teethDown${side < 0 ? "L" : "R"}`, from: 0.3, to: 0.86, v: 0.25 + side * 0.12, height: (x: Expr<"f32">) => tooth(x, 0.3), paint: () => vec3(...S.bone), sag: 0, rays: 1, rake: 0, rows: 48 }),
]);

export default scene({
  sea: sea({
    swell: [
      { direction: [1, 0.3], length: 14, height: 0.35, speed: 1.1 },
      { direction: [-0.3, 1], length: 6.5, height: 0.12, speed: 0.8, phase: 1.3 },
    ],
  }),
  things: [body, head, jaw, ...parts, ...teeth, dorsal, tailFin],
  style: toon({ bands: 3, outline: 0.04 }),
  sun: [-0.45, 0.75, 0.5],
  cameras: {
    front: { kind: "perspective", eye: [21, 5, 5], target: [3, 1.6, 0], fovY: 0.6 },
    side: { kind: "perspective", eye: [-0.5, 4, 28], target: [-0.5, 1.2, 0], fovY: 0.6 },
    top: { kind: "orthographic", eye: [-0.5, 40, 0.001], target: [-0.5, 0, 0], height: 19 },
    three: { kind: "perspective", eye: [14, 8.5, 16], target: [0, 1, 0], fovY: 0.6 },
  },
});

// The Maelstrom Wyrm, as a woodblock print. An ancient sea serpent coiled round the whirlpool it keeps spinning: only
// its four arched coils and its horned, gilled head break the surface; between them the water turns and drains. Each
// arch carries a pearl gland (one pearl in a vermilion socket). Subject: the game's boss (room-runners-arena-lib
// notes/design/2026-11-01_fleet_spells_bosses.md, 6.1); look: docs/art_direction/DIRECTION.md, row A.
//
// The body is one strand whose spine is written in world coordinates in closed form: a spiral from the head at the
// rim inward to the tail diving into the throat, its height a sum of four arches over a line below the water. At rest
// the arches heave in sequence, the whole coil slithers along its own path, the head sways and breathes and the gills
// flare with the breath. Every rest motion repeats in LOOP seconds.
//
// The signature attack, Coil Lash: one arch rears into a tall loop, leaning back, then topples outward and slams down
// flat across a capsule on the water (src/spacetime/rear.ts). It is an `attack` like any other: the warning is the
// capsule drawn on the water (`telegraph`), the pose is derived from the attack's envelopes, and after the slam that
// arch's pearl gland swells open (the game's exposure window). Here the attack loops on time (it arms LASH.at seconds
// into every loop); a game sets `lash.at` and `lash.warn` instead.
//
// The style is woodblock with its body inks retuned for this monster (DIRECTION.md: slate and olive coils, vermilion
// gills and pearl glands): every paint below is one of the keys in INKS, and each key prints in two inks, shadow and
// lit. The sea, the key line and the paper are woodblock's own.

import {
  abs, add, clamp, cos, crest, distance, div, dot, draw, floor, fract, horn, input, keys, lift, lt, max, min, mix, mod, mul,
  on, place, points, pow, rotateAbout as rotate, scene, sea, select, ship, sin, smoothstep, step, strand, sub, TAU, through, time, vec2, vec3,
  vec4, whirlpool, length, woodblock, type Expr, type F, type In, type P3, type ShadeCtx, type Style,
} from "../../src/index.ts";
import { animate, attack, hit, part, telegraph } from "../../src/spacetime/index.ts";
import { landsInside } from "../../src/check.ts";
import { edgeOf } from "../../src/style/shared.ts";
import { sloop } from "../scenes/lib/fleet.ts";

type Rgb = readonly [number, number, number];

// ==== the numbers an art director changes ===========================================================================

/** Seconds: every rest motion, and the attack, repeats in this time. */
const LOOP = 6;

const POOL = {
  radius: 15,          // whirlpool radius (the coil keeps inside it)
  spin: 0.94,          // radians per second at the centre (0.15 turns per second, as in the game)
  depth: 3.2,          // funnel depth at the centre
  drift: 0.6,          // inward speed at half the radius
  throat: 0.26,        // throat radius as a share of the radius
  arms: 4,             // foam arms
};

const BODY = {
  radius: 1.15,        // half-thickness of the coil
  headRadius: 13.4,    // the spiral starts here (the neck, at the rim)...
  tailRadius: 2.0,     // ...and winds in to here, where the tail dives into the throat
  turns: 1.3,          // turns of the spiral from neck to tail
  start: -1.5,         // bearing of the neck from the centre, radians from +x toward +z
  dip: 1.7,            // how far below the water the body runs between arches
  neckEnd: 0.16,       // the neck rises out of the water from this m up to the head at m = 0
  headHeight: 6.2,     // height of the head above the water
  headIn: 2.6,         // how far the head leans in over the pool from the spiral
  slither: 0.05,       // radians the coil slides along its own path over the loop
  heave: 0.14,         // share each arch rises and falls, in sequence round the coil
  sway: 0.5,           // world units the head sways
};

/** The four arches: centre on the body (m), half-width (m) and apex height above the water. */
const ARCHES = [
  { at: 0.25, half: 0.062, height: 3.9 },
  { at: 0.43, half: 0.064, height: 4.3 },
  { at: 0.61, half: 0.064, height: 3.7 },
  { at: 0.79, half: 0.066, height: 2.9 },
] as const;

const HEAD = {
  length: 4.6,         // skull length
  width: 1.35,         // skull half-width at its widest
  pitch: 0.08,         // nose-down tilt, radians
  gape: 0.22,          // jaw opening at rest, radians; it breathes by `breath`
  breath: 0.1,
  horn: 2.6,           // horn length
  hornCurl: 1.5,
  gill: 2.6,           // gill frill height
};

/** Coil Lash: which arch, where it falls, and its timing. */
const LASH = {
  arch: 2,             // index into ARCHES
  at: 3.2,             // seconds into each loop when it lands (arms)
  warn: 1.7,           // seconds of warning on the water before
  bearing: 0.2,        // radians the lane turns from straight out of the pool
  length: 7.8,         // capsule length (a to b)
  radius: 4.0,         // capsule radius (the loop's half-width is this less the body's radius: keep it open)
  lean: 0.45,          // radians the reared loop leans back before it falls
  height: 0.45,        // height above the water it lands at
  spread: 0.1,         // body m on each side of the arch's centre that rears
};

/** Inks: each paint key prints in two inks (in shadow, lit). Paints elsewhere in the file are exactly these keys. */
const INKS = {
  black: { key: [0.05, 0.05, 0.06], shadow: [0.075, 0.07, 0.09], lit: [0.075, 0.07, 0.09] },
  slate: { key: [0.3, 0.33, 0.38], shadow: [0.17, 0.19, 0.24], lit: [0.4, 0.45, 0.5] },
  olive: { key: [0.4, 0.4, 0.2], shadow: [0.24, 0.26, 0.14], lit: [0.5, 0.51, 0.29] },
  cream: { key: [0.92, 0.87, 0.74], shadow: [0.7, 0.66, 0.55], lit: [0.93, 0.88, 0.75] },
  vermilion: { key: [0.85, 0.22, 0.12], shadow: [0.6, 0.15, 0.1], lit: [0.82, 0.25, 0.16] },
  pearl: { key: [0.97, 0.97, 0.95], shadow: [0.66, 0.72, 0.78], lit: [0.98, 0.96, 0.9] },
  wood: { key: [0.45, 0.28, 0.14], shadow: [0.27, 0.16, 0.09], lit: [0.5, 0.33, 0.18] },
  sail: { key: [0.2, 0.4, 0.9], shadow: [0.14, 0.26, 0.6], lit: [0.26, 0.46, 0.86] },
} as const satisfies Record<string, { key: Rgb; shadow: Rgb; lit: Rgb }>;

/** How dark the funnel's shading must be (the whirlpool's crest shade, negative) before it prints as the black eye. */
const SEA_EYE = 1.5;

/** The scale ship: a small sloop with blue sails, at the rim beyond where the lash lands. */
const SHIP = { length: 3.8, beyond: 4.2, along: -1.4 };

// ==== the style: woodblock, with this monster's inks ================================================================

/** An index expression equals a whole number. */
const is = (i: F, k: number) => lt(abs(sub(i, k)), 0.5);
const K = (k: keyof typeof INKS) => INKS[k].key;
const rgb = (c: Rgb) => vec3(c[0], c[1], c[2]);

/** The nearest key's ink in shadow or lit (two tones of light, as woodblock's own shade). */
function printed(ctx: ShadeCtx): Expr<"vec3"> {
  const lit = edgeOf(dot(ctx.normal, ctx.sun), 0.0, 1e-5);
  const inks = Object.values(INKS);
  let best: F = distance(ctx.base, rgb(inks[0]!.key));
  let colour: Expr<"vec3"> = mix(rgb(inks[0]!.shadow), rgb(inks[0]!.lit), lit);
  for (const ink of inks.slice(1)) {
    const d = distance(ctx.base, rgb(ink.key));
    colour = select(lt(d, best), mix(rgb(ink.shadow), rgb(ink.lit), lit), colour);
    best = min(best, d);
  }
  return colour;
}

/**
 * Woodblock's water, plus what it leaves out: the whirlpool's foam arms (`ctx.foam` carries them; woodblock prints only
 * its own clawed contact foam) printed in cream, and the eye of the pool printed near black.
 */
function wyrmPrint(): Style {
  const base = woodblock();
  return {
    ...base,
    shade: printed,
    water: (ctx) => {
      const c = base.water(ctx);
      const arms = edgeOf(ctx.foam, 0.45, 0.02);
      const eye = edgeOf(mul(ctx.crest, -1), SEA_EYE, 0.02);
      const inked = mix(mix(c.xyz, rgb(INKS.cream.lit), arms), rgb(INKS.black.lit), mul(eye, 0.85));
      return vec4(inked, c.w);
    },
  };
}

// ==== the water =====================================================================================================

const pool = whirlpool({ at: [0, 0], ...POOL });
const water = sea({
  swell: [
    { direction: [1, 0.3], length: 9, height: 0.2, speed: 1.2 },
    { direction: [-0.3, 1], length: 4, height: 0.025, speed: 0.8, phase: 1.3 },
  ],
  deform: [pool], reach: 260, foam: 0.9,
});
const vortex = pool.in(water);

// ==== the coil ======================================================================================================

const W = TAU / LOOP;
const B = BODY;

/** The funnel's dip at radius r (whirlpool.ts's profile), so the arches stand on the water, not on the still level. */
const funnel = (r: F): F => {
  const s = clamp(div(r, POOL.radius), 0, 1);
  const s2 = mul(s, s), q = sub(1, s2);
  return div(mul(mul(mul(q, q), q), POOL.depth), add(1, mul(s2, 1 / (POOL.throat * POOL.throat))));
};

/** Height of the spine above the still water at m: under the water, the neck, four arches, the tail diving. */
function heightAt(m: F): F {
  let y: F = sub(-B.dip, mul(smoothstep(0.86, 1, m), 3.5));
  // A swan's neck: it leaves the water steeply and levels out under the head.
  const neck = pow(sub(1, smoothstep(0, B.neckEnd, m)), 0.5);
  y = add(y, mul(neck, B.headHeight + B.dip));
  for (const [k, a] of ARCHES.entries()) {
    const u = clamp(div(sub(m, a.at - a.half), 2 * a.half), 0, 1);
    const heave = add(1, mul(sin(sub(mul(time, W), k * 1.6)), B.heave));
    // sin^0.6: the arch leaves the water steeply, like a half-torus, and is round on top.
    y = add(y, mul(mul(heave, a.height + B.dip), pow(max(sin(mul(u, Math.PI)), 0), 0.6)));
  }
  return y;
}

/** The spine at rest (attacks apply on top). Head at m = 0. */
function restSpine(m: F): P3 {
  const neck = sub(1, smoothstep(0, B.neckEnd * 1.4, m));
  const angle = add(add(B.start, mul(m, B.turns * TAU)), mul(sin(sub(mul(time, W), mul(m, 2 * TAU))), B.slither));
  // The head leans in over the pool and sways across it.
  const r = sub(sub(mix(B.headRadius, B.tailRadius, m), mul(neck, B.headIn)), mul(mul(neck, neck), mul(sin(mul(time, W)), B.sway)));
  const y = sub(heightAt(m), funnel(r));
  return vec3(mul(r, cos(angle)), y, mul(r, sin(angle)));
}

// ==== the Coil Lash =================================================================================================

const lashArch = ARCHES[LASH.arch];
/** The lane: from the arch's foot out of the pool (turned by `bearing`), measured at rest at time 0. */
const lashAngle = B.start + lashArch.at * B.turns * TAU;
const lashR = B.headRadius + (B.tailRadius - B.headRadius) * lashArch.at;
const laneAngle = lashAngle + LASH.bearing;
const laneA: [number, number] = [lashR * Math.cos(lashAngle), lashR * Math.sin(lashAngle)];
const laneB: [number, number] = [laneA[0] + LASH.length * Math.cos(laneAngle), laneA[1] + LASH.length * Math.sin(laneAngle)];

/** The loop's arming time: `lash.at` seconds into every loop. */
const cycleStart = mul(floor(div(time, LOOP)), LOOP);
const lash = attack({
  name: "lash",
  arm: add(cycleStart, input("lash.at", "f32", [LASH.at])),
  warn: input("lash.warn", "f32", [LASH.warn]),
  target: { kind: "disc", centre: lift<"vec2">(laneB), radius: lift<"f32">(0.5) },
  part: "coil",
  motion: {
    kind: "rear", hit: hit.capsule({ a: laneA, b: laneB, radius: LASH.radius }),
    from: lashArch.at - LASH.spread, to: lashArch.at + LASH.spread, height: LASH.height, inset: B.radius * 1.25, lean: LASH.lean,
  },
  timing: { windup: 1.1, strike: 0.38, zeta: 0.55, hold: 0.45, recover: 0.62, travel: 1.6 },
});
/** 0..1: the lashed arch's gland stands open from the slam for a while (the game's exposure window). */
const open = (() => {
  const e = lash.envelope(time);
  return mul(smoothstep(0, 0.15, e.tau), sub(1, smoothstep(1.6, 2.2, e.tau)));
})();

// ==== the wyrm's parts ==============================================================================================

const BODY_LENGTH = 62; // world length of the spine at rest, for patterns in world units (about)

/** 0..1 on each arch's crown: the pearl gland's vermilion socket (paint, no draw of its own). */
function glandAt(m: F, v: F): F {
  let g: F = lift<"f32">(0);
  // An oval: 0.017 of m along (about 1 unit), 0.11 of the way round across.
  for (const a of ARCHES) g = max(g, sub(1, step(1, length(vec2(div(sub(m, a.at), 0.017), div(sub(v, 0.25), 0.11))))));
  return g;
}

/** Body paint: slate back with crescent scales, an olive flank band, cream belly plates; ink lines between. */
function bodyPaint(m: F, v: F): P3 {
  const s = mul(m, BODY_LENGTH), around = mul(v, TAU * B.radius);
  const dorsal = abs(sub(v, 0.25)), ventral = abs(sub(v, 0.75));
  // Crescent scales: rows along, staggered; a line on each row's lower arc.
  const SCALE = 0.75;
  const row = floor(div(s, SCALE));
  const x = fract(add(div(around, SCALE), mul(mod(row, 2), 0.5)));
  const y = fract(div(s, SCALE));
  const arc = abs(sub(distance(vec3(x, mul(y, 1.3), 0), vec3(0.5, 0, 0)), 0.55));
  const scaleLine = sub(1, step(0.075, arc));
  // Belly plates: transverse lines.
  const plate = sub(1, step(0.09, fract(div(s, 0.55))));
  const back = select(lt(dorsal, 0.3), rgb(K("slate")), rgb(K("olive")));
  const belly = lt(ventral, 0.15);
  const skin = select(belly, rgb(K("cream")), back);
  const line = select(belly, plate, scaleLine);
  // A thin black seam where the cream belly meets the flank.
  const seam = mul(step(0.135, ventral), sub(1, step(0.165, ventral)));
  const lined = select(lt(0.5, max(line, seam)), rgb(K("black")), skin);
  return select(lt(0.5, glandAt(m, v)), rgb(K("vermilion")), lined);
}

const girthCurve = keys([[0, 0.78], [0.04, 0.86], [0.14, 0.98], [0.4, 1], [0.75, 0.92], [0.92, 0.6], [1, 0.2]]);
const restAnimated = animate(restSpine, [lash], (spine) => {
  const body = strand({
    name: "wyrm", spine, stations: 120,
    girth: (m) => mul(girthCurve(m).x, B.radius), section: { aspect: 1.0, squareness: 2.2 },
    tips: [{ round: 0.01 }, { point: 0.05 }], level: 1.2, paint: bodyPaint,
    detail: { along: 300, around: 20 },
  });
  return { things: [body], // The part that lands is the loop between the rear's blend ramps (its outer fifth each side stays in the rest
  // posture, under water at the arch's feet); checks measure this stretch.
  parts: { coil: part("coil", body, { from: lashArch.at - 0.6 * LASH.spread, to: lashArch.at + 0.6 * LASH.spread, tip: lashArch.at }) } };
});
const body = restAnimated.parts.coil!.body;

// ---- the head: skull, jaw, horns, eyes, teeth; placed on the body at m = 0, facing toward -m ----------------------
const X: In<"vec3"> = [1, 0, 0], Y: In<"vec3"> = [0, 1, 0], Z: In<"vec3"> = [0, 0, 1];
const forward = rotate(mul(lift<"vec3">(X), -1), Z, HEAD.pitch);
const crown = rotate(Y, Z, HEAD.pitch);
const hl = HEAD.length, hw = HEAD.width, hh = HEAD.width * 0.72;
const skullPoints = [0, 0.35, 0.7, 1].map((u) => add(mul(lift<"vec3">(X), 0.1 * hl), sub(mul(forward, hl * u), mul(crown, 0.07 * hl * u * u))));
const skullW = keys([[0, hw * 0.85], [0.3, hw], [0.65, hw * 0.72], [1, hw * 0.4]]);
const skullA = keys([[0, 0.85], [0.3, 0.72], [0.65, 0.62], [1, 0.55]]);
/** Skull paint: slate crown with brow scales, olive cheeks, a cream lip line along the mouth. */
const headPaint = (m: F, v: F): P3 => {
  const mouth = smoothstep(0.84, 0.85, mul(sin(mul(v, TAU)), -1));
  // Brow and snout plates across the crown; slate above, olive under the cheeks (as the body).
  const brow = mul(sub(1, step(0.1, abs(sub(fract(mul(m, 6)), 0.5)))), sub(1, step(0.24, abs(sub(v, 0.25)))));
  const skin = select(lt(abs(sub(v, 0.75)), 0.22), rgb(K("olive")), rgb(K("slate")));
  return select(lt(0.5, mouth), select(lt(0.5, step(0.3, m)), rgb(K("vermilion")), skin), select(lt(0.5, brow), rgb(K("black")), skin));
};
const head = place(strand({
  name: "wyrmHead", spine: (m) => through(m, skullPoints), stations: 12,
  girth: (m) => skullW(m).x, section: { aspect: (m) => skullA(m).x, squareness: 2.8 },
  tips: [{ round: 0.25 }, { round: 0.12 }], up: crown, paint: headPaint, detail: { along: 40, around: 24 },
}), on(body, [0, 0.25, 0]));

const gape = add(HEAD.gape, mul(sin(mul(time, W * 2)), HEAD.breath));
const jawDir = vec3(cos(gape), sin(gape), 0);
const hinge = vec3(0, hh * 0.8, 0);
const jl = hl * 0.66;
const jawW = keys([[0, hw * 0.72], [0.5, hw * 0.66], [1, hw * 0.36]]);
const jaw = place(strand({
  name: "wyrmJaw",
  spine: (m) => through(m, [sub(hinge, mul(jawDir, 0.12 * jl)), add(hinge, mul(jawDir, 0.5 * jl)), add(hinge, mul(jawDir, jl))]),
  stations: 8, girth: (m) => jawW(m).x, section: { aspect: 0.4, squareness: 2.6 },
  tips: [{ round: 0.15 }, { round: 0.12 }], up: vec3(sin(gape), mul(cos(gape), -1), 0),
  paint: (_m, v) => select(lt(0.5, smoothstep(0.7, 0.71, sin(mul(v, TAU)))), rgb(K("vermilion")), rgb(K("cream"))),
  detail: { along: 24, around: 20 },
}), on(head, [0.22, 0.75, 0]));

const hornR = horn({
  name: "wyrmHornR", length: HEAD.horn, curl: HEAD.hornCurl, taper: 0.16, girth: 0.34,
  out: [-1, 0.55, 0.4], toward: [0.2, -1, 0.1], paint: K("cream"),
});
const hornL = horn({
  name: "wyrmHornL", length: HEAD.horn, curl: HEAD.hornCurl, taper: 0.16, girth: 0.34,
  out: [-1, 0.55, -0.4], toward: [0.2, -1, -0.1], paint: K("cream"),
});
const horns = [place(hornR, on(head, [0.12, 0.25 - 0.08, 0.85])), place(hornL, on(head, [0.12, 0.25 + 0.08, 0.85]))];

const eye = (name: string, v: number) => place(strand({
  name, spine: (m) => through(m, [[0, -0.2, 0], [0, 0.2, 0]]), stations: 4, girth: 0.3,
  tips: [{ round: 0.5 }, { round: 0.5 }], up: [0, 0, -Math.sign(Math.cos(TAU * v))],
  // A vermilion iris with a black slit, sunk under the brow (a white eye read as a cartoon's).
  paint: (m, vv) => select(lt(0.5, mul(step(0.7, m), sub(1, step(0.35, abs(cos(mul(vv, TAU))))))), rgb(K("black")), rgb(K("vermilion"))),
  detail: { along: 12, around: 12 },
}), on(head, [0.34, v, 0.97]));
const eyes = [eye("wyrmEyeR", 0.25 - 0.13), eye("wyrmEyeL", 0.25 + 0.13)];

const tooth = strand({
  name: "wyrmTooth", spine: (m) => through(m, [[0, -0.05, 0], [0.04, 0.2, 0], [0.1, 0.42, 0]]), stations: 4,
  girth: 0.1, tips: ["open", { point: 0.85 }], up: [1, 0, 0], paint: K("cream"), detail: { along: 6, around: 8 },
});
const TEETH = 5;
const teeth = draw.points("wyrmTeeth", place(points(2 * TEETH, (i) => {
  const j = mod(i, TEETH), side = select(lt(i, TEETH), -1, 1);
  return { at: vec3(add(0.42, mul(j, 0.5 / TEETH)), add(0.75, mul(side, 0.11)), 0.9), scale: add(1.3, mul(j, -0.12)) };
}), head), tooth);

// ---- gills: vermilion frills on both sides of the neck, flaring with the breath -------------------------------------
const gillProfile = keys([[0, 0.2], [0.25, 1], [0.6, 0.8], [1, 0]]);
const ridgeProfile = keys([[0, 0], [0.05, 0.55], [0.9, 0.4], [1, 0]]);
const flare = add(1, mul(sin(mul(time, W * 2)), 0.18));
// One frill a side (a second, lower pair cost 0.08 ms of GPU from the game camera for little that reads there).
const gills = [0.03, 0.47].map((v, k) => crest({
  on: body, from: 0.004, to: 0.065, v, name: `wyrmGill${k}`,
  height: (x: In<"f32">) => mul(mul(gillProfile(x).x, HEAD.gill), flare),
  paint: K("vermilion"), rays: 6, sag: 0.35, rake: 0.55,
  flutter: (m) => mul(sin(sub(mul(m, 60), mul(time, W * 3))), 0.15), rows: 24,
}));

/** A low dark fin down the back, along the arches. */
const ridge = crest({
  on: body, from: 0.06, to: 0.9, name: "wyrmRidge",
  height: (x: In<"f32">) => ridgeProfile(x).x,
  paint: K("black"), rays: 90, sag: 0.6, rake: 0.5, rows: 360,
});

// ---- pearl glands: on each arch's crown, one pearl in a vermilion socket (painted on the body) ----------------------
// Three pearls in a row read as a pair of cartoon eyes from every side camera (two visible, white with a black ring);
// one larger pearl reads as a pearl.
const pearl = strand({
  name: "wyrmPearl", spine: (m) => through(m, [[0, -0.42, 0], [0, 0.42, 0]]), stations: 4, girth: 0.45,
  tips: [{ round: 0.5 }, { round: 0.5 }], up: [1, 0, 0], paint: K("pearl"), detail: { along: 10, around: 12 },
});
const archAt = (k: F, pick: (a: (typeof ARCHES)[number]) => number): F =>
  ARCHES.slice(1).reduce<F>((acc, a, i) => select(is(k, i + 1), pick(a), acc), lift<"f32">(pick(ARCHES[0])));
/** How far the pearl sits out of the skin (rho), and how much it swells while its gland is open. */
const PEARL = { out: 0.86, open: 0.45 };
const pearls = draw.points("wyrmPearls", place(points(ARCHES.length, (k) => ({
  at: vec3(archAt(k, (a) => a.at), 0.25, PEARL.out),
  // The lashed arch's gland swells open after the slam.
  scale: add(1, mul(select(is(k, LASH.arch), open, 0), PEARL.open)),
})), body), pearl);

// ==== the ship ======================================================================================================

const shipBearing = laneAngle + SHIP.along / (POOL.radius + SHIP.beyond);
const shipR = Math.hypot(laneB[0], laneB[1]) + SHIP.beyond;
const shipAt: [number, number] = [shipR * Math.cos(shipBearing), shipR * Math.sin(shipBearing)];
const WIND = [0.6, 0.8] as const;
const scaleShip = ship(sloop("skiff", SHIP.length, {
  hull: K("wood"), bottom: K("wood"), wale: K("black"), trim: K("cream"), deck: K("wood"), spar: K("wood"),
  cloth: K("sail"), rope: K("black"), team: K("cream"),
})).afloat(vortex, { at: shipAt, heading: shipBearing + Math.PI / 2, wind: WIND });

// ==== the scene =====================================================================================================

export const strikes = restAnimated.strikes;

export default scene({
  sea: water,
  things: [
    ...restAnimated.things, head, jaw, ...horns, ...eyes, teeth, ...gills, ridge, pearls,
    telegraph(lash, { style: { fill: INKS.vermilion.lit, bone: INKS.cream.lit, ink: INKS.black.lit, flash: [1, 0.97, 0.9], fillFrom: 0.4, fillTo: 0.92, spacing: 1.1, edge: 0.2 } }),
    ...scaleShip,
  ],
  style: wyrmPrint(),
  sun: [-0.45, 0.75, 0.5],
  cameras: {
    game: { kind: "perspective", eye: [5, 31, 29], target: [0.5, 0, 0.5], fovY: 0.62 },
    three: { kind: "perspective", eye: [-24, 15, 19], target: [-4.5, 1.5, -3], fovY: 0.7 },
    close: { kind: "perspective", eye: [-7, 9, 2], target: [0, 5.2, -9.5], fovY: 0.5 },
    top: { kind: "orthographic", eye: [0, 50, 0.001], target: [0, 0, 0], height: 38 },
  },
});

export const checks = [landsInside(strikes.lash!)];

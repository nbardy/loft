// The serpent strikes: a bite along a capsule at a ship ahead, then a tail sweep through an arc at a second ship
// astern. Each attack is defined once, from what a game's simulation would supply (a hit shape, an arming time, a
// warning duration, all host inputs), and everything else is derived from it: the posed body through time, the
// telegraph on the water, the smear of the striking part, the wake, and the checks exported below.
//
// The "simulation" here is a stand-in: it aims each hit shape at its ship's position input, so moving a ship moves
// the shape and the strike follows. Run `loft check examples/strike.ts` for the checks.

import {
  abs, add, atan2, div, input, length, lift, mix, mod, mul, normalize, palette, scene, sea, sin, smoothstep, strand, sub, TAU, through, time, toon, vec2, vec3,
  type Expr,
} from "../src/index.ts";
import { animate, attack, hit, part, smear, telegraph, wake, type Target } from "../src/spacetime/index.ts";
import { budget, landsInside, minScreenSize, staysClear, stopsAt } from "../src/check.ts";
import { restGape, restSpine, S, serpent } from "./serpent.ts";

const water = sea({
  swell: [
    { direction: [1, 0.3], length: 14, height: 0.35, speed: 1.1 },
    { direction: [-0.3, 1], length: 6.5, height: 0.12, speed: 0.8, phase: 1.3 },
  ],
});

// ---- a ship stand-in: a hull about 5 long riding the swell, broadside to the serpent -------------------------------

const HALF_BEAM = 0.95, HULL = 5;
function ship(name: string, at: Expr<"vec2">, heading: Expr<"vec2">) {
  const along = (s: number) => add(at, mul(heading, (s - 0.5) * HULL));
  const deck = (s: number) => { const xz = along(s); return vec3(xz.x, add(water.height(xz), 0.28 + 0.12 * (s - 0.5) ** 2 * 4), xz.y); };
  const hull = strand({
    name, spine: (m) => through(m, [0, 0.35, 0.7, 1].map(deck)), stations: 8,
    girth: (m) => mul(sub(1, mul(mul(sub(m, 0.45), sub(m, 0.45)), 1.6)), HALF_BEAM), section: { aspect: 0.55, squareness: 3.2 },
    tips: [{ round: 0.06 }, { point: 0.22 }],
    paint: (m, v) => {
      const side = palette.countershade(v, { back: [0.62, 0.43, 0.25], belly: [0.42, 0.1, 0.08] });
      // A pale strake along each side at mid-height (v = 0 and 0.5), stopping short of the bow.
      const strake = mul(smoothstep(0.16, 0.1, abs(sin(mul(v, TAU)))), smoothstep(0.9, 0.8, m));
      return mix(side, vec3(0.95, 0.92, 0.84), strake);
    },
    detail: { along: 40, around: 20 },
  });
  // A cabin and a mast, so it reads as a ship from above.
  const mid = deck(0.42);
  const cabin = strand({
    name: `${name}Cabin`, spine: (m) => through(m, [add(mid, mul(vec3(heading.x, 0, heading.y), -0.7)), add(mid, mul(vec3(heading.x, 0, heading.y), 0.5))].map((p) => add(p, vec3(0, 0.55, 0)))),
    stations: 4, girth: 0.5, section: { aspect: 0.9, squareness: 4 }, tips: [{ round: 0.15 }, { round: 0.15 }], paint: [0.9, 0.86, 0.76], detail: { along: 12, around: 16 },
  });
  const mast = strand({
    name: `${name}Mast`, spine: (m) => through(m, [add(deck(0.6), vec3(0, 0.3, 0)), add(deck(0.6), vec3(0, 3.2, 0))]), stations: 4, girth: 0.07,
    tips: ["open", { point: 0.1 }], paint: [0.35, 0.24, 0.15], detail: { along: 8, around: 8 },
  });
  return { hull, things: [hull, cabin, mast] };
}

/** The ships' positions: what the game moves. */
const bow = input("ship.at", "vec2", [14.5, 1.5]);
const stern = input("ship2.at", "vec2", [-3.3, -5.8]);

// ---- the bite: a lunge along a capsule from the serpent's neck to the ship's near side ----------------------------

/** Where the simulation's lane starts: in front of the serpent's neck on the water. */
const ANCHOR: readonly [number, number] = [7.4, 0];
const biteDir = normalize(sub(bow, lift<"vec2">(ANCHOR)));
const bowHeading = vec2(mul(biteDir.y, -1), biteDir.x);
const bowShip = ship("ship", bow, bowHeading);
const biteTarget: Target = { kind: "body", body: bowShip.hull };
const bite = attack({
  name: "bite",
  arm: input("bite.arm", "f32", [2.4]),
  warn: input("bite.warn", "f32", [1.6]),
  target: biteTarget,
  part: "head",
  motion: {
    kind: "lunge",
    hit: hit.capsule({ a: add(lift<"vec2">(ANCHOR), mul(biteDir, 1.2)), b: sub(bow, mul(biteDir, HALF_BEAM + 0.05)), radius: input("bite.radius", "f32", [0.95]) }),
    reach: 0.3, lead: 1.95, height: 0.8, coil: 1.6, rise: 1.1, approach: 0.3, neck: 1.6,
  },
});

// ---- the lash: the tail sweeps through an arc round a point behind the body, toward the second ship --------------

const PIVOT: readonly [number, number] = [-3, 0];
const SWEEP_FROM = Math.PI - 0.25;
const toStern = sub(stern, lift<"vec2">(PIVOT));
// The ship's bearing from the pivot, unwrapped past the sweep's start, less the angle its near half-beam subtends.
const sternBearing = add(SWEEP_FROM, mod(sub(atan2(toStern.y, toStern.x), SWEEP_FROM), Math.PI * 2));
const sternHeading = normalize(toStern);
const sternShip = ship("ship2", stern, sternHeading);
const lashTarget: Target = { kind: "body", body: sternShip.hull };
const lash = attack({
  name: "lash",
  arm: input("lash.arm", "f32", [5.6]),
  warn: input("lash.warn", "f32", [1.4]),
  target: lashTarget,
  part: "tail",
  motion: {
    kind: "sweep",
    hit: hit.arc({ centre: PIVOT, inner: 1.2, outer: 7.4, from: SWEEP_FROM, to: sub(sternBearing, div(HALF_BEAM + 0.1, length(toStern))) }),
    root: 0.72, height: 0.25, inset: 0.45,
  },
  timing: { windup: 0.7, strike: 0.32, travel: 3 },
});

// ---- the serpent, posed by both attacks ---------------------------------------------------------------------------

const biteOpen = (() => {
  const e = bite.envelope(time);
  // The jaw opens wide through the wind-up and snaps shut on landing.
  return mul(mul(e.p, sub(1, smoothstep(-0.03, 0.08, e.tau))), S.jawGape * 3.2);
})();

const creature = animate(restSpine, [bite, lash], (spine) => {
  const s = serpent({ spine, gape: add(restGape, biteOpen) });
  return {
    things: s.things,
    parts: { head: part("head", s.head, { tip: 1 }), tail: part("tail", s.body, { from: 0.8, to: 1, tip: 1 }), eye: part("eye", s.eyes[1], { tip: 0.5 }), body: part("body", s.body, { tip: 0 }) },
  };
});
const parts = creature.parts;

export const strikes = creature.strikes;

export default scene({
  sea: water,
  things: [
    ...creature.things, ...bowShip.things, ...sternShip.things,
    telegraph(bite), telegraph(lash),
    wake(parts.body.body, { over: 1.6, samples: 10 }),
    smear(strikes.bite, { over: 0.12, samples: 4 }),
    smear(strikes.lash, { over: 0.1, samples: 4 }),
  ],
  style: toon({ bands: 3, outline: 0.04 }),
  sun: [-0.45, 0.75, 0.5],
  cameras: {
    three: { kind: "perspective", eye: [13, 16, 18], target: [3.5, 0, -0.5], fovY: 0.62 },
    side: { kind: "perspective", eye: [3, 4.5, 33], target: [3, 1.2, 0], fovY: 0.62 },
    top: { kind: "orthographic", eye: [3, 40, 0.001], target: [3, 0, 0], height: 24 },
    bite: { kind: "perspective", eye: [16, 10, 13], target: [11, 1, 0.5], fovY: 0.6 },
  },
});

export const checks = [
  landsInside(strikes.bite),
  stopsAt(strikes.bite, biteTarget),
  staysClear(parts.body, biteTarget, { until: bite.arm, label: "bite: body clear of the ship until arming" }),
  staysClear(parts.head, biteTarget, { until: bite.arm, margin: 0.02, label: "bite: head clear of the ship until arming" }),
  landsInside(strikes.lash),
  stopsAt(strikes.lash, lashTarget),
  staysClear(parts.body, lashTarget, { until: lash.arm, label: "lash: body clear of ship2 until arming" }),
  minScreenSize(parts.eye, "three", 4, { at: bite.arm }),
  budget({ triangles: 400_000 }),
];

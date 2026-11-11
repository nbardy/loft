// The spiral shell's closed form (docs/art_direction/DIRECTION.md, Round 5): one tube coiled round an axis by Raup's
// law (Raup 1966: every length grows by W in one turn, the tube's centre stands off the axis in proportion, and steps
// along the axis in proportion), with a mouth the crab's body sits in. Plain numbers first (`planOf`), then the map
// (`shellMap`): the same expressions draw the shell (back/spiral.ts) and, compiled for the CPU, place what grows on it
// and predict its picture (tools/spiral_theory.ts).
//
// Coordinates are the crab's (x forward, y up, z to its right; the body's origin rides `ride` above the still water),
// so everything here is carried by `rig.shell`. Nothing is a world coordinate.
//
// The coil:
//
//   point(back, phi) = tip + W^-back * ( (c + r cos phi) * out(back) + (-t + a sin phi) * axis )
//
// - `back`: turns back from the mouth (0 at the lip, `whorls` at the tip); out(back) is the unit direction from the
//   axis to the tube's centre, turning once per turn in the coil's plane;
// - `phi`: round the tube's section from its outer side (0) toward the spiral's face (a quarter turn), the axis (half)
//   and the far flank (three quarters); r and a are the section's half-extents across the coil and along the axis;
// - c = r: the tube touches the axis (Raup's D = 0, no navel); t = a + spire * c: every turn's crown lies on a cone
//   through the tip, `spire` steep (0: a flat spiral face).
//
// What follows from the law, and is used below:
// - Seen along the axis, the line where a turn meets the next one in (the suture) and any line of constant phi are
//   equiangular spirals of ratio W a turn. A band printed between two values of phi is therefore a spiral band whose
//   rings stand W apart; bands at 1 + cos(phi) = x and W x lie on one spiral (the outer turn's band runs on, one turn
//   in, as the next band). So the paint is a function of phi alone: `bandAt`.
// - In the mouth's plane the turn before takes the inner 1/W of the mouth; the body has the outer crescent, 2 r (1 - 1/W) deep.

import { add, compileJs, cos, exp, floor, log, max, mul, saturate, sin, smoothstep, sqrt, sub, variable, vec3, type F, type P3 } from "../../../../src/index.ts";
import { TAU, type V3 } from "../ink.ts";
import type { Numbers } from "../numbers.ts";

const addV = (a: V3, b: V3): V3 => [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
const mulV = (a: V3, k: number): V3 => [a[0] * k, a[1] * k, a[2] * k];
const dotV = (a: V3, b: V3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const crossV = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unitV = (a: V3): V3 => mulV(a, 1 / Math.hypot(a[0], a[1], a[2]));

/** The surface's grid: cells along the outside from tip to lip, round the lip's roll, down the throat; and round the section. */
export const GRID = { outer: 124, roll: 6, throat: 14, round: 36 } as const;
const NU = GRID.outer + GRID.roll + GRID.throat;
/** The stations in u, each on a grid line: where the outside ends at the lip, and where the roll ends in the throat. */
export const U = { lip: GRID.outer / NU, throat: (GRID.outer + GRID.roll) / NU } as const;

/**
 * The shell in plain numbers, from the build's: the axis (unit, out of the spiral's face), the coil plane's two axes
 * (`fore` the forward one, `rise` = axis x fore), the tip, the section, where the suture runs and where the bands are.
 */
export function planOf(n: Numbers) {
  const S = n.spiral, W = S.growth;
  if (!(W > 1)) throw new Error(`spiral: growth must be above 1 (every turn larger than the last), got ${W}`);
  const r = S.mouth.radial, a = S.mouth.axial, c = r, t = a + S.spire * c;
  const axis: V3 = [-Math.sin(S.rake) * Math.cos(S.tilt), Math.sin(S.tilt), Math.cos(S.rake) * Math.cos(S.tilt)];
  const fore = unitV(addV([1, 0, 0], mulV(axis, -axis[0])));
  const rise = crossV(axis, fore);
  const outAt = (back: number): V3 => { const psi = S.mouthAt - TAU * back; return addV(mulV(fore, Math.cos(psi)), mulV(rise, Math.sin(psi))); };
  /** The body's point in the mouth (crab coordinates), and from it the tip: the outer lip passes `seat.lip` beyond the body's point. */
  const seat: V3 = [n.carapace.from + S.seat.in, S.seat.up, S.seat.side];
  const tip = addV(addV(seat, mulV(outAt(0), -(2 * r - S.seat.lip))), mulV(axis, t));

  // The suture: where the turn before (the section shrunk by W about the tip) comes out of this one, on the face's
  // side. In units of r, the section is the ellipse centred (1, -t / r) with half-axes (1, a / r); the root is found
  // on the smaller ellipse by bisection between its outer side (phi = 0: inside the larger turn) and its crown (a
  // quarter turn: outside it, on the face).
  const q = a / r, tq = t / r;
  const inside = (phi: number): number => {
    const x = (1 + Math.cos(phi)) / W, h = (-tq + q * Math.sin(phi)) / W;
    return (x - 1) ** 2 + ((h + tq) / q) ** 2 - 1;
  };
  let lo = 0, hi = Math.PI / 2;
  if (!(inside(lo) < 0 && inside(hi) > 0)) throw new Error(`spiral: the turns do not overlap on the face's side (growth ${W}, spire ${S.spire}): no suture to print along`);
  for (let i = 0; i < 60; i++) { const mid = (lo + hi) / 2; if (inside(mid) < 0) lo = mid; else hi = mid; }
  /** The suture's distance from the axis in units of the turn's own r that it lies on (the larger one): 1 + cos(phi) there. */
  const sutureX = (1 + Math.cos(lo)) / W;
  return {
    W, whorls: S.whorls, r, a, c, t, axis, fore, rise, tip, seat, outAt, sutureX,
    /** Where each turn but the last shows, round its section: from where the next turn out covers it to its own suture. */
    shows: [lo, Math.acos(sutureX - 1)] as const,
    lnW: Math.log(W),
  };
}
export type Plan = ReturnType<typeof planOf>;

/**
 * Which ring of the printed spiral a point of the section is in, and how far across it (0 at the ring's inner edge,
 * the suture's side; 1 at the next ring): rings are counted outward from the suture's own (0), so ring k lies
 * W^k times as far from the axis. Inside the suture (covered by the turn before) the ring is below 0.
 */
export function ringAt(p: Plan, phi: F): { readonly ring: F; readonly across: F } {
  const g = mul(log(max(mul(add(1, cos(phi)), 1 / p.sutureX), 1e-4)), 1 / p.lnW);
  const ring = floor(g);
  return { ring, across: sub(g, ring) };
}

/** The lip's roll and the throat, as the build's numbers give them: the section's size at the mouth outside and inside. */
const lipOf = (n: Numbers) => { const L = n.spiral.lip; return { out: 1 + L.flare, in: 1 + L.flare - L.thick, advance: L.thick / (4 * Math.PI), flareOver: 0.07 }; };

/**
 * The shell's map in crab coordinates. `outside(back, phi)` is the bare coil with its proud bands (what things stand
 * on); `point(u, v)` is the drawn surface: the outside from tip (u = 0) to lip (U.lip), the lip's roll (to U.throat),
 * then the throat running back inside to a dark end (u = 1). v runs once round the section.
 */
export function shellMap(n: Numbers) {
  const p = planOf(n), S = n.spiral, L = lipOf(n);
  const axis = vec3(...p.axis), fore = vec3(...p.fore), rise = vec3(...p.rise), tip = vec3(...p.tip);
  /** The coil's point for a section scaled by `k`: turns back from the mouth, round the section. */
  const coil = (back: F, phi: F, k: F): P3 => {
    const s = exp(mul(back, -p.lnW)), psi = sub(S.mouthAt, mul(back, TAU));
    const out = add(mul(fore, cos(psi)), mul(rise, sin(psi)));
    return add(tip, mul(add(mul(out, add(p.c, mul(mul(k, p.r), cos(phi)))), mul(axis, add(-p.t, mul(mul(k, p.a), sin(phi))))), s));
  };
  /** How proud of the tube a cream band stands at phi: a rounded hump across the band, none inside the suture. */
  const proud = (phi: F): F => {
    const { ring, across } = ringAt(p, phi), w = S.band.cream;
    const hump = mul(smoothstep(0, 0.3 * w, across), sub(1, smoothstep(0.7 * w, w, across)));
    return mul(mul(hump, S.band.proud), smoothstep(-1.5, -0.5, ring));
  };
  /** The flare toward the mouth: the section grows by `lip.flare` over the last `flareOver` of a turn. */
  const flared = (back: F): F => add(1, mul(sub(1, smoothstep(0, L.flareOver, back)), S.lip.flare));
  const outside = (back: F, phi: F): P3 => coil(back, phi, mul(flared(back), add(1, proud(phi))));
  // v runs with phi: with u from tip to mouth, d/du x d/dv then faces out of the tube (checked in tools/spiral_theory.ts).
  const phiOf = (v: F): F => mul(v, TAU);
  const point = (u: F, v: F): P3 => {
    const phi = phiOf(v);
    // Along the outside: `whorls` back at u = 0, the lip at U.lip; the tip closes over the first two cells.
    const along = saturate(mul(u, 1 / U.lip));
    const closeTip = sqrt(saturate(mul(u, NU / 2)));
    // Round the lip (w from 0 to 1): the section shrinks from its flared size to the throat's while the roll leans
    // a little past the mouth's plane, a half-round bead.
    const w = saturate(mul(sub(u, U.lip), 1 / (U.throat - U.lip)));
    const rolled = sub(L.out, mul(sub(1, cos(mul(w, Math.PI))), (L.out - L.in) / 2));
    // Down the throat (d from 0 to 1): back inside by `lip.throat` of a turn, closing to a point at its end.
    const d = saturate(mul(sub(u, U.throat), 1 / (1 - U.throat)));
    const back = add(add(mul(sub(1, along), p.whorls), mul(sin(mul(w, Math.PI)), -L.advance)), mul(d, S.lip.throat));
    const onOutside = sub(1, smoothstep(0, 0.5, w));
    const k = mul(mul(mul(mix1(rolled, flared(back), onOutside), add(1, mul(proud(phi), onOutside))), closeTip), sqrt(max(sub(1, mul(d, d)), 0)));
    return coil(back, phi, k);
  };
  return { plan: p, outside, point, phiOf };
}
const mix1 = (a: F, b: F, t: F): F => add(a, mul(sub(b, a), t));

/** The shell's closed forms on the CPU: the outside's point and unit normal (out of the tube) at (turns back, phi). */
export function shellTwin(n: Numbers) {
  const m = shellMap(n), B = variable("spiral_back", "f32"), PH = variable("spiral_phi", "f32");
  const f = compileJs([m.outside(B, PH)]);
  const at = (back: number, phi: number): V3 => f({ vars: { spiral_back: back, spiral_phi: phi }, inputs: {}, params: {} })[0] as unknown as V3;
  const normal = (back: number, phi: number): V3 => {
    const e = 1e-3, db = addV(at(back - e, phi), mulV(at(back + e, phi), -1)), dp = addV(at(back, phi + e), mulV(at(back, phi - e), -1));
    // `back` falls toward the mouth and phi turns from the outer side to the face: (toward the mouth) x (d/dphi) points out.
    return unitV(crossV(db, dp));
  };
  const U2 = variable("spiral_u", "f32"), V2 = variable("spiral_v", "f32");
  const g = compileJs([m.point(U2, V2)]);
  const drawn = (u: number, v: number): V3 => g({ vars: { spiral_u: u, spiral_v: v }, inputs: {}, params: {} })[0] as unknown as V3;
  return { plan: m.plan, at, normal, drawn, dot: dotV };
}

/** The plain vector arithmetic the plan is written in, for what is placed from it. */
export const vmath = { add: addV, mul: mulV, dot: dotV, cross: crossV, unit: unitV };

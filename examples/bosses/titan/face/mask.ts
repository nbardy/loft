// The war mask (the mask direction): the whole front of the carapace is a scowling samurai menpo in red lacquer.
//
// Temperament: furious, proud, immovable. Every feature serves it: heavy black brows in a deep V, deep sockets with
// cream eye slits whose upper lids slant down to the nose, a broad nose with flared nostrils, cheeks bunched by a
// grimace, a wide down-turned mouth of clenched cream teeth inside raised lips, dark lacquer cheek guards and a
// lacquer brim above.
//
// Built as one closed-form surface: a sphere patch on the carapace's front (placed from the carapace's numbers, so it
// follows any carapace that sets them), mapped from the square to a disc so its border is the mask's round rim, and
// pushed out along its radius by a height field of the face's features in the mask's own (X across, Y up). The paint is
// flat inks in the same (X, Y), ink lines on every crease. Forms come from the relief and the overprint shadow; the marks
// that must read from far away (brows, eye slits, teeth) are paint.
//
// Motion (faceKit's clocks): in Claw Crush's wind-up the brows knit down, the jaw drops open in a roar and the pupils
// fix on the disc; at the slam the jaw clamps and the eyes screw to slits; through the Grab the eyes go to the ship and
// the right corner of the mouth lifts in a sneer. At rest the jaw grinds a little and the brows twitch.

import {
  abs, add, alive, cos, div, draw, fract, framed, length, lift, max, min, mix, mul, sin, smoothstep, spacing, sqrt, sub, surface, time, vec2,
  vec3, type F, type P3,
} from "../../../../src/index.ts";
import { cut, perFrame } from "../ink.ts";
import type { FaceOption } from "../slots.ts";
import { faceKit } from "./base.ts";

/** A smooth bump: 1 at distance 0, 0 from distance 1. */
const bump = (d: F): F => sub(1, smoothstep(0, 1, d));
/** An ink line of half-width w along the zero set of d. */
const line = (d: F, w: number): F => sub(1, cut(abs(d), w));
/** 1 where d < 0, with a crisp edge. */
const inside = (d: F): F => sub(1, cut(d, 0));
/** Elliptic distance from (cx, cy) with radii (rx, ry): 1 on the ellipse. */
const ell = (x: F, y: F, cx: number, cy: number, rx: number, ry: number): F => length(vec2(mul(sub(x, cx), 1 / rx), mul(sub(y, cy), 1 / ry)));

/** The mask's sphere: radius, and how far it stands in front of the carapace's front end; tilted up toward the high cameras. */
const R = 3.25, PROUD = 0.75, TILT = 0.15;
/** Half-angles the mask spans: across (wider at the cheeks than the chin) and up/down. */
const ACROSS = 1.0, CHIN = 0.8, UP = 0.55, DOWN = 0.62;
/** How far the jaw drops when fully open (world). */
const JAW = 0.42;
/** Half-height (in Y) of the band between the teeth rows that stretches when the jaw opens. */
const GAP = 0.025;

export const mask: FaceOption = {
  about: "a scowling samurai war mask: black V brows, eye slits, clenched teeth, lacquer cheek guards",
  draw: (rig) => {
    const { n, body, grabOn } = rig;
    const I = n.ink, C = n.carapace;
    const k = faceKit(rig);
    const tabF = (name: string, e: F): F => perFrame(`titan_mask_${name}`, e);

    // ---- the face's clocks, once per frame ----
    const rouse = tabF("rouse", k.rouse), brace = tabF("brace", k.brace);
    // The jaw: a slow grind at rest, dropped open in the wind-up's roar, clamped at the slam.
    const grind = mul(max(sin(mul(time, 1.3)), 0), 0.15);
    const open = tabF("open", mul(mix(grind, 1, rouse), sub(1, brace)));
    // Brows: knit down when roused and at the slam, a twitch at rest.
    const knit = tabF("knit", add(add(rouse, mul(brace, 0.6)), alive(time, 0.2, 71, 0.4)));
    // Eyes: the slits' half-height, wider when roused (the glare), screwed nearly shut at the slam.
    const slit = tabF("slit", mul(mix(0.07, 0.1, rouse), sub(1, mul(brace, 0.8))));
    // Gaze across (X): drifting at rest, to the ship (ahead left, -X) in the Grab, to the disc (ahead right, +X) roused.
    const lookX = tabF("look", mix(mix(alive(time, 0.5, 72, 0.15), -0.9, grabOn), 0.85, rouse));
    const sneer = tabF("sneer", grabOn);

    /** Square (p up, q across, both 0..1) to disc (X across, Y up): the square's border goes to the unit circle. */
    const disc = (p: F, q: F) => {
      const U = sub(mul(q, 2), 1), V = sub(mul(p, 2), 1);
      return { X: mul(U, sqrt(sub(1, mul(mul(V, V), 0.5)))), Y: mul(V, sqrt(sub(1, mul(mul(U, U), 0.5)))) };
    };

    // ---- the features, in (X, Y): each a few distances, shared by the relief and the paint ----
    const brow = (X: F, Y: F) => {
      const ax = abs(X);
      // The brow's lower edge: a deep V, low at the nose, rising outward, falling at the temples; it knits down.
      const yB = sub(add(add(0.13, mul(ax, 0.5)), mul(max(sub(ax, 0.58), 0), -1.2)), mul(mul(knit, 0.08), sub(1, ax)));
      const d = sub(Y, yB); // 0 at the lower edge, the brow above it to 0.13
      const along = mul(smoothstep(0.015, 0.07, ax), sub(1, smoothstep(0.76, 0.86, ax)));
      const thick = mix(0.1, 0.15, smoothstep(0.1, 0.45, ax));
      const on = mul(inside(sub(abs(sub(d, mul(thick, 0.5))), mul(thick, 0.5))), along);
      return { d, on, ridge: mul(bump(div(abs(sub(d, 0.04)), 0.14)), along) };
    };
    const eye = (X: F, Y: F) => {
      const ax = abs(X);
      const ex = mul(sub(ax, 0.34), 1 / 0.19);
      // The lid line slants down toward the nose (the inner end lower): anger.
      const cyE = add(0.03, mul(ex, 0.05));
      const half = mul(slit, max(sub(1, mul(ex, ex)), 0));
      // Outside the slit's length (|ex| > 1) `half` is 0 and this edge's zero set would run on across the face as a
      // line: push it away there.
      const edge = add(sub(abs(sub(Y, cyE)), half), mul(max(sub(abs(ex), 1), 0), 10));
      const white = inside(edge);
      // The pupil, shifted by the gaze; the same on both eyes.
      const px = sub(X, mul(lookX, 0.07));
      const pupil = inside(sub(length(vec2(sub(abs(px), 0.34), mul(sub(Y, cyE), 1.2))), 0.06));
      const socket = bump(div(ell(ax, Y, 0.33, 0.04, 0.27, 0.16), 1.25));
      return { ex, edge, white, pupil, socket };
    };
    const mouth = (X: F, Y: F) => {
      const ax = abs(X);
      const W = add(0.4, mul(rouse, 0.06));
      // The mouth's line: down-turned at the corners, more when roused; the right corner (+X) sneers up in the Grab.
      const yM = add(sub(-0.47, mul(mul(X, X), add(0.55, mul(rouse, 0.35)))), mul(mul(sneer, 0.12), smoothstep(0.05, 0.42, X)));
      const across = min(div(ax, W), 1.2);
      const taper = max(sub(1, mul(mul(across, across), mul(across, across))), 0);
      const dy = sub(Y, yM);
      const hOpen = mul(0.12, taper); // the opening's half-height
      return { ax, W, yM, dy, hOpen, taper };
    };

    /** The relief: height above the sphere, world units. */
    const relief = (X: F, Y: F): F => {
      const b = brow(X, Y), e = eye(X, Y), m = mouth(X, Y);
      const ax = abs(X), rho = length(vec2(X, Y));
      const rim = sub(mul(smoothstep(0.84, 0.94, rho), 0.16), mul(smoothstep(0.95, 1.0, rho), 0.4));
      const nose = add(mul(bump(ell(X, Y, 0, -0.06, 0.12, 0.24)), 0.42), mul(bump(ell(ax, Y, 0.12, -0.17, 0.1, 0.08)), 0.2));
      const cheek = mul(bump(ell(ax, Y, 0.52, -0.2, 0.28, 0.24)), 0.24);
      const chin = mul(bump(ell(X, Y, 0, -0.82, 0.3, 0.16)), 0.14);
      const lips = mul(bump(div(abs(m.dy), add(m.hOpen, 0.12))), mul(m.taper, 0.17));
      const opening = mul(smoothstep(0.02, -0.02, sub(abs(m.dy), m.hOpen)), 0.18); // geometry: smooth, not `cut`
      const guard = mul(mul(smoothstep(0.72, 0.86, ax), smoothstep(0.25, -0.15, Y)), 0.18);
      return sub(add(add(add(add(add(add(rim, mul(b.ridge, 0.38)), nose), cheek), chin), lips), guard), add(mul(e.socket, 0.28), opening));
    };

    const point = (p: F, q: F): P3 => {
      const { X, Y } = disc(p, q);
      const m = mouth(X, Y);
      // The jaw: everything below the mouth's line drops, the band between the teeth rows stretching to the gap.
      const drop = mul(mul(mul(open, JAW), smoothstep(GAP, -GAP, m.dy)), sub(1, smoothstep(0.55, 0.85, m.ax)));
      const th = mul(X, mix(CHIN, ACROSS, smoothstep(-0.9, 0.0, Y)));
      const ph = mul(Y, mix(DOWN, UP, smoothstep(-0.1, 0.1, Y)));
      const r = add(R, relief(X, Y));
      // The direction, then the tilt up about z.
      const dx = mul(cos(ph), cos(th)), dy = sin(ph), dz = mul(cos(ph), sin(th));
      const x = sub(mul(dx, Math.cos(TILT)), mul(dy, Math.sin(TILT))), y = add(mul(dx, Math.sin(TILT)), mul(dy, Math.cos(TILT)));
      return vec3(add(C.to + PROUD - R, mul(x, r)), sub(add(0.4, mul(y, r)), drop), mul(dz, r));
    };

    const paint = (p: F, q: F): P3 => {
      const { X, Y } = disc(p, q);
      const b = brow(X, Y), e = eye(X, Y), m = mouth(X, Y);
      const ax = abs(X), rho = length(vec2(X, Y));
      const RED = vec3(...I.brick), BLACK = vec3(...I.black), CREAM = vec3(...I.cream), LAC = vec3(...I.lacquer);

      // Creases: the forehead's two furrows over the nose, the nose's outline and nostrils, the folds from the nostrils
      // to the mouth's corners, the line under the lower lip, the cheek's top.
      const furrow = mul(line(sub(ax, add(0.045, mul(sub(Y, 0.2), 0.12))), 0.008), mul(smoothstep(0.17, 0.2, Y), sub(1, smoothstep(0.36, 0.42, Y))));
      const noseLine = mul(line(sub(ell(ax, Y, 0.12, -0.17, 0.11, 0.09), 1), 0.05), cut(Y, -0.24));
      const nostril = inside(sub(ell(ax, Y, 0.085, -0.2, 0.045, 0.025), 1));
      const fold = mul(line(sub(ax, add(0.2, mul(sub(-0.15, Y), 0.75))), 0.009), mul(smoothstep(-0.12, -0.17, Y), smoothstep(-0.6, -0.48, Y)));
      const underLip = mul(line(sub(m.dy, sub(-0.2, mul(m.hOpen, 0.2))), 0.008), smoothstep(0.42, 0.3, ax));
      const cheekLine = mul(line(sub(ell(ax, Y, 0.55, -0.25, 0.3, 0.26), 1), 0.04), mul(cut(Y, -0.12), smoothstep(0.75, 0.6, ax)));
      const creases = max(max(max(furrow, noseLine), max(nostril, fold)), max(underLip, cheekLine));

      // The mouth: inside its opening, clenched teeth (an upper and a lower row with black gaps between teeth), black
      // behind them and in the jaw's gap; the lips' outline.
      const inMouth = inside(sub(abs(m.dy), m.hOpen));
      const gap = line(m.dy, GAP * 0.9);
      const teethRow = mul(inside(sub(abs(m.dy), mul(m.hOpen, 0.85))), smoothstep(0.98, 0.85, div(m.ax, m.W)));
      const toothGap = line(sub(fract(add(mul(X, 7.5), 0.5)), 0.5), 0.05);
      const teeth = mix(mix(CREAM, BLACK, toothGap), BLACK, gap);
      const mouthInk = mix(BLACK, teeth, teethRow);
      const lipLine = line(sub(abs(m.dy), m.hOpen), 0.012);

      // Eyes: cream slits with black pupils, a heavy black outline; the socket darkened round them.
      const eyeInk = mix(mix(CREAM, BLACK, e.pupil), BLACK, line(e.edge, 0.012));
      const socketShade = mul(smoothstep(0.55, 0.85, e.socket), 0.35);

      // Lacquer: the cheek guards below and outside the cheeks, and the brim above; a cream edge line on each.
      const guardD = sub(0.8, add(ax, mul(min(Y, 0), -0.15)));
      const guard = mul(inside(guardD), smoothstep(0.35, 0.25, Y));
      const brimD = sub(0.82, add(Y, mul(mul(X, X), -0.25)));
      const brim = inside(brimD);
      const lacquer = max(guard, brim);
      const lacquerEdge = max(mul(line(guardD, 0.012), smoothstep(0.35, 0.25, Y)), line(brimD, 0.012));
      const rimLine = line(sub(rho, 0.955), 0.012);

      let c = mix(RED, vec3(...I.brickDark), socketShade);
      c = mix(c, BLACK, creases);
      c = mix(c, LAC, lacquer);
      c = mix(c, CREAM, lacquerEdge);
      c = mix(c, BLACK, b.on);
      c = mix(c, eyeInk, max(e.white, line(e.edge, 0.012)));
      c = mix(c, mouthInk, inMouth);
      c = mix(c, BLACK, lipLine);
      return mix(c, BLACK, rimLine);
    };

    const shape = framed(surface(point, { u: ["free", "free"], v: "free" }), { frame: body, scale: lift<"f32">(1) });
    return [draw.surface("mask", shape, { paint, grid: { u: spacing.even(96), v: spacing.even(96) }, sides: "one", outline: "border" })];
  },
};


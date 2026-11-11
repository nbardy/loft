// The spiral shell: the islander's back (docs/art_direction/DIRECTION.md, Round 5; the picked picture
// docs/art_direction/crab_round5/back_spiral_pick.webp). A hermit's shell: one tube coiled by Raup's law
// (back/spiral_shell.ts), lying on its side with its spiral face to the crab's right and a little up, its mouth at the
// front facing down over the carapace's back half; brick bands between cream ridges winding in to a tight curl; a
// red-brick lighthouse on a ring of barnacles on the crown toward the rear; two groves on turf; strings of barnacles
// along the cream ridges.
//
// Everything is placed in crab coordinates from the build's numbers (`n.spiral`, `n.carapace` through the seat, the ride
// height through the waterline) and rides `rig.shell`. Nothing is a world coordinate.

import {
  add, draw, fract, framed, inFrame, lift, max, min, mix, mul, pick, place, rigid, sin, strand, sub, surface, through,
  vec2, vec3, type Drawable, type F, type P3, type Target,
} from "../../../../src/index.ts";
import { hash3, type Branch } from "../../../../src/lib/index.ts";
import { cut, TAU, type V3 } from "../ink.ts";
import type { Numbers } from "../numbers.ts";
import type { Rig } from "../rig.ts";
import type { BackOption } from "../slots.ts";
import { lightOn } from "./lighthouse.ts";
import { bandAt, bandEdges, GRID, gridU, shellMap, shellTwin, U, vmath } from "./spiral_shell.ts";

const { add: addV, mul: mulV, unit: unitV } = vmath;
const hash = (k: number) => { const x = Math.sin(k * 127.1 + 311.7) * 43758.5453; return x - Math.floor(x); };

/** What a copy in the growth list is: the paint dispatches on it (the id's first lane is its index here). */
const KINDS = ["barnacle", "trunk", "crown", "plinth"] as const;
type Kind = (typeof KINDS)[number];
const cone = (kind: Kind, a: V3, b: V3, ra: number, rb: number, level: number, salt: number): Branch => ({ a, b, ra, rb, level, id: [KINDS.indexOf(kind), salt, 0] });

/** A place on the shell: the point and the shell's outward unit normal there (crab coordinates). */
type Site = { readonly at: V3; readonly out: V3 };

/**
 * One barnacle: a squat round cream cone standing out of the shell at a site. The only barnacle this back draws; the
 * growth kit (lib/growth.ts, on the terraces branch) replaces this function with `sprout({ kind: "barnacle" }, site,
 * size, seed)` mapped through `branchOf` (and its paints spread into `paints` below).
 */
function barnacle(site: Site, size: number, salt: number): Branch {
  const a = addV(site.at, mulV(site.out, -0.15 * size)), b = addV(site.at, mulV(site.out, 0.55 * size));
  return cone("barnacle", a, b, 0.62 * size, 0.42 * size, 1, salt);
}

/** A broadleaf tree: a short trunk and three round crown blobs (the headland's, rounder). Trees grow up, not out. */
function tree(foot: V3, h: number, salt: number): Branch[] {
  const lean = [0.2 * (hash(salt) - 0.5), 0.2 * (hash(salt + 1) - 0.5)] as const;
  const top: V3 = [foot[0] + lean[0] * h, foot[1] + h, foot[2] + lean[1] * h];
  const blob = (dx: number, dy: number, dz: number, r: number, j: number) => cone("crown", [top[0] + dx, top[1] + dy, top[2] + dz], [top[0] + dx, top[1] + dy + 0.01, top[2] + dz], r, r, 0, salt * 3 + j);
  return [
    cone("trunk", [foot[0], foot[1] - 0.25, foot[2]], top, 0.13, 0.09, 0, salt),
    blob(0, 0.3, 0, 0.42 * h, 0), blob(0.3, 0.05, 0.22, 0.3 * h, 1), blob(-0.28, 0.1, -0.25, 0.32 * h, 2),
  ];
}

/** The light's neck, classified once from its pose's `rise`: down (nothing built) or risen by an amount. */
type Neck = { readonly kind: "down" } | { readonly kind: "risen"; readonly by: number };
const neckOf = (rise: number): Neck => (rise === 0 ? { kind: "down" } : { kind: "risen", by: rise });
/** The lantern's shutters, classified once from `shutters`: shut (the boards lie on the black lantern: nothing more to
 * draw) or open by a share of their swing. */
type Shutters = { readonly kind: "shut" } | { readonly kind: "open"; readonly by: number };
const shuttersOf = (s: number): Shutters => (s === 0 ? { kind: "shut" } : { kind: "open", by: s });

/** The light's plan in plain numbers: its foot on the shell (sunk a little), its lean, and where its tower stands. */
function lightPlan(n: Numbers, twin: ReturnType<typeof shellTwin>) {
  const L = n.spiral.light, at = twin.at(L.back, L.round), out = twin.normal(L.back, L.round);
  // Leaning back and out: out is the shell's normal laid level, back is the crab's -x; together, then `lean` of it.
  const away = unitV(addV(unitV([out[0], 0, out[2]]), [-0.8, 0, 0]));
  const lean: V3 = [away[0] * L.pose.lean, 1, away[2] * L.pose.lean], dir = unitV(lean);
  const foot = addV(at, mulV(out, -0.2));
  const neck = neckOf(L.pose.rise);
  const towerFoot = neck.kind === "down" ? foot : addV(foot, mulV(dir, neck.by));
  return { L, at, out, foot, lean, dir, neck, towerFoot };
}

export const spiral: BackOption = {
  about: "a hermit's spiral shell: brick bands between cream ridges to a tight curl, a lighthouse on the crown, two groves, barnacle strings",
  // What a thrown ship must fly clear of beyond the atoll's cylinder (the design's `fliesClear`): the shell's plan and
  // the light, each a cylinder riding the shell frame. Both lie inside the atoll's cylinder for the numbers as written;
  // they are stated so that numbers which move the shell or the light out are still checked.
  keepClear: ({ n, shell }) => {
    const twin = shellTwin(n), { towerFoot, dir, L } = lightPlan(n, twin);
    const pts = Array.from({ length: 40 * 24 }, (_, i) => twin.at((Math.floor(i / 24) / 40) * n.spiral.whorls, ((i % 24) / 24) * TAU));
    const lo = [0, 2].map((k) => Math.min(...pts.map((p) => p[k]!))), hi = [0, 2].map((k) => Math.max(...pts.map((p) => p[k]!)));
    const disc = (x: number, z: number, radius: number): Target => {
      const c = add(shell.o, inFrame(shell, vec3(x, 0, z)));
      return { kind: "disc", centre: vec2(c.x, c.z), radius: lift<"f32">(radius) };
    };
    const mid = addV(towerFoot, mulV(dir, L.height / 2));
    return [
      disc((lo[0]! + hi[0]!) / 2, (lo[1]! + hi[1]!) / 2, Math.hypot(hi[0]! - lo[0]!, hi[1]! - lo[1]!) / 2),
      disc(mid[0], mid[2], L.girth[0] + Math.hypot(dir[0], dir[2]) * L.height / 2 + 0.3),
    ];
  },
  draw: (rig: Rig) => {
    const { n, shell } = rig;
    const I = n.ink, SP = n.spiral, m = shellMap(n), p = m.plan, twin = shellTwin(n);
    const ink = (k: V3): P3 => vec3(...k);
    const water = -n.ride;

    // ---- the shell: one surface, painted along the sweep ----
    /** Turns back from the mouth at u (on the outside; beyond the lip it is the lip's and the throat's). */
    const backOf = (u: F): F => mul(sub(1, min(mul(u, 1 / U.lip), 1)), p.whorls);
    /** Turf under a grove: 1 inside its patch (a lumpy round of `radius` on the surface), 0 outside; and its edge. */
    const turf = (back: F, phi: F) => SP.groves.map(([b0, p0, , radius]) => {
      const size = p.W ** -b0;
      const along = mul(sub(back, b0), TAU * (p.c + p.r * Math.cos(p0)) * size), across = mul(mul(sin(mul(sub(phi, p0), 0.5)), 2), p.r * size);
      const d = add(mul(along, along), mul(across, across)), ang = add(mul(along, 1.7), mul(across, 2.3));
      const edge = radius * radius;
      return sub(edge * 1.0, mul(d, add(1, mul(sin(mul(ang, 2.1)), 0.12))));
    });
    const paint = (u: F, v: F): P3 => {
      const phi = m.phiOf(v), back = backOf(u);
      const { ring, across } = bandAt(p, SP.band, phi);
      // Brick with a few growth lines across the tube, unevenly spaced (a shell grows in fits): about three a turn. The
      // first build had 23 a turn and the face read as a ribbed barrel, not as bands.
      const growth = abs1(sub(fract(add(mul(back, 3.1), mul(sin(mul(back, 4.3)), 0.35))), 0.5));
      const brick = mix(ink(I.brick), ink(I.brickDark), mul(sub(1, cut(growth, 0.012)), 0.9));
      const inBand = mul(sub(1, cut(across, 1)), cut(ring, -1.5));
      const key = mul(max(sub(1, cut(abs1(across), 0.07)), sub(1, cut(abs1(sub(across, 1)), 0.07))), cut(ring, -1.5));
      let shellInk: P3 = mix(mix(brick, ink(I.cream), inBand), ink(I.black), key);
      // The two groves' turf: moss on a cream cut edge, inked.
      for (const t of turf(back, phi)) {
        const g = cut(t, 0), rim = mul(cut(t, -0.35), sub(1, g)), line = mul(sub(1, cut(abs1(add(t, 0.35)), 0.04)), 1);
        shellInk = mix(mix(mix(shellInk, ink(I.cream), rim), ink(I.moss), g), ink(I.black), line);
      }
      // The lip: thick cream with a black key line where it turns from the outside; the throat dark brick inside (near
      // black, it read from behind as a hole cut in the shell beside the eyestalks).
      const lipLine = sub(1, cut(abs1(sub(u, U.lip - 0.004)), 0.004));
      const onLip = cut(u, U.lip - 0.002), throat = cut(u, U.throat + 0.01);
      return mix(mix(mix(shellInk, ink(I.cream), onLip), ink(I.black), lipLine), mix(ink(I.brickDark), ink(I.black), 0.2), throat);
    };
    const grid = gridU(n);
    const shellDrawn = draw.surface("spiral", framed(surface(m.point, { u: ["cap", "cap"], v: "periodic" }), { frame: shell, scale: lift<"f32">(1) }), {
      paint, grid: { u: { count: GRID.outer + GRID.roll + GRID.throat, at: grid }, v: { count: GRID.round, at: (s) => s } },
      sides: "one", outline: "hull",
    });

    // ---- what grows on it ----
    const site = (back: number, phi: number): Site => ({ at: twin.at(back, phi), out: twin.normal(back, phi) });
    // Barnacle strings along the cream ridges on the face side: in runs of three to six with rests between, larger
    // low on the shell (by height over the water) and with the tube's size.
    const edges = bandEdges(p, SP.band);
    const strings: Branch[] = [];
    edges.slice(0, 3).forEach(([a, b], k) => {
      const phi = (a + b) / 2;
      let back = 0.04 + 0.1 * hash(k + 3), salt = 500 + 100 * k;
      // Runs of two to five, bunched (a barnacle now and then beside the ridge, not on it), then a rest of an eighth to a
      // third of a turn: the first build's runs of three to six with short rests read as an even row of teeth.
      while (back < 2.2) {
        const run = 2 + Math.floor(4 * hash(salt));
        for (let j = 0; j < run && back < 2.2; j++, salt++) {
          const off = (hash(salt + 7) - 0.5) * (j % 2 === 1 ? 0.35 : 0.1);
          const s = site(back, phi + off);
          if (s.at[1] < water + 0.05) { back += 0.03; continue; }
          const low = 1 - Math.min(1, Math.max(0, (s.at[1] - water) / 6));
          const size = p.r * p.W ** -back * (0.09 + 0.07 * low) * (0.75 + 0.5 * hash(salt + 11));
          strings.push(barnacle(s, size, salt));
          back += (size * (j % 2 === 1 ? 1.2 : 2.0)) / (TAU * (p.c + p.r * Math.cos(phi)) * p.W ** -back);
        }
        back += 0.12 + 0.22 * hash(salt + 31);
      }
    });
    // The groves: trees on the turf, the tallest near the middle.
    const groves = SP.groves.flatMap(([b0, p0, trees, radius], g) => Array.from({ length: trees }, (_, k) => {
      const ang = k * 2.4 + g, r = k === 0 ? 0 : radius * (0.45 + 0.25 * hash(g * 10 + k)), size = p.W ** -b0;
      const back = b0 + (r * Math.cos(ang)) / (TAU * (p.c + p.r * Math.cos(p0)) * size), phi = p0 + (r * Math.sin(ang)) / (p.r * size);
      return tree(twin.at(back, phi), (k === 0 ? 1.05 : 0.8 + 0.2 * hash(g * 10 + k + 5)) * (g === 0 ? 1 : 0.85), 40 + g * 10 + k);
    }).flat());
    // The light's base: a cream plinth grown out of the shell, a ring of barnacles round its foot.
    const LP = lightPlan(n, twin), G = LP.L.girth[0];
    const plinth = cone("plinth", addV(LP.foot, mulV(LP.out, -0.35)), addV(LP.foot, mulV(LP.dir, 0.4)), G * 1.7, G * 1.2, 0, 700);
    const up0 = LP.out, side0 = unitV(vmath.cross(up0, [1, 0, 0])), side1 = vmath.cross(up0, side0);
    const ring = Array.from({ length: 11 }, (_, k) => {
      const a = (k / 11) * TAU + 0.3 * hash(k + 710), rr = G * (1.75 + 0.2 * hash(k + 720));
      const at = addV(addV(LP.foot, mulV(side0, Math.cos(a) * rr)), mulV(side1, Math.sin(a) * rr));
      return barnacle({ at: addV(at, mulV(up0, 0.05)), out: up0 }, 0.3 + 0.12 * hash(k + 730), 740 + k);
    });

    const paints: Record<Kind, (hit: { p: P3; id: P3 }) => P3> = {
      // Barnacles: cream, pitted with small dark specks.
      barnacle: ({ p: q }) => mix(ink(I.cream), ink(I.bark), cut(mul(mul(sin(mul(q.x, 11)), sin(add(mul(q.y, 11), 1))), sin(add(mul(q.z, 11), 2))), 0.42)),
      trunk: () => ink(I.bark),
      crown: ({ p: q, id }) => {
        const swirl = add(sin(add(mul(q.x, 2.5), mul(sin(mul(q.y, 3)), 1.5))), sin(add(mul(q.z, 2.5), mul(sin(mul(q.x, 2.2)), 1.5))));
        return mix(ink(I.mossDeep), ink(I.black), mul(sub(1, cut(abs1(sin(mul(swirl, 2.2))), 0.12)), cut(hash3(id), 0.5)));
      },
      // The plinth: cream stone in rough courses.
      plinth: ({ p: q }) => mix(ink(I.cream), ink(I.stone), sub(1, cut(abs1(sub(fract(add(mul(q.y, 4.5), mul(sin(mul(q.x, 6)), 0.15))), 0.5)), 0.08))),
    };
    const growth = draw.copies("spiral_growth", {
      kind: "cone", copies: [...strings, ...groves, plinth, ...ring], carry: shell,
      paint: (hit) => pick(hit.id.x, KINDS.map((k) => paints[k](hit))),
      outline: "silhouette", minPixels: 1, keepLevels: 2, inkBack: 6,
    });

    // ---- the light: the fiddler's (back/lighthouse.ts), smaller, on the plinth; a neck under it if risen ----
    const light = lightOn(rig, { foot: LP.towerFoot, height: LP.L.height, girth: LP.L.girth, lean: LP.lean, size: LP.L.size, gulls: LP.L.gulls });
    const neck: Drawable[] = (() => {
      switch (LP.neck.kind) {
        case "down": return [];
        case "risen": {
          const by = LP.neck.by, foot = vec3(...LP.foot), dir = vec3(...LP.dir);
          return [place(strand({
            name: "spiral_neck", spine: (s) => add(foot, mul(dir, mul(s, by))), stations: 4, girth: G * 0.85, tips: ["open", "open"], up: [1, 0, 0],
            // Drums: cream rings with dark joints, one a unit.
            paint: (s) => mix(ink(I.cream), ink(I.bark), sub(1, cut(abs1(sub(fract(mul(s, by)), 0.5)), 0.42))),
            detail: { along: 12, around: 16 },
          }), rigid(shell))];
        }
      }
    })();
    const shutters: Drawable[] = (() => {
      const S = shuttersOf(LP.L.pose.shutters);
      switch (S.kind) {
        case "shut": return [];
        case "open": {
          // Two boards hinged at the lantern's top on either side, swung out by `by` of a right angle and a half.
          const k = LP.L.size, top = addV(LP.towerFoot, mulV(LP.dir, LP.L.height + 0.9 * k)), across = unitV(vmath.cross(LP.dir, [0, 0, 1]));
          const swingTo = (sgn: number): V3 => {
            const a = S.by * 1.5 * (Math.PI / 2), r = 0.62 * k;
            const hinge = addV(top, mulV(across, sgn * 0.56 * k));
            return addV(hinge, addV(mulV(across, sgn * Math.sin(a) * r), mulV(LP.dir, -Math.cos(a) * r)));
          };
          return [place(strand({
            name: "spiral_shutters", spine: (s) => through(s, [swingTo(-1), addV(top, mulV(across, -0.56 * k)), top, addV(top, mulV(across, 0.56 * k)), swingTo(1)]),
            stations: 5, girth: 0.3 * k, section: { aspect: 0.2 }, tips: ["round", "round"], up: [LP.dir[0], LP.dir[1], LP.dir[2]],
            paint: () => ink(I.black), detail: { along: 16, around: 6 },
          }), rigid(shell))];
        }
      }
    })();

    return [shellDrawn, growth, ...neck, ...light, ...shutters];
  },
};

const abs1 = (x: F): F => max(x, mul(x, -1));

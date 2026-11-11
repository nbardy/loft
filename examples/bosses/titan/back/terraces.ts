// The terraces: the bruiser's back (docs/art_direction/DIRECTION.md, Round 5; the pictures are
// docs/art_direction/crab_round5/back_paintover_*.webp and back_model_sheet.webp). The island and the shell as ONE
// form: shell plates rise from the rim, lean over as they climb and become terraces that wind in to the crown.
//
// One closed-form surface, and its second coordinate is the thing to understand. A surface swept round an axis usually
// takes the bearing as its v, so its edges are rings (the headland: cap, cliff, tier, each closed by a level line). Here
// v is the **thread** of a screw:
//
//   bearing(G, v) = warp(TAU * (psi(G) / 3 - v) + phase)        G: 0 at the crown, 1 at the rim
//
// `psi` is how many thread groups lie between the crown and G (1.9 in all). A grid column (constant v) is therefore a
// line that leaves the rim straight up (psi is flat there), leans over as it climbs and winds half a turn round the cap.
// Three groups go round; within a group `fract(3 v)` is the place across it (KNOT): two blades and one broad plate,
// each followed by a recessed band. Every edge of the form is a column, and every ink line (rib, stratum, turf line) is
// a line of the same coordinate, so the form's lines run on through shell, stone and turf.
//
// Across a group the hull is displaced two ways, blended by height (`terraced`):
// - low, each plate is a shingle: a convex lobe swelling out of the hull to a lip, then a band falling back to the hull;
// - high, the group is one terrace. DERIVATION (level treads): let p = psi(G) and f = fract(3 v), so the group's cell
//   starts at p0 = p - f on this bearing. Take the hull's radius at psi = p0 + A(f) and its height at psi = p0 + B(f),
//   with A = min(f / lip, 1) and B = max((f - lip) / (1 - lip), 0). For f below the lip the height is the hull's at p0
//   whatever f is: a level tread whose radius runs the cell's whole width; beyond the lip the radius holds and the
//   height falls: a riser. A cell that starts above the crown (p0 < 0) is clamped to it, so the three treads meet there
//   in a pinwheel. At the lip and at f = 0 the surface has a crease; both are grid columns (KCOL).
//
// Below the rim the shore is one of the headland's four kinds at each bearing (its `Shore` and `blend`): shell (a cream
// belly into the water), beach (sand under the plates' lifted rim), apron (the reef shelf the stacks stand on) or cape
// (the point, with a mound for the light).
//
// Everything hangs off the shell frame (`rig.shell`) and the build's numbers: the island's centre and radii (`n.atoll`),
// the carapace's rim line (`n.carapace`), the ride (the water) and `n.terraces`. Nothing is a world coordinate. Growth
// is the kit's (examples/bosses/lib/growth.ts), placed at named places of this surface (`thread`, `ashore`, `onPoint`).

import {
  abs, add, clamp, compileJs, cos, div, draw, floor, fract, framed, inFrame, lift, max, min, mix, mul, pick, pow, saturate, sin, smoothstep, sqrt, sub, surface,
  tabulate, variable, vec2, vec3, vec4, type Drawable, type F, type P3, type Target,
} from "../../../../src/index.ts";
import type { Branch } from "../../../../src/lib/index.ts";
import { branchOf, cluster, GROWTH_KINDS, growthPaints, RECIPES, run, sprout, type Site, type Sprig } from "../../lib/growth.ts";
import { cut, TAU, type V3 } from "../ink.ts";
import type { Numbers } from "../numbers.ts";
import type { BackOption } from "../slots.ts";
import { beaconOn } from "./beacon.ts";
import { blend, broadleaf, cape, crownPaint, hash, rockPaint, stations } from "./headland.ts";

/** The grid: rows from the crown to under the water (the rim on row ROW_RIM), columns round (48 to a group). */
const NU = 46, NV = 144, GROUPS = 3, ROW_RIM = 30;
/** The rim's u: above it u / U_RIM is G, below it the shore runs to u = 1. */
const U_RIM = 0.74;
/** Across a group (f = fract(3 v)): where each plate starts and where its lip is: blade, blade, broad plate; the bands
 * lie between a lip and the next start. And the grid columns that fall on those lines (of 48; even, so the grid's
 * halved levels keep every crease). */
const KNOT = [0, 0.18, 0.24, 0.44, 0.5, 0.86, 1] as const, KCOL = [0, 10, 14, 26, 30, 42, 48] as const;
const [, L1, S2, L2, S3, L3] = KNOT;
/** The shore's stations below the rim (sigma: 0 the rim, 1 closed under the water): under the lip, the far edge of
 * the top, the waterline, the foot. */
const SIGMA = [0, 0.1, 0.6, 0.75, 0.87, 1] as const;
/** Rib lines to a group. */
const RIBS = 22;

/** What a copy in the reef list is (the id's first lane is its index here): the kit's growth, then this back's own. */
const KINDS = [...GROWTH_KINDS, "trunk", "crown", "rock"] as const;
type Kind = (typeof KINDS)[number];
const cone = (kind: Kind, a: V3, b: V3, ra: number, rb: number, level: number, salt: number, mark = 0): Branch =>
  ({ a, b, ra, rb, level, id: [KINDS.indexOf(kind), salt, mark] });

const rad = (degrees: number): number => (degrees * Math.PI) / 180;

/** The plan in plain numbers, from the build's. Lengths become shares of the atoll's mean radius; bearings radians. */
function planOf(n: Numbers) {
  const A = n.atoll, T = n.terraces, R = (A.rx + A.rz) / 2;
  const mid = (T.outline.bow + T.outline.tail) / 2, egg = (T.outline.tail - T.outline.bow) / 2;
  /** A point `rho` (a share of the radii) out from the island's centre at a bearing (crab x, z). */
  const polar = (rho: number, bearing: number): readonly [number, number] => [A.centre[0] + rho * A.rx * Math.cos(bearing), A.centre[1] + rho * A.rz * Math.sin(bearing)];
  const point = { bearing: rad(T.point.bearing), long: T.point.tip / R, half: T.point.half / R };
  const beach = { bearing: rad(T.beach.bearing), long: T.outline.tail + T.beach.sand / R, half: Math.sin(rad(T.beach.span)) * T.outline.tail * 1.12 };
  const shelf = { bearing: rad(T.shelf.bearing), long: mid + T.shelf.reach / R, half: T.shelf.half / R };
  return { R, mid, egg, polar, point, beach, shelf, lightRho: T.light.at / R };
}

export const terraces: BackOption = {
  about: "one screw of shell plates that become terraces: a point with a crusted light, a beach under the plates, three stacks",
  // What a thrown ship must fly clear of beyond the atoll's cylinder (the design's `fliesClear`): the rim with its
  // lobes, the point from the rim to its tip, the shelf with its stacks, and the light as it leans. They ride the shell.
  keepClear: ({ n, shell }) => {
    const { R, polar, point, shelf, lightRho } = planOf(n), T = n.terraces, L = T.light;
    const disc = ([x, z]: readonly [number, number], radius: number): Target => {
      const c = add(shell.o, inFrame(shell, vec3(x, 0, z)));
      return { kind: "disc", centre: vec2(c.x, c.z), radius: lift<"f32">(radius) };
    };
    const rim = T.outline.tail + 2 * T.outline.lumps, reach = (L.height + 1.5 * L.size) * L.lean, foot = polar(lightRho, point.bearing);
    const farthest = Math.max(...T.stacks.map(([r]) => r / R));
    return [
      disc(n.atoll.centre, rim * Math.max(n.atoll.rx, n.atoll.rz) + Math.max(...T.lobes)),
      disc(polar((rim + point.long) / 2, point.bearing), Math.max(((point.long - rim) / 2) * R, point.half * R) + 0.3),
      disc(polar((rim + farthest) / 2, shelf.bearing), Math.max(((farthest - rim) / 2) * R, shelf.half * R) + 0.6),
      disc([foot[0] + Math.cos(point.bearing) * reach / 2, foot[1] + Math.sin(point.bearing) * reach / 2], L.girth[0] * 1.2 + reach / 2 + 0.3),
    ];
  },
  draw: (rig) => {
    const { n, shell } = rig;
    const I = n.ink, A = n.atoll, T = n.terraces, C = n.carapace;
    const { R, mid, egg, polar, point: POINT, beach: BEACH, shelf: SHELF, lightRho } = planOf(n);
    // Heights (crab y): the still water, the carapace's rim line (carapace/base.ts inks it at sin = -0.15 of its
    // half-height), the crown.
    const water = -n.ride, rimY = -0.15 * C.height, crownY = A.top + T.crown;
    const ink = (k: V3): P3 => vec3(...k);

    // ---- the hull and the screw ----
    /** The hull's radius (a share of the outline) and height at G: a mound, steep at the rim like the carapace's flank
     * (the radius is a quarter turn's sine; the height falls as G^1.6, so the cap is a low cone and its risers stand 0.6 to 1.5). */
    const quarter = (G: F): F => mul(clamp(G, 0, 1), Math.PI / 2);
    const hullR = (G: F): F => pow(max(sin(quarter(G)), 1e-6), T.cone);
    /** The rim's height at a bearing: the carapace's rim line beside the carapace; round the tail, where the island
     * stands over open water, it comes down toward the water (the plates run down to the sea, as the pictures have them;
     * a rim at the carapace's height all round left a cream plinth under the island). */
    const rimLow = water + T.rim, rimAt = (a: F): F => mix(rimLow, rimY, smoothstep(-0.55, 0.2, cos(a)));
    const hullY = (G: F, a: F): F => { const rim = rimAt(a); return add(rim, mul(sub(crownY, rim), sub(1, pow(max(clamp(G, 0, 1), 1e-6), 1.6)))); };
    /** Thread groups between the crown and G, and its inverse. */
    const TURNS = T.screw.turns, LEAN = T.screw.lean;
    const psi = (G: F): F => mul(TURNS, sub(1, pow(max(sub(1, clamp(G, 0, 1)), 1e-6), LEAN)));
    const levelOf = (p: F): F => sub(1, pow(max(sub(1, div(clamp(p, 0, TURNS), TURNS)), 1e-6), 1 / LEAN));
    /** The groups are wider toward `widest`: the bearing of an even screw, warped. */
    const UNEVEN = T.screw.uneven, WIDEST = rad(T.screw.widest), PHASE = rad(T.screw.phase);
    const warp = (a0: F): F => add(a0, mul(UNEVEN, sin(sub(a0, WIDEST))));
    const bearingAt = (G: F, v: F): F => warp(add(mul(sub(div(psi(G), GROUPS), v), TAU), PHASE));

    // ---- outlines: radius (a share of the atoll's radii) over the bearing a ----
    /** The rim: an egg, narrow at the bow, with slight lumps. */
    const rimOf = (a: F): F => add(sub(mid, mul(egg, cos(a))), mul(add(sin(add(mul(a, 3), 1.1)), mul(sin(add(mul(a, 5), 2.3)), 0.7)), T.outline.lumps));
    const pointOut = (a: F): F => cape(a, POINT.bearing, POINT.long, POINT.half);
    const sandEdge = (a: F): F => cape(a, BEACH.bearing, BEACH.long, BEACH.half);
    const shelfEdge = (a: F): F => cape(a, SHELF.bearing, SHELF.long, SHELF.half);
    /** How much the shore at a bearing is each kind but shell (which is the rest): rock past the rim wins over sand. */
    const capeAt = (a: F): F => smoothstep(0, 0.06, sub(pointOut(a), rimOf(a)));
    const apronAt = (a: F): F => smoothstep(0, 0.06, sub(shelfEdge(a), rimOf(a)));
    const beachAt = (a: F): F => smoothstep(0.01, 0.08, sub(sandEdge(a), rimOf(a)));
    const shoreAt = (a: F) => blend({ beach: beachAt(a), apron: apronAt(a), cape: capeAt(a) });

    /** The point's top at (rho, a): low rock running out, a mound where the light stands, high at its root (it comes
     * out from under the plates), rounded across. Crab y. */
    const capeTop = (rho: F, a: F): F => {
      const s = mul(mul(rho, R), cos(sub(a, POINT.bearing))), t = mul(mul(rho, R), sin(sub(a, POINT.bearing)));
      const from = sub(s, lightRho * R), mound = sub(1, smoothstep(0.35, 1.35, sqrt(add(mul(from, from), mul(t, t)))));
      const root = sub(1, smoothstep(T.outline.tail * R, T.outline.tail * R + 1.3, s));
      const across = mul(div(t, POINT.half * R), div(t, POINT.half * R));
      return add(water, sub(add(add(T.point.low, mul(mound, T.point.top - T.point.low)), mul(root, T.rim - T.point.low - 0.1)), mul(across, 0.18)));
    };

    // ---- the surface ----
    /** Across a group: how far a plate's lobe stands out at f (0 at its start, its amount at its lip, back to 0 over the band). */
    const lobes = (f: F): F => [[KNOT[0], L1, S2, T.lobes[0]], [S2, L2, S3, T.lobes[1]], [S3, L3, 1, T.lobes[2]]].map(([from, lip, next, amount]) => {
      const x = saturate(div(sub(f, from!), lip! - from!)), fall = saturate(div(sub(f, lip!), next! - lip!));
      return mul(mul(sub(1, mul(sub(1, x), sub(1, x))), sub(1, fall)), amount!);
    }).reduce((s, x) => add(s, x));
    /** 1 on the terraces, 0 on the plates. */
    const terraced = (G: F): F => sub(1, smoothstep(T.terraced[0], T.terraced[1], G));
    const islandPoint = (u: F, v: F): P3 => {
      const G = min(div(u, U_RIM), 1), sigma = saturate(div(sub(u, U_RIM), 1 - U_RIM));
      const a = bearingAt(G, v), f = fract(mul(v, GROUPS)), p = psi(G), tau = terraced(G), shore = shoreAt(a);
      // The terrace (see the DERIVATION at the top): the hull's radius and height read at two places in the cell.
      const Gr = levelOf(add(p, mul(tau, sub(min(div(f, L3), 1), f)))), Gy = levelOf(add(p, mul(tau, sub(max(div(sub(f, L3), 1 - L3), 0), f))));
      // The shingle: out along the hull's normal (level at the rim, up at the crown).
      const lobe = mul(lobes(f), sub(1, tau)), rim = rimOf(a);
      const hullRho = add(mul(hullR(Gr), rim), mul(lobe, div(sin(quarter(G)), R)));
      // Over the beach the rim lifts: the plates arch over the sand.
      const hullHeight = add(add(hullY(Gy, a), mul(lobe, cos(quarter(G)))), mul(mul(beachAt(a), T.beach.arch), smoothstep(0.7, 1, G)));
      // Below the rim, by the kind of shore: [under the lip, the far edge of the top, the waterline, the foot].
      const sand = sandEdge(a), ledge = shelfEdge(a), tip = pointOut(a);
      const rhoAt = (k: 0 | 1 | 2 | 3): F => shore({
        shell: sub(rim, [0.01, 0.035, 0.05, 0.06][k]!), beach: [sub(rim, 0.035), sub(sand, 0.02), add(sand, 0.01), sand][k]!,
        apron: [rim, sub(ledge, 0.03), ledge, sub(ledge, 0.01)][k]!, cape: [add(rim, 0.04), sub(tip, 0.05), tip, sub(tip, 0.01)][k]!,
      });
      const rho = stations(sigma, SIGMA, [hullRho, rhoAt(0), rhoAt(1), rhoAt(2), rhoAt(3), lift<"f32">(0)]);
      const top = capeTop(rho, a);
      const yAt = (k: 0 | 1 | 2 | 3): F => shore({
        shell: [sub(rimAt(a), 0.12), lift<"f32">(water + 0.1), lift<"f32">(water - 0.3), lift<"f32">(A.base)][k]!, beach: lift<"f32">([water + 0.5, water + 0.05, water - 0.25, A.base][k]!),
        apron: lift<"f32">([water + 0.42, water + 0.2, water - 0.2, A.base][k]!), cape: [top, top, lift<"f32">(water - 0.2), lift<"f32">(A.base)][k]!,
      });
      const y = stations(sigma, SIGMA, [hullHeight, yAt(0), yAt(1), yAt(2), yAt(3), lift<"f32">(A.base - 0.2)]);
      return vec3(add(A.centre[0], mul(mul(rho, A.rx), cos(a))), y, add(A.centre[1], mul(mul(rho, A.rz), sin(a))));
    };

    // What the inks read of the shore, per bearing, from one static table (x: the even screw's bearing over a turn).
    const shoreTable = tabulate("terraces_shore", (x) => { const a = warp(add(mul(x, TAU), PHASE)); return vec4(capeAt(a), apronAt(a), beachAt(a), 0); }, 1025, "static");
    const islandPaint = (u: F, v: F): P3 => {
      const G = min(div(u, U_RIM), 1), sigma = saturate(div(sub(u, U_RIM), 1 - U_RIM)), f = fract(mul(v, GROUPS));
      const at = shoreTable(fract(sub(div(psi(G), GROUPS), v)));
      const shore = blend({ beach: cut(at.z, 0.5), apron: cut(at.y, 0.5), cape: cut(at.x, 0.5) });
      // Which plate this is, and where across it (0 its start, 1 its lip, beyond 1 its band).
      const second = cut(f, S2), third = cut(f, S3);
      const from = add(mul(second, S2), mul(third, S3 - S2)), lip = add(L1, add(mul(second, L2 - L1), mul(third, L3 - L2)));
      const x = div(sub(f, from), sub(lip, from));
      // The broad plate's band is stone from the rim to the crown (low a recess, high a riser). A blade's band is stone
      // only while it is a blade.
      const band = max(third.mul(cut(f, L3)), mul(cut(x, 1), mul(sub(1, third), cut(G, T.terraced[0] + 0.07))));
      // Ribs: lines of the thread. On the plates dark red, close; on the turf every fourth, broken into tufts.
      // (`1 - cut(|x - 0.5|, w)` is 1 inside the half-width w: w is half the line's share of its period.)
      const rib = sub(1, cut(abs(sub(fract(mul(f, RIBS)), 0.5)), 0.085));
      const plate = mix(mix(mix(ink(I.brick), ink(I.brickDark), mul(rib, 0.85)), ink(I.cream), mul(cut(x, 0.88), sub(1, cut(x, 0.965)))), ink(I.black), cut(x, 0.965));
      const tuft = mul(sub(1, cut(abs(sub(fract(add(mul(f, RIBS / 5.5), 0.3)), 0.5)), 0.035)), cut(sin(add(mul(G, 52), mul(f, 31))), 0.2));
      const turf = mix(ink(I.moss), ink(I.mossDeep), tuft);
      // Red reaches highest along a plate's lip and ends in a point there; turf hangs over the rest in tongues on the ribs.
      const red = cut(G, add(add(T.tip, mul(T.tongue, pow(max(sub(1, x), 1e-4), 1.3))), mul(cos(mul(f, RIBS * TAU)), 0.014)));
      // Stone: pale, with strokes across the band (down a riser).
      const stroke = sub(1, cut(abs(sub(fract(add(mul(G, 19), mul(sin(mul(f, 37)), 0.12))), 0.5)), 0.07));
      const stone = mix(ink(I.stone), ink(I.bark), mul(stroke, 0.8));
      // Key lines where the form creases: at the broad plate's lip and its band's foot everywhere, at a blade's while it is a blade.
      const blade = cut(G, T.terraced[0] + 0.07);
      const near = (at: number): F => sub(1, cut(abs(sub(f, at)), 0.0045));
      const crease = max(max(near(L3), max(near(0), near(1))), mul(blade, max(max(near(L1), near(S2)), max(near(L2), near(S3)))));
      const above = mix(mix(mix(turf, plate, red), stone, band), ink(I.black), crease);

      // Below the rim, by the kind of shore.
      const dots = mul(sin(mul(v, 67 * TAU)), sin(mul(sigma, 60)));
      // Shell: the carapace's black rim line; under it the shell's dark underside (cream, as the carapace's belly is, read as a plinth).
      const belly = mix(ink(I.black), ink(I.brickDark), cut(sigma, 0.03));
      // Beach: dark under the plates' lip, then sand, speckled.
      const sandInk = mix(ink(I.bark), mix(ink(I.cream), ink(I.stone), cut(dots, 0.8)), cut(sigma, SIGMA[1]));
      // Rock (the shelf, the point): stone with level strata, cracked on top, dark and wet at the water.
      const strata = sub(1, cut(abs(sub(fract(add(mul(sigma, 26), mul(sin(mul(v, 40 * TAU)), 0.3))), 0.5)), 0.09));
      const rock = mix(mix(ink(I.stone), ink(I.bark), mul(strata, 0.85)), ink(I.bark), cut(sigma, SIGMA[3] - 0.035));
      // The point's top is paler, crusted: cream with stone blots (the light's crust runs down into it).
      const crust = mix(mix(ink(I.cream), ink(I.stone), cut(dots, 0.35)), rock, cut(sigma, SIGMA[2] - 0.02));
      const below = shore({ shell: belly, beach: sandInk, apron: rock, cape: crust });
      return mix(above, below, cut(u, U_RIM + 0.002));
    };

    // Rows: ROW_RIM of them from the crown to the rim, the rest for the shore. Columns: the knots of each group on their
    // grid lines (KCOL), even between.
    const rows = (s: F): F => stations(s, [0, ROW_RIM / NU, 1], [lift<"f32">(0), lift<"f32">(U_RIM), lift<"f32">(1)]);
    const columns = (s: F): F => {
      const x = mul(s, GROUPS), group = floor(x);
      return div(add(group, stations(sub(x, group), KCOL.map((k) => k / (NV / GROUPS)), KNOT.map((k) => lift<"f32">(k)))), GROUPS);
    };
    const island = draw.surface("island", framed(surface(islandPoint, { u: ["cap", "cap"], v: "periodic" }), { frame: shell, scale: lift<"f32">(1) }), {
      paint: islandPaint, grid: { u: { count: NU, at: rows }, v: { count: NV, at: columns } }, sides: "one", outline: "hull",
    });

    // ---- the same closed forms on the CPU, for placing things on the surface ----
    const UV = [variable("terraces_u", "f32"), variable("terraces_v", "f32")] as const;
    const twin = <X>(e: F | P3) => { const g = compileJs([e]); return (x: number, y: number): X => g({ vars: { terraces_u: x, terraces_v: y }, inputs: {}, params: {} })[0] as X; };
    const onIsland = twin<V3>(islandPoint(UV[0], UV[1]));
    const psiJs = ((g) => (G: number) => g(G, 0))(twin<number>(psi(UV[0])));
    const capeTopJs = twin<number>(capeTop(UV[0], UV[1]));
    /** The site at (u, v): the surface's point and its outward normal, by differences. */
    const siteAt = (u: number, v: number): Site => {
      const p = onIsland(u, v), e = 2e-3, du = onIsland(Math.min(u + e, 1), v), dv = onIsland(u, v + e);
      const a: V3 = [du[0] - p[0], du[1] - p[1], du[2] - p[2]], b: V3 = [dv[0] - p[0], dv[1] - p[1], dv[2] - p[2]];
      const c: V3 = [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]], l = Math.hypot(c[0], c[1], c[2]) || 1;
      // Outward: away from the island's axis, or up where the surface is level.
      const sign = c[0] * (p[0] - A.centre[0]) + c[2] * (p[2] - A.centre[1]) + c[1] * 2 >= 0 ? 1 : -1;
      return { at: p, out: [(c[0] / l) * sign, (c[1] / l) * sign, (c[2] / l) * sign] };
    };
    /** Named places. `thread`: on the hull at G, in thread group `group` (0 to 2), `f` across it. */
    const thread = (group: number, f: number, G: number): Site => siteAt(G * U_RIM, (group + f) / GROUPS);
    /** The v of a bearing at G: the screw's warp undone (a fixed point; it contracts by `uneven` a round). */
    const vOf = (G: number, bearing: number): number => {
      let a0 = bearing;
      for (let k = 0; k < 24; k++) a0 = bearing - UNEVEN * Math.sin(a0 - WIDEST);
      return psiJs(G) / GROUPS - (a0 - PHASE) / TAU;
    };
    /** On the hull at G, at a bearing. */
    const facing = (G: number, bearing: number): Site => siteAt(G * U_RIM, vOf(G, bearing));
    /** On the shore below the rim at a bearing: sigma 0 the rim, SIGMA's stations below it. */
    const ashore = (sigma: number, bearing: number): Site => siteAt(U_RIM + sigma * (1 - U_RIM), vOf(1, bearing));
    /** The thread group and the place across it that lie at a bearing on the rim. */
    const threadAt = (bearing: number): { readonly group: number; readonly f: number } => {
      const x = (((vOf(1, bearing) % 1) + 1) % 1) * GROUPS;
      return { group: Math.floor(x), f: x - Math.floor(x) };
    };
    /** The valley (a band's foot, where a plate starts) nearest a bearing on the rim, as a curve up it: t = 0 at the
     * rim, 1 where the plates give way to terraces. Growth gathers in these. */
    const valley = (bearing: number): ((t: number) => Site) => {
      const { group, f } = threadAt(bearing), starts = [0, S2, S3, 1], foot = starts.reduce((best, s) => (Math.abs(s - f) < Math.abs(best - f) ? s : best));
      return (t) => thread(group, foot - 0.012, 0.99 - t * (0.99 - T.terraced[1]));
    };

    // ---- growth: a few large clusters at low seams, thinning upward, rests between (the kit's recipes) ----
    const P = rad(T.point.bearing), lightFoot = polar(lightRho, P), moundY = capeTopJs(lightRho, P);
    const mound: Site = { at: [lightFoot[0], moundY, lightFoot[1]], out: [0, 1, 0] };
    /** A site on the mound's flank, `far` from the light's foot toward a bearing (radians, crab), sunk as the mound falls. */
    const onMound = (far: number, toward: number, drop: number): Site => ({ at: [mound.at[0] + Math.cos(toward) * far, moundY - drop, mound.at[2] + Math.sin(toward) * far], out: unitOf([Math.cos(toward) * 0.7, 0.7, Math.sin(toward) * 0.7]) });
    const G0 = T.light.girth[0];
    const growth: Sprig[] = [
      // The light's foot: crust all round it, thickest to seaward; a seam's worth where the mound meets the water.
      ...[0, 1, 2, 3, 4].flatMap((k) => cluster(RECIPES.crust, onMound(G0 * 1.15, P + 0.4 + k * 1.26, 0.12), k === 0 ? 0.95 : 0.7, 500 + k)),
      ...cluster(RECIPES.seam, onMound(1.25, P - 0.9, 0.42), 1.25, 510),
      ...cluster(RECIPES.reef, onMound(1.3, P + 1.6, 0.45), 1.0, 511),
      // Three seams: where the point leaves the plates, between the beach and the shelf, and on the right flank.
      ...run(valley(P + 0.42), { recipe: RECIPES.seam, clusters: 4, size: 1.25, seed: 21 }),
      ...run(valley(rad(T.beach.bearing - T.beach.span - 9)), { recipe: RECIPES.seam, clusters: 4, size: 1.15, seed: 22 }),
      ...run(valley(rad(96)), { recipe: RECIPES.seam, clusters: 3, size: 1.0, seed: 23, thin: 0.65 }),
      // The shelf: a reef's worth at the foot of the tallest stack.
      ...cluster(RECIPES.reef, ashore(0.35, rad(T.shelf.bearing + 9)), 1.2, 30),
      // Beads at the plates' tips, where each turns to turf (the pictures' signature): large on the three broad plates,
      // small on the blades.
      ...[0, 1, 2].flatMap((g) => [
        ...cluster(RECIPES.bead, thread(g, L3 - 0.03, T.tip + 0.03), 0.95, 40 + g),
        ...cluster(RECIPES.bead, thread(g, L1 - 0.025, T.tip + 0.04), 0.6, 43 + g), ...cluster(RECIPES.bead, thread(g, L2 - 0.025, T.tip + 0.04), 0.65, 46 + g),
      ]),
      // Where the island meets the carapace, either side of the head: crust creeping forward onto the shell.
      ...[-58, 52].flatMap((b, k) => cluster(RECIPES.crust, facing(0.74, rad(b)), 0.75, 50 + k)),
    ];

    // ---- trees: two groves on the treads, open turf between ----
    const trees = T.groves.flatMap(([G, bearing, count], g) => Array.from({ length: count }, (_, k) => {
      // Round the grove's middle, not in a row: each tree its own step of bearing and height.
      const turn = (k / count) * TAU + g, far = k === 0 ? 0 : 0.085;
      const foot = facing(Math.max(G + far * Math.cos(turn), 0.04), rad(bearing) + (far * 5.5 * Math.sin(turn)) / Math.max(G * 4, 0.6)).at;
      return broadleaf(cone, foot, 1.15 + 0.4 * hash(g * 7 + k), 40 + g * 10 + k);
    }).flat());

    // ---- the stacks on the shelf: three tapered forms, shrinking outward; boulders on the sand ----
    const stacks = T.stacks.map(([r, bearing, tall, girth], k) => {
      const [x, z] = polar(r / R, rad(bearing)), lean = 0.16 * (hash(200 + k) - 0.5);
      return cone("rock", [x, A.base - 0.3, z], [x + lean, water + tall, z - lean], girth, girth * 0.42, 0, 200 + k, water);
    });
    const boulders = [0.3, 0.72].map((f, k) => {
      const p = ashore(0.42, rad(T.beach.bearing + T.beach.span * (2 * f - 1) * 0.7)).at, s = 0.2 + 0.07 * k;
      return cone("rock", [p[0], p[1], p[2]], [p[0] + 0.05, p[1] + 0.1, p[2]], s, s * 0.8, 0, 230 + k, p[1] - s);
    });

    const paints: Record<Kind, (hit: { p: P3; normal: P3; id: P3 }) => P3> = {
      ...growthPaints(I), trunk: () => ink(I.bark), crown: crownPaint(I), rock: rockPaint(I),
    };
    const reefList = draw.copies("reef", {
      kind: "cone",
      copies: [...growth.map((g) => branchOf(g, KINDS.indexOf(g.kind))), ...trees, ...stacks, ...boulders],
      carry: shell,
      // One thin dispatcher on the copy's kind; p is in the crab's coordinates, so every pattern rides it.
      paint: (hit) => pick(hit.id.x, KINDS.map((k) => paints[k](hit))),
      outline: "silhouette", minPixels: 1, keepLevels: 2, inkBack: 6,
    });

    // ---- the light, on the mound ----
    const L = T.light;
    const light: Drawable[] = beaconOn(rig, {
      foot: [mound.at[0], moundY - 0.3, mound.at[2]], out: [Math.cos(P), Math.sin(P)], height: L.height, girth: L.girth, crust: L.crust, size: L.size, gulls: L.gulls,
      lean: L.lean, rise: L.rise, shutters: L.shutters,
    });

    return [island, reefList, ...light];
  },
};

const unitOf = (a: V3): V3 => { const l = Math.hypot(a[0], a[1], a[2]); return [a[0] / l, a[1] / l, a[2] / l]; };

// Woodblock: a colour print. Bodies are flat areas from a fixed palette of five inks (indigo, a paler blue, cream,
// one vermilion accent, black), each chosen by the lit value of the paint in bands, so nothing on a body is a
// gradient. The black key line is the outline hull: geometry, thick and of even weight, antialiased by the frame's
// own multisampling. (It was a post pass that cut lines from depth and normal edges, one decision per pixel: the lines
// came out ragged and broken beside the hull outlines of the other styles, and it cost 0.37 ms a frame. 2026-11-11.)
// Then paper. The sea is flat blues with printed lines along the swell's contours and foam where it meets a body, in one
// of several patterns (`ContactFoam`).

import { abs, add, and, atan2, distance, div, dot, f, floor, fract, gt, length, lt, max, min, mix, mul, param, saturate, select, sin, smoothstep, step, sub, time, vec2, vec3, vec4, type Expr } from "../expr.ts";
import { noise2, hash22, luminance } from "../lib/index.ts";
import { noise3 } from "../lib/noise.ts";
import { fwidth, pixelWorld } from "../lib/wgsl.ts";
import type { PostSpec } from "../compile/context.ts";
import type { Contact, Rgb, ShadeCtx, Style, WaterCtx } from "../style.ts";
import { blockInk, castOf, keyOf, light, lit, LIGHT, markOf, overlapOf, printedBlock, pullOf, type BlockInk, type Key, type LightRecipe } from "./light.ts";
import { edgeOf, POST_COMMON, rgb } from "./shared.ts";
import { along, curledHeads, rollingFoam, rollOf, type White } from "./whitewater.ts";

/** The five inks. Everything a body shows is one of these (antialiased edges aside), unless a scene states its own. */
export const WOODBLOCK_PALETTE = {
  black: [0.075, 0.07, 0.09],
  indigo: [0.13, 0.2, 0.37],
  pale: [0.47, 0.6, 0.72],
  cream: [0.93, 0.88, 0.75],
  vermilion: [0.82, 0.25, 0.16],
} as const satisfies Record<string, Rgb>;

/** Sea inks: the print's water is its own blues (not body inks), darker in the troughs; haze at the horizon. */
export const WOODBLOCK_SEA = { deep: [0.11, 0.18, 0.35], mid: [0.16, 0.26, 0.45], line: [0.58, 0.68, 0.78], haze: [0.7, 0.76, 0.78] } as const satisfies Record<string, Rgb>;
const SEA = WOODBLOCK_SEA;

/**
 * The key block's four inks: the bands a body prints in by value (black, indigo, pale, cream), and what the sky, the
 * foam, the shadows and the paper are printed in. A scene may state its own.
 */
export type WoodblockInks = { readonly black: Rgb; readonly indigo: Rgb; readonly pale: Rgb; readonly cream: Rgb };

/**
 * Value bands, in lit luminance: below `black` the key block's black, then indigo, the pale blue, and cream. A paint
 * whose red stands above both its green and blue by `warm` prints vermilion whatever its value.
 */
const BAND = { black: 0.08, indigo: 0.34, pale: 0.6, warm: 0.28 } as const;
/** Two tones of light: a paint's luminance is multiplied by these on its shadow and lit sides before the bands. */
const TONES = { shadow: 0.66, lit: 1.06 } as const;

// ---- which paints print in which inks -------------------------------------------------------------------------------

/**
 * How a paint is told to stand out as the accent: `red` when its red stands above both green and blue (vermilion's
 * test), `yellow` when red and green both stand above blue (lantern yellow's).
 */
export type AccentHue = "red" | "yellow";
/** One accent ink, printed where the paint's luminance is at least `from` (the first from 0; a paler one above it). */
export type AccentInk = { readonly ink: Rgb; readonly from: number };
/** The warm ink: paints of `hue` standing out by `margin` print in it, flat (unlit: the accent is its own light). */
export type Accent = { readonly hue: AccentHue; readonly margin: number; readonly inks: readonly AccentInk[] };

/**
 * Which paints print in which inks (the rule a scene states):
 *  - `bands`: by value. The paint's luminance times a two-tone light, in bands of the key block's inks; an accent
 *    paint prints the accent. The print's own rule. A paint within `reach` (rgb distance) of a `named` key prints in
 *    that key's two inks instead: the one blue sail in a print that is otherwise black, indigo, cream and one warm ink.
 *  - `keys`: by name. Each key is a paint colour that prints in two inks of its own (in shadow, lit); a paint prints as
 *    the nearest key. A palette per monster (slate and olive coils); a key whose two inks are one is flat.
 *  - `overprint`: as painted. Every paint is already an ink: the lit side prints it, the shadow side prints it
 *    overprinted by `ink` (`amount` of it, then times `value`); a paint darker than `black` prints the key black.
 */
export type Print =
  | { readonly kind: "bands"; readonly accent: Accent; readonly named: readonly InkKey[]; readonly reach: number }
  | { readonly kind: "keys"; readonly keys: readonly InkKey[] }
  | { readonly kind: "overprint"; readonly ink: Rgb; readonly amount: number; readonly value: number; readonly black: number };
/** A key of the `keys` rule: the paint colour that names it, and the inks it prints in. */
export type InkKey = { readonly key: Rgb; readonly shadow: Rgb; readonly lit: Rgb };

const VERMILION: Accent = { hue: "red", margin: BAND.warm, inks: [{ ink: WOODBLOCK_PALETTE.vermilion, from: 0 }] };

export const print = {
  /** By value in bands, with the accent (vermilion for red paints by default). */
  bands(o: { readonly accent?: Accent; readonly named?: Readonly<Record<string, InkKey>>; readonly reach?: number } = {}): Print {
    const accent = o.accent ?? VERMILION;
    if (accent.inks.length === 0 || accent.inks[0]!.from !== 0) throw new Error("woodblock accent: give at least one ink, the first from 0");
    accent.inks.forEach((k, i) => { if (i > 0 && !(k.from > accent.inks[i - 1]!.from)) throw new Error(`woodblock accent: ink ${i} must start above ink ${i - 1} (from ${k.from})`); });
    const reach = o.reach ?? 0.2;
    if (!(reach > 0)) throw new Error(`woodblock bands: reach must be positive, got ${reach}`);
    return { kind: "bands", accent, named: Object.values(o.named ?? {}), reach };
  },
  /** By name: each paint prints in the inks of the nearest key. */
  keys(keys: Readonly<Record<string, InkKey>>): Print {
    const list = Object.values(keys);
    if (list.length === 0) throw new Error("woodblock keys: no keys");
    return { kind: "keys", keys: list };
  },
  /** As painted, the shadow side overprinted. */
  overprint(o: { readonly ink: Rgb; readonly amount: number; readonly value: number; readonly black: number }): Print {
    return { kind: "overprint", ...o };
  },
};

// ---- the foam where a body meets the water --------------------------------------------------------------------------

/**
 * The pattern printed where the water meets a body (docs/art_direction/DIRECTION.md, "Contact foam"). Each is a function
 * of the sea's contact band and the water's world position, and prints exactly nothing where no body is near. Lengths
 * are world units; a pattern's sizes suit a sea whose foam width (`SeaSpec.foam`) is about three of its periods.
 *  - `ridge`: a collar, filaments along the zero lines of a noise field, an indigo rim and specks. The print as it was.
 *  - `rings`: `count` thin arcs along the band's level lines, broken, moving outward `drift` rings a second; no fill.
 *  - `apron`: one flat shape with a scalloped edge (lobes `lobe` wide) and one key line; nothing inside it.
 *  - `stipple`: printed dots only (the larger `dot` apart), dense at the body and thinning outward.
 *  - `breakers`: a narrow collar, and a blade of foam along each of the sea's printed swell lines where it meets the body.
 *  - `curls`: a low collar of round lobes `lobe` apart lying over each other, the larger with a curled head, and a
 *    ripple arc beyond: the round 4 pictures' rolling foam.
 *  - `roll`: white water as the round 4 and 5 pictures draw it (src/style/whitewater.ts): a rolling mass, a few large
 *    hooked heads of unequal size curling with the swell, droplets beyond, a ripple arc outside.
 * `ridge` reads the sea's contact band; the others its contact level (`Contact`), which costs four more depth reads.
 */
export type ContactFoam =
  | { readonly kind: "ridge" }
  | { readonly kind: "rings"; readonly count: number; readonly drift: number }
  | { readonly kind: "apron"; readonly lobe: number }
  | { readonly kind: "stipple"; readonly dot: number }
  | { readonly kind: "breakers"; readonly reach: number }
  | { readonly kind: "curls"; readonly lobe: number }
  | { readonly kind: "roll"; readonly head: HeadSize };

/** How large a roll's largest curled head is: a share of the sea's foam width (so it follows the scene's sea), or a world radius. */
export type HeadSize = { readonly kind: "ofWidth"; readonly share: number } | { readonly kind: "world"; readonly radius: number };

/**
 * How the foam the sea's deformations mark on the water (a ring wave's crest, a whirlpool's arms) is printed: `flat`,
 * cream where the mark is over 0.45, the print as it was; `heads`, a line of curled heads of world radius `head` along
 * it, leaning the way the deformation says (`WaterCtx.marksLean`), over a thin flat core.
 */
export type MarksPrint = { readonly kind: "flat" } | { readonly kind: "heads"; readonly head: number };

const positive = (what: string, x: number): number => {
  if (!(x > 0)) throw new Error(`woodblock foam: ${what} must be positive, got ${x}`);
  return x;
};

export const foam = {
  /** The print as it was: filaments along the zero lines of a noise. */
  ridge(): ContactFoam { return { kind: "ridge" }; },
  /** Ripple arcs. `count`: arcs across the band, default 3; `drift`: arcs a second outward, default 0.4. */
  rings(o: { readonly count?: number; readonly drift?: number } = {}): ContactFoam {
    return { kind: "rings", count: positive("rings count", o.count ?? 3), drift: o.drift ?? 0.4 };
  },
  /** A flat scalloped apron. `lobe`: the width of a scallop, default 0.36. */
  apron(o: { readonly lobe?: number } = {}): ContactFoam { return { kind: "apron", lobe: positive("apron lobe", o.lobe ?? 0.36) }; },
  /** Printed dots. `dot`: the spacing of the larger dots, default 0.17. */
  stipple(o: { readonly dot?: number } = {}): ContactFoam { return { kind: "stipple", dot: positive("stipple dot", o.dot ?? 0.17) }; },
  /** The swell's own lines breaking on the body. `reach`: how far down the level a blade runs (0..1), default 0.72. */
  breakers(o: { readonly reach?: number } = {}): ContactFoam { return { kind: "breakers", reach: positive("breakers reach", o.reach ?? 0.72) }; },
  /** Rolling lobes with curled heads. `lobe`: the spacing of the lobes, default 0.5. */
  curls(o: { readonly lobe?: number } = {}): ContactFoam { return { kind: "curls", lobe: positive("curls lobe", o.lobe ?? 0.5) }; },
  /**
   * The rolling mass with curled heads. `head`: the world radius of the largest head; left out, it is half the
   * sea's foam width (0.42 on the crab's sea of 0.85, 0.15 on a sea of 0.3).
   */
  roll(o: { readonly head?: number } = {}): ContactFoam {
    return { kind: "roll", head: o.head === undefined ? { kind: "ofWidth", share: 0.5 } : { kind: "world", radius: positive("roll head", o.head) } };
  },
};

export const marksAs = {
  /** Cream where the mark is over 0.45: the print as it was. */
  flat(): MarksPrint { return { kind: "flat" }; },
  /** A line of curled heads. `head`: the world radius of the largest, default 0.35. */
  heads(o: { readonly head?: number } = {}): MarksPrint { return { kind: "heads", head: positive("crest head", o.head ?? 0.35) }; },
};

// ---- the sky and the water --------------------------------------------------------------------------------------------

/**
 * The sky's bokashi, in the view direction's height y: cream at the horizon graded to the haze over `haze`, the
 * indigo band printed in over `band`; below the horizon `below`.
 */
export type WoodblockSky = {
  readonly haze: readonly [number, number]; readonly band: readonly [number, number]; readonly below: Rgb;
  /** How the sky above the horizon is graded. Default `bokashi`, the print's own (`haze`, `band`). */
  readonly grade: SkyGrade;
  /** A sun printed flat in the sky. Default none. */
  readonly sun: SkySun;
  /** Cloud bands printed flat over the grade and the sun. Default none. */
  readonly clouds: SkyClouds;
};
/**
 * The grade above the horizon: the print's bokashi (cream at the horizon to the haze over `haze`, indigo printed in
 * over `band`), or stops: an ink at each view height (ascending, the first at or below 0), graded smoothly between
 * neighbours (a sunset: orange-gold at the horizon through vermilion and violet to indigo).
 */
export type SkyGrade = { readonly kind: "bokashi" } | { readonly kind: "stops"; readonly stops: readonly { readonly at: number; readonly ink: Rgb }[] };
/**
 * A sun disc: none, or one in the direction `toward` (world; normalised here), `radius` radians across its half, in
 * `ink`, flat. Below the horizon the sky prints `below`, so a disc whose centre is near the horizon is half set.
 */
export type SkySun = { readonly kind: "none" } | { readonly kind: "disc"; readonly toward: readonly [number, number, number]; readonly radius: number; readonly ink: Rgb };
/**
 * Cloud bands: none, or flat bands with a key line, each lying on a view height `at` (its flat underside) between two
 * bearings (radians about up, from +x toward +z), its top scalloped in `lumps` across that span and `thick` at most.
 * `line` is the key line's width in view height.
 */
export type SkyClouds =
  | { readonly kind: "none" }
  | { readonly kind: "bands"; readonly ink: Rgb; readonly line: number; readonly bands: readonly { readonly at: number; readonly thick: number; readonly from: number; readonly to: number; readonly lumps: number }[] };
const SKY: WoodblockSky = { haze: [0, 0.12], band: [0.18, 0.5], below: SEA.deep, grade: { kind: "bokashi" }, sun: { kind: "none" }, clouds: { kind: "none" } };

/** The sky's grade at view height `up`. One handler per grade. */
function graded(sky: WoodblockSky, inks: WoodblockInks, up: Expr<"f32">): Expr<"vec3"> {
  const g = sky.grade;
  switch (g.kind) {
    // Bokashi: cream at the horizon graded to the haze, with a band of indigo printed across the top.
    case "bokashi": return mix(mix(rgb(inks.cream), rgb(SEA.haze), smoothstep(sky.haze[0], sky.haze[1], up)), rgb(inks.indigo), smoothstep(sky.band[0], sky.band[1], up));
    case "stops": {
      if (g.stops.length < 2) throw new Error("woodblock sky: a graded sky needs at least two stops");
      g.stops.forEach((s, i) => { if (i > 0 && !(s.at > g.stops[i - 1]!.at)) throw new Error(`woodblock sky: stops must ascend; got [${g.stops.map((x) => x.at).join(", ")}]`); });
      return g.stops.slice(1).reduce<Expr<"vec3">>((c, s, i) => mix(c, rgb(s.ink), smoothstep(g.stops[i]!.at, s.at, up)), rgb(g.stops[0]!.ink));
    }
  }
}
/** A sun printed over the sky, its edge a pixel or so soft. */
function sunned(s: SkySun, c: Expr<"vec3">, dir: Expr<"vec3">): Expr<"vec3"> {
  switch (s.kind) {
    case "none": return c;
    case "disc": {
      const l = Math.hypot(...s.toward);
      if (!(l > 0) || !(s.radius > 0)) throw new Error("woodblock sky: a sun needs a direction and a positive radius");
      const toward = s.toward.map((x) => x / l) as [number, number, number];
      // The angle from the sun's centre, small-angle: 1 - cos a is about a^2 / 2, so a = sqrt(2 (1 - cos a)).
      const off = mul(max(sub(1, dot(dir, rgb(toward))), 0), 2);
      return mix(c, rgb(s.ink), sub(1, smoothstep(s.radius * s.radius * 0.985, s.radius * s.radius, off)));
    }
  }
}
/** Cloud bands over the sky: a flat underside on its height, a scalloped top tapering to nothing at both ends, a key line. */
function clouded(k: SkyClouds, inks: WoodblockInks, c: Expr<"vec3">, dir: Expr<"vec3">): Expr<"vec3"> {
  switch (k.kind) {
    case "none": return c;
    case "bands": {
      const bearing = atan2(dir.z, dir.x), up = dir.y;
      return k.bands.reduce<Expr<"vec3">>((out, b) => {
        if (!(b.to > b.from) || !(b.thick > 0)) throw new Error("woodblock sky: a cloud band needs from < to and a positive thickness");
        // The bearing unwrapped past `from`: atan2 jumps from pi to -pi straight behind +x, which is where a camera
        // looking along -x sees the sky, so a band may run past pi (from 2.85 to 3.22 is the middle of such a view).
        const unwrapped = select(lt(bearing, b.from), add(bearing, 2 * Math.PI), bearing);
        const u = saturate(mul(sub(unwrapped, b.from), 1 / (b.to - b.from)));
        // Its height across the span: tapered by a half sine, scalloped by |sin| (round tops meeting in notches).
        const top = mul(mul(sin(mul(u, Math.PI)), b.thick), add(0.6, mul(abs(sin(mul(u, Math.PI * b.lumps))), 0.4)));
        const h = sub(up, b.at);
        // Inside: above the underside and below the top (both with the key line's width as the edge).
        const inside = mul(smoothstep(0, k.line * 0.3, h), sub(1, smoothstep(sub(top, k.line * 0.3), top, h)));
        const core = mul(smoothstep(k.line, k.line * 1.3, h), sub(1, smoothstep(sub(top, k.line * 1.3), sub(top, k.line), h)));
        return mix(mix(out, rgb(inks.black), inside), rgb(k.ink), core);
      }, c);
    }
  }
}

/**
 * The eye of a whirlpool: where a deformation's shading darkens the water below `below` (in the crest's units,
 * negative down), the key black is printed, `amount` of it. None by default.
 */
export type WoodblockEye = { readonly kind: "none" } | { readonly kind: "ink"; readonly below: number; readonly amount: number };

// ---- the reference palette: paint treatments, each off by default ----------------------------------------------------

/**
 * The warm paints hotter: a paint whose red stands above its green and blue by `margin` is multiplied by `gain`. The
 * reference prints (crab_round4, lineup) have the crab at about (0.84, 0.26, 0.13) where ours paints brick (0.66, 0.22,
 * 0.13): gain (1.27, 1.18, 1). `margin` 0.33 takes the brick (0.44) and leaves the dark brick and coral (0.30) alone.
 */
export type Warm = { readonly kind: "asPainted" } | { readonly kind: "hotter"; readonly margin: number; readonly gain: Rgb };
/**
 * Darker blotches on the warm paints, in the surface's own coordinates (its anchor, the skin at rest), so they ride the
 * body and never swim: two octaves of noise, the larger `size` world units across, cut hard where they pass `cut`
 * (higher: sparser), printed at `value` of the paint (the reference's blotches are 0.72 of the shell's red).
 */
export type Mottle = { readonly kind: "off" } | { readonly kind: "blotches"; readonly size: number; readonly cut: number; readonly value: number; readonly margin: number };
/**
 * The key line's weight. `even`: the hull alone. `swelling`: the line drawn heavier on the side away from `key`, inside
 * the hull, up to `width` world units more (never under a pixel): an inker's line, thin in the light and thick in shadow.
 */
export type LineWeight = { readonly kind: "even" } | { readonly kind: "swelling"; readonly width: number; readonly key: Key };
/**
 * The sea's inks. `swell`: the print's own (two blues split at the crest, pale lines). `deep`: one flat deep ink with
 * thin cream lines, as the reference prints (sea about (0.09, 0.2, 0.3), lines (0.78, 0.65, 0.5)); `trough` the darker
 * ink the troughs take, `width` the lines' half width as a share of their spacing, at trough and crest.
 */
export type SeaPalette =
  | { readonly kind: "swell" }
  | { readonly kind: "deep"; readonly sea: Rgb; readonly trough: Rgb; readonly line: Rgb; readonly width: readonly [number, number]; readonly haze: Rgb };

export const paint = {
  hotter: (o: { readonly margin?: number; readonly gain?: Rgb } = {}): Warm => ({ kind: "hotter", margin: o.margin ?? 0.33, gain: o.gain ?? [1.27, 1.18, 1] }),
  mottle: (o: { readonly size?: number; readonly cut?: number; readonly value?: number; readonly margin?: number } = {}): Mottle =>
    ({ kind: "blotches", size: positive("mottle size", o.size ?? 0.42), cut: o.cut ?? 0.3, value: o.value ?? 0.74, margin: o.margin ?? 0.3 }),
  swelling: (o: { readonly width?: number; readonly key?: Key } = {}): LineWeight => ({ kind: "swelling", width: positive("line swelling", o.width ?? 0.05), key: o.key ?? light.key.offLens(45) }),
};
export const seaPalette = {
  swell: (): SeaPalette => ({ kind: "swell" }),
  deep: (o: Partial<Omit<Extract<SeaPalette, { kind: "deep" }>, "kind">> = {}): SeaPalette => ({
    kind: "deep", sea: o.sea ?? [0.08, 0.17, 0.3], trough: o.trough ?? [0.055, 0.12, 0.24], line: o.line ?? [0.8, 0.68, 0.52], width: o.width ?? [0.012, 0.04], haze: o.haze ?? [0.17, 0.24, 0.38],
  }),
};

export type WoodblockSpec = {
  /** The key block's inks. Default `WOODBLOCK_PALETTE`'s. */
  readonly inks?: Partial<WoodblockInks>;
  /** Which paints print in which inks. Default `print.bands()`. */
  readonly print?: Print;
  readonly sky?: Partial<WoodblockSky>;
  readonly water?: { readonly eye?: WoodblockEye; /** The sea's inks. Default `seaPalette.swell()`, the print as it was. */ readonly palette?: SeaPalette };
  /**
   * The key line's width as a multiple of the param `woodblock.outline` (0.05 world units unless tuned). Default 1. A
   * factor, not a width: the param is one per process, and two styles declaring it with different defaults collide.
   */
  readonly outline?: number;
  /** How paints are inked before the print rule: hotter warm paints, mottling, the line's weight. Each off by default. */
  readonly paint?: { readonly warm?: Warm; readonly mottle?: Mottle; readonly line?: LineWeight };
  /** The pattern of the foam where a body meets the water. Default `foam.ridge()`, the print as it was. */
  readonly foam?: ContactFoam;
  /** How the foam the sea's deformations mark is printed. Default `marksAs.flat()`, the print as it was. */
  readonly marks?: MarksPrint;
  /**
   * How the bodies are lit: a recipe of shading treatments (src/style/light.ts; `LIGHT` has the named ones). Absent:
   * the print as it was, the rule's own two tones split at N.L = 0. Under a recipe the rule prints its lit inks and the
   * shadow is one overprinted block.
   */
  readonly light?: LightRecipe;
};

/**
 * Paper: fibres (screen-fixed value noise stretched along x, the direction the paper was laid) and a fine tooth that
 * shows most in the pale inks, where a print's paper shows through.
 */
const PAPER = /* wgsl */ `${POST_COMMON}
@fragment fn fs(in: PostIn) -> @location(0) vec4<f32> {
  let c = post_color(in.uv).rgb;
  let scale = max(frame.viewport.y / 800.0, 0.375);
  let q = in.px.xy / scale;
  let fibre = sty_noise(q * vec2<f32>(0.08, 0.7)) * 0.6 + sty_noise(q * vec2<f32>(0.03, 0.25)) * 0.4;
  let tooth = sty_hash(vec2<i32>(in.px.xy));
  let light = dot(c, vec3<f32>(0.2126, 0.7152, 0.0722));
  let g = (fibre - 0.5) * 0.07 + (tooth - 0.5) * 0.035 * (0.4 + light);
  return vec4<f32>(c * (1.0 + g), 1.0);
}
`;

/** The woodblock's post pass. */
export const WOODBLOCK_POST = {
  paper: { label: "woodblock paper", wgsl: PAPER },
} as const satisfies Record<string, PostSpec>;

/** How far a paint stands out as the accent's hue (positive: it does). */
function standing(hue: AccentHue, c: Expr<"vec3">): Expr<"f32"> {
  switch (hue) {
    case "red": return min(sub(c.x, c.y), sub(c.x, c.z));
    case "yellow": return sub(min(c.x, c.y), c.z);
  }
}

/** The accent ink for a paint: the first, and each paler one over it from its luminance up. */
function accentInk(a: Accent, base: Expr<"vec3">): Expr<"vec3"> {
  return a.inks.slice(1).reduce<Expr<"vec3">>((c, k) => mix(c, rgb(k.ink), edgeOf(luminance(base), k.from, 1e-5)), rgb(a.inks[0]!.ink));
}

/** 0 on the shadow side of the terminator, 1 lit: two tones of light, a hard edge. */
const litSide = (ctx: ShadeCtx): Expr<"f32"> => edgeOf(dot(ctx.normal, ctx.sun), 0.0, 1e-5);
/**
 * Which of a rule's two tones a fragment takes, asked for wherever the rule reads it: `litSide` for the print as it
 * was, all light under a recipe (whose block prints the shadow).
 */
type Lit = () => Expr<"f32">;

// Every edge here is antialiased by about a pixel and no more (`soft` is tiny): a wider minimum turned a slowly
// changing value (n.l across a grazing flank, a soft paint gradient) into a visible gradient between two inks, which
// the palette test caught on the shell.
/** By value: black, indigo, pale blue or cream by the lit value in bands; the accent's ink for an accent paint. */
function bands(inks: WoodblockInks, p: Extract<Print, { kind: "bands" }>, ctx: ShadeCtx, lit: Lit): Expr<"vec3"> {
  const accent = p.accent;
  const base = ctx.base;
  const value = mul(luminance(base), mix(TONES.shadow, TONES.lit, lit()));
  let c: Expr<"vec3"> = rgb(inks.black);
  c = mix(c, rgb(inks.indigo), edgeOf(value, BAND.black, 1e-5));
  c = mix(c, rgb(inks.pale), edgeOf(value, BAND.indigo, 1e-5));
  c = mix(c, rgb(inks.cream), edgeOf(value, BAND.pale, 1e-5));
  const byValue = mix(c, accentInk(accent, base), edgeOf(standing(accent.hue, base), accent.margin, 1e-5));
  return p.named.reduce<Expr<"vec3">>((out, k) =>
    mix(out, mix(rgb(k.shadow), rgb(k.lit), lit()), edgeOf(sub(p.reach, distance(base, rgb(k.key))), 0, 1e-5)), byValue);
}

/** By name: the nearest key's ink, in shadow or lit. */
function keyed(keys: readonly InkKey[], ctx: ShadeCtx, lit: Lit): Expr<"vec3"> {
  const side = lit();
  const inkOf = (k: InkKey) => mix(rgb(k.shadow), rgb(k.lit), side);
  let best: Expr<"f32"> = distance(ctx.base, rgb(keys[0]!.key));
  let colour = inkOf(keys[0]!);
  for (const k of keys.slice(1)) {
    const d = distance(ctx.base, rgb(k.key));
    colour = select(lt(d, best), inkOf(k), colour);
    best = min(best, d);
  }
  return colour;
}

/** As painted: lit side the paint, shadow side overprinted; near-black paint the key black on both. */
function overprinted(inks: WoodblockInks, o: Extract<Print, { kind: "overprint" }>, ctx: ShadeCtx, lit: Lit): Expr<"vec3"> {
  const shadow = mul(mix(ctx.base, rgb(o.ink), o.amount), o.value);
  return mix(rgb(inks.black), mix(shadow, ctx.base, lit()), edgeOf(luminance(ctx.base), o.black, 1e-5));
}

/** The rule's shading. One handler per rule. */
function printed(inks: WoodblockInks, p: Print, ctx: ShadeCtx, lit: Lit): Expr<"vec3"> {
  switch (p.kind) {
    case "bands": return bands(inks, p, ctx, lit);
    case "keys": return keyed(p.keys, ctx, lit);
    case "overprint": return overprinted(inks, p, ctx, lit);
  }
}

/**
 * The shadow block a rule prints under a recipe that states none: an overprint rule's own ink; for the rules whose
 * shadow is a second ink per paint (bands, keys), the key block's indigo at the crab's depth.
 */
function ownBlock(inks: WoodblockInks, p: Print): BlockInk {
  switch (p.kind) {
    case "bands": case "keys": return { ink: inks.indigo, amount: 0.42, value: 0.78 };
    case "overprint": return { ink: p.ink, amount: p.amount, value: p.value };
  }
}

/** The warm paints, as painted or hotter. One handler per kind. */
function warmed(w: Warm, c: Expr<"vec3">): Expr<"vec3"> {
  switch (w.kind) {
    case "asPainted": return c;
    case "hotter": return mix(c, saturate(mul(c, rgb(w.gain))), edgeOf(standing("red", c), w.margin, 1e-5));
  }
}

/** Blotches on the warm paints, in the surface's own coordinates. */
function mottled(m: Mottle, c: Expr<"vec3">, ctx: ShadeCtx): Expr<"vec3"> {
  switch (m.kind) {
    case "off": return c;
    case "blotches": {
      const a = mul(ctx.anchor, 1 / m.size);
      const n = add(noise3(a), mul(noise3(add(mul(a, 2.3), vec3(7.1, 3.3, 1.9))), 0.45));
      const blot = mul(edgeOf(n, m.cut, 1e-5), edgeOf(standing("red", c), m.margin, 1e-5));
      return mix(c, mul(c, m.value), blot);
    }
  }
}

/** The key line swelled inside the hull on the shadow side, or the hull alone. */
function weighted(l: LineWeight, c: Expr<"vec3">, black: Rgb, ctx: ShadeCtx): Expr<"vec3"> {
  switch (l.kind) {
    case "even": return c;
    case "swelling": {
      // Pixels in from the silhouette, to first order (as the rim measures it); the line's extra width in pixels grows
      // from nothing where the surface faces the key to `width` where it faces away.
      const facing = dot(ctx.normal, ctx.view);
      const away = div(facing, max(fwidth(facing), 1e-5));
      const turned = saturate(mul(sub(0.35, dot(ctx.normal, keyOf(l.key, ctx))), 1 / 0.9));
      const wide = mul(div(l.width, pixelWorld(ctx.world)), turned);
      const inside = mul(sub(1, smoothstep(sub(wide, 0.75), add(wide, 0.75), away)), saturate(sub(wide, 0.25)));
      return mix(c, rgb(black), inside);
    }
  }
}

/** The paints as a spec's paint options ink them: the shade context with its base repainted, and the line over a colour. */
type Painting = { readonly base: (ctx: ShadeCtx) => ShadeCtx; readonly line: (c: Expr<"vec3">, ctx: ShadeCtx) => Expr<"vec3"> };
function paintingOf(p: NonNullable<WoodblockSpec["paint"]>, inks: WoodblockInks): Painting {
  const warm = p.warm ?? { kind: "asPainted" }, mottle = p.mottle ?? { kind: "off" }, line = p.line ?? { kind: "even" };
  return {
    base: (ctx) => ({ ...ctx, base: mottled(mottle, warmed(warm, ctx.base), ctx) }),
    line: (c, ctx) => weighted(line, c, inks.black, ctx),
  };
}
/** No paint options: the context and the colour untouched (the print as it was, the same shader to the byte). */
const AS_PAINTED: Painting = { base: (ctx) => ctx, line: (c) => c };

/** The sea's inks: trough, crest, line, the lines' half widths (trough, crest) and the far haze. One handler per palette. */
type SeaInks = { readonly deep: Rgb; readonly mid: Rgb; readonly line: Rgb; readonly width: readonly [number, number]; readonly haze: Rgb };
function seaInks(p: SeaPalette): SeaInks {
  switch (p.kind) {
    case "swell": return { deep: SEA.deep, mid: SEA.mid, line: SEA.line, width: [0.025, 0.075], haze: SEA.haze };
    case "deep": return { deep: p.trough, mid: p.sea, line: p.line, width: p.width, haze: p.haze };
  }
}

/** What a body's fragments are, for a style: its colour, its mark, and the shadow block if one is printed afterwards. */
type Bodies = Pick<Style, "shade" | "mark" | "block">;

/** The print as it was: the rule's two tones, split at N.L = 0. No post pass reads a mark. */
function asPrinted(inks: WoodblockInks, rule: Print, pt: Painting): Bodies {
  return { shade: (raw) => { const ctx = pt.base(raw); return pt.line(printed(inks, rule, ctx, () => litSide(ctx)), ctx); }, mark: () => f(1) };
}

/**
 * Under a recipe: the rule's lit inks, and the recipe's tone of the shadow block. With nothing cast from one thing on
 * another the block is printed in the fragment; otherwise the tone goes in the mark and the block is printed after the
 * bodies (`Style.block`), `shade` staying the whole colour for a fragment shaded alone.
 */
function underRecipe(inks: WoodblockInks, rule: Print, recipe: LightRecipe, pt: Painting): Bodies {
  const pull = pullOf(blockInk(recipe.block, ownBlock(inks, rule)));
  // The line goes on the colour before the block: the block over black is black.
  const both = (raw: ShadeCtx) => { const ctx = pt.base(raw); const b = lit(recipe, printed(inks, rule, ctx, () => f(1)), ctx); return { colour: pt.line(b.colour, ctx), tone: b.tone }; };
  const whole: Bodies["shade"] = (ctx) => { const b = both(ctx); return printedBlock(pull, b.colour, b.tone); };
  const cast = castOf(recipe.cast), overlap = overlapOf(recipe.overlap);
  if (cast.kind === "none" && overlap.kind === "none") return { shade: whole, mark: () => f(1) };
  return { shade: whole, mark: (ctx) => markOf(pull, both(ctx).tone), block: { body: (ctx) => both(ctx).colour, ink: pull.add, floor: pull.floor, cast, overlap } };
}

/** The whirlpool's eye printed black over the water, or nothing. */
function eyed(e: WoodblockEye, inks: WoodblockInks, c: Expr<"vec3">, crest: Expr<"f32">): Expr<"vec3"> {
  switch (e.kind) {
    case "none": return c;
    case "ink": return mix(c, rgb(inks.black), mul(edgeOf(mul(crest, -1), -e.below, 0.02), e.amount));
  }
}

/**
 * The woodblock print, with the scene's own inks, rule, sky and eye if it states them. Stated nothing, it is the print
 * as it was (`woodblock()` is unchanged pixel for pixel; examples/bosses/*.ts each wrapped it before these options).
 */
export function woodblock(spec: WoodblockSpec = {}): Style {
  const inks: WoodblockInks = { ...WOODBLOCK_PALETTE, ...spec.inks };
  const rule = spec.print ?? print.bands();
  const sky: WoodblockSky = { ...SKY, ...spec.sky };
  const eye: WoodblockEye = spec.water?.eye ?? { kind: "none" };
  const pattern: ContactFoam = spec.foam ?? foam.ridge();
  const crestPrint: MarksPrint = spec.marks ?? marksAs.flat();
  const width = param("woodblock.outline", 0.05, { min: 0, max: 0.2 });
  const outline = spec.outline === undefined ? width : mul(width, positive("woodblock outline factor", spec.outline));
  const pt: Painting = spec.paint === undefined ? AS_PAINTED : paintingOf(spec.paint, inks);
  const bodies: Bodies = spec.light === undefined ? asPrinted(inks, rule, pt) : underRecipe(inks, rule, spec.light, pt);
  const sea = seaInks(spec.water?.palette ?? seaPalette.swell());
  return {
    name: "woodblock",
    ...bodies,
    // The named recipes on this same print: what `--light name` chooses.
    lights: Object.fromEntries(Object.entries(LOOKS).map(([name, look]) => [name, () => woodblock(look.on(spec))])),
    // The key line: the hull, this wide in world units and never under 1.5 px.
    outline,
    clear: inks.cream,
    post: [WOODBLOCK_POST.paper],
    ink: () => rgb(inks.black),
    water: (ctx) => {
      const xz = ctx.world.xz;
      // Two flat blues, trough and crest, split along the swell's own contour (a little wobble, no blobs: the lines
      // must follow the swell, not a noise field).
      const wobble = mul(noise2(mul(xz, 0.6)), 0.08);
      const h = add(ctx.crest, wobble);
      let c: Expr<"vec3"> = mix(rgb(sea.deep), rgb(sea.mid), edgeOf(h, 0.2, 0.01));
      // Printed lines along the swell: four per wavelength of the largest swell, bent by the whole height field (so
      // the smaller swells ripple them), heavier toward the crests and absent in the troughs, fading out where they
      // would alias.
      const phase = add(mul(ctx.phase, 5), mul(h, 0.7));
      const fw = max(fwidth(phase), 1e-4);
      const d = abs(sub(fract(add(phase, 0.5)), 0.5)); // 0 on a line, 0.5 halfway between two
      const width = mix(sea.width[0], sea.width[1], saturate(add(mul(ctx.crest, 0.6), 0.5)));
      // Where the lines crowd (toward the horizon, or seen along the swell) each is replaced by the share of the period
      // it covers: the same tone with no stripes. It used to keep drawing them until three pixels apart, which printed
      // the far sea as a comb.
      const crowd = smoothstep(0.03, 0.1, fw);
      const drawn = sub(1, smoothstep(sub(width, fw), add(width, fw), d));
      const line = mul(mix(drawn, mul(width, 2), crowd), smoothstep(-0.6, -0.1, h));
      c = mix(c, rgb(sea.line), mul(line, 0.9));
      // What the swell's lines offer a foam that rides them: how far this is from the nearest line (0 on it, in line
      // spacings) and how much of a line is printed here (none in the troughs). Not their crowding: that is a screen
      // derivative, and a foam's edge takes the derivative of whatever shapes it (a derivative of a derivative, which
      // the sea's cases refuse); where lines crowd the band is a pixel or two wide and has no room for a blade anyway.
      const lines: SwellLines = { off: d, live: smoothstep(-0.6, -0.1, h) };
      c = foamed(pattern, ctx.contact, { under: c, inks, xz, swell: h, lines, drift: ctx.swell });
      // The far sea fades toward the haze at the horizon as a bokashi (a graded wiping of the block, as the sky's): two
      // flat steps at 70 and 140 units printed hard pale bands across the far sea, the second straight across a
      // maelstrom's view (examples/maelstrom.ts --style woodblock).
      const far = mul(smoothstep(50, 220, ctx.distance), 0.85);
      c = mix(c, rgb(sea.haze), far);
      // The foam the sea's deformations give (a whirlpool's arms), printed cream over everything: it was left out, and
      // examples/bosses/maelstrom_wyrm.ts printed it back in by hand (2026-11-11). Zero without a deformation.
      c = crested(crestPrint, c, inks, ctx);
      // Opaque: a print shows nothing under its water.
      return vec4(eyed(eye, inks, c, ctx.crest), 1);
    },
    shadow: (ctx) => vec4(rgb(inks.indigo), mul(edgeOf(ctx.amount, 0.3, 0.01), 0.5)),
    // A plane: the paper under a light wash, its shadow cut flat in indigo as on the water, hazed toward the sky's own
    // horizon (whatever its grade) as the far sea is.
    ground: (ctx) => mix(mix(mul(rgb(inks.cream), 0.88), rgb(inks.indigo), mul(edgeOf(ctx.shadow, 0.3, 0.01), 0.5)), graded(sky, inks, f(0)), mul(smoothstep(50, 220, ctx.distance), 0.85)),
    sky: (ctx) => {
      // The grade, then the sun and the clouds printed over it; below the horizon `below`. With the defaults each step
      // returns what it was given, so the sky is the print's as it was, expression for expression.
      const up = ctx.dir.y;
      const above = clouded(sky.clouds, inks, sunned(sky.sun, graded(sky, inks, up), ctx.dir), ctx.dir);
      return mix(above, rgb(sky.below), sub(1, smoothstep(-0.002, 0.0, up)));
    },
  };
}

/** The swell's printed lines here: `off` 0 on a line to 0.5 half way between two; `live` 0..1, how much of a line prints. */
type SwellLines = { readonly off: Expr<"f32">; readonly live: Expr<"f32"> };
/** What a foam pattern is printed on: the water's colour so far, where, and the swell there (-1 trough .. 1 crest). */
type FoamOn = {
  readonly under: Expr<"vec3">; readonly inks: WoodblockInks; readonly xz: Expr<"vec2">; readonly swell: Expr<"f32">; readonly lines: SwellLines;
  /** The direction the swell travels (`WaterCtx.swell`). */
  readonly drift: Expr<"vec2">;
};
/** The same, where a body is near: `band`, `level` and `width` as `Contact` gives them (1 at the body, 0 at the sea's foam width). */
type FoamNear = FoamOn & { readonly band: Expr<"f32">; readonly level: Expr<"f32">; readonly toward: Expr<"vec2">; readonly width: number };

/** The contact foam: nothing where the sea found no body near, the pattern where one may be. */
function foamed(p: ContactFoam, contact: Contact, on: FoamOn): Expr<"vec3"> {
  switch (contact.kind) {
    case "clear": return on.under;
    case "near": return patterned(p, { ...on, band: contact.band, level: contact.level, toward: contact.toward, width: contact.width });
  }
}

/** One handler per pattern. */
function patterned(p: ContactFoam, on: FoamNear): Expr<"vec3"> {
  switch (p.kind) {
    case "ridge": return ridge(on);
    case "rings": return rings(p, on);
    case "apron": return apron(p, on);
    case "stipple": return stipple(p, on);
    case "breakers": return breakers(p, on);
    case "curls": return curls(p, on);
    case "roll": return rolled(p, on);
  }
}

/** White water printed over the water's colour: its key line, then its cream. */
function printedWhite(under: Expr<"vec3">, inks: WoodblockInks, w: White): Expr<"vec3"> {
  return mix(mix(under, rgb(inks.black), mul(w.key, KEY)), rgb(inks.cream), w.cream);
}

/** A head's world radius on a sea whose foam is `width` wide. */
function headOf(h: HeadSize, width: number): number {
  switch (h.kind) {
    case "ofWidth": return h.share * width;
    case "world": return h.radius;
  }
}

/**
 * The collar as white water: the painter's rolling mass and curled heads, sized by the contact level and curling the
 * way the swell travels, over a ripple arc or two on the level's own lines.
 */
function rolled(p: Extract<ContactFoam, { kind: "roll" }>, on: FoamNear): Expr<"vec3"> {
  // The level rises toward the body by 1 / width a world unit: its slope, so lobes and heads are sized whole.
  const white = rollingFoam({ xz: on.xz, mass: on.level, slope: mul(on.toward, 1 / on.width), lean: { kind: "toward", dir: on.drift } }, rollOf(headOf(p.head, on.width)));
  const ripple = arcs(on, { count: 5, drift: 0, from: 0.02, to: 0.34, weight: 0.24 });
  return printedWhite(mix(on.under, rgb(on.inks.cream), ripple), on.inks, white);
}

/** The deformations' foam printed over the water. One handler per print. */
function crested(p: MarksPrint, c: Expr<"vec3">, inks: WoodblockInks, ctx: WaterCtx): Expr<"vec3"> {
  switch (p.kind) {
    case "flat": return mix(c, rgb(inks.cream), edgeOf(ctx.marks, 0.45, 0.02));
    case "heads": {
      // The mark is over 0.45 where the flat print is cream: the heads are sized from a little under that, so they
      // stand along the flat print's edge and outlast it as the wave weakens; the flat core is its upper part only.
      const white = curledHeads({ xz: ctx.world.xz, mass: smoothstep(0.2, 0.7, ctx.marks), slope: vec2(0, 0), lean: { kind: "toward", dir: ctx.marksLean } }, rollOf(p.head));
      const core = edgeOf(ctx.marks, 0.62, 0.02);
      return printedWhite(mix(c, rgb(inks.cream), core), inks, white);
    }
  }
}

/**
 * Ridge foam, from how close a body is: a collar at the body, and filaments that taper outward along the zero lines of
 * a noise field (thin where |noise| is small, narrower the farther out), with an indigo line round the whole shape and
 * loose specks of spray beyond it. None where no body is near: there the band is below -800, where the foam (band >
 * 0.02), the rim (> 0.05) and the specks (> -0.4) are all exactly 0. (The zero lines of a noise are closed curves: it
 * prints a net, not claws. It is the default so that every picture made before the other patterns is unchanged.)
 */
function ridge(on: FoamNear): Expr<"vec3"> {
  const near = on.band;
  const ridge = abs(noise2(mul(on.xz, 3.4)));
  const shape = max(sub(near, 0.6), sub(mul(near, 0.3), ridge));
  const foam = mul(edgeOf(shape, 0, 0.004), edgeOf(near, 0.02, 0.01));
  const rim = mul(mul(edgeOf(shape, -0.06, 0.004), sub(1, foam)), edgeOf(near, 0.05, 0.01));
  const speck = mul(specks(mul(on.xz, 8)), mul(smoothstep(-0.4, -0.1, near), sub(1, smoothstep(0.1, 0.3, near))));
  return mix(mix(on.under, rgb(on.inks.indigo), rim), rgb(on.inks.cream), max(foam, speck));
}

/** How much of the key black a foam's key line prints: a little of the water shows through, as the crash foam's does. */
const KEY = 0.9;

/**
 * A flat cream shape with one key line round it: cream where `shape` (in units of the level) is above 0, the key line
 * where it is within `line` below. Both only where the level is above 0.04, so nothing prints where no body is near.
 */
function flat(on: FoamNear, shape: Expr<"f32">, line: number): Expr<"vec3"> {
  const near = edgeOf(on.level, 0.04, 0.01);
  const cream = mul(edgeOf(shape, 0, 0.004), near);
  const key = mul(mul(edgeOf(shape, -line, 0.004), sub(1, cream)), near);
  return mix(mix(on.under, rgb(on.inks.black), mul(key, KEY)), rgb(on.inks.cream), cream);
}

/**
 * Thin arcs along the level's own lines, `count` of them across the level, drifting outward `drift` arcs a second (0:
 * still): each thins to nothing toward the level's outer edge, and is broken by its own slow mask, which also tapers
 * its ends. `from`..`to` is the part of the level they are drawn in. Coverage 0..1.
 */
function arcs(on: FoamNear, o: { readonly count: number; readonly drift: number; readonly from: number; readonly to: number; readonly weight: number }): Expr<"f32"> {
  const phase = add(mul(on.level, o.count), mul(time, o.drift));
  const off = abs(sub(fract(phase), 0.5)); // 0 on an arc
  const n = floor(phase);
  const mask = mul(sin(add(along(on.xz, 25, 0.45), mul(n, 2.4))), sin(add(along(on.xz, 115, 0.38), mul(n, 1.7))));
  const inside = mul(smoothstep(o.from, o.from + 0.25, on.level), sub(1, smoothstep(o.to - 0.08, o.to, on.level)));
  const half = mul(mul(inside, smoothstep(-0.45, 0.05, mask)), o.weight);
  return mul(edgeOf(sub(half, off), 0.01, 0.004, 0.08), edgeOf(on.level, 0.04, 0.01));
}

/** Ripple arcs only: no fill. */
function rings(p: Extract<ContactFoam, { kind: "rings" }>, on: FoamNear): Expr<"vec3"> {
  return mix(on.under, rgb(on.inks.cream), arcs(on, { count: p.count, drift: p.drift, from: 0.06, to: 1.2, weight: 0.2 }));
}

/**
 * Round lobes on a jittered lattice, `period` apart, each growing with the level: full size near the body, nothing at
 * the level's outer edge. A lobe's edge is a circle in the world, so it is clean whatever the level's small steps do;
 * the level only sizes it. Of the lobes that cover a pixel (it is inside one, or within `line` cells outside it, on
 * its key line) the one with the higher hash is in front. `sizes`: the least and greatest radius at full growth, in
 * cells. (A wider range, 0.3 to 0.8 with most lobes small, was tried for the curls: the lobes stopped overlapping and
 * the collar read as loose bubbles, not a mass.) Returned for the pixel: `front`, the front lobe's hash (-1
 * where none covers it); `inside`, how far inside that lobe's edge this is, in cells (negative on its key line);
 * `local`, where in that lobe (the offset from its centre over its radius); `union`, how far inside the nearest edge
 * of all the lobes together (negative outside them all).
 */
function lobes(on: FoamNear, period: number, line: number, sizes: readonly [number, number] = [0.44, 0.78]): { readonly front: Expr<"f32">; readonly inside: Expr<"f32">; readonly local: Expr<"vec2">; readonly union: Expr<"f32"> } {
  const q = vec2(along(on.xz, 21, period), along(on.xz, 111, period));
  const cell = floor(q), at = sub(q, cell);
  // The four cells whose lobes can reach here: this one and the three on the pixel's side of it (a lobe's centre is
  // within 0.2 of its cell's middle and its radius at most 0.78, so it never reaches past the middle of a neighbour).
  const side = vec2(select(lt(at.x, 0.5), -1, 1), select(lt(at.y, 0.5), -1, 1));
  const grow = smoothstep(0.08, 0.6, on.level);
  let front: Expr<"f32"> = f(-1), inside: Expr<"f32"> = f(-1), local: Expr<"vec2"> = vec2(0, 0), union: Expr<"f32"> = f(-1);
  for (const [i, j] of [[0, 0], [1, 0], [0, 1], [1, 1]] as const) {
    const c = add(cell, vec2(mul(side.x, i), mul(side.y, j)));
    const h = hash22(c);
    const off = sub(q, add(add(c, 0.5), mul(sub(h, 0.5), 0.4)));
    const radius = mul(grow, mix(sizes[0], sizes[1], h.y));
    // A lobe under an eighth of a cell is not drawn, and a small one has a thinner key line: at full width the line
    // round a lobe that small printed a black blot with a cream dot in it.
    const s = select(gt(radius, 0.12), sub(radius, length(off)), -1);
    const wins = and(gt(s, mul(smoothstep(0.12, 0.4, radius), -line)), gt(h.x, front));
    inside = select(wins, s, inside);
    local = select(wins, off.div(max(radius, 1e-3)), local);
    front = select(wins, h.x, front);
    union = max(union, s);
  }
  return { front, inside, local, union };
}

/** A flat apron with a scalloped edge: the lobes as one cream shape, one key line round it, nothing inside. */
function apron(p: Extract<ContactFoam, { kind: "apron" }>, on: FoamNear): Expr<"vec3"> {
  // `flat` takes a shape in units of the level; the lobes' is in cells, about a third of a foam width each.
  return flat(on, mul(lobes(on, p.lobe, 0).union, 0.4), 0.075);
}

/**
 * Printed dots. Two lattices of cells, one dot a cell: the dot's radius grows from nothing as the level passes the
 * cell's own hashed threshold, so dots are dense and large at the body and few and small outward, and a dot comes and
 * goes by size, never at once.
 */
function stipple(p: Extract<ContactFoam, { kind: "stipple" }>, on: FoamNear): Expr<"vec3"> {
  // The pixel's size on the water, world units: smooth (the position is), where fwidth of a dot's own distance would
  // jump at every cell border.
  const pixel = max(fwidth(on.xz.x), fwidth(on.xz.y));
  const layer = (degrees: number, period: number, big: number): Expr<"f32"> => {
    const q = vec2(along(on.xz, degrees, period), along(on.xz, degrees + 90, period));
    const h = hash22(q);
    const r = length(sub(sub(fract(q), 0.5), mul(sub(h, 0.5), 0.44)));
    const radius = mul(mul(saturate(mul(sub(on.level, add(mul(h.x, 0.75), 0.05)), 1 / 0.3)), big), mix(0.55, 1, h.y));
    const fw = max(mul(pixel, 0.75 / period), 1e-4);
    const disc = sub(1, smoothstep(sub(radius, fw), add(radius, fw), r));
    // Where a cell is under about two pixels the dots cannot be drawn: print their share of the cell instead (the
    // same tone, no shimmer). A dot smaller than a pixel prints as its share of that pixel; none prints none.
    const share = mul(mul(radius, radius), Math.PI);
    return mul(mix(disc, share, smoothstep(0.25, 0.6, fw)), saturate(mul(radius, 1 / 0.02)));
  };
  const dots = max(layer(12, p.dot, 0.27), layer(51, p.dot * 0.62, 0.27));
  return mix(on.under, rgb(on.inks.cream), dots);
}

/**
 * The swell's own lines breaking on the body: a low collar of small lobes, and along each printed line that comes
 * within the level a blade of foam that tapers away from the body. A blade's sides are cut by the distance to its line
 * (smooth, in the world), only its tip by the level. The blades move with the swell because the lines do.
 */
function breakers(p: Extract<ContactFoam, { kind: "breakers" }>, on: FoamNear): Expr<"vec3"> {
  // 1 on a line, 0 from 0.06 of the spacing away; squared, so a blade's sides are concave.
  const on_line = saturate(sub(1, mul(on.lines.off, 1 / 0.06)));
  const blade = add(sub(on.level, 0.8), mul(mul(mul(on_line, on_line), on.lines.live), p.reach));
  // The collar is the lobes drawn in by a third of a cell: low, so the blades stand clear of it.
  const collar = sub(mul(lobes(on, 0.3, 0).union, 0.4), 0.13);
  return flat(on, max(blade, collar), 0.05);
}

/**
 * Rolling foam, after the round 4 pictures (docs/art_direction/crab_round4): a low cream collar of round lobes, each
 * with its key line and lying over the one behind it, the larger ones with a curled head (a turn and a half of a
 * spiral cut out of the cream, so the water shows in it); and a ripple arc or two beyond.
 */
function curls(p: Extract<ContactFoam, { kind: "curls" }>, on: FoamNear): Expr<"vec3"> {
  const l = lobes(on, p.lobe, 0.11);
  const covered = edgeOf(l.front, 0, 1e-5);
  const cream = mul(edgeOf(l.inside, 0, 0.004), covered);
  // The curled head: an arm of a spiral from the lobe's edge inward, widest at the edge and tapering to its middle,
  // in the lobes whose hash is over 0.6 (the ones most in front), and only where a cell is over about 18 pixels: in a
  // smaller lobe the arm is a blot (from the game camera the collar is plain lobes, as in the round 4 game view).
  const rn = length(l.local);
  const turn = add(mul(atan2(l.local.y, l.local.x), 1 / (2 * Math.PI)), mul(rn, 1.5));
  const arm = abs(sub(fract(add(turn, mul(l.front, 7))), 0.5));
  const room = sub(1, smoothstep(0.045, 0.07, max(fwidth(on.xz.x), fwidth(on.xz.y)).div(p.lobe)));
  const curl = mul(mul(mul(edgeOf(sub(mul(smoothstep(0.1, 0.9, rn), 0.14), arm), 0, 0.004, 0.1), cream), step(0.6, l.front)), room);
  const near = edgeOf(on.level, 0.04, 0.01);
  // Beyond the collar, a ripple arc or two on the level's own lines.
  const ripple = mul(arcs(on, { count: 5, drift: 0, from: 0.02, to: 0.34, weight: 0.24 }), sub(1, covered));
  const water = mix(on.under, rgb(on.inks.cream), ripple);
  const keyed = mix(water, rgb(on.inks.black), mul(mul(sub(covered, cream), KEY), near));
  return mix(keyed, rgb(on.inks.cream), mul(mul(cream, sub(1, curl)), near));
}

/** Round specks of spray, one per cell of p at a hashed spot, on about a third of the cells; 0..1 coverage. */
function specks(p: Expr<"vec2">): Expr<"f32"> {
  const h = hash22(p);
  const at = sub(sub(fract(p), 0.5), mul(sub(h, 0.5), 0.5));
  const r = length(at);
  const radius = mul(smoothstep(0.62, 0.9, h.x), 0.22);
  const fw = max(fwidth(r), 1e-4);
  return sub(1, smoothstep(sub(radius, fw), add(radius, fw), r));
}

// ---- looks: the named ways to print a scene, chosen by name (`--light`, a design's `light` look) ------------------------

/** The reference pictures' paint (crab_round4, lineup): hotter warm paints, mottled, the line swelling in shadow. */
export const REFERENCE_PAINT: NonNullable<WoodblockSpec["paint"]> = { warm: paint.hotter(), mottle: paint.mottle(), line: paint.swelling() };

/** A look: what it is for, and the spec it makes of a scene's own (the scene's inks, rule and foam kept). */
export type WoodblockLook = { readonly about: string; readonly on: (s: WoodblockSpec) => WoodblockSpec };
const lightOnly = (about: string, r: LightRecipe): WoodblockLook => ({ about, on: (s) => ({ ...s, light: r }) });
/** The reference palette over a spec: paint, a key line 1.4 times as heavy (0.07), the deep sea. */
const referenced = (s: WoodblockSpec): WoodblockSpec => ({ ...s, paint: REFERENCE_PAINT, outline: 1.4, water: { ...s.water, palette: seaPalette.deep() } });

/**
 * Every named look of the woodblock print: each recipe of `LIGHT` alone, the reference palette alone, and the two the
 * four-crab video uses. docs/lighting.md has a line each.
 */
export const LOOKS = {
  ...(Object.fromEntries(Object.entries(LIGHT).map(([name, r]) => [name, lightOnly(`the ${name} lighting recipe`, r)])) as Record<keyof typeof LIGHT, WoodblockLook>),
  /** The reference palette (hotter red, mottling, swelling lines, the deep sea with cream lines) with the print's own shading. */
  referencePalette: { about: "the reference palette with the print's own two tones", on: referenced },
  /** The video's look: the engraving (`hatched`) in the reference palette. */
  engraving: { about: "hatched, in the reference palette: the four-crab video", on: (s) => referenced({ ...s, light: LIGHT.hatched }) },
  /** The video's look with the sun low behind: `sunset` in the reference palette. */
  engravingSunset: { about: "the sunset recipe, in the reference palette: the video at sunset", on: (s) => referenced({ ...s, light: LIGHT.sunset }) },
} satisfies Record<string, WoodblockLook>;
export type LookName = keyof typeof LOOKS;

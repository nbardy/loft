// Woodblock: a colour print. Bodies are flat areas from a fixed palette of five inks (indigo, a paler blue, cream,
// one vermilion accent, black), each chosen by the lit value of the paint in bands, so nothing on a body is a
// gradient. The black key line is the outline hull: geometry, thick and of even weight, antialiased by the frame's
// own multisampling. (It was a post pass that cut lines from depth and normal edges, one decision per pixel: the lines
// came out ragged and broken beside the hull outlines of the other styles, and it cost 0.37 ms a frame. 2026-11-11.)
// Then paper. The sea is flat blues with printed lines along the swell's contours and clawed foam where it meets a body.

import { abs, add, distance, dot, f, fract, length, lt, max, min, mix, mul, param, saturate, select, smoothstep, sub, vec4, type Expr } from "../expr.ts";
import { noise2, hash22, luminance } from "../lib/index.ts";
import { fwidth } from "../lib/wgsl.ts";
import type { PostSpec } from "../compile/context.ts";
import type { Contact, Rgb, ShadeCtx, Style } from "../style.ts";
import { edgeOf, POST_COMMON, rgb } from "./shared.ts";

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
const LIGHT = { shadow: 0.66, lit: 1.06 } as const;

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

// ---- the sky and the water --------------------------------------------------------------------------------------------

/**
 * The sky's bokashi, in the view direction's height y: cream at the horizon graded to the haze over `haze`, the
 * indigo band printed in over `band`; below the horizon `below`.
 */
export type WoodblockSky = { readonly haze: readonly [number, number]; readonly band: readonly [number, number]; readonly below: Rgb };
const SKY: WoodblockSky = { haze: [0, 0.12], band: [0.18, 0.5], below: SEA.deep };

/**
 * The eye of a whirlpool: where a deformation's shading darkens the water below `below` (in the crest's units,
 * negative down), the key black is printed, `amount` of it. None by default.
 */
export type WoodblockEye = { readonly kind: "none" } | { readonly kind: "ink"; readonly below: number; readonly amount: number };

export type WoodblockSpec = {
  /** The key block's inks. Default `WOODBLOCK_PALETTE`'s. */
  readonly inks?: Partial<WoodblockInks>;
  /** Which paints print in which inks. Default `print.bands()`. */
  readonly print?: Print;
  readonly sky?: Partial<WoodblockSky>;
  readonly water?: { readonly eye?: WoodblockEye };
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

// Every edge here is antialiased by about a pixel and no more (`soft` is tiny): a wider minimum turned a slowly
// changing value (n.l across a grazing flank, a soft paint gradient) into a visible gradient between two inks, which
// the palette test caught on the shell.
/** By value: black, indigo, pale blue or cream by the lit value in bands; the accent's ink for an accent paint. */
function bands(inks: WoodblockInks, p: Extract<Print, { kind: "bands" }>, ctx: ShadeCtx): Expr<"vec3"> {
  const accent = p.accent;
  const base = ctx.base;
  const value = mul(luminance(base), mix(LIGHT.shadow, LIGHT.lit, litSide(ctx)));
  let c: Expr<"vec3"> = rgb(inks.black);
  c = mix(c, rgb(inks.indigo), edgeOf(value, BAND.black, 1e-5));
  c = mix(c, rgb(inks.pale), edgeOf(value, BAND.indigo, 1e-5));
  c = mix(c, rgb(inks.cream), edgeOf(value, BAND.pale, 1e-5));
  const byValue = mix(c, accentInk(accent, base), edgeOf(standing(accent.hue, base), accent.margin, 1e-5));
  return p.named.reduce<Expr<"vec3">>((out, k) =>
    mix(out, mix(rgb(k.shadow), rgb(k.lit), litSide(ctx)), edgeOf(sub(p.reach, distance(base, rgb(k.key))), 0, 1e-5)), byValue);
}

/** By name: the nearest key's ink, in shadow or lit. */
function keyed(keys: readonly InkKey[], ctx: ShadeCtx): Expr<"vec3"> {
  const lit = litSide(ctx);
  const inkOf = (k: InkKey) => mix(rgb(k.shadow), rgb(k.lit), lit);
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
function overprinted(inks: WoodblockInks, o: Extract<Print, { kind: "overprint" }>, ctx: ShadeCtx): Expr<"vec3"> {
  const shadow = mul(mix(ctx.base, rgb(o.ink), o.amount), o.value);
  return mix(rgb(inks.black), mix(shadow, ctx.base, litSide(ctx)), edgeOf(luminance(ctx.base), o.black, 1e-5));
}

/** The rule's shading. One handler per rule. */
function printed(inks: WoodblockInks, p: Print, ctx: ShadeCtx): Expr<"vec3"> {
  switch (p.kind) {
    case "bands": return bands(inks, p, ctx);
    case "keys": return keyed(p.keys, ctx);
    case "overprint": return overprinted(inks, p, ctx);
  }
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
  const outline = param("woodblock.outline", 0.05, { min: 0, max: 0.2 });
  return {
    name: "woodblock",
    // The key line: the hull, this wide in world units and never under 1.5 px.
    outline,
    clear: inks.cream,
    post: [WOODBLOCK_POST.paper],
    shade: (ctx) => printed(inks, rule, ctx),
    ink: () => rgb(inks.black),
    // No post pass reads a mark.
    mark: () => f(1),
    water: (ctx) => {
      const xz = ctx.world.xz;
      // Two flat blues, trough and crest, split along the swell's own contour (a little wobble, no blobs: the lines
      // must follow the swell, not a noise field).
      const wobble = mul(noise2(mul(xz, 0.6)), 0.08);
      const h = add(ctx.crest, wobble);
      let c: Expr<"vec3"> = mix(rgb(SEA.deep), rgb(SEA.mid), edgeOf(h, 0.2, 0.01));
      // Printed lines along the swell: four per wavelength of the largest swell, bent by the whole height field (so
      // the smaller swells ripple them), heavier toward the crests and absent in the troughs, fading out where they
      // would alias.
      const phase = add(mul(ctx.phase, 5), mul(h, 0.7));
      const fw = max(fwidth(phase), 1e-4);
      const d = abs(sub(fract(add(phase, 0.5)), 0.5)); // 0 on a line, 0.5 halfway between two
      const width = mix(0.025, 0.075, saturate(add(mul(ctx.crest, 0.6), 0.5)));
      // Where the lines crowd (toward the horizon, or seen along the swell) each is replaced by the share of the period
      // it covers: the same tone with no stripes. It used to keep drawing them until three pixels apart, which printed
      // the far sea as a comb.
      const crowd = smoothstep(0.03, 0.1, fw);
      const drawn = sub(1, smoothstep(sub(width, fw), add(width, fw), d));
      const line = mul(mix(drawn, mul(width, 2), crowd), smoothstep(-0.6, -0.1, h));
      c = mix(c, rgb(SEA.line), mul(line, 0.9));
      c = clawed(c, inks, ctx.contact, xz);
      // The far sea fades toward the haze at the horizon as a bokashi (a graded wiping of the block, as the sky's): two
      // flat steps at 70 and 140 units printed hard pale bands across the far sea, the second straight across a
      // maelstrom's view (examples/maelstrom.ts --style woodblock).
      const far = mul(smoothstep(50, 220, ctx.distance), 0.85);
      c = mix(c, rgb(SEA.haze), far);
      // The foam the sea's deformations give (a whirlpool's arms), printed cream over everything: it was left out, and
      // examples/bosses/maelstrom_wyrm.ts printed it back in by hand (2026-11-11). Zero without a deformation.
      c = mix(c, rgb(inks.cream), edgeOf(ctx.marks, 0.45, 0.02));
      // Opaque: a print shows nothing under its water.
      return vec4(eyed(eye, inks, c, ctx.crest), 1);
    },
    shadow: (ctx) => vec4(rgb(inks.indigo), mul(edgeOf(ctx.amount, 0.3, 0.01), 0.5)),
    sky: (ctx) => {
      // Bokashi: cream at the horizon graded to the haze, with a band of indigo printed across the top.
      const up = ctx.dir.y;
      const above = mix(mix(rgb(inks.cream), rgb(SEA.haze), smoothstep(sky.haze[0], sky.haze[1], up)), rgb(inks.indigo), smoothstep(sky.band[0], sky.band[1], up));
      return mix(above, rgb(sky.below), sub(1, smoothstep(-0.002, 0.0, up)));
    },
  };
}

/**
 * Clawed foam, from how close a body is: a collar at the body, and fingers that taper outward along the zero lines of a
 * noise field (thin where |noise| is small, narrower the farther out), with an indigo line round the whole shape and
 * loose specks of spray beyond it. None where no body is near: there the band is below -800, where the foam (band >
 * 0.02), the rim (> 0.05) and the specks (> -0.4) are all exactly 0.
 */
function clawed(c: Expr<"vec3">, inks: WoodblockInks, contact: Contact, xz: Expr<"vec2">): Expr<"vec3"> {
  switch (contact.kind) {
    case "clear": return c;
    case "near": {
      const near = contact.band;
      const ridge = abs(noise2(mul(xz, 3.4)));
      const shape = max(sub(near, 0.6), sub(mul(near, 0.3), ridge));
      const foam = mul(edgeOf(shape, 0, 0.004), edgeOf(near, 0.02, 0.01));
      const rim = mul(mul(edgeOf(shape, -0.06, 0.004), sub(1, foam)), edgeOf(near, 0.05, 0.01));
      const speck = mul(specks(mul(xz, 8)), mul(smoothstep(-0.4, -0.1, near), sub(1, smoothstep(0.1, 0.3, near))));
      return mix(mix(c, rgb(inks.indigo), rim), rgb(inks.cream), max(foam, speck));
    }
  }
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

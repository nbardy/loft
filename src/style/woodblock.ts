// Woodblock: a colour print. Bodies are flat areas from a fixed palette of five inks (indigo, a paler blue, cream,
// one vermilion accent, black), each chosen by the lit value of the paint in bands, so nothing on a body is a
// gradient. The black key line is the outline hull: geometry, thick and of even weight, antialiased by the frame's
// own multisampling. (It was a post pass that cut lines from depth and normal edges, one decision per pixel: the lines
// came out ragged and broken beside the hull outlines of the other styles, and it cost 0.37 ms a frame. 2026-11-11.)
// Then paper. The sea is flat blues with printed lines along the swell's contours and foam where it meets a body, in one
// of several patterns (`ContactFoam`).

import { abs, add, and, atan2, distance, dot, f, floor, fract, gt, length, lt, max, min, mix, mul, param, saturate, select, sin, smoothstep, step, sub, time, vec2, vec4, type Expr } from "../expr.ts";
import { noise2, hash22, luminance } from "../lib/index.ts";
import { fwidth } from "../lib/wgsl.ts";
import type { PostSpec } from "../compile/context.ts";
import type { Contact, Rgb, ShadeCtx, Style } from "../style.ts";
import { blockInk, castOf, lit, LIGHT, markOf, overlapOf, printedBlock, pullOf, type BlockInk, type LightRecipe } from "./light.ts";
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
 * `ridge` reads the sea's contact band; the others its contact level (`Contact`), which costs four more depth reads.
 */
export type ContactFoam =
  | { readonly kind: "ridge" }
  | { readonly kind: "rings"; readonly count: number; readonly drift: number }
  | { readonly kind: "apron"; readonly lobe: number }
  | { readonly kind: "stipple"; readonly dot: number }
  | { readonly kind: "breakers"; readonly reach: number }
  | { readonly kind: "curls"; readonly lobe: number };

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
  /** The pattern of the foam where a body meets the water. Default `foam.ridge()`, the print as it was. */
  readonly foam?: ContactFoam;
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

/** What a body's fragments are, for a style: its colour, its mark, and the shadow block if one is printed afterwards. */
type Bodies = Pick<Style, "shade" | "mark" | "block">;

/** The print as it was: the rule's two tones, split at N.L = 0. No post pass reads a mark. */
function asPrinted(inks: WoodblockInks, rule: Print): Bodies {
  return { shade: (ctx) => printed(inks, rule, ctx, () => litSide(ctx)), mark: () => f(1) };
}

/**
 * Under a recipe: the rule's lit inks, and the recipe's tone of the shadow block. With nothing cast from one thing on
 * another the block is printed in the fragment; otherwise the tone goes in the mark and the block is printed after the
 * bodies (`Style.block`), `shade` staying the whole colour for a fragment shaded alone.
 */
function underRecipe(inks: WoodblockInks, rule: Print, recipe: LightRecipe): Bodies {
  const pull = pullOf(blockInk(recipe.block, ownBlock(inks, rule)));
  const both = (ctx: ShadeCtx) => lit(recipe, printed(inks, rule, ctx, () => f(1)), ctx);
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
  const outline = param("woodblock.outline", 0.05, { min: 0, max: 0.2 });
  const bodies: Bodies = spec.light === undefined ? asPrinted(inks, rule) : underRecipe(inks, rule, spec.light);
  return {
    name: "woodblock",
    ...bodies,
    // The named recipes on this same print: what `--light name` chooses.
    lights: Object.fromEntries(Object.entries(LIGHT).map(([name, recipe]) => [name, () => woodblock({ ...spec, light: recipe })])),
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
      // What the swell's lines offer a foam that rides them: how far this is from the nearest line (0 on it, in line
      // spacings) and how much of a line is printed here (none in the troughs). Not their crowding: that is a screen
      // derivative, and a foam's edge takes the derivative of whatever shapes it (a derivative of a derivative, which
      // the sea's cases refuse); where lines crowd the band is a pixel or two wide and has no room for a blade anyway.
      const lines: SwellLines = { off: d, live: smoothstep(-0.6, -0.1, h) };
      c = foamed(pattern, ctx.contact, { under: c, inks, xz, swell: h, lines });
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

/** The swell's printed lines here: `off` 0 on a line to 0.5 half way between two; `live` 0..1, how much of a line prints. */
type SwellLines = { readonly off: Expr<"f32">; readonly live: Expr<"f32"> };
/** What a foam pattern is printed on: the water's colour so far, where, and the swell there (-1 trough .. 1 crest). */
type FoamOn = { readonly under: Expr<"vec3">; readonly inks: WoodblockInks; readonly xz: Expr<"vec2">; readonly swell: Expr<"f32">; readonly lines: SwellLines };
/** The same, where a body is near: `band` and `level` as `Contact` gives them (1 at the body, 0 at the sea's foam width). */
type FoamNear = FoamOn & { readonly band: Expr<"f32">; readonly level: Expr<"f32"> };

/** The contact foam: nothing where the sea found no body near, the pattern where one may be. */
function foamed(p: ContactFoam, contact: Contact, on: FoamOn): Expr<"vec3"> {
  switch (contact.kind) {
    case "clear": return on.under;
    case "near": return patterned(p, { ...on, band: contact.band, level: contact.level });
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

/** xz along the unit direction at `degrees` from +x, in periods of `period`. */
const along = (xz: Expr<"vec2">, degrees: number, period: number): Expr<"f32"> => {
  const a = (degrees * Math.PI) / 180;
  return dot(xz, vec2(Math.cos(a) / period, Math.sin(a) / period));
};

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

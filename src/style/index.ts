// Styles by name, and a scene drawn in another style. A look is a program (`Style`, src/style.ts); swapping it never
// touches the scene: the things, the sea and the cameras stay as written, only the program that lights and paints
// them changes.

import { restyled, type Scene } from "../scene.ts";
import { toon, type Style } from "../style.ts";
import { biolume } from "./biolume.ts";
import { papercraft } from "./papercraft.ts";
import { woodblock } from "./woodblock.ts";

/**
 * Every named style, each at its defaults. Constructors, not values: a style declares params, and params are global
 * by name (DESIGN.md 7.6), so a style is built when it is chosen, not when this module loads.
 */
export const STYLES = {
  toon: () => toon({ bands: 3, outline: 0.04 }),
  woodblock,
  biolume,
  papercraft,
} as const satisfies Record<string, () => Style>;

export type StyleName = keyof typeof STYLES;
export const STYLE_NAMES = Object.keys(STYLES) as readonly StyleName[];

/** The named style; an unknown name throws, listing the known ones. */
export function styleNamed(name: string): Style {
  if (!(name in STYLES)) throw new Error(`unknown style "${name}" (styles: ${STYLE_NAMES.join(", ")})`);
  return STYLES[name as StyleName]();
}

/** The same scene drawn in another style. */
export function withStyle(scene: Scene, style: Style): Scene {
  return restyled(scene, style);
}

/** The same scene, its style in one of that style's named lightings (`Style.lights`); an unknown name throws, listing them. */
export function withLight(scene: Scene, name: string): Scene {
  const lights = scene.style.lights;
  if (lights === undefined) throw new Error(`the style "${scene.style.name}" has one lighting (no named ones to choose "${name}" from)`);
  const made = lights[name];
  if (made === undefined) throw new Error(`the style "${scene.style.name}" has no lighting "${name}" (lightings: ${Object.keys(lights).join(", ")})`);
  return restyled(scene, made());
}

export { light, LIGHT, type Block, type Bokashi, type Cast, type Core, type Hatch, type Highlight, type Key, type LightName, type LightRecipe, type Overlap, type Rim, type Terminator, type Waterline } from "./light.ts";
export { woodblock, print, foam, marksAs, paint, seaPalette, LOOKS, REFERENCE_PAINT, type LookName, type WoodblockLook, type Warm, type Mottle, type LineWeight, type SeaPalette, WOODBLOCK_PALETTE, WOODBLOCK_POST, WOODBLOCK_SEA, type Accent, type ContactFoam, type MarksPrint, type HeadSize, type AccentHue, type AccentInk, type InkKey, type Print, type WoodblockEye, type WoodblockInks, type WoodblockSky, type WoodblockSpec } from "./woodblock.ts";
export { biolume } from "./biolume.ts";
export { papercraft } from "./papercraft.ts";

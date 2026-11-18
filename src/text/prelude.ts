// The prelude: the library's expression functions under their exported names, in scope in every graph function of WGSL
// text (after the module's own names, holes and `time`). Each is an ordinary TypeScript function of expressions; text
// calls it as TypeScript would. `loft context` lists these names.

import { mod, pick } from "../expr.ts";
import { cellular2, cellular3, hash2, hash22, hash3, hash33, noise2, noise3 } from "../lib/noise.ts";
import { cartesian, fresnel, hsv, luminance, polar, remap, remapClamped, rotate2, smax, smin } from "../lib/util.ts";
import { coverage, repeatAngle, sdBox, sdCapsule, sdCircle, sdRing, sdSegment, sdTriangleIsosceles, stroke } from "../lib/sdf2.ts";
import {
  elongate, extrude, intersect, revolve, sdBox3, sdCapsule3, sdCylinder, sdEllipsoid, sdPlane, sdRoundBox, sdRoundCone, sdSphere, sdTorus,
  shell, smoothIntersect, smoothSubtract, smoothUnion, subtract, union,
} from "../lib/sdf3.ts";
import { fold2, fold3 } from "../lib/fold.ts";

/**
 * Name to value. Not here: names WGSL already gives a meaning (`round`, whose WGSL meaning text keeps), and functions
 * that take option records (`fbm2`, `kifs`), which text cannot write.
 */
export const PRELUDE: ReadonlyMap<string, unknown> = new Map<string, unknown>([
  // Floored modulo (WGSL's % is the truncated remainder) and a per-index choice.
  ["mod", mod], ["pick", pick],
  ["hash2", hash2], ["hash3", hash3], ["hash22", hash22], ["hash33", hash33], ["noise2", noise2], ["noise3", noise3], ["cellular2", cellular2], ["cellular3", cellular3],
  ["remap", remap], ["remapClamped", remapClamped], ["polar", polar], ["cartesian", cartesian], ["rotate2", rotate2], ["fresnel", fresnel],
  ["smin", smin], ["smax", smax], ["luminance", luminance], ["hsv", hsv],
  ["sdCircle", sdCircle], ["sdBox", sdBox], ["sdSegment", sdSegment], ["sdCapsule", sdCapsule], ["sdRing", sdRing], ["sdTriangleIsosceles", sdTriangleIsosceles],
  ["repeatAngle", repeatAngle], ["stroke", stroke], ["coverage", coverage],
  ["sdSphere", sdSphere], ["sdBox3", sdBox3], ["sdRoundBox", sdRoundBox], ["sdCapsule3", sdCapsule3], ["sdRoundCone", sdRoundCone], ["sdTorus", sdTorus],
  ["sdCylinder", sdCylinder], ["sdPlane", sdPlane], ["sdEllipsoid", sdEllipsoid],
  ["union", union], ["subtract", subtract], ["intersect", intersect], ["smoothUnion", smoothUnion], ["smoothSubtract", smoothSubtract], ["smoothIntersect", smoothIntersect],
  ["shell", shell], ["elongate", elongate], ["revolve", revolve], ["extrude", extrude],
  ["fold2", fold2], ["fold3", fold3],
]);

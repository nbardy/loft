// Read the TS consumers' values for loft-lower's byte comparison.
import { POST_WGSL, PRELUDE_WGSL, SCENE_CLEARANCE_WGSL, SCENE_DEPTH_WGSL } from "../artefact.ts";
import { CORE_WGSL } from "../compile/scene_wgsl.ts";
import { INK_POST, MARK_WGSL } from "../draw/ink.ts";
import { libsOf, variable } from "../expr.ts";
import { CLEARANCE_WGSL, DEPTH_RESOLVE_WGSL } from "../host/lower.ts";
import { cellular2, cellular3, hash2, hash3, hash22, hash33, noise2, noise3 } from "../lib/noise.ts";
import { GAUSS_WGSL } from "../soft/gauss.ts";
import { BANK_LIB, GAUSS_LIB } from "../soft/medium.ts";
import { LONG_DIR, SCREEN_RATE, TOWARD_EYE } from "../style/light.ts";
import { FACET } from "../style/papercraft.ts";
import { POST_COMMON } from "../style/shared.ts";
import { slopeOf } from "../style/whitewater.ts";
import { RAW_PRELUDE } from "../text/raw_prelude.ts";

const p2 = variable("p2", "vec2"), p3 = variable("p3", "vec3");
const libs = libsOf([
  hash2(p2), hash3(p3), hash22(p2), hash33(p3),
  noise2(p2), noise3(p3), cellular2(p2), cellular3(p3),
  slopeOf(1, p2),
]);

export const texts = {
  PRELUDE_WGSL, SCENE_DEPTH_WGSL, SCENE_CLEARANCE_WGSL, POST_WGSL,
  CORE_WGSL, MARK_WGSL, INK_POST, DEPTH_RESOLVE_WGSL, CLEARANCE_WGSL,
  POST_COMMON, GAUSS_WGSL,
  GAUSS_LIB: GAUSS_LIB.wgsl,
  BANK_SPLIT_WGSL: BANK_LIB.wgsl,
  ...Object.fromEntries([...libs, ...RAW_PRELUDE.values(), TOWARD_EYE, LONG_DIR, SCREEN_RATE, FACET]
    .map(({ name, wgsl }) => [name.toUpperCase(), wgsl])),
};

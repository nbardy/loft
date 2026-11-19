// WGSL functions raw text (raw functions and entry points) may call by name, emitted only into the shaders that call
// them. Graph text cannot call these: they read the camera of the frame being drawn, which the CPU twin has no notion
// of (src/text/parse.ts makes a function that names one raw, with its reason).

import cameraRayWgsl from "../wgsl/camera_ray.wgsl" with { type: "text" };
import type { LibFn } from "../expr.ts";

/** The view ray through pixel `px` (pixels, origin top left): `o` on the near plane, `d` a unit direction. */
const cameraRay: LibFn = {
  name: "camera_ray", ret: "vec3", js: "gpu-only", wgsl: cameraRayWgsl,
};

export const RAW_PRELUDE: ReadonlyMap<string, LibFn> = new Map([["camera_ray", cameraRay]]);

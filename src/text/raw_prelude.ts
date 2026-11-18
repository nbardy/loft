// WGSL functions raw text (raw functions and entry points) may call by name, emitted only into the shaders that call
// them. Graph text cannot call these: they read the camera of the frame being drawn, which the CPU twin has no notion
// of (src/text/parse.ts makes a function that names one raw, with its reason).

import type { LibFn } from "../expr.ts";

/** The view ray through pixel `px` (pixels, origin top left): `o` on the near plane, `d` a unit direction. */
const cameraRay: LibFn = {
  name: "camera_ray", ret: "vec3", js: "gpu-only", wgsl: /* wgsl */ `
struct Ray { o: vec3<f32>, d: vec3<f32> };
fn camera_ray(px: vec2<f32>) -> Ray {
  let ndc = vec2<f32>(px.x * frame.viewport.z * 2.0 - 1.0, 1.0 - px.y * frame.viewport.w * 2.0);
  let near = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
  let far = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  let o = near.xyz / near.w;
  return Ray(o, normalize(far.xyz / far.w - o));
}`,
};

export const RAW_PRELUDE: ReadonlyMap<string, LibFn> = new Map([["camera_ray", cameraRay]]);

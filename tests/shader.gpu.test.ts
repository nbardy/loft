// Rungs 3 and 4 of the escape ladder read Loft values through holes (docs/proposals/2026-11-18_authoring_api.md
// section 4). Each test names the bug it catches.
import { expect, test } from "bun:test";
import { draw, nothing, param, plane, scene, toon, vec3, wgsl, type Scene } from "../src/index.ts";
import { compileJs, variable } from "../src/expr.ts";
import { compile } from "../src/compile/index.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { loadBundle } from "../src/host/host.ts";
import { lower } from "../src/host/lower.ts";
import { baked, shape } from "../examples/compute_volume.ts";

type View = { readonly camera: { kind: "perspective"; eye: [number, number, number]; target: [number, number, number]; fovY: number }; readonly width: number; readonly height: number };

async function pixels(s: Scene, view: View, time: number, params: Record<string, number> = {}): Promise<Uint8Array> {
  const device = await headlessDevice();
  try {
    const gpu = await loadBundle(device, lower(compile(s)), { kind: "none" });
    try { return new Uint8Array(await gpu.frame({ ...view, time, params, inputs: {} })); } finally { gpu.destroy(); }
  } finally { device.destroy(); }
}

// Catches: a hole accessor bound to the wrong slot or emitted as a constant (the picture would not follow --param),
// `time` not reaching raw text, and camera_ray's direction (blue is -d.z through the centre: 1 looking down -z).
test("a raw drawable reads a param, time and the camera ray through holes", async () => {
  const level = param("level", 0.25, { min: 0, max: 1 });
  const flat = draw.shader("flat", wgsl`
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  return vec4f(array(vec2f(-1.0, -1.0), vec2f(3.0, -1.0), vec2f(-1.0, 3.0))[i], 0.5, 1.0);
}
@fragment fn fs(@builtin(position) at: vec4f) -> @location(0) vec4f {
  return vec4f(${level}, 0.5 * time, -camera_ray(at.xy).d.z, 1.0);
}`, { vertices: 3 });
  const s = scene({ ground: nothing, style: toon(), sun: [0, 1, 1], things: [flat], cameras: {} });
  const view: View = { camera: { kind: "perspective", eye: [0, 0, 2], target: [0, 0, 0], fovY: 0.6 }, width: 9, height: 9 };
  const centre = (p: Uint8Array) => [...p.subarray(4 * (4 * 9 + 4), 4 * (4 * 9 + 5))];
  expect(centre(await pixels(s, view, 1))).toEqual([64, 128, 255, 255]);
  expect(centre(await pixels(s, view, 1, { level: 0.75 }))).toEqual([191, 128, 255, 255]);
}, 120_000);

// Catches: a field that reads a compute output (examples/compute_volume.ts) not depth-tested against the opaque scene (the plane drawn over the
// whole ring, or the ring over the plane). Prediction, from the source function marched on the CPU: the plane at y = 0
// hides exactly the pixels whose first hit on the ring is below it, since the camera is above the plane.
test("the baked ring, drawn by draw.field, is cut by the plane where its first hit is below y = 0", async () => {
  const W = 160, H = 100;
  const view: View = { camera: { kind: "perspective", eye: [0, 0.5, 3.2], target: [0, 0, 0], fovY: 0.7 }, width: W, height: H };
  const sdf = compileJs([shape.ring!(variable("p", "vec3"))]);
  const d = (p: number[]) => sdf({ vars: { p }, inputs: {}, params: {} })[0] as number;
  // The camera's rays: the host's perspective (y up, fovY vertical, square pixels), through pixel centres.
  const { eye, target, fovY } = view.camera;
  const sub = (a: number[], b: number[]) => a.map((x, i) => x - b[i]!);
  const norm = (a: number[]) => { const l = Math.hypot(...a); return a.map((x) => x / l); };
  const cross = (a: number[], b: number[]) => [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!];
  const f = norm(sub(target, eye)), r = norm(cross(f, [0, 1, 0])), u = cross(r, f);
  const k = Math.tan(fovY / 2);
  // Per pixel: 0 no hit, 1 a first hit at y >= 0, 2 a first hit below the plane.
  const cpu = new Uint8Array(W * H);
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const sx = ((x + 0.5) / W * 2 - 1) * k * W / H, sy = (1 - (y + 0.5) / H * 2) * k;
    const dir = norm(f.map((c, i) => c + sx * r[i]! + sy * u[i]!));
    let t = 0;
    for (let n = 0; n < 400 && t < 8; n++) {
      const p = eye.map((c, i) => c + t * dir[i]!);
      const g = d(p);
      if (g < 1e-4) { cpu[y * W + x] = p[1]! >= 0 ? 1 : 2; break; }
      t += g * 0.8;
    }
  }
  // The example's own bake, drawn by a field painted pure green without ink, so a pixel of the ring is told by its hue.
  const field = wgsl`fn field(p: vec3f) -> f32 { return ${baked.sdf}(p * 0.5 + 0.5).x; }`;
  const solid = draw.field("ring", field.field!, { bounds: { kind: "box", min: [-1, -1, -1], max: [1, 1, 1] }, steps: 64, safety: 0.8, paint: () => vec3(0, 1, 0), outline: "none", schedule: "trace" });
  const of = (ground: typeof plane | typeof nothing) => scene({ ground, style: toon(), sun: [-1, 2, -1], compute: [baked], things: [solid], cameras: {} });
  const green = (p: Uint8Array) => Uint8Array.from({ length: W * H }, (_, i) => (p[4 * i + 1]! > p[4 * i]! + 40 && p[4 * i + 1]! > p[4 * i + 2]! + 40 ? 1 : 0));
  const over = green(await pixels(of(plane), view, 0)), alone = green(await pixels(of(nothing), view, 0));
  // A pixel's class drawn: 0 no ring, 1 ring over the plane, 2 ring the plane hid. Edges are partly covered (a pixel's
  // coverage band) and the bake is 48³, so compare only pixels 2 or more from any change of the CPU's class.
  const drawn = (i: number) => (alone[i] === 0 ? 0 : over[i] === 1 ? 1 : 2);
  const n = [0, 0, 0], wrong = [0, 0, 0];
  for (let y = 2; y < H - 2; y++) for (let x = 2; x < W - 2; x++) {
    const i = y * W + x, c = cpu[i]!;
    let settled = true;
    for (let dy = -2; dy <= 2; dy++) for (let dx = -2; dx <= 2; dx++) if (cpu[i + dy * W + dx] !== c) settled = false;
    if (!settled) continue;
    n[c]!++;
    if (drawn(i) !== c) wrong[c]!++;
  }
  console.log(`settled pixels, CPU source vs GPU bake (none, over the plane, hidden by it): ${n.join(", ")}; differing ${wrong.join(", ")}`);
  expect(n[2]).toBeGreaterThan(100);
  for (const c of [0, 1, 2]) expect(wrong[c]! / n[c]!).toBeLessThan(0.01);
}, 120_000);

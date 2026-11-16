// The exported .glb played on the CPU (tools/gltf_play.ts: its skins, morph targets and keys, from the file's bytes) and
// drawn by a small z-buffered rasteriser with flat shading, through one of the scene's cameras, at a few times: the
// same contact-sheet layout `loft frames` writes, so the two sheets sit side by side. No GPU.
//   bun tools/gltf_skin_pictures.ts <scene.ts> <file.glb> <camera> <t1,t2,...> <WxH> <out.webp>
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { resolveCamera } from "../src/cli/cameras.ts";
import { contactSheet, writeWebp, type Frame } from "../src/cli/image.ts";
import type { Scene } from "../src/scene.ts";
import { pose, read, readGlb, type Glb } from "./gltf_play.ts";

const [scenePath, glbPath, cameraName, timesArg, sizeArg, out] = process.argv.slice(2) as [string, string, string, string, string, string];
const scene = ((await import(resolve(scenePath))) as { default: Scene }).default;
const camera = resolveCamera(scene, cameraName);
if (camera.kind !== "perspective") throw new Error(`gltf_skin_pictures: camera "${cameraName}" is ${camera.kind}; a perspective one is drawn`);
const [W, H] = sizeArg.split("x").map(Number) as [number, number];
const g = readGlb(new Uint8Array(readFileSync(glbPath)));
const times = timesArg.split(",").map(Number);

const sub = (a: readonly number[], b: readonly number[]) => [a[0]! - b[0]!, a[1]! - b[1]!, a[2]! - b[2]!];
const cross = (a: readonly number[], b: readonly number[]) => [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!];
const dot = (a: readonly number[], b: readonly number[]) => a[0]! * b[0]! + a[1]! * b[1]! + a[2]! * b[2]!;
const unit = (a: readonly number[]) => { const l = Math.hypot(a[0]!, a[1]!, a[2]!); return a.map((x) => x / l); };
// The host's camera (src/host/camera.ts): look from eye to target, y up, vertical field of view fovY.
const fwd = unit(sub(camera.target, camera.eye)), right = unit(cross(fwd, [0, 1, 0])), up = cross(right, fwd);
const focal = 1 / Math.tan(camera.fovY / 2), aspect = W / H;
const LIGHT = unit([0.4, 0.8, 0.45]);
const srgb = (c: number) => (c <= 0.0031308 ? 12.92 * c : 1.055 * c ** (1 / 2.4) - 0.055);

function draw(glb: Glb, t: number): Frame {
  const rgba = new Uint8Array(W * H * 4), depth = new Float32Array(W * H).fill(Infinity);
  for (let i = 0; i < W * H; i++) rgba.set([236, 236, 232, 255], 4 * i);
  pose(glb, t).forEach((p, k) => {
    const prim = glb.json.meshes[k]!.primitives[0]!, idx = read(glb, prim.indices);
    const col = prim.attributes.COLOR_0 === undefined ? undefined : read(glb, prim.attributes.COLOR_0);
    const screen = (v: number) => {
      const d = sub([p[3 * v]!, p[3 * v + 1]!, p[3 * v + 2]!], camera.eye), z = dot(d, fwd);
      return [(0.5 + (0.5 * focal * dot(d, right)) / (z * aspect)) * W, (0.5 - (0.5 * focal * dot(d, up)) / z) * H, z];
    };
    for (let f = 0; f < idx.length; f += 3) {
      const [a, b, c] = [idx[f]!, idx[f + 1]!, idx[f + 2]!];
      const P = (v: number) => [p[3 * v]!, p[3 * v + 1]!, p[3 * v + 2]!];
      const n = cross(sub(P(b), P(a)), sub(P(c), P(a)));
      if (Math.hypot(...n) === 0) continue;
      const shade = 0.35 + 0.65 * Math.abs(dot(unit(n), LIGHT));
      const rgb = [0, 1, 2].map((ch) => (col === undefined ? 0.75 : (col[3 * a + ch]! + col[3 * b + ch]! + col[3 * c + ch]!) / 3));
      const out = rgb.map((x) => Math.round(255 * Math.min(1, srgb(x * shade))));
      const [s0, s1, s2] = [screen(a), screen(b), screen(c)] as [number[], number[], number[]];
      if (s0[2]! <= 0.1 || s1[2]! <= 0.1 || s2[2]! <= 0.1) continue;
      const area = (s1[0]! - s0[0]!) * (s2[1]! - s0[1]!) - (s2[0]! - s0[0]!) * (s1[1]! - s0[1]!);
      if (area === 0) continue;
      const x0 = Math.max(0, Math.floor(Math.min(s0[0]!, s1[0]!, s2[0]!))), x1 = Math.min(W - 1, Math.ceil(Math.max(s0[0]!, s1[0]!, s2[0]!)));
      const y0 = Math.max(0, Math.floor(Math.min(s0[1]!, s1[1]!, s2[1]!))), y1 = Math.min(H - 1, Math.ceil(Math.max(s0[1]!, s1[1]!, s2[1]!)));
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        const px = x + 0.5, py = y + 0.5;
        const w0 = ((s1[0]! - px) * (s2[1]! - py) - (s2[0]! - px) * (s1[1]! - py)) / area;
        const w1 = ((s2[0]! - px) * (s0[1]! - py) - (s0[0]! - px) * (s2[1]! - py)) / area;
        const w2 = 1 - w0 - w1;
        if (w0 < 0 || w1 < 0 || w2 < 0) continue;
        const z = w0 * s0[2]! + w1 * s1[2]! + w2 * s2[2]!;
        if (z >= depth[y * W + x]!) continue;
        depth[y * W + x] = z;
        rgba.set([out[0]!, out[1]!, out[2]!, 255], 4 * (y * W + x));
      }
    }
  });
  return { width: W, height: H, rgba };
}

writeWebp(out, contactSheet(times.map((t) => draw(g, t))));
console.log(`wrote ${out}  ${times.length} frames of ${glbPath} through "${cameraName}" at t = ${times.join(", ")}`);

// The tracers (draw.field, impostors) on a real GPU: where they put the surface, what depth they write, the safety
// check, and that every traced example builds. What each test would catch is said at the test.
import { beforeAll, expect, test } from "bun:test";
import { SCENE_DEPTH_WGSL, PRELUDE_WGSL, type Artefact, type Camera, type Draw } from "../src/artefact.ts";
import coral from "../examples/coral.ts";
import nest from "../examples/nest.ts";
import pearls from "../examples/pearls.ts";
import {
  add, bound, compileScene, cos, draw, FieldSafetyError, impostors, mul, scene, sea, sin, spacing, sub, surface, toon, vec3,
  f, type F, type ImpostorShape, type Scene,
} from "../src/index.ts";
import { cameraMatrices, type CameraMatrices, type Vec3 } from "../src/host/camera.ts";
import { headlessDevice } from "../src/host/headless.ts";
import { createHost, type GpuHost } from "../src/host/index.ts";
import { fold3, kifs, sdBox3, sdSphere } from "../src/lib/index.ts";
import { FRAGMENT, gpu, layouts, VERTEX } from "./compile.harness.ts";

let host: GpuHost;
beforeAll(async () => { host = createHost(await headlessDevice()); });

// ---- a position probe ------------------------------------------------------------------------------------------
//
// A transparent draw that reads the opaque scene's depth and writes one coordinate of scene_position as 24 bits over
// [-16, 16] (resolution 2e-6), and alpha 1 where an opaque surface is. Only the drawables under test are kept, so the
// opaque depth is theirs alone.
const probeShader = (axis: "x" | "y" | "z") => /* wgsl */ `
@vertex fn vs(@builtin(vertex_index) vi: u32) -> @builtin(position) vec4<f32> {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return vec4<f32>(uv * 2.0 - 1.0, 0.0, 1.0);
}
@fragment fn fs(@builtin(position) p: vec4<f32>) -> @location(0) vec4<f32> {
  let v = floor(clamp((scene_position(p.xy).${axis} + 16.0) / 32.0, 0.0, 1.0) * 16777215.0);
  let r = floor(v / 65536.0);
  let g = floor((v - r * 65536.0) / 256.0);
  let b = v - r * 65536.0 - g * 256.0;
  return vec4<f32>(r / 255.0, g / 255.0, b / 255.0, select(1.0, 0.0, scene_depth(p.xy) >= 1.0));
}`;
const probeDraw = (shader: string): Draw => ({ label: shader, shader, vertices: 3, instances: 1, pass: "transparent", cull: "none", depthWrite: false, blend: "none", layer: "scene", constants: {}, detail: { kind: "fixed" }, sceneDepth: true });

/** The scene's draws named in `keep`, rendered with the probe: per pixel the opaque world point, or undefined. */
async function positions(s: Scene, keep: readonly string[], camera: Camera, W: number, H: number): Promise<(Vec3 | undefined)[]> {
  const a = compileScene(s).artefact;
  const probed: Artefact = {
    ...a,
    shaders: { ...a.shaders, px: probeShader("x"), py: probeShader("y"), pz: probeShader("z") },
    draws: a.draws.filter((d) => keep.some((k) => d.label === k)),
    post: undefined,
  };
  const lanes: Uint8Array[] = [];
  for (const k of ["px", "py", "pz"]) {
    const loaded = await host.load({ ...probed, draws: [...probed.draws, probeDraw(k)] });
    lanes.push(await loaded.frame({ camera, width: W, height: H, time: 0 }));
    loaded.destroy();
  }
  const dec = (px: Uint8Array, i: number) => ((px[i]! * 65536 + px[i + 1]! * 256 + px[i + 2]!) / 16777215) * 32 - 16;
  return Array.from({ length: W * H }, (_, j) => (lanes[0]![j * 4 + 3]! > 127 ? [dec(lanes[0]!, j * 4), dec(lanes[1]!, j * 4), dec(lanes[2]!, j * 4)] as Vec3 : undefined));
}

function unproject(m: CameraMatrices, ndc: readonly [number, number, number]): Vec3 {
  const v = m.invViewProj;
  const h = [0, 1, 2, 3].map((r) => v[r]! * ndc[0] + v[4 + r]! * ndc[1] + v[8 + r]! * ndc[2] + v[12 + r]!);
  return [h[0]! / h[3]!, h[1]! / h[3]!, h[2]! / h[3]!];
}
type Ray = { o: Vec3; d: Vec3 };
/** The view ray through the centre of pixel (x, y), as the host's camera defines it. */
function rayAt(m: CameraMatrices, x: number, y: number, W: number, H: number): Ray {
  const ndc = [((x + 0.5) / W) * 2 - 1, 1 - ((y + 0.5) / H) * 2] as const;
  const a = unproject(m, [ndc[0], ndc[1], 0]), b = unproject(m, [ndc[0], ndc[1], 1]);
  const d = [b[0] - a[0], b[1] - a[1], b[2] - a[2]] as Vec3, l = Math.hypot(...d);
  return { o: a, d: [d[0] / l, d[1] / l, d[2] / l] };
}
const at = (r: Ray, t: number): Vec3 => [r.o[0] + r.d[0] * t, r.o[1] + r.d[1] * t, r.o[2] + r.d[2] * t];
const dist = (a: Vec3, b: Vec3) => Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]);

/** Ray against a sphere: the entry t and how far inside the silhouette the ray passes (r minus its distance to c). */
function hitSphere(r: Ray, c: Vec3, rad: number): { t: number; inside: number } | undefined {
  const oc: Vec3 = [r.o[0] - c[0], r.o[1] - c[1], r.o[2] - c[2]];
  const b = oc[0] * r.d[0] + oc[1] * r.d[1] + oc[2] * r.d[2];
  const h = b * b - (oc[0] ** 2 + oc[1] ** 2 + oc[2] ** 2 - rad * rad);
  if (h < 0) return undefined;
  const miss = Math.sqrt(Math.max(0, oc[0] ** 2 + oc[1] ** 2 + oc[2] ** 2 - b * b));
  return { t: -b - Math.sqrt(h), inside: rad - miss };
}
/** Ray against an axis-aligned box: the entry t, and the hit point's distance to the nearest edge of its face. */
function hitBox(r: Ray, lo: Vec3, hi: Vec3): { t: number; edge: number } | undefined {
  let t0 = -Infinity, t1 = Infinity;
  for (let i = 0; i < 3; i++) {
    const a = (lo[i]! - r.o[i]!) / r.d[i]!, b = (hi[i]! - r.o[i]!) / r.d[i]!;
    t0 = Math.max(t0, Math.min(a, b)); t1 = Math.min(t1, Math.max(a, b));
  }
  if (!(t0 <= t1)) return undefined;
  const p = at(r, t0);
  // On a face one coordinate is at its bound; the edge distance is the smallest gap of the other two to theirs.
  const gaps = [0, 1, 2].map((i) => Math.min(p[i]! - lo[i]!, hi[i]! - p[i]!)).sort((u, v) => u - v);
  return { t: t0, edge: gaps[1]! };
}

const water = sea({ swell: [{ direction: [1, 0], length: 10, height: 0.1, speed: 1 }] });
const sceneOf = (...things: Parameters<typeof scene>[0]["things"]) => scene({ sea: water, things, style: toon(), sun: [0.3, 1, 0.2] });
const W = 200, H = 150;
const cameras: readonly Camera[] = [
  { kind: "perspective", eye: [3.2, 2.6, 4.1], target: [0, 1, 0], fovY: 0.8 },
  { kind: "orthographic", eye: [3.2, 4.6, 4.1], target: [0, 1, 0], height: 4.5 },
];
/** World size of a pixel at a point, for excluding pixels within a few pixels of an edge. */
const pixelAt = (cam: Camera, p: Vec3) => (cam.kind === "orthographic" ? cam.height / H : (2 * dist(p, cam.eye as Vec3) * Math.tan(cam.fovY / 2)) / H);

const C: Vec3 = [0, 1, 0];
const LO: Vec3 = [-0.6, 0.3, -0.5], HI: Vec3 = [0.7, 1.4, 0.8];

// Catches: a wrong ray origin or direction (perspective or orthographic), a march that stops a fraction of a pixel short
// (plain refining steps left 7e-3 here at oblique hits; the secant refinement gives 2e-4), depth written from the wrong point, and a half-pixel offset.
for (const camera of cameras) {
  test(`traced sphere and box hit the analytic ray intersection to 1e-3 (${camera.kind})`, async () => {
    const m = cameraMatrices(camera, W / H);
    for (const shape of ["sphere", "box"] as const) {
      const field = shape === "sphere" ? (p: Parameters<typeof sdSphere>[0]) => sdSphere(sub(p, vec3(...C)), 1) : (p: Parameters<typeof sdSphere>[0]) => sdBox3(sub(p, vec3(0.05, 0.85, 0.15)), vec3(0.65, 0.55, 0.65));
      const s = sceneOf(draw.field("probe", field, { bounds: bound.box([-1.2, -0.2, -1.2], [1.2, 2.2, 1.2]), paint: () => vec3(1, 0, 0), steps: 96, outline: "none" }));
      const got = await positions(s, ["probe"], camera, W, H);
      let worst = 0, checked = 0;
      for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
        const r = rayAt(m, x, y, W, H);
        const want = shape === "sphere" ? hitSphere(r, C, 1) : hitBox(r, LO, HI);
        if (!want) continue;
        const p = at(r, want.t);
        // Skip pixels within three pixels of a silhouette or a box edge: there the pixel's coverage is partial by design.
        const margin = 3 * pixelAt(camera, p);
        if (("inside" in want ? want.inside : want.edge) < margin) continue;
        const g = got[y * W + x];
        if (!g) throw new Error(`${shape}: pixel (${x}, ${y}) should show the surface at ${p.map((v) => v.toFixed(3))} but shows none`);
        worst = Math.max(worst, dist(g, p));
        checked++;
      }
      expect(checked).toBeGreaterThan(2000);
      expect([shape, worst < 1e-3]).toEqual([shape, true]);
    }
  }, 120000);
}

// Catches: depth written in another convention than the rasteriser's (linear depth, a missing w divide, the proxy's
// own depth): a traced sphere and a triangle sphere of the same size would then not cut each other where they meet.
test("a traced sphere and a triangle sphere interpenetrate where their analytic surfaces cross", async () => {
  const A: Vec3 = [-0.45, 1, 0], B: Vec3 = [0.45, 1, 0.1];
  const traced = draw.field("traced", (p) => sdSphere(sub(p, vec3(...A)), 1), { bounds: bound.sphere([...A], 1.05), paint: () => vec3(1, 0.05, 0.05), steps: 96, outline: "none" });
  // u from pole to pole, v round; z negated so that d/du x d/dv points outward (the lit side, back faces culled).
  const ball = surface((u: F, v: F) => {
    const b = mul(u, Math.PI), a = mul(v, Math.PI * 2);
    return add(vec3(...B), vec3(mul(sin(b), cos(a)), cos(b), mul(mul(sin(b), sin(a)), -1)));
  }, { u: ["pole", "pole"], v: "periodic" });
  const tri = draw.surface("tri", ball, { paint: () => vec3(0.05, 1, 0.05), grid: { u: spacing.even(128), v: spacing.even(256) }, sides: "one", outline: "none" });
  const s = sceneOf(traced, tri);
  const a = compileScene(s).artefact;
  const camera = cameras[0]!;
  const loaded = await host.load({ ...a, draws: a.draws.filter((d) => d.label === "traced" || d.label === "tri"), post: undefined });
  const px = await loaded.frame({ camera, width: W, height: H, time: 0 });
  loaded.destroy();
  const m = cameraMatrices(camera, W / H);
  let wrong = 0, checked = 0;
  for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
    const r = rayAt(m, x, y, W, H);
    const ha = hitSphere(r, A, 1), hb = hitSphere(r, B, 1);
    const cands = [ha && { k: "traced", ...ha }, hb && { k: "tri", ...hb }].filter((c): c is NonNullable<typeof c> => !!c);
    if (cands.length === 0) continue;
    const near = cands.reduce((u, v) => (v.t < u.t ? v : u));
    const margin = 3 * pixelAt(camera, at(r, near.t));
    // Away from silhouettes, and away from where the two surfaces are within three pixels in depth (the crossing curve).
    if (cands.some((c) => c.inside < margin) || (cands.length === 2 && Math.abs(cands[0]!.t - cands[1]!.t) < margin)) continue;
    const i = (y * W + x) * 4;
    const shows = px[i]! > px[i + 1]! ? "traced" : "tri";
    if (shows !== near.k) wrong++;
    checked++;
  }
  expect(checked).toBeGreaterThan(3000);
  expect(wrong).toBe(0);
}, 120000);

// Catches: a wrong cell walk (cells skipped, the walk stopping before a nearer copy, the wrong lattice offset), the
// per-cell shape not reaching the shader as written, and the analytic intersection itself.
test("impostor spheres hit where the analytic ray-sphere intersection says, cell by cell", async () => {
  const cell: [number, number, number] = [1.4, 1.4, 1.4];
  const imp = impostors("probe", {
    cell, bounds: { min: [-1.4, 0, -1.4], max: [1.4, 1.4, 1.4] },
    // Radius varies with the cell (0.35 to 0.6), the centre sits off the cell centre in y.
    shape: (c) => ({ kind: "sphere", centre: vec3(0, 0.1, 0), radius: add(0.45, add(mul(c.id.x, 0.08), mul(c.id.z, 0.05))) }),
    paint: () => vec3(1, 0, 0), cells: 32, outline: "none",
  });
  const spheres: { c: Vec3; r: number }[] = [];
  for (let i = -1; i <= 1; i++) for (let j = 0; j <= 1; j++) for (let k = -1; k <= 1; k++) spheres.push({ c: [i * 1.4, j * 1.4 + 0.1, k * 1.4], r: 0.45 + i * 0.08 + k * 0.05 });
  for (const camera of cameras) {
    const got = await positions(sceneOf(imp), ["probe"], camera, W, H);
    const m = cameraMatrices(camera, W / H);
    let worst = 0, checked = 0;
    for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) {
      const r = rayAt(m, x, y, W, H);
      const hits = spheres.map((s) => hitSphere(r, s.c, s.r)).filter((h): h is NonNullable<typeof h> => !!h && h.t > 0);
      if (hits.length === 0) continue;
      const near = hits.reduce((u, v) => (v.t < u.t ? v : u));
      const margin = 3 * pixelAt(camera, at(r, near.t));
      if (hits.some((h) => h.inside < margin)) continue;
      const g = got[y * W + x];
      if (!g) throw new Error(`${camera.kind}: pixel (${x}, ${y}) should show a sphere but shows none`);
      worst = Math.max(worst, dist(g, at(r, near.t)));
      checked++;
    }
    expect(checked).toBeGreaterThan(1500);
    expect([camera.kind, worst < 1e-3]).toEqual([camera.kind, true]);
  }
}, 120000);

// Catches: the check not running at compile time, not sampling where the field is used, or missing a classic mistake:
// a scale fold whose stretch factor was dropped (the distance at the folded point, used as if it were a world
// distance, overshoots by the zoom: 2^3 = 8 here). The correct field passes, and an explicit safety silences it.
test("a field with a dropped stretch factor is refused by name at compile time; the correct one compiles", () => {
  const fold = kifs({ count: 3, fold: fold3.mirror(0), scale: 2, offset: [1, 0.5, 0] });
  const b = bound.box([-1.5, -1.5, -1.5], [1.5, 1.5, 1.5]);
  const opts = { bounds: b, paint: () => vec3(1, 1, 1), steps: 32, outline: "none" } as const;
  const dropped = sceneOf(draw.field("dropped", (p) => sdSphere(fold(p).x, 0.9), opts));
  let caught: unknown;
  try { compileScene(dropped); } catch (e) { caught = e; }
  expect(caught).toBeInstanceOf(FieldSafetyError);
  const err = caught as FieldSafetyError;
  expect(err.field).toBe("dropped");
  expect(err.reason.kind).toBe("overshoots");
  if (err.reason.kind === "overshoots") expect(err.reason.measured).toBeGreaterThan(4);
  expect(err.message).toContain("dropped");
  const kept = (p: Parameters<typeof sdSphere>[0]) => { const g = fold(p); return g.toWorld(sdSphere(g.x, 0.9)); };
  expect(() => compileScene(sceneOf(draw.field("kept", kept, opts)))).not.toThrow();
  expect(() => compileScene(sceneOf(draw.field("scaled", (p) => sdSphere(fold(p).x, 0.9), { ...opts, safety: 0.12 })))).not.toThrow();
});

// Catches: WGSL the tracers emit that Dawn rejects (types, the derivative-uniformity diagnostic, sample_mask and
// frag_depth outputs, overrides), and every capsule and rounded-cone intersector, which no example draws.
test("every traced example's shaders, and impostor capsules and cones, build pipelines", async () => {
  const dev = await gpu();
  const kinds = (["capsule", "cone"] as const).map((kind) => impostors(`imp_${kind}`, {
    cell: [1, 1, 1], bounds: { min: [-2, 0, -2], max: [2, 1, 2] },
    shape: (c): ImpostorShape => kind === "capsule" ? { kind, a: vec3(0, -0.3, 0), b: vec3(0, 0.3, 0), radius: mul(c.hash.x, 0.2) } : { kind, a: vec3(0, -0.3, 0), b: vec3(0, 0.3, 0), ra: f(0.25), rb: f(0.1) },
    paint: () => vec3(1, 1, 1), cells: 16, outline: "silhouette",
  }));
  const errors: string[] = [];
  for (const s of [coral, nest, pearls, sceneOf(...kinds)]) {
    const a = compileScene(s).artefact;
    const [g0, g1] = layouts(dev, a, VERTEX | FRAGMENT);
    const depth = dev.createBindGroupLayout({ entries: [{ binding: 0, visibility: VERTEX | FRAGMENT, texture: { sampleType: "unfilterable-float" } }] });
    for (const d of a.draws) {
      dev.pushErrorScope("validation");
      const module = dev.createShaderModule({ code: PRELUDE_WGSL + (d.sceneDepth === true ? SCENE_DEPTH_WGSL : "") + a.sceneWgsl + a.shaders[d.shader] });
      try {
        await dev.createRenderPipelineAsync({
          layout: dev.createPipelineLayout({ bindGroupLayouts: d.sceneDepth === true ? [g0, g1, depth] : [g0, g1] }),
          vertex: { module, entryPoint: "vs", constants: d.constants },
          fragment: { module, entryPoint: "fs", constants: d.constants, targets: [{ format: "rgba8unorm" }] },
          primitive: { topology: "triangle-list", frontFace: "ccw", cullMode: d.cull },
          depthStencil: { format: "depth32float", depthWriteEnabled: d.depthWrite, depthCompare: "less" },
          multisample: { count: 4 },
        });
      } catch (e) { errors.push(`${d.label}: ${e instanceof Error ? e.message : String(e)}`); }
      const err = await dev.popErrorScope();
      if (err) errors.push(`${d.label}: ${err.message}`);
    }
  }
  expect(errors).toEqual([]);
}, 180000);

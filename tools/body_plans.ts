// The crabs' body plans on the CPU (docs/art_direction/DIRECTION.md, "Body plans"): each variant's surfaces evaluated
// at one time as triangles (geometry only: paint is screen-derivative and GPU-only), sorted into the crab's parts by
// name, measured in the crab's own axes, and drawn as black silhouettes through the design's cameras.
//
//   bun tools/body_plans.ts [--only a,b] [--time 1.0] [--views lowFront,game] [--out out/body_plans/silhouettes] [--names]
//
// Measured, per variant (crab axes: x forward, y up from the still water, z to its right; the crab at home faces +x):
// - body height: the carapace's top above the water; body width: its widest z extent; body length: its x extent;
// - leg span: the legs' widest z extent, tip to tip; leg reach forward: the legs' furthest x;
// - great claw: the length of the right (great) hand, wrist to tip, measured as the longest extent of the claw meshes
//   on the right side beyond the shoulder (the arm included, as the silhouette shows it): the hand's own length is
//   stated beside it from the numbers;
// - the top of everything (shell, buildings) above the water.
// Silhouettes: every crab triangle filled in black on white at the given size, through each named camera (the
// design's `camerasFor` numbers), and a sheet: rows the variants, columns the cameras. Also, for the hermit's shell:
// how many pixel rows of the back stand above the highest pixel of the face and carapace in each column, at 1080p.
import { mkdirSync } from "node:fs";
import { compileScene } from "../src/compile/index.ts";
import { variable, type Expr } from "../src/expr.ts";
import type { Scene } from "../src/scene.ts";
import type { Camera } from "../src/artefact.ts";
import { design } from "../examples/bosses/reefback_titan.ts";

type V3 = [number, number, number];
export type Part = "carapace" | "legs" | "claws" | "face" | "back" | "world";
export type Mesh = { readonly name: string; readonly part: Part; readonly pos: Float32Array; readonly tri: Uint32Array };

const U = variable("u", "f32"), W = variable("v", "f32");

/** Which part of the crab a drawable is, by the names the slots give them (docs/variants.md: names are the scene's). */
export function partOf(name: string, parts: { readonly carapace: ReadonlySet<string>; readonly legs: ReadonlySet<string>; readonly claws: ReadonlySet<string>; readonly face: ReadonlySet<string>; readonly back: ReadonlySet<string> }): Part {
  for (const p of ["carapace", "legs", "claws", "face", "back"] as const) if (parts[p].has(name)) return p;
  return "world";
}

/** Every surface of a scene as triangles at `time` (the host's default inputs). */
export function bakeGeometry(scene: Scene, time: number, along = 24, around = 12): { name: string; pos: Float32Array; tri: Uint32Array }[] {
  const c = compileScene(scene), a = c.artefact;
  const inputs = new Float32Array(Math.max(1, a.inputFloats));
  for (const s of a.inputs) inputs.set(s.name === "time" ? [time] : s.def, s.slot);
  const frame = c.frame({ time, inputs, params: Float32Array.from(a.params.map((p) => p.def)), curves: a.curves });
  const out: { name: string; pos: Float32Array; tri: Uint32Array }[] = [];
  for (const t of scene.things) for (const s of t.surfaces) {
    if ("mesh" in s) { out.push({ name: s.name, pos: Float32Array.from(s.mesh.positions), tri: Uint32Array.from(s.mesh.indices) }); continue; }
    const run = c.cpu([s.surface.point(U, W) as Expr]);
    const uAt = c.cpu([s.grid.u.at(U)]), vAt = c.cpu([s.grid.v.at(W)]);
    const us = Array.from({ length: along + 1 }, (_, i) => uAt(frame, { u: i / along })[0] as number);
    const vs = Array.from({ length: around + 1 }, (_, j) => vAt(frame, { v: j / around })[0] as number);
    const pos: number[] = [], tri: number[] = [];
    for (const u of us) for (const v of vs) pos.push(...(run(frame, { u, v })[0] as number[]));
    const at = (i: number, j: number) => i * (around + 1) + j;
    for (let i = 0; i < along; i++) for (let j = 0; j < around; j++) tri.push(at(i, j), at(i + 1, j), at(i + 1, j + 1), at(i, j), at(i + 1, j + 1), at(i, j + 1));
    // Caps: a fan over each end ring (a strand's round tips close at a point, so this only matters for flat ends).
    for (const i of [0, along]) { const c0 = pos.length / 3; const ring = vs.map((_, j) => at(i, j)); const m = [0, 1, 2].map((k) => ring.reduce((x, r) => x + pos[r * 3 + k]!, 0) / ring.length); pos.push(...m); ring.forEach((r, j) => tri.push(c0, r, ring[(j + 1) % ring.length]!)); }
    out.push({ name: s.name, pos: Float32Array.from(pos), tri: Uint32Array.from(tri) });
  }
  return out;
}

/** World to camera pixels (perspective or orthographic), y down; NaN behind the eye. */
export function projector(cam: Camera, w: number, h: number): (p: V3) => [number, number, number] {
  const eye = cam.eye as V3, tgt = cam.target as V3;
  const f = norm(sub(tgt, eye)), r = norm(cross(f, [0, 1, 0])), u = cross(r, f);
  return (p) => {
    const d = sub(p, eye), x = dot(d, r), y = dot(d, u), z = dot(d, f);
    if (cam.kind === "orthographic") { const s = h / cam.height; return [w / 2 + x * s, h / 2 - y * s, z]; }
    const s = h / 2 / Math.tan(cam.fovY / 2);
    return z <= 0.01 ? [NaN, NaN, z] : [w / 2 + (x / z) * s, h / 2 - (y / z) * s, z];
  };
}
const sub = (a: V3, b: V3): V3 => [a[0] - b[0], a[1] - b[1], a[2] - b[2]];
const dot = (a: V3, b: V3) => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: V3, b: V3): V3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const norm = (a: V3): V3 => { const l = Math.hypot(...a); return [a[0] / l, a[1] / l, a[2] / l]; };

/** A coverage mask per part (bit per part) and a depth buffer: which part is nearest at each pixel. */
export function rasterise(meshes: readonly Mesh[], cam: Camera, w: number, h: number): { part: Int8Array; any: Uint8Array } {
  const P = projector(cam, w, h), depth = new Float32Array(w * h).fill(Infinity), part = new Int8Array(w * h).fill(-1), any = new Uint8Array(w * h);
  const code: Record<Part, number> = { carapace: 0, legs: 1, claws: 2, face: 3, back: 4, world: 5 };
  for (const m of meshes) {
    if (m.part === "world") continue;
    const q: [number, number, number][] = [];
    for (let i = 0; i < m.pos.length; i += 3) q.push(P([m.pos[i]!, m.pos[i + 1]!, m.pos[i + 2]!]));
    for (let t = 0; t < m.tri.length; t += 3) {
      const a = q[m.tri[t]!]!, b = q[m.tri[t + 1]!]!, c = q[m.tri[t + 2]!]!;
      if (![a, b, c].every((v) => Number.isFinite(v[0]))) continue;
      const x0 = Math.max(0, Math.floor(Math.min(a[0], b[0], c[0]))), x1 = Math.min(w - 1, Math.ceil(Math.max(a[0], b[0], c[0])));
      const y0 = Math.max(0, Math.floor(Math.min(a[1], b[1], c[1]))), y1 = Math.min(h - 1, Math.ceil(Math.max(a[1], b[1], c[1])));
      const area = (b[0] - a[0]) * (c[1] - a[1]) - (b[1] - a[1]) * (c[0] - a[0]);
      if (Math.abs(area) < 1e-9) continue;
      for (let y = y0; y <= y1; y++) for (let x = x0; x <= x1; x++) {
        const px = x + 0.5, py = y + 0.5;
        const l0 = ((b[0] - px) * (c[1] - py) - (b[1] - py) * (c[0] - px)) / area, l1 = ((c[0] - px) * (a[1] - py) - (c[1] - py) * (a[0] - px)) / area, l2 = 1 - l0 - l1;
        if (l0 < 0 || l1 < 0 || l2 < 0) continue;
        const z = l0 * a[2] + l1 * b[2] + l2 * c[2], k = y * w + x;
        any[k] = 1;
        if (z < depth[k]!) { depth[k] = z; part[k] = code[m.part]; }
      }
    }
  }
  return { part, any };
}

/** Extent of a part's vertices (crab axes = world for a crab at home). */
export function extent(meshes: readonly Mesh[], keep: (m: Mesh) => boolean): { lo: V3; hi: V3 } {
  const lo: V3 = [Infinity, Infinity, Infinity], hi: V3 = [-Infinity, -Infinity, -Infinity];
  for (const m of meshes) if (keep(m)) for (let i = 0; i < m.pos.length; i += 3) for (let k = 0; k < 3; k++) { lo[k] = Math.min(lo[k]!, m.pos[i + k]!); hi[k] = Math.max(hi[k]!, m.pos[i + k]!); }
  return { lo, hi };
}

/** A crab's meshes at `time`, each sorted into its part by the names its slots drew. */
export function crabMeshes(variant: string, time: number, also: Record<string, unknown> = {}): { meshes: Mesh[]; cameras: Readonly<Record<string, Camera>> } {
  const r = design.resolve({ variant, picks: [] }, also as never);
  const b = design.build(r);
  const scene = b.scene as Scene;
  // The parts, by name: carapace "titan" (and any carapace extras), legs "leg*", claws by the claw names, the face's,
  // the back's. Read from the names the scene's drawables carry; anything unknown is the world (boats, foam, rings).
  const names = scene.things.map((t) => t.name);
  const is = (re: RegExp) => new Set(names.filter((n) => re.test(n)));
  const parts = {
    carapace: is(/^(titan|kabuto|sode|tassel|carapace|thorn)/i),
    legs: is(/^leg[LR]/),
    claws: is(/^(crusher|cutter|great|fiddler|claw|tong|gauntlet|dactyl|pollex|arm)/i),
    face: is(/^(cheek|maxilliped|rostrum|eye|stalk|face|mask|brow|mouth|beard|antenna|mandible|jaw|gullet|plate|lid|pupil|tooth|moustache|whisk|head|dandy|jitter|mourner)/i),
    back: is(/^(gallery|rock|island|atoll|fort|reef|coral|tree|spiral|shell|light|keep|pine|shrine|terrace|grove|gull|beacon|lighthouse|mound|turf|cliff|plate|stack|tower|lantern|barnacle|collar)/i),
  };
  const geo = bakeGeometry(scene, time);
  const meshes = geo.map((g) => ({ ...g, part: partOf(g.name, parts) }));
  // And the lineup's two cameras the hermit's shell is judged from, for this crab standing at the V's front place (0, 0)
  // facing the camera (lineup_shots.ts: the group's wide V, `raised(0, 3, 20, 34, 15, 0.8)`; the islander's low single,
  // `single`: 36 in front, the eye 3 up, aimed 4.9 up). The dance's pose is not applied: the crab at rest.
  const p = (15 * Math.PI) / 180;
  const extra: Record<string, Camera> = {
    V: { kind: "perspective", eye: [34, 20, 3], target: [34 - 10 * Math.cos(p), 20 - 10 * Math.sin(p), 3], fovY: 0.8 },
    single: { kind: "perspective", eye: [36, 3, 0], target: [0, 4.9, 0], fovY: 0.4 },
  };
  return { meshes, cameras: { ...(scene.cameras as Readonly<Record<string, Camera>>), ...extra } };
}

if (import.meta.main) {
  const arg = (k: string, d: string) => { const i = process.argv.indexOf(k); return i > 0 ? process.argv[i + 1]! : d; };
  const only = arg("--only", "fiddler,mask,islander,bruiser,stilt").split(",");
  // --set '{"islander": {"ride": 1.0}}': numbers overriding a variant's own (exploring; nothing is saved).
  const sets = JSON.parse(arg("--set", "{}")) as Record<string, Record<string, unknown>>;
  const time = Number(arg("--time", "1.0"));
  if (process.argv.includes("--names")) {
    for (const v of only) { const { meshes } = crabMeshes(v, time); for (const m of meshes) console.log(v, m.part, m.name); }
    process.exit(0);
  }
  const views = arg("--views", "lowFront,game").split(","), out = arg("--out", "out/body_plans/silhouettes");
  const [W, H] = arg("--size", "480x270").split("x").map(Number) as [number, number];
  mkdirSync(out, { recursive: true });
  const rows: string[] = [];
  const tiles: Uint8Array[][] = [];
  for (const v of only) {
    const t0 = performance.now();
    const { meshes, cameras } = crabMeshes(v, time, sets[v] ?? {});
    const ex = (p: Part | Part[]) => extent(meshes, (m) => ([] as Part[]).concat(p).includes(m.part));
    const car = ex("carapace"), legs = ex("legs"), all = ex(["carapace", "legs", "claws", "face", "back"]);
    // The great hand's length: the largest distance between two of its points (palm and fingers, no arm).
    const hand: V3[] = [];
    for (const m of meshes) if (m.part === "claws" && /^crusher(Palm|Dactyl|Fixed)/.test(m.name)) for (let i = 0; i < m.pos.length; i += 9) hand.push([m.pos[i]!, m.pos[i + 1]!, m.pos[i + 2]!]);
    let clawLen = 0;
    for (const a of hand) for (const b of hand) clawLen = Math.max(clawLen, Math.hypot(a[0] - b[0], a[1] - b[1], a[2] - b[2]));
    rows.push([v, (car.hi[1]).toFixed(2), (car.hi[2] - car.lo[2]).toFixed(2), (car.hi[0] - car.lo[0]).toFixed(2), (legs.hi[2] - legs.lo[2]).toFixed(2), legs.hi[0].toFixed(2), clawLen.toFixed(2), all.hi[1].toFixed(2), `${((performance.now() - t0) / 1000).toFixed(1)}s`].join("\t"));
    const row: Uint8Array[] = [];
    for (const name of views) {
      const cam = cameras[name]; if (!cam) throw new Error(`no camera ${name}`);
      const { any } = rasterise(meshes, cam, W, H);
      row.push(any);
    }
    // The shell over the head, at 1920 x 1080: the highest visible row of the shell (the coil's tube, not the light on
    // it) against the highest row of the face, the carapace and the claws (whatever stands in front of it).
    if (meshes.some((m) => m.name === "spiral")) for (const name of ["V", "single", "lowFront"]) {
      const cam = cameras[name]!, w = 1920, h = 1080;
      const shell = meshes.map((m) => (m.name === "spiral" ? m : { ...m, part: m.part === "back" ? ("world" as Part) : m.part }));
      const { part } = rasterise(shell, cam, w, h);
      let shellTop = h, headTop = h, eyeTop = h, shellAbove = 0;
      for (let k = 0; k < w * h; k++) { const y = Math.floor(k / w); if (part[k] === 4) shellTop = Math.min(shellTop, y); if (part[k] === 3) eyeTop = Math.min(eyeTop, y); if (part[k] === 0 || part[k] === 2 || part[k] === 3) headTop = Math.min(headTop, y); }
      for (let k = 0; k < w * eyeTop; k++) if (part[k] === 4) shellAbove++;
      console.log(`${v} ${name}: shell top row ${shellTop}, eyes' top ${eyeTop}, everything in front's top ${headTop}: the shell stands ${eyeTop - shellTop} px over the eyes (${shellAbove} px of shell above them), ${headTop - shellTop} px over all of the front`);
    }
    tiles.push(row);
  }
  console.log(["variant", "bodyTop", "bodyWidth", "bodyLength", "legSpan", "legFront", "greatClawBox", "top"].join("\t"));
  for (const r of rows) console.log(r);
  // The sheet as a PGM, converted by the caller (cwebp reads it? no: written as PGM then sips/cwebp via png).
  const SW = W * views.length, SH = H * only.length, img = new Uint8Array(SW * SH).fill(255);
  tiles.forEach((row, ri) => row.forEach((any, ci) => { for (let y = 0; y < H; y++) for (let x = 0; x < W; x++) if (any[y * W + x]) img[(ri * H + y) * SW + ci * W + x] = 0; for (let y = 0; y < H; y++) img[(ri * H + y) * SW + ci * W] = 128; for (let x = 0; x < W; x++) img[(ri * H) * SW + ci * W + x] = 128; }));
  const header = new TextEncoder().encode(`P5\n${SW} ${SH}\n255\n`);
  await Bun.write(`${out}/sheet.pgm`, new Uint8Array([...header, ...img]));
  console.log(`wrote ${out}/sheet.pgm ${SW}x${SH} rows ${only.join(",")} cols ${views.join(",")}`);
}

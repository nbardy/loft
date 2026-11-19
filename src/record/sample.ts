// One job with every scene-level decl layout (src/record/decls.ts): no corpus scene records one yet, so this is the
// cross-language check of their layouts and print until lane 3.1's library does.
//
//   bun src/record/sample.ts DIR     writes DIR/__scene_decls.job and its TS print, DIR/__scene_decls.txt
//
// Put them in the corpus folder and loft-ir's `corpus_prints_equal` compares the Rust print of the same bytes.

import { writeFileSync } from "node:fs";
import { join } from "node:path";
import { add, dot, fetch, ge, length, lib, mapOf, mul, param, sub, time, variable, vec2, vec3, vec4, type Expr, type LibFn, type TableSource } from "../expr.ts";
import { compute } from "../compute.ts";
import { pass } from "../frame/pass.ts";
import { recur } from "../map/recur.ts";
import { wgsl } from "../text/index.ts";
import { role, type Camera, type Draw, type Scene } from "./decls.ts";
import { printJob } from "./print.ts";
import { rec } from "./recorder.ts";
import { pack, Program, type Job } from "./wire.ts";

export function sampleJob(): Job {
  const glow = param("sample.glow", 0.5, { min: 0, max: 1 });
  const table: TableSource = { name: "sample_pose", lanes: 1, entries: 4, role: "data", program: mapOf(4, (i) => [vec4(i, mul(time, i), 0, 0.5)]) };
  const paint = role("sample_paint", ["vec3", "vec3", "vec2", "f32"], (p, n, uv, id) => vec4(mul(n, glow), add(length(p), add(id, uv.x))));
  const hit = role("sample_hit", ["vec3", "vec3", "vec2", "vec2", "f32"], (o, d) => vec4(d, sub(dot(o, d), 1)));
  const place = role("sample_place", ["vec2", "f32"], (uv, id) => vec3(dot(uv, uv), id, length(fetch(table, id, 0))));
  const normal = role("sample_normal", ["vec2", "f32"], (uv) => vec3(0, 1, uv.x));
  const cells = role("sample_cells", ["f32"], (id) => vec2(8, add(2, id)));
  const quads = role("sample_quads", ["f32"], (id) => fetch(table, id, 0));
  const density = role("sample_density", ["vec3"], (p) => sub(1, length(p)));
  const ink = role("sample_ink", ["vec3", "vec3"], (c, q) => vec4(c, length(q)));
  const raster = { cull: "back", depthWrite: true, blend: "opaque" } as const;
  const camera: Camera = { name: "game", kind: "look", eye: vec3(0, 2, mul(time, -5)), target: vec3(0, 0, 0), up: vec3(0, 1, 0), fovy: add(0.8, glow) };
  const draws: Draw[] = [
    { name: "sheet", layer: "subject", raster, paint, cover: { kind: "grid", u: 8, v: 4, place, normal, cells, count: 2 } },
    { name: "body", layer: "subject", raster, paint, cover: { kind: "ray", proxy: { kind: "box", min: [-1, 0, -1], max: [1, 2, 1] }, find: { kind: "hit", hit } } },
    { name: "fog", layer: "surroundings", raster: { ...raster, depthWrite: false, blend: "alpha" }, paint, cover: {
      kind: "ray", proxy: { kind: "instances", table, local: [[-1, -1, -1], [1, 1, 1]] },
      find: { kind: "medium", medium: { kind: "march", density, within: [[-2, 0, -2], [2, 1, 2]], steps: 48, shadowSteps: 8 } },
    } },
    { name: "sparks", layer: "subject", raster: { ...raster, blend: "add" }, paint, cover: { kind: "quads", place: quads, count: 4 } },
    { name: "hull", layer: "subject", raster, paint, cover: { kind: "mesh", table, count: 1 } },
    { name: "sky", layer: "surroundings", raster, paint, cover: { kind: "screen" } },
  ];
  const t = variable("sample_t", "f32"), jets = [0, 1, 2, 3].map((k) => variable(`sample_y${k}`, "f32")) as [Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">];
  const scene: Scene = {
    cameras: [],
    passes: [
      { camera, draws: draws.slice(0, 4), clear: { kind: "colour", rgba: [0.1, 0.2, 0.3, 1] }, ground: { kind: "traced", hit, paint }, outline: { kind: "ink", outline: ink } },
      { camera: { name: "top", kind: "matrix", cols: [vec4(1, 0, 0, 0), vec4(0, 1, 0, 0), vec4(0, 0, 1, 0), vec4(0, 0, 0, 1)] }, draws: draws.slice(4), clear: { kind: "load" }, ground: { kind: "none" }, outline: { kind: "none" } },
    ],
    checks: [{
      label: "clears", how: { kind: "prove", boxes: 20000 },
      body: { kind: "always", leaves: [{ leaf: time, lo: vec2(0, 0).x, hi: add(2, glow) }], body: {
        kind: "and", parts: [
          { kind: "atom", margin: sub(fetch(table, 1, 0).y, 0.1), test: ge(fetch(table, 1, 0).y, 0.1) },
          { kind: "not", part: { kind: "eventually", leaves: [{ leaf: t, lo: vec2(0, 0).x, hi: vec2(1, 0).x }], body: { kind: "atom", margin: sub(t, glow), test: ge(t, glow) } } },
        ],
      } },
    }],
    solves: [{
      label: "leap", over: [0, 2], delta: 4e-6, jerk: 1, acceleration: 0, nearWeight: 0.5,
      unknowns: [{ name: "y", numbers: 8, from: mul(time, 0.5), jets }],
      near: [{ unknown: 0, to: time }],
      holds: [
        { kind: "bound", margin: sub(jets[0], 0.2), times: { kind: "span" }, said: "ge", site: "sample.ts:1" },
        { kind: "equal", margin: vec2(jets[0], jets[1]), within: 1e-3, times: { kind: "at", t: 1 }, said: "equal", site: "sample.ts:2" },
        { kind: "bound", margin: jets[2], times: { kind: "during", from: 0.5, to: 1.5 }, said: "gt", site: "sample.ts:3" },
      ],
    }],
  };
  // Wire 3: a recurrence (its kept states and previous state), two compute passes (one reading the other through its
  // handle), an offscreen scene pass and a raw function reading a table by its accessor, all reached from the roots.
  const flock = recur({ name: "sample_flock", size: 4, rate: 10, until: 1, start: (i) => [vec4(i, 0, 0, 1)], step: (prev, i) => [add(prev(add(i, 1))[0]!, vec4(time, 0, 0, 0))] });
  const tones = compute("sample_tones", { when: { kind: "load" }, writes: { tones: { kind: "buffer", entries: 2, lanes: 1 } }, dispatch: [1], wgsl: wgsl`
@compute @workgroup_size(1) fn fill() { tones[0] = vec4<f32>(0.1, 0.2, 0.3, 1.0); tones[1] = vec4<f32>(1.0); }` });
  const field = compute("sample_field", { when: { kind: "steps", start: "seed", count: 8, rate: 30, keep: 2 }, writes: { cells: { kind: "texture", size: [8, 8], format: "rgba8unorm" } }, dispatch: [1, 1], wgsl: wgsl`
@compute @workgroup_size(8, 8) fn seed(@builtin(global_invocation_id) id: vec3<u32>) { textureStore(cells, vec2<i32>(id.xy), ${tones.tones}(0.0, 0.0)); }
@compute @workgroup_size(8, 8) fn step(@builtin(global_invocation_id) id: vec3<u32>) { textureStore(cells, vec2<i32>(id.xy), textureLoad(prev_cells, vec2<i32>(id.xy), 0) * 0.5); }` });
  const lit = compute("sample_lit", { when: { kind: "frame" }, writes: { glow: { kind: "buffer", entries: 1, lanes: 1 } }, dispatch: [1], wgsl: wgsl`
@compute @workgroup_size(1) fn fill() { glow[0] = ${tones.tones}(1.0, 0.0) * ${glow} + vec4<f32>(time); }` });
  const mirror = pass({ name: "sample_mirror", size: { scale: 0.5 }, camera: { kind: "mirror", normal: [0, 1, 0], offset: 0 } });
  const accessor: LibFn = { name: "sample_row", ret: "vec4", js: "gpu-only", wgsl: "fn sample_row(i: f32) -> vec4<f32> { return t_sample_pose(u32(i)); }", reads: [{ kind: "table", table }] };
  const wire3 = add(flock.at(1)[0]!, field.cells(vec2(0.5, 0.5)), lit.glow(0, 0), mirror.at(vec2(0.25, 0.75)), lib<"vec4">(accessor, glow));
  return rec.job(Program.Scene, [mul(glow, time), wire3], scene);
}

if (import.meta.main) {
  const dir = Bun.argv[2] ?? ".", job = sampleJob();
  writeFileSync(join(dir, "__scene_decls.job"), pack(job));
  writeFileSync(join(dir, "__scene_decls.txt"), printJob(job));
  console.log(`wrote ${join(dir, "__scene_decls")}.{job,txt}`);
}

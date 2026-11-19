// A drawable Bundle V1 for the loft-draw tests: a unit sphere found by a Ray cover's hit function, painted by its
// normal times `ball.tint`, seen by the bundle camera "front" (eye (0, 0, 5), fovy 0.8). No corpus scene records draws
// yet (the TS library's hit, place and paint functions), so this job is written with the recorder's decls directly.
import { add, dot, gt, max, mul, neg, normalize, param, select, sqrt, sub, vec3, vec4 } from "../../src/expr.ts";
import { native, UNLIMITED } from "../../src/core/native.ts";
import { role, type Scene } from "../../src/record/decls.ts";
import { rec } from "../../src/record/recorder.ts";
import { pack, Program } from "../../src/record/wire.ts";

/** Writes `dir/scene.loft`. */
export function writeBall(dir: string): void {
  const tint = param("ball.tint", 1, { min: 0, max: 1 });
  const hit = role("ball_hit", ["vec3", "vec3", "vec2", "vec2"], (o, d) => {
    const b = dot(o, d), disc = sub(mul(b, b), sub(dot(o, o), 1));
    const t = sub(neg(b), sqrt(max(disc, 0)));
    return vec4(normalize(add(o, mul(d, t))), select(gt(disc, 0), t, -1));
  });
  const paint = role("ball_paint", ["vec3", "vec3", "vec2", "f32"], (_p, n) => vec4(mul(add(mul(n, 0.5), 0.5), tint), 1));
  const scene: Scene = {
    cameras: [],
    passes: [{
      camera: { name: "front", kind: "look", eye: vec3(0, 0, 5), target: vec3(0, 0, 0), up: vec3(0, 1, 0), fovy: vec3(0.8, 0, 0).x },
      draws: [{ name: "ball", layer: "subject", raster: { cull: "back", depthWrite: true, blend: "opaque" }, paint, cover: { kind: "ray", proxy: { kind: "box", min: [-1, -1, -1], max: [1, 1, 1] }, find: { kind: "hit", hit } } }],
      clear: { kind: "colour", rgba: [0, 0, 0, 1] }, ground: { kind: "none" }, outline: { kind: "none" },
    }],
    checks: [], solves: [],
  };
  const limits = { nodes: UNLIMITED, tapeOps: UNLIMITED, boxes: UNLIMITED, gpuNanos: UNLIMITED };
  native().run(pack(rec.job(Program.Scene, [], scene)), limits, `${dir}/scene.loft`, { naga: "emit", precision: "f32" });
}

if (import.meta.main) writeBall(Bun.argv[2]!);

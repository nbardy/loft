// One live graph table is shared by two independent observation programs.
import { mapOf, time, vec4, type TableSource } from "../src/expr.ts";
import { ring } from "../examples/slice/washer.ts";
import { inspect, print, twin, type Scene } from "loft/render";
import { build, draw, run, out } from "./slice_measure.ts";
import { readPng } from "./picdiff.ts";
import { writeFileSync } from "node:fs";
const pose: TableSource = {
  name: "POSE",
  role: "data",
  entries: 1,
  lanes: 1,
  program: mapOf(1, () => [vec4(time, 0, 1, 0)]),
};
const cameras: Scene["cameras"] = { front: { kind: "orthographic", eye: [0, 0, 3], target: [0, 0, 0], height: 8 } };
const engines = [
  print({ samples: 1, shade: "fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f{return vec4f(1,0,0,1);}" }),
  inspect("fn pixel(px:vec2f)->vec4f{return vec4f(f32(trace(camera_ray(px)).t>=0),0,0,1);}"),
];
const scene = { cameras, things: [ring], tables: [pose], origin: "fn origin(i:u32)->vec3f{return t_POSE(i).xyz;}" };
const cpu = await twin(scene, { POSE: Float32Array.of(1, 0, 1, 0) });
const edge = cpu.trace([1, 0.75, 3], [0, 0, -1]);
const hole = cpu.trace([1, 0, 3], [0, 0, -1]);
const snapshot = { edge, hole, pass: edge.kind === "hit" && edge.id === 7 && edge.t === 2 && hole.kind === "miss" };
const pictures = engines.map((engine, k) => {
  const file = build("pose" + k, { ...scene, engine });
  const stem = out + "/pose" + k;
  run([
    draw,
    "still",
    file,
    "--view",
    "front",
    "--time",
    "0",
    "--size",
    "64x64",
    "--out",
    stem + "-0.png",
    "--shot",
    "front@1@64x64=" + stem + "-1.png",
  ]);
  return [0, 1].map((t) => readPng(stem + "-" + t + ".png").rgba);
});
const results = [0, 1].map((t) => {
  let wrong = 0,
    hits = 0,
    sum = 0;
  for (let i = 0; i < 4096; i++) {
    const x = i % 64,
      y = Math.floor(i / 64),
      r2 = (x + 0.5 - 32 - t * 8) ** 2 + (y + 0.5 - 32) ** 2;
    const v = pictures[0][t][4 * i],
      expected = r2 >= 16 && r2 <= 64 ? 255 : 0;
    wrong += Number(v !== expected || v !== pictures[1][t][4 * i]);
    if (v) {
      hits++;
      sum += x + 0.5;
    }
  }
  return { t, wrong, hits, centre: sum / hits };
});
console.log(results, snapshot);
writeFileSync(out + "/pose.json", JSON.stringify({ results, snapshot }, null, 2));
process.exitCode = Number(
  !snapshot.pass || results.some((r) => r.wrong !== 0 || r.hits !== 156 || r.centre !== 32 + r.t * 8),
);

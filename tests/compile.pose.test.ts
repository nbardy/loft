// The pose pass (src/compile/pose.ts, tables.ts): pure, slices at other times, attachment, curve edits, the budget.
import { expect, test } from "bun:test";
import serpent from "../examples/serpent.ts";
import {
  add, attach, compile, compileScene, input, keys, mul, scene, sea, sin, strand, TableBudgetError, through, time, toon, vec3,
  type Drawable, type StationRange,
} from "../src/index.ts";
import { STATION_VEC4S, v3, type V3 } from "../src/geom/stations.ts";
import { stateAt } from "./compile.harness.ts";

const flat = sea({ swell: [] });

// Catches: the rest-pose memo returning a stale rest after params change (the pose would then depend on what was
// asked before, not only on its argument), or any other hidden state in the pose pass.
test("pose is a function of its state alone", () => {
  const a = compile(serpent);
  const set = { params: { "serpent.neckRise": 6, "serpent.girth": 1.2 } };
  const first = a.pose(stateAt(a, 1.3));
  a.pose(stateAt(a, 0.2, set));
  const again = a.pose(stateAt(a, 1.3));
  for (const k of Object.keys(first)) expect(Array.from(again[k])).toEqual(Array.from(first[k]));
  const fresh = compile(serpent).pose(stateAt(a, 0.2, set));
  const changed = a.pose(stateAt(a, 0.2, set));
  for (const k of Object.keys(fresh)) expect(Array.from(changed[k])).toEqual(Array.from(fresh[k]));
});

const girth = keys([[0, 0.3], [1, 0.6]]);
const eel = strand({
  name: "eel",
  spine: (m) => add(through(m, [[-3, 0.2, 0], vec3(0, add(1, sin(mul(time, 1.7))), 0.5), vec3(3, 0.4, mul(time, 0.3))]), vec3(0, 0, mul(m, sin(time)))),
  stations: 10, girth: (m) => girth(m).x, paint: [1, 1, 1],
});
const stationsOf = (t: Float32Array, r: StationRange) => Array.from(t.subarray(r.base * STATION_VEC4S * 4, (r.base + r.count) * STATION_VEC4S * 4));

// Catches: a slice that substitutes time in only part of the strand (the spine but not its frame seed, or not its
// rest), a slice range that overlaps another, or an input-timed slice reading the wrong slot. The body at time + 0.5
// must be exactly the body at a frame 0.5 s later, and the body at an input's time the body at that time.
test("a strand at time + offset, or at an input's time, is the same strand at that time", () => {
  const arm = input("attack.arm", "f32", [0]);
  const ranges: StationRange[] = [];
  const slicer: Drawable = { name: "slicer", draws: (ctx) => { ranges.push(ctx.stations(eel), ctx.stations(eel, add(time, 0.5)), ctx.stations(eel, arm)); return { shaders: {}, draws: [] }; } };
  const c = compileScene(scene({ sea: flat, things: [slicer], style: toon(), sun: [0, 1, 0] }));
  const [now, later, armed] = ranges;
  expect(new Set(ranges.map((r) => r.base)).size).toBe(3);
  const at = (t: number, arming = 0) => c.frame(stateAt(c.artefact, t, { inputs: { "attack.arm": arming } })).table(now.table);
  // Times exact in f32 (inputs are f32 slots), so the two routes compute from the same numbers.
  expect(stationsOf(at(1.25), later)).toEqual(stationsOf(at(1.75), now));
  expect(stationsOf(at(0.375, 2.25), armed)).toEqual(stationsOf(at(2.25), now));
});

// Catches: an attached part placed from a stale or wrong host frame (host posed after the part, the wrong slice, the
// frame's axes permuted), so it drifts off its host as the host moves. The part's root is the local origin and its
// tip 0.5 along local y: the root must sit on the host's surface point at (m, v, rho) and the tip 0.5 out along the
// chart's y, at every time of a sweep.
test("an attached part stays on its host's surface through the animation", () => {
  const spec = { to: eel, m: 0.37, v: 0.31, rho: 1 };
  const stub = attach(strand({ name: "stub", spine: (m) => through(m, [[0, 0, 0], [0, 0.5, 0]]), stations: 4, girth: 0.05, tips: "open", up: [1, 0, 0], paint: [1, 1, 1] }), spec);
  const c = compileScene(scene({ sea: flat, things: [eel, stub], style: toon(), sun: [0, 1, 0] }));
  const host = c.chart(eel), part = c.chart(stub);
  let worst = 0;
  for (let t = 0; t < 6; t += 0.37) {
    const frame = c.frame(stateAt(c.artefact, t));
    const b = host.cpu(frame)(spec.m, spec.v, spec.rho);
    const table = frame.table(part.range.table);
    const p = (k: number): V3 => { const o = (part.range.base + k) * STATION_VEC4S * 4; return [table[o], table[o + 1], table[o + 2]]; };
    worst = Math.max(worst, v3.len(v3.sub(p(0), b.pos)), v3.len(v3.sub(p(3), v3.axpy(0.5, b.y, b.pos))));
  }
  // Stored as f32: positions of a few units hold about 1e-6.
  expect(worst).toBeLessThan(1e-5);
});

// Catches the staleness reported at M1: a host's `setCurves` edit reached the GPU but the CPU pose still read the
// curves the scene was authored with. The pose must read the curve table it is handed.
test("an edited curve table moves the CPU pose", () => {
  const c = compileScene(scene({ sea: flat, things: [eel], style: toon(), sun: [0, 1, 0] }));
  const a = c.artefact, r = c.chart(eel).range;
  const doubled = a.curves.map((x) => x * 2);
  const before = c.frame(stateAt(a, 0.5)).table(r.table), after = c.frame(stateAt(a, 0.5, { curves: doubled })).table(r.table);
  for (let k = 1; k < 9; k++) {
    const o = (r.base + k) * STATION_VEC4S * 4 + 8;
    expect(after[o]).toBeCloseTo(before[o] * 2, 5);
  }
});

// Catches: tables that cannot fit the 5 storage buffers a stage has left being accepted, so the failure would come
// later as an opaque pipeline error on the host.
test("a scene whose tables exceed what a shader stage can bind fails by name", () => {
  const hog: Drawable = { name: "hog", draws: (ctx) => {
    for (let i = 0; i < 6; i++) ctx.table({ name: `hog${i}`, vec4s: (128 * 1024 * 1024) / 16, fill: () => {} });
    return { shaders: {}, draws: [] };
  } };
  expect(() => compile(scene({ sea: flat, things: [hog], style: toon(), sun: [0, 1, 0] }))).toThrow(TableBudgetError);
});

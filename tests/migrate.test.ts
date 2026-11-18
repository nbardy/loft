import { expect, test } from "bun:test";
import { migrate } from "../tools/migrate.ts";

const frozen = "/archive/frozen-loft/src/";

function port(before: string, after: string | readonly string[]) {
  const result = migrate(before);
  expect(result.changed).toBe(true);
  expect(result.unported).toEqual([]);
  for (const part of typeof after === "string" ? [after] : after) expect(result.code).toContain(part);
  expect(migrate(result.code)).toEqual({ code: result.code, changed: false, unported: [] });
  return result.code;
}

test("archive and deep imports follow public entries while preserving the fold overload", () => {
  port(`import { strand } from "${frozen}index.ts";
import { scene } from "${frozen}core.ts";
import { sdSphere } from "../src/lib/sdf3.ts";
import { round } from "${frozen}lib/sdf3.ts";
import { movable } from "${frozen}sun.ts";
import { strand as own } from "./other.ts";`, [
    'import { strand } from "loft";',
    'import { scene } from "loft";',
    'import { sdSphere } from "loft";',
    'import { round } from "./src/lib/sdf3.ts";',
    'import { movable } from "loft";',
    'import { strand as own } from "./other.ts";',
  ]);
});

test("the instancer becomes posed and keeps its unturned points", () => {
  port(`import { draw as d, points, type Points, type Instanceable } from "loft";
const pts: Points = points(2, (i) => ({ at: [i, 0, 0], scale: 1 }));
const inst = (thing: Instanceable) => [d["points"]("copies", pts, thing), thing.instanced("again", pts)];
const grains = d.points("grains", 2, grain);`, [
    'import { draw as d, points, type Poses as Points, type Poseable as Instanceable }',
    'd["posed"]("copies", pts, thing), thing.posed("again", pts)',
    'd.points("grains", 2, grain)',
  ]);
});

test("specks and its options move to points, removing a bound slot parameter", () => {
  port(`import { draw, type SpecksOptions } from "loft";
const dust = draw.specks("dust", 16, (i, t) => ({
  at: path(i, t), radius: 0.02, colour, opacity: ((t) => t)(0),
}), {} satisfies SpecksOptions);`, [
    'type PointsOptions as SpecksOptions',
    'draw.points("dust", 16, (i) => ({',
    'at: path(i, time), radius: 0.02, colour, opacity: ((t) => t)(0)',
  ]);
});

test("volume replaces time in density, albedo and emission without capturing a shadow", () => {
  port(`import { draw, f, time } from "loft";
const density = (p, t) => f(p.x).add(t);
const dust = draw.volume("dust", density, {
  albedo: (p, t) => ({ t }),
  emission: function (p, t) { const time = 0; return colour(p, t, time); },
});`, [
    'const density = (p) => f(p.x).add(time);',
    'albedo: (p) => ({ t: time })',
    'emission: function (p) { const time = 0; return colour(p, loftTime, time); }',
    'import { time as loftTime } from "loft";',
  ]);
});

test("atLeast migrates bare, instant and interval holds and ignores shadowed names", () => {
  port(`import { atLeast as hold, time } from "loft";
const x = [hold(a, b), hold(a, b, undefined), hold(a, b, { at: 2 }), hold(a, b, { from: 1, to })];
const f = (hold) => hold(a, b, { from: 1, to: 2 });
const text = "hold(a, b, { at: 3 })";`, [
    'import { time } from "loft";',
    '[ge(a, b), ge(a, b), always([time, 2, 2], ge(a, b)), always([time, 1, to], ge(a, b))]',
    'const f = (hold) => hold(a, b, { from: 1, to: 2 });',
    'const text = "hold(a, b, { at: 3 })";',
  ]);
});

test("equal keeps the tolerance and uses the shared window vocabulary", () => {
  port(`import * as L from "loft";
const END = 3;
const a = L.equal(x, y, { at: END, within: 0.1 });
const b = L.equal(x, y, { from: 0, to: 1 });
const c = L.equal(x, y, { within: 0.2 });
const d = L.equal(x, y, 0.3);`, [
    'always([time, END, END], L.equal(x, y, 0.1))',
    'always([time, 0, 1], L.equal(x, y))',
    'const c = L.equal(x, y, 0.2);',
    'const d = L.equal(x, y, 0.3);',
  ]);
});

test("staysClear keeps the range, margin and label in a clearance condition", () => {
  port(`import { staysClear, ge } from "loft";
const check = staysClear(body, target, { until: 3, margin: 0.2, label: "gap" });
const other = staysClear(body, target, { from: 1, until: 3, rate: 60 });`, [
    'always([time, 0, 3], ge(clearance(body, target), 0.2), { label: "gap" })',
    'always([time, 1, 3], ge(clearance(body, target), 0))',
  ]);
});

test("a standalone raw literal migrates to wgsl with its callable intact", () => {
  port(`import { wgslFn as old } from "loft";
const rate = old<"f32">({ name: "rate", ret: "f32", wgsl: "fn rate(x: f32) -> f32 { return fwidth(x); }" });
const edge = rate(p);`,
    'const rate = wgsl("fn rate(x: f32) -> f32 { return fwidth(x); }")["rate"]!;');
});

test("the four removed screen helpers become raw functions and keep local call signatures", () => {
  port(`import { fwidth as fw, eyePosition, pixelWorld } from "${frozen}lib/wgsl.ts";
import * as L from "loft";
const values = [fw(x), eyePosition(), pixelWorld(p), L.viewDir(p)];`, [
    'const fw = wgsl("fn pixel_rate(x: f32) -> f32 { return fwidth(x); }").pixel_rate!;',
    'const eyePosition = wgsl("fn eye_point() -> vec3f { return frame.eye.xyz; }").eye_point!;',
    'const pixelWorld = wgsl("fn pixel_size(p: vec3f) -> f32 { return pixel_world(p); }").pixel_size!;',
    'const viewDir = wgsl("fn toward_point(p: vec3f) -> vec3f { return view_dir(p); }").toward_point!;',
    '[fw(x), eyePosition(), pixelWorld(p), viewDir(p)]',
  ]);
});

test("cellular2 preserves a frozen vec2 while keeping component and current vec4 access", () => {
  port(`import { cellular2 as cells } from "${frozen}index.ts";
const a = cells(p);
const b = cells(p).y;`, ['const a = cells(p).xy;', 'const b = cells(p).y;']);
  const current = 'import { cellular2 } from "loft";\nconst p = cellular2(uv).zw;\n';
  expect(migrate(current)).toEqual({ code: current, changed: false, unported: [] });
});

test("an empty Drawable surface list becomes an empty forms list", () => {
  port(`import { plane, type Drawable } from "loft";
const backdrop: Drawable = { name: "backdrop", draws, skeletons: [], surfaces: [] };
const ground = plane.surfaces;`, [
    'draws, skeletons: [], forms: []',
    'const ground = plane.forms;',
  ]);
});

test("the seed wrapper preserves the frozen preset even when declared after the default export", () => {
  port(`import { strand } from "${frozen}index.ts";
const things = [strand(o)];
export default things;
export const renderStyle = "woodblock" as const;`, [
    'export default scene({ things: things,',
    'ground: sea({ swell: [] }), style: styleNamed("woodblock"), sun: [-0.45, 0.8, 0.5]',
    'cameras: { fixed: { kind: "perspective", eye: [4.5, 4.5, 8], target: [0, 0.7, 0], fovY: 0.62 } }',
  ]);
  port(`import { strand } from "loft";
export default [strand(o)];
export const renderStyle = "smooth";`,
    'style: toon({ bands: 32, outline: 0 })');
});

test("unsupported options, raw dependencies and geometric decisions keep their source and original line", () => {
  const source = `import { atLeast, stopsAt, staysClear, wgslFn, draw, flash, medium, equal } from "loft";
const bind = use(atLeast);
const a = atLeast(x, y, opts);
const b = stopsAt(strike, hull);
const c = staysClear(tail, hull, { until: arm, rate: 24 });
const d = wgslFn({ name, ret: "vec3", wgsl: dynamic, deps });
const e = draw.volume("dust", (p, t) => { t = f(0); return t; }, { bounds, steps: 16 });
const lights = [flash(opts)];
const fog = medium("fog", { flashes: lights });
const eq = equal(a, b, { ...window });
const shared = { bounds, steps: 16, emission: (p, t) => colour(p, t) };
const d1 = draw.volume("one", () => 1, shared);
const d2 = draw.volume("two", () => 1, shared);
`;
  expect(migrate(source)).toEqual({ code: source, changed: false, unported: [
    { line: 2, reason: "condition-options" },
    { line: 3, reason: "condition-options" },
    { line: 4, reason: "stops-at-contact-and-span" },
    { line: 5, reason: "clearance-window" },
    { line: 6, reason: "raw-wgsl" },
    { line: 7, reason: "slot-time" },
    { line: 8, reason: "scene-lights" },
    { line: 9, reason: "scene-lights" },
    { line: 10, reason: "condition-options" },
    { line: 12, reason: "slot-time" },
    { line: 13, reason: "slot-time" },
  ] });
});

test("a syntax refusal leaves the file byte-for-byte intact", () => {
  const code = `import { strand } from "${frozen}index.ts";
const broken = ;`;
  expect(migrate(code)).toEqual({ code, changed: false, unported: [{ line: 2, reason: "typescript-syntax" }] });
});

test("batch and single-file JSON return the same per-program result", async () => {
  const code = 'import { atLeast as hold } from "loft";\nconst a = hold(x, y);';
  const single = Bun.spawnSync(["bun", "tools/migrate.ts", "-", "--json"], { stdin: Buffer.from(code) });
  expect(single.exitCode).toBe(0);
  const expected = migrate(code);
  expect(JSON.parse(single.stdout.toString())).toEqual(expected);
  const batch = Bun.spawnSync(["bun", "tools/migrate.ts", "--batch", "-"], {
    stdin: Buffer.from([JSON.stringify({ id: "one", code }), JSON.stringify({ id: 2, code: "// untouched" })].join("\n")),
  });
  expect(batch.exitCode).toBe(0);
  expect(batch.stdout.toString().trimEnd().split("\n").map((row) => JSON.parse(row))).toEqual([
    { id: "one", ...expected }, { id: 2, code: "// untouched", changed: false, unported: [] },
  ]);
});

// The sweep sampler is inherited from a19a5ae:corpus/l5e/regenerate.ts.
// Run tools/regenerate_corpus.sh to stage its pinned legacy dependencies.
import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { migrate } from "./tools/migrate.ts";
import { readTablesAlone } from "./src/compile/index.ts";
import { spacing } from "./src/draw/surface.ts";
import { compileJs, evaluate, Expr, f, variable } from "./src/expr.ts";
import { Strand } from "./src/geom/strand.ts";
import type { Scene } from "./src/scene.ts";

type V3 = [number, number, number];
type Ring = [V3, V3, V3];
type Seed = { id: string; code: string };
type Strip = { around: number; exponent: number; paint: V3; rings: [Ring, Ring][] };
type Column = { name: string; lanes: 1 | 3; type: string; fallback?: number[] };
type Row = number[][];

const times = [0, 0.25, 0.5, 0.75, 0.9375, 1.37, 2.9];
const vector = (value: unknown): V3 => {
  if (!Array.isArray(value) || value.length !== 3 || !value.every(Number.isFinite)) {
    throw new Error(`expected finite vec3, got ${JSON.stringify(value)}`);
  }
  return value as V3;
};
const sub = (a: V3, b: V3): V3 => vector(a.map((v, i) => v - b[i]!));
const mean = (a: V3, b: V3): V3 => vector(a.map((v, i) => (v + b[i]!) * 0.5));
const number = (x: number) => Number(Math.fround(x).toPrecision(9)).toString();
const literal = (v: V3) => `[${v.map(number).join(", ")}]`;
const finite = (x: number) => {
  if (!Number.isFinite(x)) throw new Error(`nonfinite sample ${x}`);
  return x;
};
const axis = (x: number, square: number, exponent: number) => x * (square + 1e-12) ** exponent;

function capture(scene: Scene, orbit: boolean) {
  const camera = scene.cameras.fixed;
  if (camera?.kind !== "perspective") throw new Error("expected the R0 fixed perspective camera");
  const eye = vector(camera.eye);
  const target = vector(camera.target);
  const [x, height, z] = sub(eye, target);
  const radius = Math.hypot(x, z);
  // The original l5e port rotates the rings and sun into the t=0 orbit view.
  // Later ports use the pinned fixed eye and the unrotated samples.
  const orient = ([a, b, c]: V3): V3 => orbit
    ? [(z * a - x * c) / radius, b, (x * a + z * c) / radius]
    : [a, b, c];
  const strands = scene.things.filter((part): part is Strand => part instanceof Strand);
  const other = scene.things.filter(part => !strands.includes(part as Strand));
  if (!strands.length || other.some(part => !["sky", "ground", "sea", "shadows", "light"].includes(part.name))) {
    throw new Error(`unexpected non-strand drawables: ${other.map(part => part.name).join(", ")}`);
  }
  const u = variable("corpus_u", "f32");
  const v = variable("corpus_v", "f32");
  const strips: Strip[] = [];
  let vertices = 0;
  let maxError = 0;
  let maxPaintDelta = 0;
  for (const strand of strands) {
    const { along, around } = strand.look;
    const exponent = (2 / strand.section.squareness - 1) * 0.5;
    const coordinates = Array.from({ length: along + 1 }, (_, i) => Number(evaluate(spacing.ends(along).at(f(i / along)))));
    const at = times.map(t => {
      const body = strand.at(f(t));
      const tables = readTablesAlone([body.table]);
      const point = body.point(u, v);
      const swatch = strand.look.paint(u, v, { world: point, normal: body.normal(u, v), copy: f(0) });
      if (!(swatch instanceof Expr)) throw new Error(`${strand.name}: painted channels need a native material port`);
      const sample = compileJs([point, swatch]);
      const get = (j: number, a: number) => {
        const [p, paint] = sample({ vars: { corpus_u: j, corpus_v: a }, inputs: { time: t }, params: {}, tables });
        return { p: orient(vector(p)), paint: vector(paint) };
      };
      return coordinates.map(m => {
        const p0 = get(m, 0).p;
        const center = mean(p0, get(m, 0.5).p);
        const ring: Ring = [center, sub(p0, center), sub(get(m, 0.25).p, center)];
        const paint = get(m, 0.25).paint;
        for (let j = 0; j <= around; j++) {
          const a = 0.25 + j / around;
          const p = get(m, a);
          const angle = 2 * Math.PI * a;
          const square = 0.5 * Math.cos(2 * angle);
          const c = axis(Math.cos(angle), 0.5 + square, exponent);
          const d = axis(Math.sin(angle), 0.5 - square, exponent);
          const error = Math.hypot(...ring[0].map((x, k) => x + c * ring[1][k]! + d * ring[2][k]! - p.p[k]!));
          maxError = Math.max(maxError, finite(error));
          maxPaintDelta = Math.max(maxPaintDelta, Math.hypot(...sub(p.paint, paint)));
          vertices++;
        }
        return { ring, paint };
      });
    });
    for (let i = 0; i < along; i++) {
      const paint = mean(at[0]![i]!.paint, at[0]![i + 1]!.paint);
      const rings = at.map((a): [Ring, Ring] => [a[i]!.ring, a[i + 1]!.ring]);
      for (const a of at) {
        for (const s of [a[i]!, a[i + 1]!]) maxPaintDelta = Math.max(maxPaintDelta, Math.hypot(...sub(s.paint, paint)));
      }
      strips.push({ around, exponent, paint, rings });
    }
  }
  return { strips, vertices, maxError, maxPaintDelta, fov: camera.fovY,
    eye, at: orient(target), sun: orient(vector(scene.sun)), radius, height };
}

const ringColumns = ["center_a", "side_a", "up_a", "center_b", "side_b", "up_b"];
function isMoving(rings: [Ring, Ring][]) {
  const base = rings[0]!.flatMap(ring => ring.flat()).map(Math.fround);
  return rings.slice(1).some(pair => pair.flatMap(ring => ring.flat()).some((x, i) => Math.fround(x) !== base[i]));
}
const phase = (i: number) => `clamp((time - ${number(times[i - 1]!)}s) / ${number(times[i]! - times[i - 1]!)}s, 0, 1)`;

function modal(rows: Row[], column: number): number[] {
  const counts = new Map<string, { value: number[]; count: number }>();
  for (const row of rows) {
    const value = row[column]!;
    const key = value.join(",");
    const seen = counts.get(key);
    if (seen) seen.count++;
    else counts.set(key, { value, count: 1 });
  }
  return [...counts.values()].reduce((a, b) => a.count >= b.count ? a : b).value;
}

function sampledRow(strip: Strip, animated: boolean): Row {
  // The old decimal writer canonicalized negative zero and rounded to f32.
  const sample = (x: number) => {
    const value = finite(Math.fround(x));
    return value === 0 ? 0 : value;
  };
  const previous = strip.rings[0]!.flat();
  const row = [strip.paint, [strip.exponent], ...previous];
  if (animated) {
    for (const pair of strip.rings.slice(1)) {
      const now = pair.flat();
      for (let i = 0; i < previous.length; i++) {
        const delta = sub(now[i]!, previous[i]!);
        row.push(delta);
        previous[i] = now[i]!;
      }
    }
  }
  return row.map(values => values.map(sample));
}

function asset(columns: Column[], rows: Row[]): Buffer {
  const header = Buffer.alloc(14);
  header.write("LOFTTAB1");
  header.writeUInt32LE(rows.length, 8);
  header.writeUInt16LE(columns.length, 12);
  const schema = columns.map(col => {
    const name = Buffer.from(col.name);
    return Buffer.concat([Buffer.from([name.length]), name, Buffer.from([col.lanes])]);
  });
  const payload = rows.map(row => {
    const mask = Buffer.alloc(Math.ceil(columns.length / 8));
    const values: number[] = [];
    for (const [i, col] of columns.entries()) {
      const supplied = row[i]!;
      if (col.fallback && supplied.every((value, lane) => value === col.fallback![lane])) continue;
      mask[i >> 3]! |= 1 << (i & 7);
      values.push(...supplied);
    }
    const words = Buffer.alloc(values.length * 4);
    values.forEach((value, index) => words.writeFloatLE(value, index * 4));
    return Buffer.concat([mask, words]);
  });
  return Buffer.concat([header, ...schema, ...payload]);
}

const section = [
  "    let angle = TAU * (v + 0.25)",
  "    let square = 0.5 * cos(2 * angle)",
  "    let x = cos(angle) * pow(0.5 + square + 1e-12, expo)",
  "    let y = sin(angle) * pow(0.5 - square + 1e-12, expo)",
];

function surface(name: string, around: number, animated: boolean, columns: Column[], inline: boolean) {
  const declaration = (col: Column) => {
    const { name, lanes, fallback } = col;
    const init = fallback ? " = " + (lanes === 1 ? number(fallback[0]!) : literal(vector(fallback))) : "";
    return `  ${name}: ${col.type}${init},`;
  };
  const out = [`kind ${name} @World (`, ...columns.map(declaration), ") {",
    `  surface(u, v) grid 1x${around} {`,
    ...(inline ? section : [])];
  if (animated) {
    for (let i = 1; i < times.length; i++) out.push(`    let t${i} = ${phase(i)}`);
    for (const c of ringColumns) {
      out.push(`    let ${c}_now = ${c} + ${times.slice(1).map((_, j) => `${c}_d${j + 1} * t${j + 1}`).join(" + ")}`);
    }
  }
  const [a, s, d, b, q, e] = ringColumns.map(c => animated ? `${c}_now` : c);
  const point = inline
    ? [`    let left = ${a} + ${s} * x + ${d} * y`, `    let right = ${b} + ${q} * x + ${e} * y`,
      "    [left * (1 - u) + right * u]@World"]
    : [`    sweep_span(u, v, expo, ${a}, ${s}, ${d}, ${b}, ${q}, ${e})`];
  out.push(...point, "  }", "", "  material(p, n, uv) = ggx(paint, 0.55, 0)", "}");
  return out.join("\n");
}

function encode(id: string, file: string, scene: ReturnType<typeof capture>, orbit: boolean) {
  const stem = file.split("/").at(-1)!.replace(/\.loft$/, "");
  const tables = new Map<string, { around: number; animated: boolean; rows: Row[] }>();
  for (const strip of scene.strips) {
    const animated = isMoving(strip.rings);
    const name = `Sweep${strip.around}${animated ? "Motion" : ""}`;
    if (!tables.has(name)) tables.set(name, { around: strip.around, animated, rows: [] });
    tables.get(name)!.rows.push(sampledRow(strip, animated));
  }
  const assets = new Map<string, Buffer>();
  const cameras = orbit ? `radius: ${number(scene.radius)}m, height: ${number(scene.height)}m, period: 1e9s`
    : `eye: ${literal(scene.eye)}@World`;
  const prefix = [
    "use stdlib::geometry::analytic::{ray_ground}",
    "use stdlib::materials::bsdf::{ggx}",
    "use stdlib::sdf::solid::{box_sdf}", "",
    `# R0 seed ${id}`,
    "# Binary tables carry adjacent ring samples and linear keyframe deltas.",
    "space World", "",
    `camera Main { ${cameras}, at: ${literal(scene.at)}@World, fov: ${number(scene.fov)}rad }`, "",
  ];
  const helper = "use stdlib::geometry::sampled_sweep::{sweep_span}";
  const kinds = [];
  const inlineKinds = [];
  const build = ["kind Water @World (size: m > 0m) {",
    "  visible(ro, rd) = ray_ground(ro, rd, size)",
    "  trace(p) = box_sdf(p - [0, -0.05, 0]@World, [size, 0.05, size])",
    "  material(p, n) = ggx([0.055, 0.17, 0.19], 0.38, 0)",
    "}", "", "build {", `  emit Sun(dir: dir(${literal(scene.sun)}@World), color: [2.8, 2.6, 2.3])`,
    "  emit Sky(zenith: [0.35, 0.56, 0.88], horizon: [0.82, 0.86, 0.9])", "  emit Water(size: 100m)"];
  for (const [name, { around, animated, rows }] of tables) {
    const columns: Column[] = [
      { name: "paint", type: "rgb", lanes: 3, fallback: modal(rows, 0) },
      { name: "expo", type: "f", lanes: 1, fallback: modal(rows, 1) },
      ...ringColumns.map(name => ({ name, type: "v3", lanes: 3 as const })),
    ];
    if (animated) {
      for (let i = 1; i < times.length; i++) {
        columns.push(...ringColumns.map(c => ({ name: `${c}_d${i}`, type: "v3", lanes: 3 as const, fallback: [0, 0, 0] })));
      }
    }
    kinds.push(surface(name, around, animated, columns, false), "");
    inlineKinds.push(surface(name, around, animated, columns, true), "");
    const filename = `samples/${stem}/${name}.bin`;
    assets.set(filename, asset(columns, rows));
    build.push(`  build for i in 0..${rows.length} { import_table("${filename}", ${name}, i) }`);
  }
  build.push("}", "");
  return {
    source: [helper, ...prefix, ...kinds, ...build].join("\n"),
    inline: [...prefix, ...inlineKinds, ...build].join("\n"),
    assets,
  };
}

const [root, ...requested] = process.argv.slice(2);
if (!root) throw new Error("missing repository root");
const readJson = (path: string) => JSON.parse(readFileSync(join(root, path), "utf8"));
const { seeds } = readJson("corpus/sources.json");
const { batches } = readJson("corpus/manifest.json");
const creative = process.env[seeds.env] ?? resolve(seeds.default.replace(/^~/, process.env.HOME!));
const input = new Map<string, string>();
for (const pin of seeds.sets) {
  const bytes = execFileSync("git", ["-C", creative, "show", `${seeds.commit}:${pin.path}`], { maxBuffer: 4 * 1024 * 1024 });
  if (createHash("sha256").update(bytes).digest("hex") !== pin.sha256) {
    throw new Error(`${pin.set} differs from corpus/sources.json`);
  }
  for (const line of bytes.toString().trimEnd().split("\n")) {
    const seed: Seed = JSON.parse(line);
    input.set(`${pin.set}/${seed.id}`, seed.code);
  }
}
const evidence = process.env.LOFT_CORPUS_EVIDENCE;
if (evidence) mkdirSync(evidence, { recursive: true });
const selected = new Set(requested);
const unknown = new Set(selected);
let wrote = 0;
for (const { batch, lane, scenes } of batches) {
  for (const id of scenes) {
    if (!input.has(id) || (selected.size && !selected.has(id))) continue;
    unknown.delete(id);
    const stem = id.replaceAll("/", "__");
    const orbit = lane === "l5e";
    const file = join(root, "corpus", orbit ? "l5e" : `seeds/batch-${batch}`, `${stem}.loft`);
    // Read the target so a manifest/path mismatch never grows an unintended port.
    readFileSync(file);
    const migrated = migrate(input.get(id)!, "scene.ts", null);
    if (migrated.unported.length) throw new Error(`${id}: ${migrated.unported.join(", ")}`);
    const legacy = join(import.meta.dir, `${stem}.ts`);
    writeFileSync(legacy, migrated.code);
    const samples = capture((await import(legacy)).default, orbit);
    if (samples.maxError > 1e-5 || samples.maxPaintDelta !== 0) {
      throw new Error(`${id}: sampler error ${samples.maxError} m, paint delta ${samples.maxPaintDelta}`);
    }
    const { source, inline, assets } = encode(id, file, samples, orbit);
    for (const [name, bytes] of assets) {
      const path = join(dirname(file), name);
      mkdirSync(dirname(path), { recursive: true });
      writeFileSync(path, bytes);
    }
    writeFileSync(file, source);
    if (evidence) writeFileSync(join(evidence, `${stem}.loft`), inline);
    console.log(JSON.stringify({ id, rows: samples.strips.length, vertices: samples.vertices, maxError: samples.maxError }));
    wrote++;
  }
}
if (unknown.size || wrote !== (selected.size || input.size)) {
  throw new Error(`regenerated ${wrote} seeds, missing ${[...unknown].join(", ")}`);
}

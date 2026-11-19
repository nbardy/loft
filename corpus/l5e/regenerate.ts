import { createHash } from "node:crypto";
import { execFileSync } from "node:child_process";
import { mkdirSync, mkdtempSync, renameSync, writeFileSync } from "node:fs";
import { resolve } from "node:path";
import manifest from "../manifest.json";
import parity from "../../tools/parity/corpus.json";
import { migrate } from "../../tools/migrate.ts";
import { readTablesAlone } from "../../src/compile/index.ts";
import { spacing } from "../../src/draw/surface.ts";
import { compileJs, evaluate, Expr, f, variable } from "../../src/expr.ts";
import { Strand } from "../../src/geom/strand.ts";
import type { Scene } from "../../src/scene.ts";

type V3 = [number, number, number];
type Ring = [V3, V3, V3];
type Seed = { id: string; code: string };
type Strip = { part: string; around: number; exponent: number; paint: V3; rings: [Ring, Ring][] };

const root = resolve(import.meta.dir, "../..");
const scratch = resolve(root, "out/meta/i5-fix/seed-samples");
const input = new Map<string, string>();
for (const pinned of parity.seeds.sets) {
  const seeds = execFileSync("git", ["-C", process.env.LOFT_CREATIVE ?? resolve(process.env.HOME!, "git/creative"),
    "show", `${parity.seeds.commit}:${pinned.path}`], { maxBuffer: 4 * 1024 * 1024 });
  if (createHash("sha256").update(seeds).digest("hex") !== pinned.sha256) {
    throw new Error(`${pinned.set} differs from the corpus pin`);
  }
  for (const line of seeds.toString().trimEnd().split("\n")) {
    const row = JSON.parse(line) as Seed;
    input.set(`${pinned.set}/${row.id}`, row.code);
  }
}
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

function capture(scene: Scene) {
  const camera = scene.cameras.fixed;
  if (camera?.kind !== "perspective") throw new Error("expected the R0 fixed perspective camera");
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
        return { p: vector(p), paint: vector(paint) };
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
      strips.push({ part: strand.name, around, exponent, paint, rings });
    }
  }
  return { strips, vertices, maxError, maxPaintDelta, fov: camera.fovY,
    eye: vector(camera.eye), at: vector(camera.target), sun: vector(scene.sun) };
}

const columns = ["center_a", "side_a", "up_a", "center_b", "side_b", "up_b"];
function isMoving(rings: [Ring, Ring][]) {
  const base = rings[0]!.flatMap(ring => ring.flat()).map(Math.fround);
  return rings.slice(1).some(pair => pair.flatMap(ring => ring.flat()).some((x, i) => Math.fround(x) !== base[i]));
}
const phase = (i: number) => `clamp((time - ${number(times[i - 1]!)}s) / ${number(times[i]! - times[i - 1]!)}s, 0, 1)`;

function surface(around: number, animated: boolean) {
  const name = `Sweep${around}${animated ? "Motion" : ""}`;
  const fields = columns.map(c => `${c}: v3`);
  if (animated) {
    for (let i = 1; i < times.length; i++) fields.push(...columns.map(c => `${c}_d${i}: v3 = [0, 0, 0]`));
  }
  const out = [`kind ${name} @World (`, "  paint: rgb, expo: f,",
    ...fields.map(c => `  ${c},`), ") {", `  surface(u, v) grid 1x${around} {`,
    "    let angle = TAU * (v + 0.25)",
    "    # The double-angle identity gives interval derivation a nonnegative square.",
    "    let square = 0.5 * cos(2 * angle)",
    "    let x = cos(angle) * pow(0.5 + square + 1e-12, expo)",
    "    let y = sin(angle) * pow(0.5 - square + 1e-12, expo)"];
  if (animated) {
    for (let i = 1; i < times.length; i++) out.push(`    let t${i} = ${phase(i)}`);
    for (const c of columns) {
      out.push(`    let ${c}_now = ${c} + ${times.slice(1).map((_, j) => `${c}_d${j + 1} * t${j + 1}`).join(" + ")}`);
    }
  }
  const [a, s, d, b, q, e] = columns.map(c => animated ? `${c}_now` : c);
  out.push(`    let left = ${a} + ${s} * x + ${d} * y`, `    let right = ${b} + ${q} * x + ${e} * y`,
    "    [left * (1 - u) + right * u]@World", "  }", "", "  material(p, n, uv) = ggx(paint, 0.55, 0)", "}");
  return { name, source: out.join("\n") };
}

function encode(id: string, captured: ReturnType<typeof capture>) {
  const { strips, eye, at, fov, sun } = captured;
  const kinds = new Map<string, ReturnType<typeof surface>>();
  const samples: string[] = [];
  const counts = new Map<string, number>();
  let lastPart: string | undefined;
  const rows = strips.map(({ part, around, exponent, paint, rings }) => {
    const animated = isMoving(rings);
    const key = `${around}:${animated}`;
    let kind = kinds.get(key);
    if (!kind) {
      kind = surface(around, animated);
      kinds.set(key, kind);
    }
    const row = counts.get(kind.name) ?? 0;
    counts.set(kind.name, row + 1);
    // The pose-at-rest probe uses an interior coordinate away from poles and seams.
    const angle = 2 * Math.PI * (0.23125 + 0.25);
    const c = Math.cos(angle);
    const d = Math.sin(angle);
    const rest = (ring: Ring): V3 => vector(ring[0].map((x, k) =>
      x + c * (c * c + 1e-12) ** exponent * ring[1][k]! + d * (d * d + 1e-12) ** exponent * ring[2][k]!));
    const expected = mean(rest(rings[0]![0]), rest(rings[0]![1]));
    samples.push([id.replaceAll("/", "__"), kind.name, row, ...expected].join("\t"));
    const previous = rings[0]!.flat();
    const args = [`paint: ${literal(paint)}`, `expo: ${number(exponent)}`,
      ...columns.map((c, i) => `${c}: ${literal(previous[i]!)}`)];
    if (animated) {
      for (let i = 1; i < rings.length; i++) {
        const now = rings[i]!.flat();
        for (let c = 0; c < columns.length; c++) {
          const delta = sub(now[c]!, previous[c]!);
          if (delta.some(x => number(x) !== "0")) args.push(`${columns[c]}_d${i}: ${literal(delta)}`);
          previous[c] = now[c]!;
        }
      }
    }
    const heading = part === lastPart ? "" : `  # ${part}\n`;
    lastPart = part;
    return `${heading}  emit ${kind.name}(\n${args.map(arg => `    ${arg},`).join("\n")}\n  )`;
  });
  const source = [`# R0 seed ${id}`, "# Ring centers and section axes sampled from the pinned legacy CPU twin.",
    "# Each row spans adjacent rings, with linear keyframe deltas at the corpus sample times.", "space World", "",
    `camera Main { eye: ${literal(eye)}@World, at: ${literal(at)}@World, fov: ${number(fov)}rad }`,
    "",
    ...[...kinds.values()].flatMap(k => [k.source, ""]),
    "kind Water @World (size: m > 0m) {",
    "  visible(ro, rd) = ray_ground(ro, rd, size)",
    "  trace(p) = box_sdf(p - [0, -0.05, 0]@World, [size, 0.05, size])",
    "  material(p, n) = ggx([0.055, 0.17, 0.19], 0.38, 0)",
    "}", "", "build {", `  emit Sun(dir: dir(${literal(sun)}@World), color: [2.8, 2.6, 2.3])`,
    "  emit Sky(zenith: [0.35, 0.56, 0.88], horizon: [0.82, 0.86, 0.9])", "  emit Water(size: 100m)",
    ...rows, "}", ""].join("\n");
  return { source, samples };
}

mkdirSync(scratch, { recursive: true });
const results = [];
const samples: string[] = [];
const batches = new Set(process.argv.slice(2).map(Number));
if (!batches.size || [...batches].some(batch => !manifest.batches.some(b => b.batch === batch))) {
  throw new Error("supply the corpus batch numbers to regenerate");
}
const outputRoot = resolve(root, "corpus/seeds");
mkdirSync(outputRoot, { recursive: true });
for (const batch of manifest.batches.filter(b => batches.has(b.batch))) {
  const scenes = batch.scenes.filter(id => input.has(id));
  if (!scenes.length) continue;
  const dir = mkdtempSync(resolve(scratch, `batch-${batch.batch}-`));
  const start = results.length;
  for (const id of scenes) {
    const name = id.replaceAll("/", "__");
    try {
      const code = input.get(id);
      if (!code) throw new Error("seed is missing from the pinned input");
      const migrated = migrate(code, "scene.ts", null);
      if (migrated.unported.length) throw new Error(`legacy input migration has unported constructs: ${migrated.unported.join(", ")}`);
      const legacyFile = resolve(scratch, `${name}.ts`);
      writeFileSync(legacyFile, migrated.code);
      const scene = (await import(legacyFile)).default as Scene;
      const captured = capture(scene);
      const output = encode(id, captured);
      writeFileSync(resolve(dir, `${name}.loft`), output.source);
      samples.push(...output.samples);
      const result = { id, batch: batch.batch, rows: captured.strips.length, vertices: captured.vertices,
        maxError: captured.maxError, maxPaintDelta: captured.maxPaintDelta };
      results.push(result);
      console.log(JSON.stringify(result));
    } catch (e) {
      const result = { id, batch: batch.batch, error: String(e) };
      results.push(result);
      console.error(JSON.stringify(result));
    }
  }
  if (results.slice(start).every(r => !("error" in r) && r.maxError <= 1e-5 && r.maxPaintDelta === 0)) {
    renameSync(dir, resolve(outputRoot, `batch-${batch.batch}`));
  }
}
writeFileSync(resolve(scratch, "cpu-geometry.json"), JSON.stringify(results, null, 2) + "\n");
writeFileSync(resolve(scratch, "cpu-probes.tsv"), samples.join("\n") + "\n");
if (results.some(r => "error" in r || r.maxError > 1e-5 || r.maxPaintDelta !== 0)) process.exitCode = 1;

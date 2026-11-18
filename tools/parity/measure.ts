// Scores plan 2026-11-19 §8.1 (lane 4.2): the cold, GPU-less build of Owl, Titan and heron, three alternating runs.
//
//   bun tools/parity/measure.ts [--runs 3]         every run in a fresh process at nice 15; a table and out/parity/measure/<commit>.json
//   bun tools/parity/measure.ts child <scene.ts>   (child) one build, its phases, one JSON line
//
// Per run: CPU and peak RSS of the whole process (getrusage), CPU per phase (`decode` is the wire decode alone, the
// first part of the model's decode+intern+inline row), and the counts the model is built on
// (U, recorded R, the live job). The counts are checked before any time: a count off the model is investigated, never
// absorbed by retuning a rate (§8.1). Phases the Rust program driver will own are not wired yet; for those the
// report prices the model's rate at the measured counts and says so, and the gate is `scored: false` until every
// phase is measured. Each run records {load1, cpus} at its start and end; one taken over load1 > cpus/2 is marked.
import { mkdirSync, writeFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { join, resolve } from "node:path";

const ROOT = join(import.meta.dir, "../..");
/** §8.1, verbatim: U (MEASURED created nodes), model R, the projection and the owner's gates (ms CPU, MiB). */
const MODEL = {
  owl: { path: "examples/second/owl.ts", U: 16_400, R: 49_000, projected: 72, gate: 80, rss: 101 },
  titan: { path: "examples/bosses/reefback_titan.ts", U: 182_900, R: 550_000, projected: 257, gate: 300, rss: 187 },
  heron: { path: "examples/second/heron_leap.ts", U: 39_800, R: 119_000, projected: 189, gate: 200, rss: 136 },
} as const;
type Name = keyof typeof MODEL;
/** §8.1's rows the Rust driver owns, priced per count (ns): decode+intern+inline, rewrite..rate, emit, sharing, rest. */
const RUST = (U: number, R: number) => ({
  decode: (35 * R + 25 * U) / 1e6, rewrite: (8 * 24 * U) / 1e6, emit: (2 * 55 * U) / 1e6, share: 2 + (10 * U) / 1e6,
});
const FIXED = { rest: { owl: 4.7, titan: 22, heron: 7.1 }, bundle: { owl: 4.5, titan: 14, heron: 4.8 }, solve: { owl: 0, titan: 0, heron: 87 } };

type Phases = { start: number; link: number; design: number; tsCompile: number; job: number; crossing: number; decode: number };
type Child = { scene: string; outcome: string; U: number; R: number; live: number; phases: Phases };

async function child(path: string): Promise<void> {
  const cpu = () => { const u = process.cpuUsage(); return (u.user + u.system) / 1e3; };
  const [{ subjectOf }, { choose }, { compile }, { rec }, { pack, Program }, native] = await Promise.all([
    import("../../src/design.ts"), import("../../src/cli/variant.ts"), import("../../src/compile/index.ts"),
    import("../../src/record/recorder.ts"), import("../../src/record/wire.ts"), import("../../src/core/native.ts")]);
  const t0 = cpu();
  const mod = await import(resolve(path)) as Record<string, unknown>;
  const t1 = cpu(), stop = rec.watch();
  let outcome = "built", t2 = t1;
  try {
    const s = choose(subjectOf(mod, path), path, { kind: "default" }).scene;
    t2 = cpu();
    compile(s);
  } catch (e) { outcome = `refused: ${e instanceof Error ? e.constructor.name : "throw"}`; }
  const t3 = cpu(), bytes = pack(rec.job(Program.Scene, stop())), t4 = cpu();
  const free = native.UNLIMITED, r = native.native().run(bytes, { nodes: free, tapeOps: free, boxes: free, gpuNanos: free });
  const t5 = cpu();
  const phases = { start: t0, link: t1 - t0, design: t2 - t1, tsCompile: t3 - t2, job: t4 - t3, crossing: t5 - t4 - r.decode_ns / 1e6, decode: r.decode_ns / 1e6 };
  console.log(JSON.stringify({ scene: path, outcome, U: rec.size, R: rec.made, live: r.nodes, phases } satisfies Child));
}

type Run = Child & { name: Name; cpuMs: number; rssMiB: number; load: { start: number; end: number }; loaded: boolean };

async function run(name: Name): Promise<Run> {
  const start = loadavg()[0]!;
  const p = Bun.spawn(["nice", "-n", "15", process.execPath, join(import.meta.dir, "measure.ts"), "child", MODEL[name].path], { cwd: ROOT, stdout: "pipe", stderr: "pipe" });
  const [out, err, exit] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  if (exit !== 0) throw new Error(`measure child ${name} exited ${exit}: ${err.trim().split("\n").slice(-3).join(" | ")}`);
  const use = p.resourceUsage()!, end = loadavg()[0]!;
  // getrusage's ru_maxrss is bytes on macOS and KiB on Linux; Bun passes it through.
  const rssMiB = use.maxRSS / (process.platform === "darwin" ? 2 ** 20 : 2 ** 10);
  return { ...JSON.parse(out.trim().split("\n").at(-1)!) as Child, name, cpuMs: Number(use.cpuTime.total) / 1e3, rssMiB, load: { start, end }, loaded: Math.max(start, end) > cpus().length / 2 };
}

const median = (xs: number[]) => [...xs].sort((a, b) => a - b)[xs.length >> 1]!;
const f = (x: number) => x.toFixed(1).padStart(8);

async function score(runs: number): Promise<void> {
  const names = Object.keys(MODEL) as Name[], all: Run[] = [];
  for (let k = 0; k < runs; k++) for (const name of names) all.push(await run(name)); // alternating: owl, titan, heron, owl, ...
  const report = names.map((name) => {
    const m = MODEL[name], rs = all.filter((r) => r.name === name), at = <T,>(g: (r: Run) => T) => rs.map(g);
    const counts = { U: rs[0]!.U, R: rs[0]!.R, live: rs[0]!.live, stable: rs.every((r) => r.U === rs[0]!.U && r.R === rs[0]!.R && r.live === rs[0]!.live) };
    const miss = { U: counts.U / m.U, R: counts.R / m.R };
    const ph = (k: keyof Phases) => median(at((r) => r.phases[k]));
    const rust = RUST(counts.U, counts.R);
    // Measured: Bun start plus library, link, the design build (callbacks + recorder), the job write and the crossing.
    const measured = ph("start") + ph("link") + ph("design") + ph("job") + ph("crossing");
    const priced = rust.decode + rust.rewrite + rust.emit + rust.share + FIXED.rest[name] + FIXED.bundle[name] + FIXED.solve[name];
    return {
      name, scene: m.path, outcomes: [...new Set(at((r) => r.outcome))], counts, countMiss: { ...miss, flagged: miss.U > 1.1 || miss.R > 1 },
      phasesMs: Object.fromEntries((["start", "link", "design", "tsCompile", "job", "crossing", "decode"] as const).map((k) => [k, +ph(k).toFixed(2)])),
      rustPricedMs: { ...Object.fromEntries(Object.entries(rust).map(([k, v]) => [k, +v.toFixed(2)])), rest: FIXED.rest[name], bundle: FIXED.bundle[name], solve: FIXED.solve[name] },
      projectedMs: +(measured + priced).toFixed(1), modelMs: m.projected, gateMs: m.gate,
      todayTs: { cpuMs: at((r) => +r.cpuMs.toFixed(1)), rssMiB: at((r) => +r.rssMiB.toFixed(1)) }, rssGateMiB: m.rss,
      scored: false, why: "the Rust program driver (integrator) is not wired: intern..bundle are priced, and CPU and RSS are today's TS build",
      loads: at((r) => r.load), loaded: rs.some((r) => r.loaded),
    };
  });
  const head = Bun.spawnSync(["git", "rev-parse", "--short", "HEAD"], { cwd: ROOT }).stdout.toString().trim();
  const dirty = Bun.spawnSync(["git", "status", "--porcelain", "--", "src", "rust", "tools/parity"], { cwd: ROOT }).stdout.toString().trim() !== "";
  const file = join(ROOT, "out/parity/measure", `${head}${dirty ? "-dirty" : ""}.json`);
  mkdirSync(join(ROOT, "out/parity/measure"), { recursive: true });
  writeFileSync(file, JSON.stringify({ head, dirty, cpus: cpus().length, runs, at: new Date().toISOString(), report, all }, null, 1));
  console.log("scene        U/model  R/model   live    start    link  design  ts-comp     job crossing  decode | projected model gate | today cpu, MiB (gate)");
  for (const r of report) {
    const p = r.phasesMs as Record<string, number>;
    console.log(`${r.name.padEnd(10)} ${r.countMiss.U.toFixed(2).padStart(8)} ${r.countMiss.R.toFixed(2).padStart(8)} ${String(r.counts.live).padStart(7)}${["start", "link", "design", "tsCompile", "job", "crossing", "decode"].map((k) => f(p[k]!)).join("")} | ${f(r.projectedMs)} ${r.modelMs} ${r.gateMs} | ${median(r.todayTs.cpuMs).toFixed(0)} ms, ${median(r.todayTs.rssMiB).toFixed(0)} (${r.rssGateMiB})${r.countMiss.flagged ? "  COUNT MISS" : ""}${r.counts.stable ? "" : "  COUNTS MOVED"}${r.loaded ? "  loaded" : ""}`);
  }
  console.log(file);
}

const [cmd, arg] = Bun.argv.slice(2);
switch (cmd) {
  case "child": await child(arg!); break;
  case "--runs": await score(Number(arg)); break;
  case undefined: await score(3); break;
  default: console.error(`measure: unknown ${cmd}: [--runs N] | child <scene.ts>`); process.exit(2);
}

// Scores plan 2026-11-19 §8.1 (lane 4.2): the cold, GPU-less build of Owl, Titan and heron, three alternating runs.
//
//   bun tools/parity/measure.ts [--runs 3]         every run in a fresh process at nice 15; a table and out/parity/measure/<commit>.json
//   bun tools/parity/measure.ts child <scene.ts>   (child) one build, its phases, one JSON line
//
// Per run: CPU and peak RSS of the whole process (getrusage), CPU per phase, and the counts the model is built on
// (U, recorded R, the live job). The counts are checked before any time: a count off the model is investigated, never
// absorbed by retuning a rate (§8.1). The Rust phases are the program driver's own (loft-core `Phases`: decode, plan,
// emit, f16, bundle), measured on the job; `bundle` includes the hash, and the file write is in `crossing`.
// One of §8.1's rows has no Rust work behind it yet, so it stays priced at the model's figure and the report says so:
// `rest` (table fills: the wire carries no Kernel body). Solve is not priced: heron's `design` phase runs today's TS
// solve at import (the model's Rust solve is 87 ms), so it is measured there, not added again.
// `tsCompile` is the TS compiler the recorder still watches for the roots: the cutover removes it, so it is outside
// the projection, and `todayTs` (the whole process) includes it. Each run records {load1, cpus} at its start and end;
// one taken over load1 > cpus/2 is marked.
import { mkdirSync, writeFileSync } from "node:fs";
import { cpus, loadavg } from "node:os";
import { basename, dirname, join, resolve } from "node:path";
import type { Report } from "../../src/core/native.ts";

const ROOT = join(import.meta.dir, "../..");
/** §8.1, verbatim: U (MEASURED created nodes), model R, the projection and the owner's gates (ms CPU, MiB). */
const MODEL = {
  owl: { path: "examples/second/owl.ts", U: 16_400, R: 49_000, projected: 72, gate: 80, rss: 101 },
  titan: { path: "examples/bosses/reefback_titan.ts", U: 182_900, R: 550_000, projected: 257, gate: 300, rss: 187 },
  heron: { path: "examples/second/heron_leap.ts", U: 39_800, R: 119_000, projected: 189, gate: 200, rss: 136 },
} as const;
type Name = keyof typeof MODEL;
/** §8.1's model per row at the measured counts (ms), beside the measured Rust phases it predicts. */
const MODEL_RUST = (U: number, R: number) => ({
  decode: (35 * R + 25 * U) / 1e6, plan: (8 * 24 * U) / 1e6, emit: (2 * 55 * U) / 1e6 + 2 + (10 * U) / 1e6,
});
/** The row with no Rust work behind it yet: priced at §8.1's figure. */
const PRICED = { rest: { owl: 4.7, titan: 22, heron: 7.1 } };

const PHASES = ["start", "link", "design", "tsCompile", "job", "crossing", "decode", "plan", "emit", "f16", "bundle"] as const;
type Phases = Record<(typeof PHASES)[number], number>;
/** What the driver made of the job: a bundle, or its typed refusal (then only the Rust wall time is known, in `crossing`). */
type Rust = { kind: "built"; live: number; roots: number; draws: number; build: string } | { kind: "refused"; verdict: string; message: string };
type Child = { scene: string; outcome: string; U: number; R: number; rust: Rust; phases: Phases };

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
  const free = native.UNLIMITED, out = join(ROOT, "out/parity/measure", `${basename(path, ".ts")}.loft`);
  mkdirSync(dirname(out), { recursive: true });
  let r: Report | InstanceType<typeof native.NativeError>;
  try { r = native.native().run(bytes, { nodes: free, tapeOps: free, boxes: free, gpuNanos: free }, out); } catch (e) {
    if (!(e instanceof native.NativeError)) throw e;
    r = e;
  }
  const t5 = cpu(), base = { start: t0, link: t1 - t0, design: t2 - t1, tsCompile: t3 - t2, job: t4 - t3 };
  const [rust, phases]: [Rust, Phases] = r instanceof native.NativeError
    ? [{ kind: "refused", verdict: r.kind, message: r.message }, { ...base, crossing: t5 - t4, decode: NaN, plan: NaN, emit: NaN, f16: NaN, bundle: NaN }]
    : [{ kind: "built", live: r.nodes, roots: r.scene!.roots, draws: r.scene!.draws, build: r.scene!.build },
       { ...base, crossing: t5 - t4 - (r.phases.decode + r.phases.plan + r.phases.emit + r.phases.f16 + r.phases.bundle) / 1e6, decode: r.phases.decode / 1e6, plan: r.phases.plan / 1e6, emit: r.phases.emit / 1e6, f16: r.phases.f16 / 1e6, bundle: r.phases.bundle / 1e6 }];
  console.log(JSON.stringify({ scene: path, outcome, U: rec.size, R: rec.made, rust, phases } satisfies Child));
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
    const counts = { U: rs[0]!.U, R: rs[0]!.R, stable: rs.every((r) => r.U === rs[0]!.U && r.R === rs[0]!.R) };
    const built = rs.flatMap((r) => r.rust.kind === "built" ? [r.rust] : []), refused = rs.flatMap((r) => r.rust.kind === "refused" ? [r.rust] : []);
    const miss = { U: counts.U / m.U, R: counts.R / m.R };
    // A refused run's Rust phases are NaN in the child and null after JSON.
    const ph = (k: keyof Phases) => median(at((r) => r.phases[k] ?? NaN));
    // Measured: Bun start plus library, link, the design build (callbacks + recorder), the job write, the crossing
    // (with the file write) and every Rust phase. Not the TS compile (see the head of this file).
    // NaN (a refused run's Rust phases) counts as 0 in the sum, which is then a lower bound, not a projection.
    const measured = PHASES.filter((k) => k !== "tsCompile").reduce((n, k) => n + (Number.isNaN(ph(k)) ? 0 : ph(k)), 0);
    const priced = PRICED.rest[name];
    // A run the driver refused has no Rust phases: the scene is not scored, and says why.
    const scored = refused.length === 0;
    return {
      name, scene: m.path, outcomes: [...new Set(at((r) => r.outcome))], counts, countMiss: { ...miss, flagged: miss.U > 1.1 || miss.R > 1 },
      rust: scored ? { kind: "built", live: built[0]!.live, roots: built[0]!.roots, draws: built[0]!.draws, build: built[0]!.build, sameBuild: built.every((b) => b.build === built[0]!.build) } : { kind: "refused", refusals: [...new Set(refused.map((x) => `${x.verdict}: ${x.message}`))] },
      phasesMs: Object.fromEntries(PHASES.map((k) => [k, +ph(k).toFixed(2)])),
      rustModelMs: Object.fromEntries(Object.entries(MODEL_RUST(counts.U, counts.R)).map(([k, v]) => [k, +v.toFixed(2)])),
      pricedMs: { rest: PRICED.rest[name] },
      projectedMs: scored ? +(measured + priced).toFixed(1) : null, lowerBoundMs: scored ? null : +(measured + priced).toFixed(1),
      modelMs: m.projected, gateMs: m.gate, pass: scored && measured + priced <= m.gate, failsAnyway: !scored && measured + priced > m.gate,
      todayTs: { cpuMs: at((r) => +r.cpuMs.toFixed(1)), rssMiB: at((r) => +r.rssMiB.toFixed(1)) }, rssGateMiB: m.rss,
      scored, why: scored ? "Rust phases measured; rest priced (no Kernel bodies on the wire); heron's design holds today's TS solve; the TS compile still runs to find the roots and is outside the projection but inside todayTs" : "the driver refused the job (see rust.refusals): no Rust phases to score",
      loads: at((r) => r.load), loaded: rs.some((r) => r.loaded),
    };
  });
  const head = Bun.spawnSync(["git", "rev-parse", "--short", "HEAD"], { cwd: ROOT }).stdout.toString().trim();
  const dirty = Bun.spawnSync(["git", "status", "--porcelain", "--", "src", "rust", "tools/parity"], { cwd: ROOT }).stdout.toString().trim() !== "";
  const file = join(ROOT, "out/parity/measure", `${head}${dirty ? "-dirty" : ""}.json`);
  mkdirSync(join(ROOT, "out/parity/measure"), { recursive: true });
  writeFileSync(file, JSON.stringify({ head, dirty, cpus: cpus().length, runs, at: new Date().toISOString(), report, all }, null, 1));
  console.log(`scene        U/model  R/model    live${PHASES.map((k) => k.slice(0, 8).padStart(8)).join("")} | projected model gate | today cpu, MiB (gate)`);
  for (const r of report) {
    const p = r.phasesMs as Record<string, number>;
    console.log(`${r.name.padEnd(10)} ${r.countMiss.U.toFixed(2).padStart(8)} ${r.countMiss.R.toFixed(2).padStart(8)} ${String(r.rust.kind === "built" ? r.rust.live : "refused").padStart(8)}${PHASES.map((k) => f(p[k]!)).join("")} | ${r.projectedMs === null ? `≥${f(r.lowerBoundMs!).trim()}`.padStart(8) : f(r.projectedMs)} ${r.modelMs} ${r.gateMs} ${r.scored ? (r.pass ? "pass" : "FAIL") : r.failsAnyway ? "FAIL (lower bound)" : "UNSCORED"} | ${median(r.todayTs.cpuMs).toFixed(0)} ms, ${median(r.todayTs.rssMiB).toFixed(0)} (${r.rssGateMiB})${r.countMiss.flagged ? "  COUNT MISS" : ""}${r.counts.stable ? "" : "  COUNTS MOVED"}${r.rust.kind === "built" && !r.rust.sameBuild ? "  BUILD MOVED" : ""}${r.loaded ? "  loaded" : ""}`);
  }
  console.log(file);
}

const [cmd, arg] = Bun.argv.slice(2);
switch (cmd) {
  case "child": await child(arg!); break;
  case "--runs": {
    const n = Number(arg);
    if (!Number.isInteger(n) || n < 1) { console.error(`measure: --runs takes a whole count of at least 1, got ${arg}`); process.exit(2); }
    await score(n); break;
  }
  case undefined: await score(3); break;
  default: console.error(`measure: unknown ${cmd}: [--runs N] | child <scene.ts>`); process.exit(2);
}

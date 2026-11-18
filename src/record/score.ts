// Scores lane 1.2 on scenes (docs/proposals/2026-11-19_lean_rust_loft.md section 8.2):
//
//   bun src/record/score.ts SCENE [--perturb SEED] [--dump F.job] [--print F.txt]   one process: R, the job's hash,
//                                                                  print equality; the job's bytes and the TS print
//   bun src/record/score.ts --compare SCENE...                       per scene: source, perturbed and bytecode runs
//
// One run builds the scene on the TS engine with the recorder watching `emitWgsl`'s roots, writes the job over those
// roots, and compares the job's canonical print (calls inlined) with the TS graph's. `--perturb` first imports every
// library module in a seeded shuffled order and builds discarded expressions, so every id, loop number and table
// number differs; `--compare` also runs a `bun build --bytecode` bundle of the same scene. The job's hash must not move.

import { createHash } from "node:crypto";
import { mkdirSync, readdirSync, rmSync, statSync, writeFileSync } from "node:fs";
import { basename, join, relative, resolve } from "node:path";
import * as X from "../expr.ts";
import { rec } from "./recorder.ts";
import { printGraph, printJob } from "./print.ts";
import { Op, pack, Program } from "./wire.ts";

const SRC = resolve(import.meta.dir, "..");

/** Score the scene module `mod` (already imported) in this process; prints one JSON line. */
export async function scoreModule(mod: Record<string, unknown>, path: string, dump?: string, print?: string): Promise<void> {
  const [{ subjectOf }, { choose }, { compile }] = await Promise.all([import("../design.ts"), import("../cli/variant.ts"), import("../compile/index.ts")]);
  const at = () => ({ R: rec.made, U: rec.size, rewrite: rec.rewritten });
  const loaded = at(), t0 = performance.now(), stop = rec.watch();
  let outcome = "built", scene = loaded;
  try {
    const s = choose(subjectOf(mod, path), path, { kind: "default" }).scene;
    scene = at();
    compile(s);
  } catch (e) { outcome = `refused: ${e instanceof Error ? e.constructor.name : "throw"}`; }
  const roots = stop(), total = at(), built = performance.now() - t0;
  const job = rec.job(Program.Scene, roots), bytes = pack(job);
  if (dump !== undefined) writeFileSync(dump, bytes);
  const t1 = performance.now(), ts = printGraph(roots), wire = printJob(job), printMs = performance.now() - t1;
  if (print !== undefined) writeFileSync(print, ts);
  const calls = Array.from({ length: job.nodes.length >> 2 }, (_, k) => job.nodes[4 * k]! & 0xff).filter((op) => op === Op.Call).length;
  console.log(JSON.stringify({
    scene: path, outcome, loaded, designed: scene, total, roots: roots.length, live: job.nodes.length >> 2, calls, bytes: bytes.length,
    hash: createHash("sha256").update(bytes).digest("hex").slice(0, 16), printEqual: ts === wire, printLines: ts.split("\n").length,
    firstDiff: ts === wire ? undefined : firstDiff(ts, wire), buildMs: Math.round(built), printMs: Math.round(printMs),
  }));
}

const firstDiff = (a: string, b: string): string => {
  const x = a.split("\n"), y = b.split("\n"), k = x.findIndex((l, i) => l !== y[i]);
  return `line ${k}: ts «${x[k]}» job «${y[k]}»`;
};

/** Import every library module in an order shuffled by `seed`, and build expressions nothing reads. */
async function perturb(seed: number): Promise<void> {
  const files: string[] = [];
  const walk = (d: string): void => { for (const f of readdirSync(d)) { const p = join(d, f); if (statSync(p).isDirectory()) { if (f !== "cli" && f !== "record") walk(p); } else if (f.endsWith(".ts") && !f.endsWith(".test.ts")) files.push(p); } };
  walk(SRC);
  let s = seed >>> 0;
  const rand = (n: number): number => { s = (Math.imul(s, 1664525) + 1013904223) >>> 0; return s % n; };
  for (let i = files.length - 1; i > 0; i--) { const j = rand(i + 1); [files[i], files[j]] = [files[j]!, files[i]!]; }
  for (const f of files) await import(f);
  const p = X.param(`discard${seed}`, 0.5);
  X.iterate(3 + rand(5), [X.sin(p)], ([a], i) => [X.add(X.mul(a!, p), i)]);
  for (let k = 0; k < 50 + rand(200); k++) X.mix(X.vec3(k, p, 1), [0.25, rand(7), 3], X.fract(X.mul(p, k)));
}

async function one(argv: readonly string[]): Promise<void> {
  const path = argv[0]!, at = (flag: string): string | undefined => { const k = argv.indexOf(flag); return k < 0 ? undefined : argv[k + 1]; };
  const seed = at("--perturb");
  if (seed !== undefined) await perturb(Number(seed));
  await scoreModule(await import(resolve(path)) as Record<string, unknown>, path, at("--dump"), at("--print"));
}

/** Source, perturbed and bytecode runs of each scene, in fresh processes; one JSON line per run, then whether the hashes agree. */
async function compare(scenes: readonly string[]): Promise<void> {
  const run = async (cmd: string[]): Promise<Record<string, unknown>> => {
    const p = Bun.spawn(cmd, { stdout: "pipe", stderr: "pipe" });
    const [out, err] = [await new Response(p.stdout).text(), await new Response(p.stderr).text()];
    await p.exited;
    const line = out.trim().split("\n").at(-1) ?? "";
    return line.startsWith("{") ? JSON.parse(line) : { error: err.trim().split("\n").slice(-3).join(" | ") };
  };
  for (const scene of scenes) {
    const dir = resolve("out/record", basename(scene, ".ts"));
    mkdirSync(dir, { recursive: true });
    const entry = join(dir, "entry.ts");
    writeFileSync(entry, `import * as mod from ${JSON.stringify(resolve(scene))};\nimport { scoreModule } from ${JSON.stringify(join(SRC, "record/score.ts"))};\nawait scoreModule(mod, ${JSON.stringify(scene)});\n`);
    // Bun gives ESM bytecode only to a compiled executable (1.3.10), so the bundle is one; it is deleted after its run.
    const exe = join(dir, "bytecode"), build = Bun.spawnSync(["bun", "build", "--compile", "--bytecode", "--format=esm", "--outfile", exe, entry], { stdout: "pipe", stderr: "pipe" });
    const runs = {
      source: await run(["bun", join(SRC, "record/score.ts"), scene]),
      perturbed: await run(["bun", join(SRC, "record/score.ts"), scene, "--perturb", "7"]),
      bytecode: build.exitCode === 0 ? await run([exe]) : { error: `bun build: ${build.stderr.toString().trim().slice(0, 300)}` },
    };
    rmSync(exe, { force: true });
    for (const [k, r] of Object.entries(runs)) console.log(JSON.stringify({ run: k, ...r }));
    const hashes = new Set(Object.values(runs).map((r) => r.hash));
    console.log(JSON.stringify({ scene: relative(process.cwd(), scene), sameHash: hashes.size === 1 && !hashes.has(undefined), printEqual: Object.values(runs).every((r) => r.printEqual === true) }));
  }
}

// In the bytecode bundle this module is not the entry, but `import.meta.main` is true there: the argv says which.
if (import.meta.main && basename(Bun.argv[1] ?? "") === "score.ts") await (Bun.argv[2] === "--compare" ? compare(Bun.argv.slice(3)) : one(Bun.argv.slice(2)));

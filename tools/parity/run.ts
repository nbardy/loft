// The picture-parity gate (docs/proposals/2026-11-19_lean_rust_loft.md §2.2, §2.3).
//
//   bun tools/parity/run.ts rust [--only text]        every entry through `LOFT_ENGINE=rust loft build` (loft-core's
//                                                     program driver), each ending against R0's build, and every
//                                                     bundle with draws drawn by loft-draw at R0's frames and compared
//                                                     under the §2.2a picture rule: out/parity/rust
//   bun tools/parity/run.ts checks <scene.ts>         (child) one scene's check verdicts as JSON on stdout
//   bun tools/parity/run.ts dawn <scene.ts> <id>      (child) a scene `loft build` refuses, drawn as `loft render` draws it
//   bun tools/parity/run.ts cross <R0 dir>            this set's frames against another backend's R0 (§2.3 cross rule)
//
// R0 (out/parity/R0) is recorded data: lane 0.2's `reference` and `repeat` modes drew TS artefact directories with
// loft-draw on loft-loader, and loft-draw now draws Bundle V1 files on loft-gpu (lean Rust draw note, 2026-11-19). To
// draw a reference set again, check out the commit that last had them (7f48109) and run `reference` there.
//
// LOFT_PARITY_OUT names the set (default out/parity/R0): the Linux runner (tools/parity/linux.sh) draws its own set on
// Vulkan and crosses it with the macOS R0.
// The corpus is tools/parity/corpus.json: every examples/**/*.ts and the creative seeds, migrated by tools/migrate.ts.
// Each entry is built by the real CLI (`loft build`); a refusal is recorded with its exit code, kind, site and text.
// Built entries are checked in a child (every Verdict object whole) and drawn by `loft-draw still` (wgpu 30, Metal) as
// lossless PNG. A scene the build refuses (a CPU table a file cannot hold: Storm) is drawn by `loft render`'s path on
// Dawn when it can be, and its frames are marked `dawn`: they are compared under the cross-executor rule. R0 is the commit of the tree, which must be clean where it decides pictures.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { homedir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = join(import.meta.dir, "../..");
const OUT = join(ROOT, process.env.LOFT_PARITY_OUT ?? "out/parity/R0");
const VIEW = join(ROOT, "rust/target/lab/loft-draw");
const DIFF = join(ROOT, "tools/parity/target/release/parity-diff");
/** The paths that decide a reference picture (the loader moved to rust/crates in lane 0.1; the workspace lock pins its wgpu). */
const corpus = JSON.parse(readFileSync(join(import.meta.dir, "corpus.json"), "utf8")) as Corpus;

type Corpus = {
  examples: string;
  seeds: { default: string; env: string; commit: string; sets: { set: string; path: string; rows: number; sha256: string }[] };
  frames: { cameras: number; times: number[]; size: [number, number]; stepsTime: number; detail: { time: number; size: [number, number] } };
};
type Mode = { kind: "rust"; only: string } | { kind: "checks" | "cross"; path: string } | { kind: "dawn"; path: string; id: string };
type Origin = { kind: "example" } | { kind: "seed"; set: string; seed: string; unported: number };
type Entry = { id: string; path: string; origin: Origin };
/** How `loft build` ended: the CLI's verdict kinds (src/cli/verdict.ts, docs/errors.md), or a process that died outside them. */
type Built =
  | { kind: "built"; steps: boolean; cameras: string[] }
  | { kind: "refused"; verdict: Kind; exit: number; head: string; site: string | null; detail: string[] }
  | { kind: "crashed"; exit: number; stderr: string };
type Kind = (typeof KIND)[keyof typeof KIND];
type Checks = { kind: "verdicts"; label: string; verdicts: unknown[] } | { kind: "absent" } | { kind: "refused"; verdict: unknown } | { kind: "crashed"; exit: number; stderr: string };
type Shot = { camera: string; time: number; size: [number, number] };
type Executor = "loft-draw" | "dawn";
type Frame = Shot & ({ kind: "drawn"; executor: Executor; file: string; sha256: string } | { kind: "refused"; executor: Executor; exit: number; stderr: string });
/** What `loft render` made of a refused build: frames, or its own refusal (the build's, usually). */
type Dawn = { kind: "drawn"; frames: Frame[] } | { kind: "refused"; exit: number; stderr: string };
type Rec = { entry: Entry; build: Built; checks: Checks | null; frames: Frame[]; dawn: Dawn | null };

const KIND = { 1: "internal", 2: "usage", 3: "load", 4: "scene", 5: "check", 6: "environment", 7: "budget" } as const;

class ParityError extends Error {}

function parse(argv: string[]): Mode {
  const [cmd, ...rest] = argv;
  switch (cmd) {
    case "reference": case "repeat":
      throw new ParityError(`${cmd}: R0 is recorded data, and loft-draw no longer draws TS artefact directories. Fix: check out 7f48109 (the last commit with loft-loader's loft-draw) to draw a reference set`);
    case "rust": {
      const i = rest.indexOf("--only");
      if (rest.length !== (i < 0 ? 0 : 2) || (i >= 0 && !rest[i + 1])) throw new ParityError(`${cmd} takes only --only <text>`);
      return { kind: cmd, only: i < 0 ? "" : rest[i + 1]! };
    }
    case "checks": case "cross":
      if (rest.length !== 1) throw new ParityError(`${cmd} takes one path`);
      return { kind: cmd, path: rest[0]! };
    case "dawn":
      if (rest.length !== 2) throw new ParityError("dawn takes a scene path and its entry id");
      return { kind: "dawn", path: rest[0]!, id: rest[1]! };
    default: throw new ParityError(`unknown mode ${cmd ?? "(none)"}: rust | checks <scene.ts> | dawn <scene.ts> <id> | cross <R0 dir>`);
  }
}

const sha = (b: Uint8Array | string) => createHash("sha256").update(b).digest("hex");
const git = (...a: string[]) => Bun.spawnSync(["git", ...a], { cwd: ROOT }).stdout.toString().trim();

/** A draw that outlives DEADLINE is killed and recorded as refused: a loft-view under load 25 once hung 15 min on one frame. */
const DEADLINE = 300_000;
async function spawn(cmd: string[]): Promise<{ exit: number; stdout: string; stderr: string }> {
  const p = Bun.spawn(["nice", "-n", "15", ...cmd], { cwd: ROOT, stdout: "pipe", stderr: "pipe", timeout: DEADLINE });
  const [stdout, stderr, exit] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { exit, stdout, stderr: p.signalCode === null ? stderr : `killed (${p.signalCode}) after the ${DEADLINE / 1000} s deadline\n${stderr}` };
}

async function entries(only: string): Promise<Entry[]> {
  const examples = [...new Bun.Glob(corpus.examples).scanSync({ cwd: ROOT })].sort()
    .map((path): Entry => ({ id: path.replace(/\.ts$/, "").replaceAll("/", "__"), path, origin: { kind: "example" } }));
  const { migrate } = await import("../migrate.ts");
  const creative = (process.env[corpus.seeds.env] ?? corpus.seeds.default).replace(/^~/, homedir());
  const seeds = corpus.seeds.sets.flatMap(({ set, path, rows, sha256 }) => {
    const file = join(creative, path);
    if (!existsSync(file)) throw new ParityError(`${file} is missing. Fix: set ${corpus.seeds.env} to a creative checkout at ${corpus.seeds.commit} or later`);
    const text = readFileSync(file, "utf8");
    if (sha(text) !== sha256) throw new ParityError(`${file} has sha256 ${sha(text)}, corpus.json pins ${sha256}. Fix: check out creative ${corpus.seeds.commit}, or re-pin corpus.json and say so in the gate report`);
    const lines = text.split("\n").filter((l) => l.trim() !== "");
    if (lines.length !== rows) throw new ParityError(`${file}: ${lines.length} rows, corpus.json says ${rows}`);
    return lines.map((line): Entry => {
      const { id, code } = JSON.parse(line) as { id: string; code: string };
      const m = migrate(code, "scene.ts", null);
      const safe = id.replaceAll("/", "__").replace(/[^A-Za-z0-9_.-]/g, "_");
      const ts = `out/parity/R0/seeds/${set}/${safe}.ts`;
      mkdirSync(join(ROOT, `out/parity/R0/seeds/${set}`), { recursive: true });
      writeFileSync(join(ROOT, ts), m.code);
      return { id: `seed__${set}__${safe}`, path: ts, origin: { kind: "seed", set, seed: id, unported: m.unported.length } };
    });
  });
  return [...examples, ...seeds].filter((e) => e.id.includes(only));
}

async function build(e: Entry): Promise<Built> {
  const dir = join(OUT, "build", e.id);
  rmSync(dir, { recursive: true, force: true });
  const r = await spawn([process.execPath, "src/cli/main.ts", "build", e.path, "--out", dir]);
  if (r.exit === 0) {
    const m = JSON.parse(readFileSync(join(dir, "manifest.json"), "utf8")) as { cameras: object; load?: { kind: string }[]; frame: { replay?: unknown[] } };
    const steps = /^bake /m.test(r.stdout) || (m.load ?? []).some((j) => j.kind === "steps") || (m.frame.replay ?? []).length > 0;
    return { kind: "built", steps, cameras: Object.keys(m.cameras) };
  }
  const verdict = KIND[r.exit as keyof typeof KIND];
  if (verdict === undefined) return { kind: "crashed", exit: r.exit, stderr: r.stderr.slice(-4000) };
  const lines = r.stderr.trimEnd().split("\n");
  const last = lines.at(-1) ?? "";
  const site = last.match(/ \(at ([^()]+)\)$/)?.[1] ?? null;
  const head = last.replace(/^loft: [^:]+: /, "").replace(/ \(at [^()]+\)$/, "");
  return { kind: "refused", verdict, exit: r.exit, head, site, detail: lines.slice(0, -1) };
}

async function checks(e: Entry): Promise<Checks> {
  const r = await spawn([process.execPath, join(import.meta.dir, "run.ts"), "checks", e.path]);
  return r.exit === 0 ? JSON.parse(r.stdout.trimEnd().split("\n").at(-1)!) as Checks : { kind: "crashed", exit: r.exit, stderr: r.stderr.slice(-4000) };
}

/** The frames of §2.2: up to three named cameras x the times (and the steps time) at 512², and one detail frame. */
function plan(b: { steps: boolean; cameras: string[] }): Shot[] {
  const f = corpus.frames;
  const times = b.steps ? [...f.times, f.stepsTime] : f.times;
  const main = b.cameras.slice(0, f.cameras).flatMap((camera) => times.map((time) => ({ camera, time, size: f.size })));
  return b.cameras.length === 0 ? [] : [...main, { camera: b.cameras[0]!, time: f.detail.time, size: f.detail.size }];
}

const frameName = (f: Shot) => `${f.camera}@${f.time}@${f.size.join("x")}.png`;

/** One frame of a bundle (`dir/scene.loft`) by `loft-draw still`, lossless. */
async function draw(dir: string, f: Shot, file: string): Promise<Frame> {
  const r = await spawn([VIEW, "still", dir, "--view", f.camera, "--time", String(f.time), "--size", f.size.join("x"), "--out", file]);
  if (r.exit !== 0) return { ...f, kind: "refused", executor: "loft-draw", exit: r.exit, stderr: r.stderr.slice(-2000) };
  return { ...f, kind: "drawn", executor: "loft-draw", file, sha256: sha(readFileSync(file)) };
}

/** Lane 4.2: every loft-draw frame of `other` (the macOS R0) against this set's (OUT, another backend) under the
 * cross-executor rule (§2.3). Every entry must also end its build the same way. */
async function cross(other: string) {
  const read = (dir: string) => new Map([...new Bun.Glob("entries/*.json").scanSync({ cwd: dir })]
    .map((f) => JSON.parse(readFileSync(join(dir, f), "utf8")) as Rec).map((r) => [r.entry.id, r] as const));
  const ours = read(OUT), theirs = read(other), builds: string[] = [], fails: { id: string; frame: string; diff: unknown }[] = [];
  let frames = 0;
  for (const [id, t] of theirs) {
    const o = ours.get(id);
    if (o?.build.kind !== t.build.kind) { builds.push(`${id}: ${t.build.kind} in ${other}, ${o?.build.kind ?? "absent"} here`); continue; }
    // Native frames only: R0 predates the loft-view → loft-draw rename and records "loft-view"; Dawn frames are not crossed.
    for (const f of t.frames.filter((f) => f.kind === "drawn" && f.executor !== "dawn")) {
      const g = o.frames.find((x) => frameName(x) === frameName(f)), frame = frameName(f);
      frames++;
      if (g?.kind !== "drawn" || f.kind !== "drawn") { fails.push({ id, frame, diff: g?.kind ?? "absent" }); continue; }
      const d = await spawn([DIFF, join(other, f.file), join(OUT, g.file), "--rule", "cross"]);
      if (d.exit !== 0) fails.push({ id, frame, diff: d.exit === 1 ? JSON.parse(d.stdout) : d.stderr.trim() });
    }
  }
  const report = { other, here: OUT, entries: theirs.size, builds, frames, fails: fails.length, failed: fails };
  writeFileSync(join(OUT, "cross.json"), json(report));
  console.log(`cross: ${theirs.size} entries, ${builds.length} build mismatches; ${frames - fails.length}/${frames} frames meet the cross rule`);
  process.exit(builds.length + fails.length === 0 && frames > 0 ? 0 : 1); // no frame compared is no pass
}

/**
 * Every entry through the Rust program driver (`LOFT_ENGINE=rust loft build`), in its own process, against how R0's
 * build ended: built against built (the bundle's sha256 and its draw count recorded), a refusal against a refusal of the
 * same verdict and head. Built with `--naga --precision mixed`: exit 0 is a bundle whose module Naga accepted as f32,
 * with its f16 verdict printed. A bundle with draws is drawn by loft-draw at R0's frames (R0's camera names: the bundle
 * must carry them) and each frame compared with R0's under the picture rule (a Dawn reference under the cross rule).
 */
async function rust(only: string) {
  const out = join(ROOT, "out/parity/rust"), list = await entries(only);
  if (!existsSync(VIEW)) throw new ParityError(`${VIEW} is missing. Fix: tools/dev.sh build loft-draw --fast`);
  if (!existsSync(DIFF)) throw new ParityError(`${DIFF} is missing. Fix: cargo build --release --manifest-path tools/parity/Cargo.toml`);
  type Compared = Frame & { pass?: boolean; diff?: unknown };
  const rows: { id: string; r0: string; rust: string; same: boolean; build?: string; draws?: number; bytes?: number; ms: number; frames?: Compared[] }[] = [];
  const outcome = (b: Built) => b.kind === "refused" ? `refused:${b.verdict}:${b.head}` : b.kind === "built" ? "built" : `crashed:${b.exit}`;
  const queue = [...list];
  const worker = async () => {
    for (let e = queue.shift(); e !== undefined; e = queue.shift()) {
      const dir = join(out, "build", e.id), t = performance.now();
      rmSync(dir, { recursive: true, force: true });
      const r = await spawn(["env", "LOFT_ENGINE=rust", process.execPath, "src/cli/main.ts", "build", e.path, "--out", dir, "--naga", "--precision", "mixed"]);
      const ms = Math.round(performance.now() - t);
      const r0 = existsSync(join(OUT, "entries", `${e.id}.json`)) ? outcome((JSON.parse(readFileSync(join(OUT, "entries", `${e.id}.json`), "utf8")) as Rec).build) : "absent";
      if (r.exit === 0) {
        const bytes = readFileSync(join(dir, "scene.loft")), n = Number(new DataView(bytes.buffer, bytes.byteOffset).getBigUint64(8, true));
        const draws = (JSON.parse(new TextDecoder().decode(bytes.subarray(16, 16 + n))) as { draws: unknown[] }).draws.length;
        // R0's frames (§2.2), drawn only when the bundle draws anything; each compared under the picture rule.
        const rec = existsSync(join(OUT, "entries", `${e.id}.json`)) ? JSON.parse(readFileSync(join(OUT, "entries", `${e.id}.json`), "utf8")) as Rec : null;
        const frames: Compared[] = [];
        for (const ref of draws === 0 ? [] : (rec?.frames ?? []).filter((f) => f.kind === "drawn")) {
          mkdirSync(join(out, "frames", e.id), { recursive: true });
          const f = await draw(dir, ref, join(out, "frames", e.id, frameName(ref)));
          if (f.kind !== "drawn" || ref.kind !== "drawn") { frames.push(f); continue; }
          const d = await spawn([DIFF, join(OUT, ref.file), f.file, "--rule", ref.executor === "dawn" ? "cross" : "picture"]);
          frames.push({ ...f, pass: d.exit === 0, diff: d.exit <= 1 ? JSON.parse(d.stdout) : d.stderr.trim() });
        }
        rows.push({ id: e.id, r0, rust: "built", same: r0 === "built", build: sha(bytes), draws, bytes: bytes.length, ms, frames });
      } else {
        const verdict = KIND[r.exit as keyof typeof KIND], last = r.stderr.trimEnd().split("\n").at(-1) ?? "";
        const rust = verdict === undefined ? `crashed:${r.exit}` : `refused:${verdict}:${last.replace(/^loft: [^:]+: /, "").replace(/ \(at [^()]+\)$/, "")}`;
        rows.push({ id: e.id, r0, rust, same: rust === r0, ms });
      }
      const x = rows.at(-1)!;
      console.log(`${rows.length}/${list.length} ${x.same ? "same" : "DIFF"} ${e.id}: ${x.rust}${x.same ? "" : ` (R0 ${x.r0})`}`);
    }
  };
  await Promise.all([worker(), worker(), worker()]);
  rows.sort((a, b) => a.id.localeCompare(b.id));
  const count = <T,>(xs: T[], k: (x: T) => string) => xs.reduce<{ [k: string]: number }>((o, x) => ({ ...o, [k(x)]: (o[k(x)] ?? 0) + 1 }), {});
  const report = { head: git("rev-parse", "HEAD"), dirty: git("status", "--porcelain", "--", "src", "rust", "tools/parity") !== "", r0: OUT, only, entries: rows.length, same: rows.filter((r) => r.same).length,
    outcomes: count(rows, (r) => r.rust.split(":").slice(0, 2).join(":")), drawable: rows.filter((r) => (r.draws ?? 0) > 0).length,
    drawn: rows.filter((r) => (r.frames ?? []).some((f) => f.kind === "drawn")).length,
    passing: rows.filter((r) => (r.frames ?? []).length > 0 && r.frames!.every((f) => f.pass === true)).length,
    frames: count(rows.flatMap((r) => r.frames ?? []), (f) => f.kind === "refused" ? "refused" : f.pass ? "pass" : "fail"), rows };
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, only === "" ? "rust.json" : `rust.only-${only}.json`), json(report));
  console.log(`rust: ${report.entries} entries, ${report.same} end as R0 did; ${json(report.outcomes, 0).trim()}; ${report.drawable} with draws, ${report.drawn} drawn, ${report.passing} passing; frames ${json(report.frames, 0).trim()}`);
}

/** Child: the verdicts `loft check` computes (src/cli/check.ts), for the variant `loft build` builds, whole. */
async function verdicts(path: string): Promise<Checks> {
  const { subjectAt, choose } = await import("../../src/cli/variant.ts");
  const { verdictOf } = await import("../../src/cli/verdict.ts");
  try {
    const c = choose(await subjectAt(path), path, { kind: "default" });
    const [{ compileScene }, { Probe, runChecks }] = await Promise.all([import("../../src/compile/index.ts"), import("../../src/check.ts")]);
    const compiled = compileScene(c.scene);
    const state = compiled.artefact.recurrences !== undefined;
    if (c.checks.kind === "absent" && !state) return { kind: "absent" };
    const list = c.checks.kind === "absent" ? [] : c.checks.list;
    // As check.ts: ranged checks of a stateful scene run before the bake; the rest after it, on the GPU's states.
    const ranged = state ? list.filter((k) => "formula" in k) : [];
    const early = ranged.map((k) => k.run(new Probe(compiled, {})));
    const remaining = state ? list.filter((k) => !("formula" in k)) : list;
    if (state) {
      const [{ headlessDevice }, { bakeRecurrences }] = await Promise.all([import("../../src/host/headless.ts"), import("../../src/host/bake.ts")]);
      await bakeRecurrences(await headlessDevice(), compiled.artefact);
    }
    return { kind: "verdicts", label: c.label, verdicts: [...early, ...runChecks(c.scene, remaining, { compiled })] };
  } catch (e) {
    return { kind: "refused", verdict: await verdictOf(e) };
  }
}

async function dawnFrames(e: Entry): Promise<Dawn> {
  const r = await spawn([process.execPath, join(import.meta.dir, "run.ts"), "dawn", e.path, e.id]);
  return r.exit === 0 ? { kind: "drawn", frames: JSON.parse(r.stdout.trimEnd().split("\n").at(-1)!) as Frame[] } : { kind: "refused", exit: r.exit, stderr: r.stderr.slice(-2000) };
}

/** Child: `loft render`'s path (compile, bake, load on Dawn, `frameTimed`), every planned frame, lossless. */
async function dawn(path: string, id: string): Promise<Frame[]> {
  const [{ loadArtefact, headlessHost }, { resolveCamera }, { encodePng }, { loopTime }] = await Promise.all([
    import("../../src/cli/load.ts"), import("../../src/cli/cameras.ts"), import("../../src/cli/image.ts"), import("../../src/artefact.ts"),
  ]);
  const artefact = await loadArtefact({ kind: "file", path, style: { kind: "scene" }, light: { kind: "scene" }, variant: { kind: "default" } });
  const loaded = await (await headlessHost()).load(artefact);
  const loop = (artefact as { loop?: Parameters<typeof loopTime>[0] }).loop ?? { kind: "open" };
  mkdirSync(join(OUT, "frames", id), { recursive: true });
  const frames: Frame[] = [];
  for (const f of plan({ steps: artefact.recurrences !== undefined, cameras: Object.keys(artefact.cameras) })) {
    const view = { camera: resolveCamera(artefact, f.camera), width: f.size[0], height: f.size[1], time: loopTime(loop, f.time), params: {}, inputs: {}, construction: false };
    const png = encodePng({ width: f.size[0], height: f.size[1], rgba: (await loaded.frameTimed(view)).pixels });
    const file = join(OUT, "frames", id, frameName(f));
    writeFileSync(file, png);
    frames.push({ ...f, kind: "drawn", executor: "dawn", file: file.slice(OUT.length + 1), sha256: sha(png) });
  }
  loaded.destroy();
  return frames;
}

/** JSON with NaN and infinities kept as strings (margins and bounds use them). */
const json = (x: unknown, indent = 1) => JSON.stringify(x, (_, v) => typeof v === "number" && !Number.isFinite(v) ? String(v) : v, indent) + "\n";

try {
  const mode = parse(Bun.argv.slice(2));
  switch (mode.kind) {
    case "rust": await rust(mode.only); break;
    case "checks": process.stdout.write(json(await verdicts(mode.path), 0)); process.exit(0);
    case "cross": await cross(resolve(mode.path)); break;
    case "dawn": process.stdout.write(json(await dawn(mode.path, mode.id), 0)); process.exit(0);
  }
} catch (e) {
  if (!(e instanceof ParityError)) throw e;
  console.error(`parity: ${e.message}`);
  process.exit(2);
}

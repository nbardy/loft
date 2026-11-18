// The reference set of the picture-parity gate (docs/proposals/2026-11-19_lean_rust_loft.md §2.2, §2.3), lane 0.2.
//
//   bun tools/parity/run.ts reference [--only text]   build, check and draw every corpus entry into out/parity/R0
//   bun tools/parity/run.ts repeat [--only text]      draw every recorded frame again: byte-identical, or the envelope
//   bun tools/parity/run.ts checks <scene.ts>         (child) one scene's check verdicts as JSON on stdout
//   bun tools/parity/run.ts dawn <scene.ts> <id>      (child) a scene `loft build` refuses, drawn as `loft render` draws it
//
// The corpus is tools/parity/corpus.json: every examples/**/*.ts and the creative seeds, migrated by tools/migrate.ts.
// Each entry is built by the real CLI (`loft build`); a refusal is recorded with its exit code, kind, site and text.
// Built entries are checked in a child (every Verdict object whole) and drawn by `loft-view` (wgpu 30, Metal) as
// lossless PNG. A scene the build refuses (a CPU table a file cannot hold: Storm) is drawn by `loft render`'s path on
// Dawn when it can be, and its frames are marked `dawn`: they are compared under the cross-executor rule. R0 is the commit of the tree, which must be clean where it decides pictures.
import { createHash } from "node:crypto";
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { cpus, homedir, loadavg } from "node:os";
import { join } from "node:path";

const ROOT = join(import.meta.dir, "../..");
const OUT = join(ROOT, "out/parity/R0");
const VIEW = join(ROOT, "rust/target/release/loft-view");
const DIFF = join(ROOT, "tools/parity/target/release/parity-diff");
/** The paths that decide a reference picture (the loader moved to rust/crates in lane 0.1; the workspace lock pins its wgpu). */
const PICTURE = ["src", "examples", "rust/crates/loft-loader", "rust/Cargo.lock", "tools/migrate.ts", "package.json"];
const corpus = JSON.parse(readFileSync(join(import.meta.dir, "corpus.json"), "utf8")) as Corpus;

type Corpus = {
  examples: string;
  seeds: { default: string; env: string; commit: string; sets: { set: string; path: string; rows: number; sha256: string }[] };
  frames: { cameras: number; times: number[]; size: [number, number]; stepsTime: number; detail: { time: number; size: [number, number] } };
};
type Mode = { kind: "reference" | "repeat"; only: string } | { kind: "checks"; path: string } | { kind: "dawn"; path: string; id: string };
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
type Executor = "loft-view" | "dawn";
type Frame = Shot & ({ kind: "drawn"; executor: Executor; file: string; sha256: string } | { kind: "refused"; executor: Executor; exit: number; stderr: string });
/** What `loft render` made of a refused build: frames, or its own refusal (the build's, usually). */
type Dawn = { kind: "drawn"; frames: Frame[] } | { kind: "refused"; exit: number; stderr: string };
type Rec = { entry: Entry; build: Built; checks: Checks | null; frames: Frame[]; dawn: Dawn | null };

const KIND = { 1: "internal", 2: "usage", 3: "load", 4: "scene", 5: "check", 6: "environment", 7: "budget" } as const;

class ParityError extends Error {}

function parse(argv: string[]): Mode {
  const [cmd, ...rest] = argv;
  switch (cmd) {
    case "reference": case "repeat": {
      const i = rest.indexOf("--only");
      if (rest.length !== (i < 0 ? 0 : 2) || (i >= 0 && !rest[i + 1])) throw new ParityError(`${cmd} takes only --only <text>`);
      return { kind: cmd, only: i < 0 ? "" : rest[i + 1]! };
    }
    case "checks":
      if (rest.length !== 1) throw new ParityError("checks takes one scene path");
      return { kind: "checks", path: rest[0]! };
    case "dawn":
      if (rest.length !== 2) throw new ParityError("dawn takes a scene path and its entry id");
      return { kind: "dawn", path: rest[0]!, id: rest[1]! };
    default: throw new ParityError(`unknown mode ${cmd ?? "(none)"}: reference | repeat | checks <scene.ts> | dawn <scene.ts> <id>`);
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

async function draw(id: string, f: Shot, file: string): Promise<Frame> {
  const r = await spawn([VIEW, join(OUT, "build", id), "--view", f.camera, "--time", String(f.time), "--size", f.size.join("x"), "--out", file]);
  adapter ??= r.stderr.match(/^adapter (.*)$/m)?.[1];
  if (r.exit !== 0) return { ...f, kind: "refused", executor: "loft-view", exit: r.exit, stderr: r.stderr.slice(-2000) };
  return { ...f, kind: "drawn", executor: "loft-view", file: file.slice(OUT.length + 1), sha256: sha(readFileSync(file)) };
}
let adapter: string | undefined;

async function reference(only: string) {
  const dirty = git("status", "--porcelain", "--", ...PICTURE);
  if (dirty !== "") throw new ParityError(`R0 must be a commit: the tree is dirty where it decides pictures:\n${dirty}\nFix: commit or stash first`);
  if (!existsSync(VIEW)) throw new ParityError(`${VIEW} is missing. Fix: tools/dev.sh build loft-loader --release`);
  // R0 is the last commit that decides pictures, so a run interrupted at that commit resumes: recorded entries are kept.
  const r0 = git("log", "-1", "--format=%H", "--", ...PICTURE), stamp = join(OUT, "r0");
  const held = existsSync(stamp) ? readFileSync(stamp, "utf8").trim() : r0;
  if (held !== r0) throw new ParityError(`${OUT} holds R0 ${held}, this tree's is ${r0}. Fix: check out ${held} to resume, or move ${OUT} aside for a new reference`);
  mkdirSync(OUT, { recursive: true });
  writeFileSync(stamp, `${r0}\n`);
  const started = { at: new Date().toISOString(), load1: loadavg()[0] };
  const list = await entries(only);
  mkdirSync(join(OUT, "entries"), { recursive: true });
  let next = 0;
  const worker = async () => {
    for (let i = next++; i < list.length; i = next++) {
      const entry = list[i]!;
      if (existsSync(join(OUT, "entries", `${entry.id}.json`))) continue;
      const b = await build(entry);
      const frames: Frame[] = [];
      if (b.kind === "built") {
        mkdirSync(join(OUT, "frames", entry.id), { recursive: true });
        for (const f of plan(b)) frames.push(await draw(entry.id, f, join(OUT, "frames", entry.id, frameName(f))));
      }
      const dawn = b.kind === "refused" && b.verdict === "scene" ? await dawnFrames(entry) : null;
      if (dawn?.kind === "drawn") frames.push(...dawn.frames);
      const rec: Rec = { entry, build: b, checks: b.kind === "built" ? await checks(entry) : null, frames, dawn };
      writeFileSync(join(OUT, "entries", `${entry.id}.json`), json(rec));
      const bad = frames.filter((f) => f.kind === "refused").length;
      console.log(`${String(i + 1).padStart(3)}/${list.length} ${b.kind === "refused" ? `${b.verdict}(${b.exit})` : b.kind} ${rec.checks?.kind ?? "-"} ${frames.length - bad}/${frames.length} frames ${entry.id}`);
    }
  };
  await Promise.all([worker(), worker()]);
  const recs = list.map((e) => JSON.parse(readFileSync(join(OUT, "entries", `${e.id}.json`), "utf8")) as Rec);
  const count = <T,>(xs: T[], key: (x: T) => string) => xs.reduce<{ [k: string]: number }>((o, x) => ({ ...o, [key(x)]: (o[key(x)] ?? 0) + 1 }), {});
  const manifest = {
    r0, head: git("rev-parse", "HEAD"), loftView: { sha256: sha(readFileSync(VIEW)), loader: git("log", "-1", "--format=%H", "--", "rust/crates/loft-loader") },
    adapter, only, corpus: sha(readFileSync(join(import.meta.dir, "corpus.json"))), started, ended: { at: new Date().toISOString(), load1: loadavg()[0] }, cpus: cpus().length,
    entries: recs.length, origins: count(recs, (r) => r.entry.origin.kind),
    builds: count(recs, (r) => r.build.kind === "refused" ? `refused:${r.build.verdict}` : r.build.kind),
    checks: count(recs, (r) => r.checks?.kind ?? "unbuilt"),
    frames: count(recs.flatMap((r) => r.frames), (f) => `${f.kind}:${f.executor}`),
  };
  writeFileSync(join(OUT, only === "" ? "manifest.json" : `manifest.only-${only}.json`), json(manifest));
  console.log(json(manifest));
}

/** §2.3 precondition: every recorded frame drawn again; differing ones are kept and measured by the comparator. */
async function repeat(only: string) {
  const recs = (await entries(only)).flatMap((e) => existsSync(join(OUT, "entries", `${e.id}.json`)) ? [JSON.parse(readFileSync(join(OUT, "entries", `${e.id}.json`), "utf8")) as Rec] : []);
  // Dawn frames are drawn per scene by their child, so the repeat covers the loft-view references only.
  const drawn = recs.flatMap((r) => r.frames.flatMap((f) => f.kind === "drawn" && f.executor === "loft-view" ? [{ id: r.entry.id, f }] : []));
  const differing: { id: string; frame: string; diff: unknown }[] = [];
  let refused = 0;
  for (const [i, { id, f }] of drawn.entries()) {
    if (i % 200 === 0) console.log(`${i}/${drawn.length} frames, ${differing.length} differ`);
    const file = join(OUT, "repeat", id, frameName(f));
    mkdirSync(join(OUT, "repeat", id), { recursive: true });
    const again = await draw(id, f, file);
    if (again.kind === "refused") { refused++; continue; }
    if (again.sha256 === f.sha256) { rmSync(file); continue; }
    const d = await spawn([DIFF, join(OUT, f.file), file, "--envelope", "0"]);
    differing.push({ id, frame: frameName(f), diff: JSON.parse(d.stdout) });
  }
  const envelope = Math.max(0, ...differing.map((d) => (d.diff as { maxDiff: number }).maxDiff));
  const report = { r0: readFileSync(join(OUT, "r0"), "utf8").trim(), only, frames: drawn.length, identical: drawn.length - differing.length - refused, refused, envelope, differing };
  writeFileSync(join(OUT, only === "" ? "repeat.json" : `repeat.only-${only}.json`), json(report));
  console.log(`${report.identical}/${drawn.length} frames byte-identical; ${differing.length} differ (envelope ${envelope} codes); ${refused} refused`);
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
    case "reference": await reference(mode.only); break;
    case "repeat": await repeat(mode.only); break;
    case "checks": process.stdout.write(json(await verdicts(mode.path), 0)); process.exit(0);
    case "dawn": process.stdout.write(json(await dawn(mode.path, mode.id), 0)); process.exit(0);
  }
} catch (e) {
  if (!(e instanceof ParityError)) throw e;
  console.error(`parity: ${e.message}`);
  process.exit(2);
}

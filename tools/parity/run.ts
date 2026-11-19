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
import { join, resolve } from "node:path";
import { corpus, corpusEntries, CorpusError, type Entry } from "./corpus.ts";

const ROOT = join(import.meta.dir, "../..");
const OUT = join(ROOT, process.env.LOFT_PARITY_OUT ?? "out/parity/R0");
const VIEW = join(ROOT, "rust/target/lab/loft-draw");
const DIFF = join(ROOT, "tools/parity/target/release/parity-diff");
/** The paths that decide a reference picture (the loader moved to rust/crates in lane 0.1; the workspace lock pins its wgpu). */
type Mode = { kind: "rust"; only: string } | { kind: "checks" | "cross"; path: string } | { kind: "dawn"; path: string; id: string };
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
  try { return await corpusEntries(only, "out/parity/R0/seeds"); } catch (e) { throw e instanceof CorpusError ? new ParityError(e.message) : e; }
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
 * Every entry through the Rust program driver (`LOFT_ENGINE=rust loft build`), in its own process, scored strictly
 * against R0's record (the audit, agent_notes/2026-11-19_audits/cut_rust.md, parity runner): every R0 outcome and every
 * R0 frame (native and Dawn) is an expected item, keyed exactly (entry; camera, time, size, executor), and each ends
 * `pass`, a named `refused` that equals R0's (an outcome only: a refusal that matches R0's verdict and head), or a
 * failure with a named reason. A frame is never skipped: a build refused, a bundle with no draws, a draw that failed or
 * a reference whose bytes moved is a failed frame saying so. Exit 0 only when nothing fails and the run was unfiltered
 * (a filtered run diagnoses; it cannot pass the gate). Built with `--naga --precision mixed`.
 */
/**
 * Build outcomes that moved on purpose, each with its reason (plan §2.2c: opaque CPU fills are gone, so a scene that R0
 * refused for one builds through the rewritten library). Their R0 frames are Dawn's and are compared under the cross rule.
 */
const DELIBERATE: Readonly<Record<string, string>> = {
  examples__open: "R0 refused its JavaScript-filled tables (plan §2.2c); the recorder has no slot for them",
  examples__storm: "as open",
  examples__scenes__stormfront: "as open",
};

async function rust(only: string) {
  const out = join(ROOT, "out/parity/rust"), list = await entries(only);
  if (!existsSync(VIEW)) throw new ParityError(`${VIEW} is missing. Fix: tools/dev.sh build loft-draw --fast`);
  if (!existsSync(DIFF)) throw new ParityError(`${DIFF} is missing. Fix: cargo build --release --manifest-path tools/parity/Cargo.toml`);
  // The expected set is R0's entry records, whole: an R0 entry the corpus no longer lists fails as `unlisted`.
  const expected = new Map([...new Bun.Glob("entries/*.json").scanSync({ cwd: OUT })].map((f) => JSON.parse(readFileSync(join(OUT, f), "utf8")) as Rec).map((r) => [r.entry.id, r] as const));
  type Scored = Shot & { executor: Executor; result: "pass" | "fail"; reason: string; file?: string; diff?: unknown };
  type Cost = { kind: "measured"; camera: string; frameP50: number; frameP95: number; loadMs: number; modules: number; renderPipelines: number; computePipelines: number } | { kind: "refused"; why: string };
  type Row = { id: string; r0: string; rust: string; outcome: "same" | "refused-same" | "fail"; build?: string; draws?: number; bytes?: number; ms: number; frames: Scored[]; cost?: Cost };
  const rows: Row[] = [];
  const outcome = (b: Built) => b.kind === "refused" ? `refused:${b.verdict}:${b.head}` : b.kind === "built" ? "built" : `crashed:${b.exit}`;
  const queue = [...list];
  const worker = async () => {
    for (let e = queue.shift(); e !== undefined; e = queue.shift()) {
      const dir = join(out, "build", e.id), t = performance.now();
      rmSync(dir, { recursive: true, force: true });
      rmSync(join(out, "frames", e.id), { recursive: true, force: true });
      const r = await spawn(["env", "LOFT_ENGINE=rust", process.execPath, "src/cli/main.ts", "build", e.path, "--out", dir, "--naga", "--precision", "mixed"]);
      const ms = Math.round(performance.now() - t);
      const rec = expected.get(e.id);
      const r0 = rec === undefined ? "absent" : outcome(rec.build);
      // Every R0 frame of this entry, deduplicated by its exact key (a duplicate key in R0 is itself a failure).
      const refs = (rec?.frames ?? []).filter((f) => f.kind === "drawn");
      const keys = new Set<string>();
      const missing = (ref: Frame, reason: string): Scored => ({ camera: ref.camera, time: ref.time, size: ref.size, executor: ref.executor, result: "fail", reason });
      const frames: Scored[] = [];
      let rust: string, build: string | undefined, draws: number | undefined, size: number | undefined;
      if (r.exit === 0) {
        const bytes = readFileSync(join(dir, "scene.loft")), n = Number(new DataView(bytes.buffer, bytes.byteOffset).getBigUint64(8, true));
        draws = (JSON.parse(new TextDecoder().decode(bytes.subarray(16, 16 + n))) as { draws: unknown[] }).draws.length;
        rust = "built", build = sha(bytes), size = bytes.length;
      } else {
        const verdict = KIND[r.exit as keyof typeof KIND], last = r.stderr.trimEnd().split("\n").at(-1) ?? "";
        rust = verdict === undefined ? `crashed:${r.exit}: ${last}` : `refused:${verdict}:${last.replace(/^loft: [^:]+: /, "").replace(/ \(at [^()]+\)$/, "")}`;
      }
      // Every frame of the entry from one load (`loft-draw still --shot`): the load is most of a frame's cost.
      const shots = rust === "built" && (draws ?? 0) > 0 ? refs : [];
      let drawn: { exit: number; stderr: string } = { exit: 0, stderr: "" };
      if (shots.length > 0) {
        mkdirSync(join(out, "frames", e.id), { recursive: true });
        const file = (f: Frame) => join(out, "frames", e.id, frameName(f));
        const [first, ...more] = shots;
        drawn = await spawn([VIEW, "still", dir, "--view", first!.camera, "--time", String(first!.time), "--size", first!.size.join("x"), "--out", file(first!),
          ...more.flatMap((f) => ["--shot", `${f.camera}@${f.time}@${f.size.join("x")}=${file(f)}`])]);
      }
      const drawnOf = (ref: Frame): Frame => {
        const file = join(out, "frames", e.id, frameName(ref));
        return drawn.exit === 0 && existsSync(file) ? { ...ref, kind: "drawn", executor: "loft-draw", file, sha256: sha(readFileSync(file)) } : { ...ref, kind: "refused", executor: "loft-draw", exit: drawn.exit, stderr: drawn.stderr.slice(-2000) };
      };
      for (const ref of refs) {
        const key = `${ref.executor} ${frameName(ref)}`;
        if (keys.has(key)) { frames.push(missing(ref, `duplicate R0 key ${key}`)); continue; }
        keys.add(key);
        if (ref.kind !== "drawn") continue;
        const refFile = join(OUT, ref.file);
        if (!existsSync(refFile) || sha(readFileSync(refFile)) !== ref.sha256) { frames.push(missing(ref, `R0 reference ${ref.file} is missing or its sha256 moved`)); continue; }
        if (rust !== "built") { frames.push(missing(ref, `not built: ${rust}`)); continue; }
        if (draws === 0) { frames.push(missing(ref, "built with no draws")); continue; }
        const got = drawnOf(ref);
        if (got.kind !== "drawn") { frames.push(missing(ref, `loft-draw refused (exit ${got.exit}): ${got.stderr.trim().split("\n").at(-1) ?? ""}`)); continue; }
        const f = got;
        const d = await spawn([DIFF, refFile, f.file, "--rule", ref.executor === "dawn" ? "cross" : "picture"]);
        const diff = d.exit <= 1 ? JSON.parse(d.stdout) : d.stderr.trim();
        frames.push({ ...missing(ref, d.exit === 0 ? "" : d.exit === 1 ? "picture rule" : `parity-diff exit ${d.exit}`), result: d.exit === 0 ? "pass" : "fail", file: f.file, diff });
      }
      // What the bundle costs (CORE.md: GPU frame time, then load time and the modules and pipelines it builds): one
      // `loft-draw profile` at 960x540, R0's first camera at t = 1.37 s, 16 frames.
      let cost: Cost | undefined;
      const camera = refs[0]?.camera;
      if (rust === "built" && (draws ?? 0) > 0 && camera !== undefined) {
        const r = await spawn([VIEW, "profile", dir, "--view", camera, "--time", "1.37", "--size", "960x540", "--frames", "16"]);
        const last = r.stdout.trimEnd().split("\n").at(-1) ?? "";
        if (r.exit !== 0 || !last.startsWith("{")) cost = { kind: "refused", why: `loft-draw profile exit ${r.exit}: ${r.stderr.trim().split("\n").at(-1) ?? ""}` };
        else {
          const q = (JSON.parse(last) as { profile: { gpu_ms: number[] | null; loads_ms: number[]; modules: number; render_pipelines: number; compute_pipelines: number } }).profile;
          cost = q.gpu_ms === null ? { kind: "refused", why: "no timestamp queries" } : { kind: "measured", camera, frameP50: q.gpu_ms[1]!, frameP95: q.gpu_ms[2]!, loadMs: q.loads_ms[0]!, modules: q.modules, renderPipelines: q.render_pipelines, computePipelines: q.compute_pipelines };
        }
      }
      // An outcome passes when it ends as R0's did: built as built, or the same refusal (verdict and head), or a
      // deliberate change listed in DELIBERATE (its frames are still scored, against R0's Dawn pictures).
      const same = r0 === rust || (DELIBERATE[e.id] !== undefined && rust === "built" && r0.startsWith("refused:"));
      rows.push({ id: e.id, r0, rust, outcome: same ? (rust === "built" ? "same" : "refused-same") : "fail", build, draws, bytes: size, ms, frames, cost });
      const x = rows.at(-1)!, fp = x.frames.filter((f) => f.result === "pass").length;
      console.log(`${rows.length}/${list.length} ${x.outcome === "fail" ? "DIFF" : "same"} ${e.id}: ${x.rust}${x.outcome === "fail" ? ` (R0 ${x.r0})` : ""}; frames ${fp}/${x.frames.length}`);
    }
  };
  await Promise.all([worker(), worker(), worker()]);
  rows.sort((a, b) => a.id.localeCompare(b.id));
  // R0 entries the corpus did not list (unfiltered runs only: a filter leaves them out on purpose).
  const listed = new Set(list.map((e) => e.id));
  const unlisted = only === "" ? [...expected.keys()].filter((id) => !listed.has(id)) : [];
  const count = <T,>(xs: T[], k: (x: T) => string) => xs.reduce<{ [k: string]: number }>((o, x) => ({ ...o, [k(x)]: (o[k(x)] ?? 0) + 1 }), {});
  const frames = rows.flatMap((r) => r.frames);
  const expectedFrames = only === "" ? [...expected.values()].reduce((n, r) => n + r.frames.filter((f) => f.kind === "drawn").length, 0) : frames.length;
  const failedOutcomes = rows.filter((r) => r.outcome === "fail").length + unlisted.length;
  const failedFrames = frames.filter((f) => f.result === "fail").length + (expectedFrames - frames.length);
  const report = {
    head: git("rev-parse", "HEAD"), dirty: git("status", "--porcelain", "--", "src", "rust", "tools/parity") !== "", r0: OUT, only, authoritative: only === "",
    strict: {
      outcomes: { expected: only === "" ? expected.size : rows.length, same: rows.filter((r) => r.outcome === "same").length, refusedSame: rows.filter((r) => r.outcome === "refused-same").length, failed: failedOutcomes, unlisted },
      frames: { expected: expectedFrames, pass: frames.filter((f) => f.result === "pass").length, failed: failedFrames, reasons: count(frames.filter((f) => f.result === "fail"), (f) => f.reason.replace(/:.*$/s, "")) },
    },
    built: rows.filter((r) => r.rust === "built").length,
    drawable: rows.filter((r) => (r.draws ?? 0) > 0).length,
    drawn: rows.filter((r) => r.frames.some((f) => f.file !== undefined)).length,
    passing: rows.filter((r) => r.frames.length > 0 && r.frames.every((f) => f.result === "pass")).length,
    outcomes: count(rows, (r) => r.rust.split(":").slice(0, 2).join(":")), rows,
  };
  mkdirSync(out, { recursive: true });
  writeFileSync(join(out, only === "" ? "rust.json" : `rust.only-${only}.json`), json(report));
  const s = report.strict;
  console.log(`rust: outcomes ${s.outcomes.same} built-as-R0 + ${s.outcomes.refusedSame} refused-as-R0 of ${s.outcomes.expected}, ${s.outcomes.failed} failed; frames ${s.frames.pass}/${s.frames.expected} pass, ${s.frames.failed} failed ${json(s.frames.reasons, 0).trim()}; built ${report.built}, with draws ${report.drawable}, drawn ${report.drawn}, all frames passing ${report.passing}${only === "" ? "" : " (filtered: not the gate)"}`);
  process.exit(failedOutcomes + failedFrames === 0 && only === "" ? 0 : 1);
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

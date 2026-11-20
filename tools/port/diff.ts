// The port's byte-diff harness (agent_notes/2026-11-19_port/PLAN.md): for every corpus entry, the TypeScript
// compiler's output and the Rust port's (rust/crates/loft-lower), compared section by section.
//
//   bun tools/port/diff.ts [--mode port|lower|shift|ts] [--scope LANE[,LANE]] [--only TEXT] [--jobs N]
//                          [--shard I/N] [--baseline RUN] [--run NAME] [--reuse-ts]
//
// Modes (what is compared with the TS compiler's output, tools/port/ts_side.ts):
//   port   (default) `loft-lower compile` on the TS side's input/: probes, compile.json and the artefact
//   lower  `loft-lower lower` on the TS compiler's own compile.json: the lower lanes (LW, SW) alone
//   shift  the TS side again with every Expr id 1000 higher (--shift): lane P0's gate, names must not follow ids
//   ts     the TS side against a previous run's TS side (--baseline out/port/<run>): a TS refactor keeps every byte
//   cli    the TS side's artefact against `loft build`'s (the real CLI): the harness's reference is the shipped compiler
//
// Sections (tools/port/sections.ts), in pipeline order: probe.* (wire, graph, eval, tape), compile.* (the Artefact
// before lowering), files.* (every manifest key, chunk and blob by index, manifest.json whole, the file set), outcome.
// Each has an owner, a lane; --scope compares only those lanes' sections. An entry passes when every compared section
// is present on both sides and equal byte for byte; otherwise the report names the first differing section, its
// owner and where it differs. A TS crash, a TS input the wire cannot write, a missing binary, a Rust crash or a
// refusal that is not the TS compiler's each fail the entry by name. Exit 0 only when every entry passes and the run
// was unfiltered (--only and --shard runs diagnose; they cannot pass).
//
// Output: out/port/<run>/entries/<id>/{ts,rust}/... and out/port/<run>/report.json; one line per failing entry.
import { existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { availableParallelism } from "node:os";
import { join } from "node:path";
import { corpusEntries, CorpusError, type Entry } from "../parity/corpus.ts";
import { compileSections, fileSections, LANES, probeSections, type Section } from "./sections.ts";

const ROOT = join(import.meta.dir, "../..");
const RUST = join(ROOT, "rust/target/lab/loft-lower");
const DEADLINE = 15 * 60_000;

type ModeName = "port" | "lower" | "shift" | "ts" | "cli";
/** What the side compared with the TS compiler's is called in a report. */
const OTHER: Readonly<Record<ModeName, string>> = { port: "rust", lower: "rust", shift: "shifted", ts: "baseline", cli: "loft build" };
type Args = { mode: ModeName; scope: readonly string[]; only: string; jobs: number; shard: readonly [number, number]; baseline: string | undefined; run: string; reuseTs: boolean };

class HarnessError extends Error {}

function parse(argv: readonly string[]): Args {
  const a: Args = { mode: "port", scope: [], only: "", jobs: Math.max(1, Math.floor(availableParallelism() / 2)), shard: [0, 1], baseline: undefined, run: "", reuseTs: false };
  for (let i = 0; i < argv.length; i++) {
    const flag = argv[i]!, value = () => { const v = argv[++i]; if (v === undefined) throw new HarnessError(`${flag} needs a value`); return v; };
    switch (flag) {
      case "--mode": {
        const m = value();
        if (m !== "port" && m !== "lower" && m !== "shift" && m !== "ts" && m !== "cli") throw new HarnessError(`--mode ${m}: port | lower | shift | ts | cli`);
        a.mode = m;
        break;
      }
      case "--scope": {
        a.scope = value().split(",");
        const bad = a.scope.filter((s) => !(LANES as readonly string[]).includes(s));
        if (bad.length > 0) throw new HarnessError(`--scope ${bad.join(",")}: lanes are ${LANES.join(", ")}`);
        break;
      }
      case "--only": a.only = value(); break;
      case "--jobs": a.jobs = Number(value()); if (!Number.isSafeInteger(a.jobs) || a.jobs < 1) throw new HarnessError("--jobs takes a whole number from 1"); break;
      case "--shard": {
        const m = /^(\d+)\/(\d+)$/.exec(value());
        if (m === null || Number(m[1]) >= Number(m[2])) throw new HarnessError("--shard I/N with 0 <= I < N");
        a.shard = [Number(m[1]), Number(m[2])];
        break;
      }
      case "--baseline": a.baseline = value(); break;
      case "--run": a.run = value(); break;
      case "--reuse-ts": a.reuseTs = true; break;
      default: throw new HarnessError(`unknown flag ${flag}. Usage: bun tools/port/diff.ts [--mode port|lower|shift|ts] [--scope LANE] [--only TEXT] [--jobs N] [--shard I/N] [--baseline RUN] [--run NAME] [--reuse-ts]`);
    }
  }
  if (a.mode === "ts" && a.baseline === undefined) throw new HarnessError("--mode ts compares with a previous run: give --baseline out/port/<run>");
  if (a.mode !== "ts" && a.baseline !== undefined) throw new HarnessError("--baseline belongs to --mode ts");
  if (a.run === "") a.run = [a.mode, ...a.scope].join("-");
  return a;
}

async function spawn(cmd: readonly string[]): Promise<{ exit: number; signal: string | null; stdout: string; stderr: string }> {
  const p = Bun.spawn(["nice", "-n", "15", ...cmd], { cwd: ROOT, stdout: "pipe", stderr: "pipe", timeout: DEADLINE });
  const [stdout, stderr, exit] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  return { exit, signal: p.signalCode === null ? null : String(p.signalCode), stdout, stderr };
}

/** How one side ended. `refused` carries the CLI's exit code and the head of its last line (src/cli/verdict.ts). */
type Outcome =
  | { kind: "built" }
  | { kind: "refused"; code: number; head: string }
  /** The scene's own module refused (it does not load, construct or style): no lowering ran, on either side. */
  | { kind: "author"; code: number; head: string }
  | { kind: "failed"; why: string };

const tail = (s: string, n = 600) => s.trimEnd().slice(-n);

/** The TS side of an entry, into `dir` (tools/port/ts_side.ts in its own process: fresh Expr ids). */
async function tsSide(e: Entry, dir: string, shift: number): Promise<Outcome> {
  const r = await spawn([process.execPath, join(import.meta.dir, "ts_side.ts"), e.path, dir, ...(shift > 0 ? ["--shift", String(shift)] : [])]);
  const file = join(dir, "outcome.json");
  if (r.exit !== 0 || !existsSync(file)) return { kind: "failed", why: `ts side ${r.signal !== null ? `killed (${r.signal})` : `exit ${r.exit}`}: ${tail(r.stderr)}` };
  const o = JSON.parse(readFileSync(file, "utf8")) as { kind: "built" } | { kind: "refused"; stage: string; code: number; head: string } | { kind: "crashed"; message: string };
  switch (o.kind) {
    case "built": return o;
    case "refused": return { kind: o.stage === "author" ? "author" : "refused", code: o.code, head: o.head };
    case "crashed": return { kind: "failed", why: `the TS compiler crashed (an untyped throw, its own bug): ${tail(o.message, 300)}` };
  }
}

/** A TS side an earlier run wrote (--reuse-ts, --mode ts), read as tsSide reads its own. */
function recorded(dir: string): Outcome {
  const file = join(dir, "outcome.json");
  if (!existsSync(file)) return { kind: "failed", why: `${file} is missing` };
  const o = JSON.parse(readFileSync(file, "utf8")) as { kind: "built" } | { kind: "refused"; stage: string; code: number; head: string } | { kind: "crashed"; message: string };
  switch (o.kind) {
    case "built": return o;
    case "refused": return { kind: o.stage === "author" ? "author" : "refused", code: o.code, head: o.head };
    case "crashed": return { kind: "failed", why: `the TS compiler crashed (an untyped throw, its own bug): ${tail(o.message, 300)}` };
  }
}

/** The Rust side of an entry, into `dir`. A refusal's last stderr line is `loft: <kind>: <head>`. */
async function rustSide(mode: "port" | "lower", ts: string, dir: string): Promise<Outcome> {
  rmSync(dir, { recursive: true, force: true });
  mkdirSync(dir, { recursive: true });
  if (mode === "port" && !existsSync(join(ts, "input", "job.bin"))) {
    const refused = join(ts, "input", "refused.json");
    return { kind: "failed", why: `no input: ${existsSync(refused) ? (JSON.parse(readFileSync(refused, "utf8")) as { why: string }).why : "the TS side wrote no input/job.bin"} (lane W)` };
  }
  const args = mode === "port" ? ["compile", join(ts, "input")] : ["lower", join(ts, "compile.json"), join(ts, "input", "baked")];
  const r = await spawn([RUST, ...args, "--out", dir]);
  if (r.exit === 0) return { kind: "built" };
  if (r.signal !== null) return { kind: "failed", why: `loft-lower killed (${r.signal}): ${tail(r.stderr)}` };
  const last = r.stderr.trimEnd().split("\n").at(-1) ?? "";
  const m = /^loft: ([a-z]+): (.*)$/.exec(last);
  if (m === null) return { kind: "failed", why: `loft-lower exit ${r.exit} without a verdict line: ${tail(r.stderr)}` };
  return { kind: "refused", code: r.exit, head: m[2]! };
}

const spell = (o: Outcome) => {
  switch (o.kind) {
    case "built": return "built";
    case "refused": return `refused (exit ${o.code}): ${o.head}`;
    case "author": return `refused by the scene's module (exit ${o.code}): ${o.head}`;
    case "failed": return `failed: ${o.why}`;
  }
};

/** Every section of one side, in pipeline order; the outcome last. */
function sectionsOf(dir: string, o: Outcome, which: { probes: boolean; compile: boolean }): Section[] {
  const enc = new TextEncoder();
  const out: Section[] = [];
  if (which.probes) out.push(...probeSections(join(dir, "probe")));
  if (which.compile && existsSync(join(dir, "compile.json"))) out.push(...compileSections(join(dir, "compile.json")));
  out.push(...fileSections(join(dir, "artefact")));
  // Last: a Rust side that stopped early is reported by its first missing section, with its verdict as the reason.
  out.push({ name: "outcome", owner: "S", bytes: enc.encode(o.kind === "failed" ? "failed" : spell(o)) });
  return out;
}

/** Where two byte strings first differ: a line and both texts for UTF-8, a byte offset otherwise. */
function where(a: Uint8Array, b: Uint8Array, other: string): string {
  let i = 0;
  const n = Math.min(a.length, b.length);
  while (i < n && a[i] === b[i]) i++;
  if (i === n && a.length === b.length) return "equal";
  const text = (x: Uint8Array) => new TextDecoder("utf-8", { fatal: true }).decode(x);
  try {
    const ta = text(a), tb = text(b);
    const la = ta.split("\n"), lb = tb.split("\n");
    let k = 0;
    while (k < la.length && k < lb.length && la[k] === lb[k]) k++;
    const cut = (s: string | undefined) => s === undefined ? "(end)" : s.length > 160 ? `${s.slice(0, 160)}...` : s;
    return `line ${k + 1}: ts ${JSON.stringify(cut(la[k]))} | ${other} ${JSON.stringify(cut(lb[k]))} (byte ${i}; ${a.length} vs ${b.length} bytes)`;
  } catch {
    return `byte ${i} (${a.length} vs ${b.length} bytes)`;
  }
}

type Diff = { section: string; owner: string; what: string };
type Row = { id: string; path: string; result: "pass" | "fail"; ts: string; rust: string; reason: string; first?: Diff; differing: number; compared: number; ms: number };

/** The comparison of two sides' sections, in the TS side's order (sections only the other side has come last). */
function compare(ts: readonly Section[], rust: readonly Section[], scope: readonly string[], other: string): { first: Diff | undefined; differing: number; compared: number; byOwner: Record<string, number> } {
  const inScope = (s: Section) => scope.length === 0 || scope.includes(s.owner);
  const theirs = new Map(rust.map((s) => [s.name, s]));
  const ours = new Set(ts.map((s) => s.name));
  const diffs: Diff[] = [];
  let compared = 0;
  for (const s of ts.filter(inScope)) {
    compared++;
    const r = theirs.get(s.name);
    if (r === undefined) { diffs.push({ section: s.name, owner: s.owner, what: `absent on the ${other} side` }); continue; }
    const w = where(s.bytes, r.bytes, other);
    if (w !== "equal") diffs.push({ section: s.name, owner: s.owner, what: w });
  }
  for (const r of rust.filter(inScope)) if (!ours.has(r.name)) { compared++; diffs.push({ section: r.name, owner: r.owner, what: `absent on the ts side (the ${other} side wrote a section the TS compiler did not)` }); }
  const byOwner: Record<string, number> = {};
  for (const d of diffs) byOwner[d.owner] = (byOwner[d.owner] ?? 0) + 1;
  return { first: diffs[0], differing: diffs.length, compared, byOwner };
}

async function entry(a: Args, e: Entry, base: string): Promise<Row & { byOwner: Record<string, number> }> {
  const t = performance.now(), dir = join(base, "entries", e.id), tsDir = join(dir, "ts"), otherDir = join(dir, "rust");
  const reused = a.reuseTs && existsSync(join(tsDir, "outcome.json"));
  const ts = reused ? recorded(tsDir) : await tsSide(e, tsDir, 0);
  const row = (result: "pass" | "fail", other: Outcome | undefined, reason: string, c?: ReturnType<typeof compare>) => ({
    id: e.id, path: e.path, result, ts: spell(ts), rust: other === undefined ? "not run" : spell(other), reason, differing: c?.differing ?? 0, compared: c?.compared ?? 0,
    byOwner: c?.byOwner ?? {}, ms: Math.round(performance.now() - t), ...(c?.first !== undefined ? { first: c.first } : {}),
  });
  if (ts.kind === "failed") return row("fail", undefined, ts.why);
  // The scene's module refused before the compiler ran: TS keeps author code, so there is nothing to port.
  if (ts.kind === "author" && a.mode !== "shift" && a.mode !== "ts") return row("pass", undefined, "the scene's module refused before lowering");
  // A scene the TS compiler refuses never reaches lowering: the lower lanes have nothing to match.
  if (ts.kind === "refused" && a.mode === "lower") return row("pass", undefined, "the TS compiler refused it; lowering is not reached");
  let other: Outcome;
  let which = { probes: true, compile: true };
  switch (a.mode) {
    case "port": other = await rustSide("port", tsDir, otherDir); break;
    case "lower": other = await rustSide("lower", tsDir, otherDir); which = { probes: false, compile: false }; break;
    case "shift": other = await tsSide(e, otherDir, 1000); break;
    case "cli": {
      rmSync(otherDir, { recursive: true, force: true });
      const r = await spawn([process.execPath, "src/cli/main.ts", "build", e.path, "--out", join(otherDir, "artefact")]);
      const last = r.stderr.trimEnd().split("\n").at(-1) ?? "";
      other = r.exit === 0 ? { kind: "built" } : r.signal !== null ? { kind: "failed", why: `loft build killed (${r.signal})` } : { kind: "refused", code: r.exit, head: last.replace(/^loft: [a-z]+: /, "").replace(/ \(at [^()]+\)$/, "") };
      which = { probes: false, compile: false };
      break;
    }
    case "ts": {
      const prev = join(a.baseline!, "entries", e.id, "ts");
      if (!existsSync(join(prev, "outcome.json"))) return row("fail", undefined, `the baseline ${a.baseline} has no TS side for this entry`);
      other = recorded(prev);
      rmSync(otherDir, { recursive: true, force: true });
      break;
    }
  }
  const otherSide = a.mode === "ts" ? join(a.baseline!, "entries", e.id, "ts") : otherDir;
  const c = compare(sectionsOf(tsDir, ts, which), sectionsOf(otherSide, other, which), a.scope, OTHER[a.mode]);
  // Nothing in scope on either side (a lane's kind the entry does not use) is a pass: neither side wrote any of it.
  if (c.first === undefined) return row("pass", other, c.compared === 0 ? "no section in scope" : "", c);
  // A refusal that is not the TS compiler's, or a Rust side that stopped, is said first: it explains the sections.
  const reason = other.kind === "failed" ? other.why : other.kind !== "built" && ts.kind === "built" ? `${OTHER[a.mode]} refused: ${other.head}` : `${c.first.section} [${c.first.owner}]: ${c.first.what}`;
  return row("fail", other, reason, c);
}

async function main(a: Args) {
  const base = join(ROOT, "out/port", a.run);
  if ((a.mode === "port" || a.mode === "lower") && !existsSync(RUST)) throw new HarnessError(`${RUST} is missing. Fix: tools/dev.sh build loft-lower --fast`);
  const all = await corpusEntries(a.only, "out/port/seeds");
  const list = all.filter((_, i) => i % a.shard[1] === a.shard[0]);
  if (list.length === 0) throw new HarnessError(`no corpus entry matches --only ${JSON.stringify(a.only)} in shard ${a.shard.join("/")}`);
  mkdirSync(join(base, "entries"), { recursive: true });
  console.log(`port diff: ${a.mode}${a.scope.length > 0 ? ` scope ${a.scope.join(",")}` : ""}, ${list.length} entries, ${a.jobs} jobs -> ${base}`);
  const queue = [...list], rows: (Row & { byOwner: Record<string, number> })[] = [];
  const worker = async () => {
    for (let e = queue.shift(); e !== undefined; e = queue.shift()) {
      const r = await entry(a, e, base);
      rows.push(r);
      if (r.result === "fail") console.log(`FAIL ${r.id}: ${r.reason}`);
    }
  };
  await Promise.all(Array.from({ length: Math.min(a.jobs, list.length) }, worker));
  rows.sort((x, y) => x.id.localeCompare(y.id));
  const passed = rows.filter((r) => r.result === "pass").length;
  const filtered = a.only !== "" || a.shard[1] > 1;
  // Per lane: entries whose first differing section is that lane's, and differing sections in all.
  const lanes: Record<string, { first: number; sections: number }> = {};
  for (const r of rows) {
    if (r.first !== undefined) (lanes[r.first.owner] ??= { first: 0, sections: 0 }).first++;
    for (const [o, n] of Object.entries(r.byOwner)) (lanes[o] ??= { first: 0, sections: 0 }).sections += n;
  }
  const reasons: Record<string, number> = {};
  for (const r of rows.filter((r) => r.result === "fail")) { const k = r.first === undefined ? r.reason.replace(/:.*$/, "") : `${r.first.owner}: ${r.first.section.replace(/\.\d+.*$|\.[^.]*$/, "")}`; reasons[k] = (reasons[k] ?? 0) + 1; }
  const report = { mode: a.mode, scope: a.scope, only: a.only, shard: a.shard, filtered, entries: rows.length, passed, failed: rows.length - passed, lanes, reasons, rows: rows.map(({ byOwner: _, ...r }) => r) };
  writeFileSync(join(base, "report.json"), JSON.stringify(report, null, 1) + "\n");
  console.log(`\n${passed}/${rows.length} entries pass${filtered ? " (a filtered run: diagnosis only, it cannot pass the gate)" : ""}`);
  for (const [k, n] of Object.entries(reasons).sort((x, y) => y[1] - x[1])) console.log(`  ${String(n).padStart(4)}  ${k}`);
  console.log(`report: ${join(base, "report.json")}`);
  process.exit(passed === rows.length && !filtered ? 0 : 1);
}

try {
  await main(parse(Bun.argv.slice(2)));
} catch (e) {
  if (!(e instanceof HarnessError) && !(e instanceof CorpusError)) throw e;
  console.error(`port diff: ${e.message}`);
  process.exit(2);
}

// Run from the worktree root: bun agent_notes/2026-11-18_rust_core/corpus.ts (needs rust/loft-core built).
// Every examples/**/*.ts through `loft build` on both cores: verdict, bytes, CPU and peak RSS per arm.
import { readdirSync, readFileSync, rmSync, existsSync, appendFileSync, writeFileSync, mkdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { join } from "node:path";
const files = [...new Bun.Glob("examples/**/*.ts").scanSync({ cwd: "." })].sort();
const out = "out/rust-core/corpus";
const log = `${out}/rows.jsonl`;
mkdirSync(out, { recursive: true });
writeFileSync(log, "");
function digest(dir: string) {
  if (!existsSync(join(dir, "manifest.json"))) return "none";
  const h = createHash("sha256");
  const names = ["bin", "wgsl"].flatMap((d) => existsSync(join(dir, d)) ? readdirSync(join(dir, d)).sort().map((f) => `${d}/${f}`) : []);
  for (const f of ["manifest.json", ...names]) { const c = readFileSync(join(dir, f)); h.update(JSON.stringify([f, c.length])).update(c); }
  return h.digest("hex").slice(0, 16);
}
async function arm(file: string, core: "ts" | "rust", i: number) {
  const dir = `${out}/${core}-${i}`;
  rmSync(dir, { recursive: true, force: true });
  const p = Bun.spawn([process.execPath, "src/cli/main.ts", "build", file, "--out", dir], { env: { ...process.env, LOFT_NO_GPU: "1", LOFT_CORE: core }, stdout: "ignore", stderr: "pipe" });
  const [exit, err] = await Promise.all([p.exited, new Response(p.stderr).text()]);
  const u = p.resourceUsage()!;
  const lines = err.split("\n").filter((l) => l !== "");
  const refusals = lines.filter((l) => l.startsWith("loft core:"));
  const verdict = lines.filter((l) => !l.startsWith("loft core:") && !l.startsWith("note:")).at(-1) ?? "";
  const d = exit === 0 ? digest(dir) : "none";
  rmSync(dir, { recursive: true, force: true });
  return { exit, cpu: Math.round(Number(u.cpuTime.total) / 1000), rss: Math.round(u.maxRSS / 1024 / 1024), digest: d, verdict: exit === 0 ? "" : verdict.slice(0, 160), refusals };
}
let next = 0;
async function worker() {
  for (;;) {
    const i = next++;
    if (i >= files.length) return;
    const f = files[i]!;
    const ts = await arm(f, "ts", i), rs = await arm(f, "rust", i);
    const row = { file: f, same: ts.exit === rs.exit && ts.digest === rs.digest && ts.verdict === rs.verdict, ts, rs };
    appendFileSync(log, JSON.stringify(row) + "\n");
    console.log(`${row.same ? "same" : "DIFF"} ${ts.exit} ${f} ts ${ts.cpu}ms ${ts.rss}MiB rust ${rs.cpu}ms ${rs.rss}MiB ${rs.refusals.join("; ")}`);
  }
}
await Promise.all([worker(), worker(), worker()]);

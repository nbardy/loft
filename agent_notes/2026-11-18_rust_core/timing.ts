// Run from the worktree root: bun agent_notes/2026-11-18_rust_core/timing.ts [rounds]
// Median CPU (user+sys, whole process) and peak RSS of `loft build` per core, alternating arms.
import { rmSync } from "node:fs";
import { loadavg } from "node:os";
const scenes = [["owl", "examples/second/owl.ts"], ["titan", "examples/bosses/reefback_titan.ts"], ["film", "examples/materials/thin_film.ts"], ["storm", "examples/storm.ts"], ["leap", "examples/second/heron_leap.ts"]] as const;
const rounds = Number(process.argv[2] ?? 3);
const med = (xs: number[]) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)]!;
for (const [name, path] of scenes) {
  const r: Record<string, { cpu: number[]; rss: number[]; wall: number[]; exit: number }> = { ts: { cpu: [], rss: [], wall: [], exit: 0 }, rust: { cpu: [], rss: [], wall: [], exit: 0 } };
  for (let i = 0; i < rounds; i++) for (const core of i % 2 === 0 ? ["ts", "rust"] : ["rust", "ts"]) {
    const dir = `out/rust-core/timing-${core}`;
    rmSync(dir, { recursive: true, force: true });
    const t = performance.now();
    const p = Bun.spawn([process.execPath, "src/cli/main.ts", "build", path, "--out", dir], { env: { ...process.env, LOFT_NO_GPU: "1", LOFT_CORE: core }, stdout: "ignore", stderr: "ignore" });
    r[core]!.exit = await p.exited;
    r[core]!.wall.push(performance.now() - t);
    const u = p.resourceUsage()!;
    r[core]!.cpu.push(Number(u.cpuTime.total) / 1000);
    r[core]!.rss.push(u.maxRSS / 1024 / 1024);
  }
  const f = (c: string) => `${med(r[c]!.cpu).toFixed(0)} ms CPU, ${med(r[c]!.wall).toFixed(0)} ms wall, ${med(r[c]!.rss).toFixed(0)} MiB (exit ${r[c]!.exit})`;
  console.log(`${name.padEnd(6)} ts ${f("ts")} | rust ${f("rust")} | load ${loadavg()[0]!.toFixed(1)}`);
}

// Format timing and profile rows from the benchmark. Times are ms and RSS values are MiB.
import { join } from "node:path";

type Phase = { wall: number; cpu: number; high: number; growth: number };
type Row = { scene: string; source: string; round: number; wall: number; cpu: number; rss: number; digest?: string;
  verdict?: string; exit: number; profile?: string; bytes?: number; files?: number; load: number;
  phases?: Record<string, Phase>; serverCompile?: number };
type ProfileNode = { id: number; children?: number[]; callFrame: { functionName: string; url: string; lineNumber: number } };
type Profile = { nodes: ProfileNode[]; samples: number[]; timeDeltas: number[] };
const out = "out/compiler-speed";
const median = (values: number[]) => {
  const sorted = [...values].sort((a, b) => a - b);
  return (sorted[Math.floor((sorted.length - 1) / 2)]! + sorted[Math.floor(sorted.length / 2)]!) / 2;
};
const f = (n: number, places = 0) => n.toFixed(places);
const mib = (n: number) => n / 2 ** 20;
const read = async (label: string): Promise<Row[]> => (await Bun.file(join(out, label, "rows.jsonl")).text()).trim().split("\n").map((line) => JSON.parse(line));
const byScene = (rows: Row[]) => new Map([...new Set(rows.map((r) => r.scene))].map((s) => [s, rows.filter((r) => r.scene === s)]));

const [mode, ...labels] = process.argv.slice(2);
for (const label of labels) {
  const rows = await read(label);
  console.log(`\n${label} (${rows.length} rows, load ${f(Math.min(...rows.map((r) => r.load)), 1)}–${f(Math.max(...rows.map((r) => r.load)), 1)})`);
  for (const [scene, runs] of byScene(rows)) {
    switch (mode) {
      case "timers": {
        const names = ["Bun start", "import", "graph", "solve", "lower", "WGSL", "tables", "write", "CLI setup"];
        const phases = names.map((name) => {
          const entries = runs.flatMap((r) => r.phases?.[name] ?? []);
          return entries.length === 0 ? "—" : [
            f(median(entries.map((p) => p.wall))), f(median(entries.map((p) => p.cpu))),
            f(mib(median(entries.map((p) => p.high)))), f(mib(median(entries.map((p) => p.growth)))),
          ].join("/");
        });
        const total = [f(median(runs.map((r) => r.wall))), f(median(runs.map((r) => r.cpu))), f(mib(median(runs.map((r) => r.rss))))].join("/");
        console.log(`| ${[scene, ...phases, total].join(" | ")} |`);
        break;
      }
      case "paired": {
        const sources = [...new Set(runs.map((r) => r.source))];
        if (sources.length !== 2) throw new Error("Paired measurement needs two source trees");
        const a = runs.filter((r) => r.source === sources[0]);
        const b = a.map((r) => {
          const pair = runs.find((p) => p.source === sources[1] && p.round === r.round);
          if (pair === undefined) throw new Error(`${scene}, round ${r.round}: wait for both timing rows`);
          return pair;
        });
        const same = a.every((r, i) => r.exit === b[i]!.exit && r.digest === b[i]!.digest && r.verdict === b[i]!.verdict);
        const columns = [scene];
        for (const key of ["wall", "cpu", "rss"] as const) {
          const unit = key === "rss" ? mib : (n: number) => n;
          const relative = a.map((r, i) => (b[i]![key] / r[key] - 1) * 100);
          columns.push(`${f(unit(median(a.map((r) => r[key]))))} → ${f(unit(median(b.map((r) => r[key]))))} (${f(median(relative), 1)}%)`);
        }
        console.log(`| ${[...columns, String(same)].join(" | ")} |`);
        console.log(`  baseline ranges ${["wall", "cpu", "rss"].map((key) => {
          const values = a.map((r) => key === "rss" ? mib(r.rss) : r[key as "wall" | "cpu"]);
          return `${f(Math.min(...values))}–${f(Math.max(...values))}`;
        }).join(" / ")}`);
        if (!same) throw new Error(`${scene}: bytes or verdict changed`);
        break;
      }
      case "profiles": {
        let samples = 0, internSamples = 0, internWall = 0, knownWall = 0;
        const self = new Map<string, number>();
        for (const r of runs) {
          if (r.profile === undefined) throw new Error(`Missing profile for ${r.scene}`);
          const dir = label.replace("sample-", "profiles-");
          const profile = (await Bun.file(join(out, dir, r.profile)).json()) as Profile;
          const byId = new Map(profile.nodes.map((n) => [n.id, n]));
          const parents = new Map(profile.nodes.flatMap((n) => (n.children ?? []).map((child) => [child, n.id] as const)));
          const interner = new Set(["trieOf", "leafOf", "internGet", "make", "fresh", "constant"]);
          for (const [i, id] of profile.samples.entries()) {
            const node = byId.get(id)!;
            const frame = node.callFrame;
            const key = `${frame.url.split("/").slice(-2).join("/")}:${frame.functionName}:${frame.lineNumber + 1}`;
            self.set(key, (self.get(key) ?? 0) + 1);
            samples++;
            const dt = profile.timeDeltas[i]! / 1000;
            knownWall += dt;
            // Charge the interner's JS and native leaf operations. Stop at the first attributable JS frame, so a
            // constructor calling through another Loft subsystem does not pull that subsystem into the bucket.
            let ancestor: ProfileNode | undefined = node;
            while (ancestor && ancestor.callFrame.url === "" && !ancestor.callFrame.functionName.startsWith("(")) {
              ancestor = byId.get(parents.get(ancestor.id)!);
            }
            if (ancestor?.callFrame.url.endsWith("/src/expr.ts") && interner.has(ancestor.callFrame.functionName)) {
              internSamples++;
              internWall += dt;
            }
          }
        }
        console.log(`| ${scene} | ${internSamples}/${samples} | ${f(100 * internSamples / samples, 1)}% | ${f(internWall / runs.length)} | ${f(100 * internWall / knownWall, 1)}% |`);
        console.log([...self].sort((a, b) => b[1] - a[1]).slice(0, 12).map(([fn, count]) => `${fn} (${count})`).join(", "));
        break;
      }
      default: throw new Error("Usage: bun summary.ts <timers|paired|profiles> <benchmark label> ...");
    }
  }
}

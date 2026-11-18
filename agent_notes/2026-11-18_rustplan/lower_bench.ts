// Lower one in-memory Titan artefact with both implementations. No scene execution, GPU, or writes in the timer.
import { resolve } from "node:path";

const [baseArg, candidateArg] = process.argv.slice(2);
if (!baseArg || !candidateArg) throw new Error("Usage: bun lower_bench.ts <baseline source directory> <candidate source directory>");
const base = resolve(baseArg), candidate = resolve(candidateArg);
const { chosenAt } = await import(`${base}/src/cli/variant.ts`) as typeof import("../../src/cli/variant.ts");
const { compile } = await import(`${base}/src/compile/index.ts`) as typeof import("../../src/compile/index.ts");
const { scene } = await chosenAt(`${base}/examples/bosses/reefback_titan.ts`, { kind: "default" });
const artefact = compile(scene);
const versions = await Promise.all([base, candidate].map(async (dir) => {
  const { lower } = await import(`${dir}/src/host/lower.ts`) as typeof import("../../src/host/lower.ts");
  return { dir, lower };
}));

for (let round = -2; round < 6; round++) {
  const order = round % 2 === 0 ? versions : [...versions].reverse();
  for (const { dir, lower } of order) {
    Bun.gc(true);
    const t = performance.now(), start = process.cpuUsage();
    const result = lower(artefact);
    const usage = process.cpuUsage(start);
    const wall = performance.now() - t;
    if (round >= 0) console.log(JSON.stringify({ round, dir, wall, cpu: (usage.user + usage.system) / 1000, modules: result.manifest.modules.length }));
  }
}

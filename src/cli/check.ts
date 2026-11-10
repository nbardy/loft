// `loft check <scene.ts>`: the scene module's exported `checks` (src/check.ts), run on the CPU against its default
// export, printed as a table. Exits 1 when any check fails, so it can gate a build.
import { resolve } from "node:path";
import type { Command } from "./args.ts";

export async function check(cmd: Extract<Command, { kind: "check" }>): Promise<void> {
  const mod = (await import(resolve(cmd.path))) as { default?: unknown; checks?: unknown };
  if (mod.default === undefined) throw new Error(`${cmd.path} has no default export (it must export a scene)`);
  if (!Array.isArray(mod.checks)) throw new Error(`${cmd.path} exports no \`checks\` array`);
  const { runChecks, formatChecks } = await import("../check.ts");
  const t = performance.now();
  const verdicts = runChecks(mod.default as Parameters<typeof runChecks>[0], mod.checks);
  console.log(formatChecks(verdicts));
  const failed = verdicts.filter((v) => !v.pass).length;
  console.log(`\n${verdicts.length - failed} passed, ${failed} failed  (${(performance.now() - t).toFixed(0)} ms, including compile)`);
  if (failed > 0) process.exitCode = 1;
}

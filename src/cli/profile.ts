// `loft profile`: GPU milliseconds per draw (timestamp queries, best of N frames), totals per thing, the fixed stages,
// and the CPU pose pass per table. The numbers come from `GpuLoaded.profile` (src/host/profile.ts), whose header says
// how a stage's time is measured.
import { resolve } from "node:path";
import { compileScene } from "../compile/index.ts";
import { createHost } from "../host/host.ts";
import { headlessDevice } from "../host/headless.ts";
import { viewState } from "../host/host.ts";
import type { Scene } from "../scene.ts";
import type { Command } from "./args.ts";
import { resolveCamera } from "./cameras.ts";
import { styled } from "./load.ts";

/** The thing a draw belongs to: its label up to the first "_" or space (the library names parts `<thing>_<part>`). */
export const thingOf = (label: string): string => /^[^_ ]+/.exec(label)![0];

const f2 = (x: number) => x.toFixed(3).padStart(8);

export async function profile(cmd: Extract<Command, { kind: "profile" }>): Promise<void> {
  const mod = (await import(resolve(cmd.path))) as { default?: Scene };
  if (mod.default === undefined) throw new Error(`${cmd.path} has no default export (it must export a scene)`);
  const compiled = compileScene((await styled(mod.default, cmd.style)) as Scene);
  const a = compiled.artefact;
  const view = { camera: resolveCamera(a, cmd.view), ...cmd.size, time: cmd.time };
  const loaded = await createHost(await headlessDevice()).load(a);
  await loaded.frame(view); // warm up: first-use costs are not the draw's
  const p = await loaded.profile(view, cmd.frames);
  loaded.destroy();

  // CPU pose per table, best of the same number of frames.
  const tables = new Map<string, number>();
  for (let i = 0; i < cmd.frames; i++) {
    for (const [name, ms] of compiled.poseProfile(viewState(a, view))) tables.set(name, Math.min(tables.get(name) ?? Infinity, ms));
  }

  const out: string[] = [];
  out.push(`${cmd.path}  view ${cmd.view}  ${cmd.size.width}x${cmd.size.height}  t=${cmd.time}  best block mean over ${p.frames} frames  ${a.draws.length} draws declared`);
  out.push(`GPU total ${p.total.toFixed(2)} ms (profiling frame: one pass per stage)   pass overhead ${p.passOverhead.toFixed(3)} ms per pass (subtracted in "net")`);
  out.push("", "draw                              ms      net   triangles  instances  calls");
  for (const d of [...p.draws].sort((x, y) => y.net - x.net)) {
    out.push(`${d.label.padEnd(28).slice(0, 28)}${f2(d.ms)} ${f2(d.net)} ${String(d.triangles).padStart(11)} ${String(d.instances).padStart(10)} ${String(d.calls).padStart(6)}`);
  }
  const things = new Map<string, { net: number; triangles: number; draws: number; calls: number }>();
  for (const d of p.draws) {
    const t = things.get(thingOf(d.label)) ?? { net: 0, triangles: 0, draws: 0, calls: 0 };
    things.set(thingOf(d.label), { net: t.net + d.net, triangles: t.triangles + d.triangles, draws: t.draws + 1, calls: t.calls + d.calls });
  }
  out.push("", "thing                       net ms   triangles  draws  calls");
  for (const [name, t] of [...things].sort((x, y) => y[1].net - x[1].net)) {
    out.push(`${name.padEnd(24).slice(0, 24)}${f2(t.net)} ${String(t.triangles).padStart(11)} ${String(t.draws).padStart(6)} ${String(t.calls).padStart(6)}`);
  }
  out.push("", "fixed stage                      ms");
  for (const s of p.stages) out.push(`${s.label.padEnd(28)}${f2(s.ms)}`);
  const poseSum = [...tables.values()].reduce((x, y) => x + y, 0);
  out.push(`${"CPU pose and upload".padEnd(28)}${f2(p.pose)}`, "", `CPU pose per table (sum of bests ${poseSum.toFixed(3)} ms; top 12)`);
  for (const [name, ms] of [...tables].sort((x, y) => y[1] - x[1]).slice(0, 12)) out.push(`  ${name.padEnd(26)}${f2(ms)}`);
  const triangles = p.draws.reduce((n, d) => n + d.triangles, 0), calls = p.draws.reduce((n, d) => n + d.calls, 0);
  out.push("", `drawn: ${triangles} triangles in ${calls} draw calls (declared maximum ${a.stats.maxTriangles})`);
  console.log(out.join("\n"));
}

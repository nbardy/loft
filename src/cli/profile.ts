// `loft profile`: GPU milliseconds per draw (timestamp queries, best of N frames), totals per thing, the fixed stages,
// and the CPU pose pass per table. The numbers come from `GpuLoaded.profile` (src/host/profile.ts), whose header says
// how a stage's time is measured.
import { compileScene } from "../compile/index.ts";
import { createHost } from "../host/host.ts";
import { POSE_ONE_PASS } from "../host/profile.ts";
import { headlessDevice } from "../host/headless.ts";
import { viewState } from "../host/host.ts";
import type { Scene } from "../scene.ts";
import type { Command } from "./args.ts";
import { resolveCamera } from "./cameras.ts";
import { styled } from "./load.ts";
import { baked } from "./state.ts";
import { chosenAt } from "./variant.ts";

/** The thing a draw belongs to: its label up to the first "_" or space (the library names parts `<thing>_<part>`). */
export const thingOf = (label: string): string => /^[^_ ]+/.exec(label)![0];

const f2 = (x: number) => x.toFixed(3).padStart(8);

export async function profile(cmd: Extract<Command, { kind: "profile" }>): Promise<void> {
  const { scene } = await chosenAt(cmd.path, cmd.variant);
  const styledScene = (await styled(scene, cmd.style, cmd.light)) as Scene;
  const compiled = compileScene(cmd.outline === "scene" ? styledScene : { ...styledScene, outline: cmd.outline });
  const a = compiled.artefact;
  const view = { camera: resolveCamera(a, cmd.view), ...cmd.size, time: cmd.time };
  const loaded = await createHost(await headlessDevice(), { precision: cmd.precision }).load(a);
  await loaded.frame(view); // warm up: first-use costs are not the draw's
  const p = await loaded.profile(view, cmd.frames);
  const normal = await loaded.frameGpu(view, cmd.frames);
  // A checkpointed state's replay: the pose pass where a frame replays the most steps (k = K - 1) and the fewest
  // (k = 0, one step after the checkpoint's copy); the difference is K - 1 steps.
  const replays: string[] = [];
  for (const r of a.recurrences ?? []) {
    replays.push(baked(r));
    if (r.kept.kind !== "checkpoints") continue;
    const K = r.kept.every, at = (n: number) => ({ ...view, time: r.start + (n + 0.5) / r.rate });
    const most = await loaded.frameGpu(at(K - 1), cmd.frames), least = await loaded.frameGpu(at(K), cmd.frames);
    replays.push(`replay   ${r.name}  pose pass ${most.pose.toFixed(3)} ms at k = ${K - 1} (the most), ${least.pose.toFixed(3)} ms at k = 0: ${((most.pose - least.pose) / (K - 1) * 1000).toFixed(1)} us a step, ${(r.data.byteLength / 1e6).toFixed(1)} MB of checkpoints`);
  }
  loaded.destroy();

  // CPU pose per table, best of the same number of frames.
  const tables = new Map<string, number>();
  for (let i = 0; i < cmd.frames; i++) {
    for (const [name, ms] of compiled.poseProfile(viewState(a, view))) tables.set(name, Math.min(tables.get(name) ?? Infinity, ms));
  }

  const out: string[] = [];
  out.push(`${cmd.path}  view ${cmd.view}  ${cmd.size.width}x${cmd.size.height}  t=${cmd.time}  precision ${loaded.precision}  best block mean over ${p.frames} frames  ${a.draws.length} draws declared`);
  out.push(`normal frame: ${normal.ms.toFixed(2)} ms GPU (pose stages ${normal.pose.toFixed(3)}, draws and passes ${normal.draws.toFixed(2)}; least of ${cmd.frames} frames; timer tick ${normal.tick.toFixed(4)} ms as observed)`);
  out.push(`GPU total ${p.total.toFixed(2)} ms (profiling frame: one pass per stage, for attribution)   pass overhead ${p.passOverhead.toFixed(3)} ms per pass (subtracted in "net")`);
  out.push(...replays);
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
  const whole = p.poseStages.find((s) => s.label === POSE_ONE_PASS), levels = p.poseStages.filter((s) => s !== whole);
  out.push("", `pose stages (GPU): ${whole === undefined ? "none" : `${whole.ms.toFixed(3)} ms in one pass`}; each level in a pass of its own (under 0.065 ms is unresolved):`);
  for (const s of levels) out.push(`  ${f2(s.ms)}  ${s.label.length > 100 ? `${s.label.slice(0, 100)}...` : s.label}`);
  const kinds = { map: 0, scan: 0, cpu: 0, replay: 0 };
  for (const t of a.tables) kinds[t.fill.kind]++;
  out.push(`tables: ${kinds.map} map, ${kinds.scan} scan, ${kinds.cpu} cpu${kinds.replay > 0 ? `, ${kinds.replay} replayed` : ""}`);
  const poseSum = [...tables.values()].reduce((x, y) => x + y, 0);
  out.push(`${"CPU pose and upload".padEnd(28)}${f2(p.pose)}`, "", `cpu tables (filled on the CPU every frame; sum of bests ${poseSum.toFixed(3)} ms)`);
  for (const t of a.tables) {
    if (t.fill.kind !== "cpu") continue;
    out.push(`  ${t.name.padEnd(26)}${f2(tables.get(t.name) ?? 0)}  ${t.fill.why}`);
  }
  const triangles = p.draws.reduce((n, d) => n + d.triangles, 0), calls = p.draws.reduce((n, d) => n + d.calls, 0);
  out.push("", `drawn: ${triangles} triangles in ${calls} draw calls (declared maximum ${a.stats.maxTriangles})`);
  console.log(out.join("\n"));
}

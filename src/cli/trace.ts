import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { native, NativeError, UNLIMITED, type Verdict } from "../core/native.ts";
import { rec } from "../record/recorder.ts";
import { pack, Program } from "../record/wire.ts";
import type { Scene } from "../record/decls.ts";
import { LOFT_DRAW } from "./serve.ts";
import type { Shown } from "./show.ts";
import { UsageError } from "./args.ts";
import { ChecksFailed } from "./verdict.ts";

function withRecording<T>(scene: Scene, read: (dir: string) => T): T {
  mkdirSync("out", { recursive: true });
  const dir = mkdtempSync(resolve("out/recorded-"));
  try {
    native().run(pack(rec.job(Program.Scene, [], scene)),
      { nodes: UNLIMITED, tapeOps: UNLIMITED, boxes: UNLIMITED, gpuNanos: UNLIMITED },
      dir + "/scene.loft", { naga: "emit", precision: "f32" });
    return read(dir);
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}
function inspect(args: string[]) {
  const result = Bun.spawnSync([LOFT_DRAW, ...args], { maxBuffer: 64 * 1024 * 1024 });
  if (result.exitCode !== 0 && result.exitCode !== 5) {
    const kinds = ["internal", "internal", "usage", "load", "scene", "internal", "environment", "budget"] as const;
    throw new NativeError(kinds[result.exitCode] ?? "internal", result.exitCode, result.stderr.toString(), undefined);
  }
  return { lines: result.stdout.toString().trimEnd().split("\n").filter(Boolean), failed: result.exitCode === 5 };
}
export function recordedModules(scene: Scene): Shown[] {
  return withRecording(scene, dir => inspect(["show", dir]).lines.map(line => JSON.parse(line) as Shown));
}
export function checkRecorded(scene: Scene) {
  const groups = scene.observed;
  if (!groups?.length) throw new UsageError("this recording declares no observations: include observed sites from the presented rows");
  withRecording(scene, dir => {
    let failed = false;
    for (const group of groups) {
      const result = inspect(["trace", dir, String(group.step), JSON.stringify(group.sites)]);
      for (const line of result.lines) {
        const { label, observations, verdict: v } = JSON.parse(line) as { label: string; observations: number; verdict: Verdict };
        if (typeof v.scope !== "object" || !("Observed" in v.scope)) throw new Error("expected a GPU trace verdict");
        const scope = v.scope.Observed;
        const digest = (bytes: number[]) => Buffer.from(bytes).toString("hex").slice(0, 12);
        console.log(`${label}: ${v.truth}, margin ${v.margin.toPrecision(5)} at tick ${v.witness[0][1]} · ${observations}/${scope.ticks} samples · ${scope.by} · input ${digest(scope.inputs)} · law ${digest(scope.law)}`);
      }
      failed ||= result.failed;
    }
    if (failed) throw new ChecksFailed("a signed margin failed on the recorded GPU take");
  });
}

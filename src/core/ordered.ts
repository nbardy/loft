import { fetch } from "../expr.ts";
import { cameraOf } from "../draw/record.ts";
import { rec } from "../record/recorder.ts";
import { pack, Program } from "../record/wire.ts";
import type { OrderedScene } from "../render/record.ts";
import { native, UNLIMITED, type Build } from "./native.ts";

export function buildOrdered(scene: OrderedScene, file: string, options: Build) {
  const cameras = Object.entries(scene.cameras).map(([name, value]) => cameraOf(name, value));
  const job = rec.job(
    Program.Scene,
    (scene.tables ?? []).map((t) => fetch(t, 0, 0)),
    { cameras, passes: [], checks: [], solves: [], ...(scene.steps?.length ? { rowSteps: scene.steps } : {}) },
  );
  return native().run(
    pack(job, scene.record()),
    {
      nodes: UNLIMITED,
      tapeOps: UNLIMITED,
      boxes: UNLIMITED,
      gpuNanos: UNLIMITED,
    },
    file,
    options,
  ).scene!;
}

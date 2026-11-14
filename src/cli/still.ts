// The one place a command chooses between today's frame and a still (src/host/still.ts), by `--samples`.
import type { View } from "../artefact.ts";
import type { GpuLoaded } from "../host/host.ts";
import type { StillSpec } from "../host/still.ts";
import { UsageError } from "./args.ts";

export function drawFrame(loaded: GpuLoaded, view: View, still: StillSpec): ReturnType<GpuLoaded["frameTimed"]> {
  switch (still.kind) {
    case "frame": return loaded.frameTimed(view);
    case "still":
      if (still.aperture > 0 && view.camera.kind !== "perspective") throw new UsageError(`--aperture: the view's camera is ${view.camera.kind}; depth of field needs a perspective camera`);
      return loaded.stillTimed(view, still);
  }
}

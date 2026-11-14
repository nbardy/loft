// What a scene's baked state took, as the CLI prints it (`render`, `build`, `profile`). Imports no compiler module: the
// files path (`render --artefact`) loads it too.
import type { Recurrence } from "../artefact.ts";

/** What a recurrence's bake took (src/host/bake.ts), in one line. */
export function baked(r: Recurrence): string {
  const b = r.baked.done;
  const kept = r.kept.kind === "every" ? "every state" : `every ${r.kept.every}th state`;
  if (b === undefined) return `bake     ${r.name}: not baked`;
  const gpu = b.gpu.kind === "timed" ? ` (GPU ${b.gpu.ms.toFixed(1)} ms)` : "";
  return `bake     ${r.name}  ${b.steps} steps in ${b.ms.toFixed(1)} ms${gpu} + ${b.setup.toFixed(1)} ms setup, ${b.readback.toFixed(1)} ms readback; ${kept}, ${r.states} kept, ${(b.bytes / 1e6).toFixed(2)} MB`;
}

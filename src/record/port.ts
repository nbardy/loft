// The port job, written once the TS artefact and its bakes are complete. The old draw/record.ts path still
// owns the redesigned renderer's job; the port can record anything the TS compiler emitted or refused.
import { rec, Unrecorded } from "./recorder.ts";
import { printJob } from "./print.ts";
import { pack, Program } from "./wire.ts";
import type { Tape } from "./tape.ts";
import type { Expr } from "../expr.ts";
import type { Scene } from "../scene.ts";

export type PortInput =
  | { readonly kind: "job"; readonly bytes: Uint8Array; readonly print: string; readonly probes: readonly (readonly [string, string])[] }
  | { readonly kind: "refused"; readonly why: string };

export function portInput(scene: Scene, tape: Tape, roots: readonly Expr[], compiled: boolean): PortInput {
  try {
    // Keep the graph probes' watched roots. The typed requests and ordered effects are the lowerer's inputs.
    const job = rec.job(Program.Scene, roots, undefined, { scene, tape, compiled });
    return { kind: "job", bytes: pack(job), print: printJob(job), probes: tape.probes(scene) };
  } catch (e) {
    if (!(e instanceof Unrecorded)) throw e;
    return { kind: "refused", why: e.message };
  }
}

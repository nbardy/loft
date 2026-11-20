// What the TypeScript side hands the Rust lowering (agent_notes/2026-11-19_port/PLAN.md, "The cut"): the author's
// program after every author closure has run, as one wire job. Owned by lane W (the wire): it closes the gaps the plan
// lists, one decl at a time, and tools/port/diff.ts writes whatever this returns as input/job.bin.
//
// Today it is the wire-4 job `LOFT_ENGINE=rust loft build` writes (src/cli/main.ts buildRust): the roots the recorder
// watched while the TS compiler ran, and the redesigned path's scene decls. It does not yet carry what the TS
// draw modules consume (the plan's gap list), so the Rust side cannot lower from it alone.
import { pack, Program } from "./wire.ts";
import { printJob } from "./print.ts";
import { rec } from "./recorder.ts";
import { sceneDecls } from "../draw/record.ts";
import type { Artefact } from "../artefact.ts";
import type { Expr } from "../expr.ts";
import type { Scene } from "../scene.ts";

export type PortInput = { readonly kind: "job"; readonly bytes: Uint8Array; readonly print: string } | { readonly kind: "refused"; readonly why: string };

/** The job for one compiled scene; `roots` are what the recorder watched while the scene compiled. */
export function portInput(compiled: { readonly scene: Scene; readonly artefact: Artefact }, roots: readonly Expr[]): PortInput {
  const decls = sceneDecls(compiled.scene, compiled.artefact);
  if (decls.unported.length > 0) return { kind: "refused", why: `the wire cannot record: ${decls.unported.join("; ")}` };
  const job = rec.job(Program.Scene, roots, decls.scene);
  // `print`: the job's canonical print (src/record/print.ts; loft-ir print.rs prints the decoded job the same way).
  return { kind: "job", bytes: pack(job), print: printJob(job) };
}

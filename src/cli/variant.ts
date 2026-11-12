// Choosing a variant: the `--variant` text is classified once by args.ts (`VariantArg`), a scene module once here
// (`subjectAt`), and one function takes the two to the scene that is drawn or checked.
import { resolve } from "node:path";
import { BASE_ASK, subjectOf, type Subject } from "../design.ts";
import type { Check } from "../check.ts";
import type { Scene } from "../scene.ts";
import type { VariantArg } from "./args.ts";
import { LoadError } from "../errors.ts";
import { isTypedRefusal, siteOf } from "./verdict.ts";

export async function subjectAt(path: string): Promise<Subject> {
  let mod: Record<string, unknown>;
  try {
    mod = (await import(resolve(path))) as Record<string, unknown>;
  } catch (e) {
    // A typed refusal raised at the module's top level is the scene's verdict; anything else stopped the module
    // loading (a syntax or import error, a throw in the user's own code): src/cli/verdict.ts.
    if (await isTypedRefusal(e)) throw e;
    const site = e instanceof Error ? siteOf(e.stack) : undefined;
    throw new LoadError(`${path} failed to load: ${e instanceof Error ? e.message : String(e)}${site === undefined ? "" : ` (at ${site})`}`);
  }
  return subjectOf(mod, path);
}

/** A scene to draw or check, named for the reader: the variant's canonical label, or "scene" for a plain module. */
export type Chosen = {
  readonly label: string;
  readonly scene: Scene;
  readonly checks: { readonly kind: "exported"; readonly list: readonly Check[] } | { readonly kind: "absent" };
};

export function choose(subject: Subject, path: string, v: VariantArg): Chosen {
  switch (subject.kind) {
    case "scene":
      if (v.kind === "ask") throw new Error(`${path} exports a scene, not a design: it has no variants (asked for --variant ${v.text})`);
      return { label: "scene", scene: subject.scene, checks: subject.checks };
    case "design": {
      const r = subject.design.resolve(v.kind === "ask" ? v.ask : BASE_ASK);
      const b = subject.design.build(r);
      return { label: r.label, scene: b.scene, checks: { kind: "exported", list: b.checks } };
    }
  }
}

/** The scene at `path` in the chosen variant. */
export async function chosenAt(path: string, v: VariantArg): Promise<Chosen> {
  return choose(await subjectAt(path), path, v);
}

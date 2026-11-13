// Choosing a variant: the `--variant` text is classified once by args.ts (`VariantArg`), a scene module once here
// (`subjectAt`), and one function takes the two to the scene that is drawn or checked.
import { relative, resolve } from "node:path";
import { BASE_ASK, subjectOf, type Subject } from "../design.ts";
import type { Check } from "../check.ts";
import type { Scene } from "../scene.ts";
import { UsageError, type VariantArg } from "./args.ts";
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
    // Paths relative to where loft runs: the same message on every machine, and short enough to read.
    const said = (e instanceof Error ? e.message : String(e)).replaceAll(`${process.cwd()}/`, "").replace(/\.$/, "");
    throw new LoadError(`${path} failed to load: ${said}${site === undefined ? "" : ` (at ${site})`}${await importHint(said)}`);
  }
  return subjectOf(mod, path);
}

/** For an import of a name Loft does not export (Bun: "Export named 'x' not found"): the nearest export, as a fix. */
async function importHint(message: string): Promise<string> {
  const name = message.match(/Export named '([^']+)' not found in module '[^']*src\/index\.ts'/)?.[1];
  if (name === undefined) return "";
  const names = Object.keys(await import("../index.ts"));
  const near = names.map((n) => [n, distance(n.toLowerCase(), name.toLowerCase())] as const).sort((a, b) => a[1] - b[1])[0];
  const guess = near !== undefined && near[1] <= Math.max(2, name.length / 3) ? `import ${near[0]} (the nearest name), or ` : "";
  return `. Fix: ${guess}look the name up with \`loft api\` (every export) or \`loft api <name>\``;
}

/** Edit distance (insertions, deletions, substitutions). */
function distance(a: string, b: string): number {
  let row = Array.from({ length: b.length + 1 }, (_, j) => j);
  for (let i = 1; i <= a.length; i++) {
    const next = [i];
    for (let j = 1; j <= b.length; j++) next.push(Math.min(row[j]! + 1, next[j - 1]! + 1, row[j - 1]! + (a[i - 1] === b[j - 1] ? 0 : 1)));
    row = next;
  }
  return row[b.length]!;
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
      if (v.kind === "ask") throw new UsageError(`--variant ${v.text}: ${path} exports a scene, not a design: it has no variants (drop --variant)`);
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

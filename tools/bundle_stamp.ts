// What tools/bundle.ts bundles and how a bundle is known to be current: the list of src modules and a stamp of their
// paths, sizes and modification times (a checkout or an edit moves it; a false "stale" costs one rebuild, a false
// "current" would run old code, so the stamp errs toward stale).
import { readdirSync, statSync } from "node:fs";
import { join } from "node:path";

export const ROOT = join(import.meta.dir, "..");
export const SRC = join(ROOT, "src");
export const BUNDLE_DIR = join(ROOT, "out/lib");

/** Every library module, sorted: src/**\/*.ts without tests. */
export function sourceFiles(): string[] {
  return (readdirSync(SRC, { recursive: true }) as string[])
    .filter((f) => f.endsWith(".ts") && !f.endsWith(".test.ts") && !f.endsWith(".d.ts"))
    .sort()
    .map((f) => join(SRC, f));
}

export function sourceStamp(files: readonly string[]): string {
  let s = "";
  for (const f of files) { const st = statSync(f); s += `${f}\0${st.size}\0${st.mtimeMs}\n`; }
  return Bun.hash(s).toString(16);
}

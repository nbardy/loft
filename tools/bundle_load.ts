// Preload (`bun --preload ./tools/bundle_load.ts ...`): every import of a library module resolves to its namespace in
// the prebuilt bundle (tools/bundle.ts). A bundle older than src/ is refused with the command that rebuilds it, before
// any library module loads. The modules the bundle leaves out (a top-level await) load from source.
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { BUNDLE_DIR, SRC, sourceFiles, sourceStamp } from "./bundle_stamp.ts";

export class StaleBundle extends Error {
  override readonly name = "StaleBundle";
}

type Stamp = { readonly stamp: string; readonly modules: Readonly<Record<string, number>>; readonly fromSource: readonly string[] };
let stamp: Stamp;
try { stamp = JSON.parse(readFileSync(join(BUNDLE_DIR, "stamp.json"), "utf8")) as Stamp; } catch {
  throw new StaleBundle(`no library bundle in ${BUNDLE_DIR}: run bun tools/bundle.ts`);
}
if (stamp.stamp !== sourceStamp(sourceFiles())) throw new StaleBundle(`the library bundle in ${BUNDLE_DIR} is older than src/: run bun tools/bundle.ts`);

const bundle = require(join(BUNDLE_DIR, "loft.js")) as Record<string, () => Record<string, unknown>>;
const esc = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
const skip = stamp.fromSource.map(esc).join("|");
const filter = new RegExp(`^${esc(SRC)}/${skip === "" ? "" : `(?!(?:${skip})$)`}.*\\.ts$`);

Bun.plugin({
  name: "loft-bundle",
  setup(b) {
    b.onLoad({ filter }, ({ path }) => {
      const k = stamp.modules[path.slice(SRC.length + 1)];
      if (k === undefined) throw new StaleBundle(`${path} is not in the library bundle: run bun tools/bundle.ts`);
      return { exports: bundle[`m${k}`]!(), loader: "object" };
    });
  },
});

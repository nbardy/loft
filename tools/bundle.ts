// The library as one prebuilt Bun bundle (plan 2026-11-19 §8.1, "bytecode library 15 ms"): every src/**/*.ts module
// (tests excepted) compiled once into out/lib/loft.js with Bun's bytecode cache, each module's namespace exported as
// `m<k>()`, which initialises that module (and its imports) on first call, as an import from source would. A process that preloads tools/bundle_load.ts resolves every import of a src file (relative, absolute or
// through `loft`) to that module's namespace inside the one bundle: one instance per source file, so `instanceof Expr`
// and the module-level interning tables are shared exactly as from source. Nothing changes without the preload.
//
//   bun tools/bundle.ts [--no-bytecode]      write out/lib/loft.js (+ .jsc) and out/lib/stamp.json
//   bun --preload ./tools/bundle_load.ts X   run X against the bundle (refused, with the rebuild command, when stale)
//
// Bytecode needs CommonJS, which cannot hold a top-level await: such a module (the CLI's entry points, the GPU host's
// workers) is left out, found by building, and loads from source; its imports of library modules still resolve to the
// bundle. `import.meta.url`, `.dir`, `.path` and `.main` are rewritten to the source file's own values before bundling,
// so a module that finds files relative to itself (src/errors.ts trims sites by src/, src/core/native.ts finds the
// addon) behaves as it does from source.
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join, relative } from "node:path";
import { BUNDLE_DIR, ROOT, SRC, sourceFiles, sourceStamp } from "./bundle_stamp.ts";

const bytecode = !Bun.argv.includes("--no-bytecode");
const files = sourceFiles();
const meta = {
  name: "loft-import-meta",
  setup(b: Bun.PluginBuilder) {
    b.onLoad({ filter: /\/src\/.*\.ts$/ }, async ({ path }) => {
      const text = await Bun.file(path).text();
      if (!text.includes("import.meta")) return { contents: text, loader: "ts" as const };
      const dir = path.slice(0, path.lastIndexOf("/"));
      const contents = text
        .replaceAll("import.meta.url", JSON.stringify(Bun.pathToFileURL(path).href))
        .replaceAll("import.meta.dir", JSON.stringify(dir))
        .replaceAll("import.meta.path", JSON.stringify(path))
        .replaceAll("import.meta.main", "false");
      return { contents, loader: "ts" as const };
    });
  },
};

rmSync(BUNDLE_DIR, { recursive: true, force: true });
mkdirSync(BUNDLE_DIR, { recursive: true });
const entry = join(BUNDLE_DIR, "entry.ts");
const left = new Set<string>();
for (;;) {
  const kept = files.filter((f) => !left.has(f));
  writeFileSync(entry, kept.map((f) => `export const m${files.indexOf(f)} = () => require(${JSON.stringify(f)});`).join("\n") + "\n");
  const r = await Bun.build({
    entrypoints: [entry], outdir: BUNDLE_DIR, naming: "loft.js", target: "bun", format: "cjs", bytecode,
    packages: "external", minify: { syntax: true, whitespace: true, identifiers: false }, plugins: [meta], throw: false,
  });
  if (r.success) break;
  // Either the module whose await the CommonJS output met, or the one a lazy require reaches.
  const awaits = r.logs.flatMap((m) => {
    const dep = /transitive dependency "([^"]+)" contains a top-level await/.exec(m.message);
    if (dep !== null) return [join(ROOT, dep[1]!)];
    return /"await" can only be used/.test(m.message) && m.position?.file ? [m.position.file] : [];
  });
  const fresh = awaits.filter((f) => files.includes(f) && !left.has(f));
  if (fresh.length === 0) { for (const m of r.logs) console.error(m); process.exit(1); }
  for (const f of fresh) left.add(f);
}
const modules = Object.fromEntries(files.filter((f) => !left.has(f)).map((f) => [relative(SRC, f), files.indexOf(f)]));
writeFileSync(join(BUNDLE_DIR, "stamp.json"), JSON.stringify({ stamp: sourceStamp(files), bytecode, modules, fromSource: [...left].map((f) => relative(SRC, f)) }));
console.log(`bundle: ${Object.keys(modules).length} modules -> ${relative(ROOT, join(BUNDLE_DIR, "loft.js"))}${bytecode ? " (+ bytecode)" : ""}; from source: ${[...left].map((f) => relative(SRC, f)).join(", ") || "none"}`);

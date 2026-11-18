// `loft show <scene> [--thing name] [--variant v]`: the WGSL the GPU compiles for a scene, as modules a reader can
// follow (docs/proposals/2026-11-18_authoring_api.md section 5). Without `--thing`, one line per module; with it, each
// module whose label names the thing, whole: every chunk the host concatenates, so the text compiles as one module, with
// a comment where each chunk begins. Statements built from the scene's WGSL text end with their site
// (`/* examples/eel.ts:9 girth */`), and each line of a raw function with the author's line (`// examples/eel.ts:14`).
// Call-form statements carry no site: they were built without one.
//
// The modules are the f32 build's (`loft build`'s chunks, the same text but for the sites). Top-level imports are
// light: args.ts imports this file to parse, and the compiler loads only when the command runs.
import { UsageError, type VariantArg } from "./args.ts";
import { parseVariantAsk } from "../design.ts";
import type { AuthorLines } from "../artefact.ts";
import type { Bundle } from "../host/manifest.ts";

export type ShowCommand = { readonly kind: "show"; readonly path: string; readonly thing: string | undefined; readonly variant: VariantArg };

export function parseShowArgs(rest: readonly string[]): ShowCommand {
  const positional: string[] = [];
  const flags = new Map<string, string>();
  for (let i = 0; i < rest.length; i++) {
    const a = rest[i]!;
    if (!a.startsWith("--")) { positional.push(a); continue; }
    if (a !== "--thing" && a !== "--variant") throw new UsageError(`show: unknown flag ${a} (show takes --thing and --variant)`);
    const v = rest[++i];
    if (v === undefined || v.startsWith("--")) throw new UsageError(`flag ${a} needs a value`);
    if (flags.has(a)) throw new UsageError(`flag ${a} given twice`);
    flags.set(a, v);
  }
  if (positional.length !== 1) throw new UsageError(`show: needs one scene file (got ${positional.length})`);
  const variant = flags.get("--variant");
  return { kind: "show", path: positional[0]!, thing: flags.get("--thing"), variant: variant === undefined ? { kind: "default" } : { kind: "ask", ask: parseVariantAsk(variant), text: variant } };
}

/** One module as `show` prints it: its label, its text, and how many of its lines carry a site. */
export type Shown = { readonly label: string; readonly text: string; readonly lines: number; readonly sited: number };

/** A site note on a generated statement: `/* file:line fn *\/` (src/expr.ts `noteStatements`). */
const NOTE = /\/\* \S+:\d+ [^*]*\*\/\s*$/;

export async function show(cmd: ShowCommand): Promise<void> {
  const modules = await shownModules(cmd.path, cmd.variant);
  if (cmd.thing === undefined) {
    const width = Math.max(...modules.map((m) => m.label.length));
    console.log(`modules of ${cmd.path} (\`loft show ${cmd.path} --thing <name>\` prints those naming it, with sites):`);
    for (const m of modules) console.log(`  ${m.label.padEnd(width)}  ${String(m.lines).padStart(6)} lines  ${m.sited} sited`);
    return;
  }
  const picked = modules.filter((m) => names(m.label).includes(cmd.thing!));
  if (picked.length === 0) {
    const things = [...new Set(modules.flatMap((m) => names(m.label)))].sort();
    throw new UsageError(`show --thing "${cmd.thing}": no module names it (names in module labels: ${things.join(", ")})`);
  }
  for (const m of picked) console.log(`// ==== ${m.label}: ${m.lines} lines, ${m.sited} sited; one module as the GPU compiles it ====\n${m.text}`);
}

/** The identifiers in a module label (`shader "specks.grains"` names specks and grains). */
function names(label: string): string[] {
  return [...new Set(label.match(/[A-Za-z_][A-Za-z0-9_]*/g) ?? [])];
}

/** Every module of the scene's f32 build, in manifest order, annotated. */
export async function shownModules(path: string, variant: VariantArg): Promise<Shown[]> {
  const [{ noteStatements, rewrittenFrom }, { siteOfNode }, { loadArtefact }, { lower }] = await Promise.all([
    import("../expr.ts"), import("../text/index.ts"), import("./load.ts"), import("../host/lower.ts"),
  ]);
  // Before the scene is imported: drawables may emit their WGSL as they are constructed. A node the compiler rewrote
  // (a frame-uniform part hoisted into a table) has the site of the node it was made from.
  noteStatements((e) => siteOfNode(e) ?? siteOfNode(rewrittenFrom(e)));
  const artefact = await loadArtefact({ kind: "file", path, style: { kind: "scene" }, light: { kind: "scene" }, variant });
  return modulesOf(lower(artefact, "f32"));
}

/** Each module's chunks joined as the host joins them, with a comment where each chunk begins and author lines sited. */
export function modulesOf(b: Bundle): Shown[] {
  const m = b.manifest;
  return m.modules.map((mod) => {
    let text = "", sited = 0;
    mod.chunks.forEach((c, k) => {
      const name = m.chunks[c]!, spans = m.authored?.[name] ?? [];
      const own = b.chunks.get(name)!.split("\n");
      let notes = 0;
      const marked = own.map((line, n) => {
        const at = spans.find((s) => n >= s.from && n < s.from + s.lines);
        if (at === undefined && NOTE.test(line)) notes++;
        return at === undefined ? line : `${line}  // ${at.file}:${at.line + n - at.from}`;
      });
      sited += notes + spans.reduce((n, s) => n + s.lines, 0);
      // A chunk not ending in a newline would share its last line with the next chunk's first: the banner breaks the
      // line there (whitespace between two declarations, so the text still compiles).
      text += `${text === "" || text.endsWith("\n") ? "" : "\n"}// ---- chunk ${k + 1} of ${mod.chunks.length}: ${origin(spans, notes)} ----\n${marked.join("\n")}`;
    });
    return { label: mod.label, text, lines: text.split("\n").length, sited };
  });
}

/** Who wrote a chunk: the library alone, or partly the scene's text (statements lowered from it, raw functions in it). */
function origin(spans: readonly AuthorLines[], notes: number): string {
  const parts = [
    ...(notes > 0 ? [`${notes} statements lowered from the scene's text`] : []),
    ...new Set(spans.map((s) => `${s.what} from ${s.file}:${s.line}`)),
  ];
  // Call-form statements carry no site, so "no sites" is the library's text or the scene's call form.
  return parts.length === 0 ? "no sites (the library's text, or the scene's call form)" : parts.join("; ");
}

if (import.meta.main) {
  // Runnable before the verb is registered in args.ts/main.ts: `bun src/cli/show.ts <scene> [--thing name]`.
  const { exitWith, verdictOf } = await import("./verdict.ts");
  try { await show(parseShowArgs(process.argv.slice(2))); } catch (e) { exitWith(await verdictOf(e)); }
}

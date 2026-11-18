// `loft context`: what a model (or a person) needs in its context to write a Loft scene, in one print read whole before
// every scene, so every word is paid for each time. docs/context.md is the written part (the craft, condensed;
// failures; where to read more). Two parts are generated where that file's markers stand, so they follow the code:
// the names WGSL text has in scope (the lowerer's builtins and the prelude, src/text/) and the API map
// (src/cli/api.ts). No GPU: it reads text and three tables.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { apiMap } from "./api.ts";
import { BUILTINS, PRELUDE_STRUCTS } from "../text/lower.ts";
import { PRELUDE } from "../text/prelude.ts";

const ROOT = resolve(import.meta.dir, "../..");
const MAP = "<!-- the API map is printed here -->";
const TEXT = "<!-- the names in scope in WGSL text are printed here -->";

/** What graph text may call by name: the lowered builtins, the prelude's functions and structs. */
export function textNames(): string {
  const structs = [...PRELUDE_STRUCTS.values()].map((s) => `${s.name} { ${s.fields.map((f) => f.name).join(", ")} }`);
  return [
    `Builtins (WGSL's meaning): ${[...BUILTINS.keys()].join(", ")}; vec2..vec4 and f32() construct.`,
    `Prelude (the library's functions, as in the API map): ${[...PRELUDE.keys()].join(", ")}; structs ${structs.join(", ")}; gradient(h, p); time.`,
  ].join("\n");
}

export function context(): string {
  const text = readFileSync(resolve(ROOT, "docs/context.md"), "utf8");
  const clean = (s: string) => s.replace(/<!--[\s\S]*?-->\n*/g, "").trim();
  const parts = text.split(new RegExp(`${literal(TEXT)}|${literal(MAP)}`));
  // A marker missing or out of order is the doc's mistake: say which, rather than print a context without it.
  if (parts.length !== 3 || text.indexOf(TEXT) > text.indexOf(MAP)) throw new Error(`docs/context.md: needs the markers "${TEXT}" then "${MAP}", once each`);
  const [head, middle, tail] = parts.map(clean) as [string, string, string];
  return `${head}\n${textNames()}\n\n${middle}\n\n${apiMap()}\n\n${tail}\n`;
}

const literal = (s: string) => s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

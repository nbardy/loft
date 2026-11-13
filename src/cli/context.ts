// `loft context`: what a model (or a person) needs in its context to use Loft well, in one print to be read whole: the
// craft guide (docs/craft.md: how to make pictures and motion that read, with range), then the API in its compact
// form (src/cli/api.ts, generated from the source). No GPU, no compiler: it reads text.
import { readFileSync } from "node:fs";
import { resolve } from "node:path";
import { apiCompact } from "./api.ts";

const ROOT = resolve(import.meta.dir, "../..");

export function context(): string {
  const craft = readFileSync(resolve(ROOT, "docs/craft.md"), "utf8").replace(/<!--[\s\S]*?-->\n*/g, "").trim();
  return `${craft}\n\n${apiCompact()}`;
}

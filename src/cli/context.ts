// `loft context`: what a model (or a person) needs in its context to write a Loft scene, in one print read whole before
// every scene, so every word is paid for each time. docs/context.md is the written part (the craft, condensed;
// failures; where to read more) and the API map is generated (src/cli/api.ts) and printed where that file's marker
// stands. No GPU, no compiler: it reads text.
import { execFileSync } from "node:child_process";
import { existsSync, readFileSync } from "node:fs";
import { resolve } from "node:path";
import { apiMap } from "./api.ts";

const ROOT = resolve(import.meta.dir, "../..");
const MARKER = "<!-- the API map is printed here -->";

// `git archive` writes the commit's hash here (.gitattributes marks this file export-subst), so a copy without .git
// still knows its commit. In a checkout the placeholder stands unexpanded and git is asked instead.
const ARCHIVED = "$Format:%H$";

/**
 * The commit this copy of Loft is, as 12 hex digits: from the archive stamp, else from git (with `-dirty` when a
 * tracked file differs from it), else `unknown`. Git is asked only when ROOT itself holds `.git` (a directory in a
 * checkout, a file in a worktree): a bare copy unpacked inside some other repository must not report that one's commit.
 */
function commit(): string {
  if (!ARCHIVED.startsWith("$Format")) return ARCHIVED.slice(0, 12);
  if (!existsSync(resolve(ROOT, ".git"))) return "unknown";
  try {
    // `--exclude=*` leaves no tag to describe from, so `--always` prints the bare hash.
    return execFileSync("git", ["-C", ROOT, "describe", "--always", "--dirty", "--abbrev=12", "--exclude=*"], { encoding: "utf8", stdio: ["ignore", "pipe", "ignore"] }).trim();
  } catch {
    return "unknown";
  }
}

/** The first line of `loft context`: which Loft the text describes. `^Loft version (\S+), commit (\S+)$`. */
export function versionLine(): string {
  const { version } = JSON.parse(readFileSync(resolve(ROOT, "package.json"), "utf8")) as { version: string };
  return `Loft version ${version}, commit ${commit()}`;
}

export function context(): string {
  const text = readFileSync(resolve(ROOT, "docs/context.md"), "utf8");
  const [before, after] = text.split(MARKER);
  if (after === undefined) throw new Error(`docs/context.md: the marker "${MARKER}" is missing`);
  const clean = (s: string) => s.replace(/<!--[\s\S]*?-->\n*/g, "").trim();
  return `${versionLine()}\n\n${clean(before!)}\n\n${apiMap()}\n\n${clean(after)}\n`;
}

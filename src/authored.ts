// The WGSL the scene's author wrote, as the compiler places it verbatim into shaders: raw functions of `wgsl` modules
// (src/text/index.ts registers each). `lower` finds these texts in each chunk it writes and records where they sit
// (`Manifest.authored`), so a host can tell a GPU refusal of the author's line from one of the library's
// (docs/proposals/2026-11-18_authoring_api.md section 5).
//
// A registry rather than a field carried through every layer: shaders are assembled by concatenation in many places
// (draw code, the scene's shared library, tables), and a text found where it ended up needs none of them to track
// offsets. Raw texts are named by a hash of their body, so a text cannot match where it was not placed. A leaf module:
// scene code reaches it (through src/text), so it imports types only.
import type { AuthorLines } from "./artefact.ts";

/** One text: its first line is line `line` of `file`. */
export type AuthorText = { readonly text: string; readonly file: string; readonly line: number; readonly what: string };

const texts = new Map<string, AuthorText>();

/** Records a text the compiler may place into WGSL verbatim. The same text twice keeps its first place. */
export function authored(t: AuthorText): void {
  if (t.text.trim() !== "" && !texts.has(t.text)) texts.set(t.text, t);
}

/** The texts in registration order, for recording them before the lowerer searches the shader chunks. */
export function authoredTexts(): Iterable<AuthorText> { return texts.values(); }

/** Where registered author texts sit in `code`, by line from 0, in order. Empty for text the library wrote alone. */
export function authorLines(code: string): AuthorLines[] {
  const out: AuthorLines[] = [];
  for (const t of texts.values()) {
    for (let at = code.indexOf(t.text); at >= 0; at = code.indexOf(t.text, at + t.text.length)) {
      out.push({ from: lineOf(code, at), lines: t.text.split("\n").length, file: t.file, line: t.line, what: t.what });
    }
  }
  return out.sort((a, b) => a.from - b.from);
}

function lineOf(code: string, offset: number): number {
  let n = 0;
  for (let i = code.indexOf("\n"); i >= 0 && i < offset; i = code.indexOf("\n", i + 1)) n++;
  return n;
}

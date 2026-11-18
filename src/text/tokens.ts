// WGSL text, tokenized. A module's text is the template's strings joined with a placeholder identifier per hole
// (`loft_hole_<k>`); authors cannot write a `loft_` identifier (refused in src/text/index.ts), so a placeholder is
// always a hole.

export type TokKind = "ident" | "num" | "punct";
export type Tok = {
  readonly kind: TokKind;
  readonly s: string;
  /** 0-based line within the module text, and 0-based column. */
  readonly line: number;
  readonly col: number;
  /** Offsets into the module text: s === text.slice(start, end). */
  readonly start: number;
  readonly end: number;
};

/** Longest first. `>>` is split by the type parser where a template closes two levels (`vec3<vec2<f32>>` is not WGSL Loft reads, but the split is cheap). */
const PUNCT = [
  "<<=", ">>=", "->", "&&", "||", "==", "!=", "<=", ">=", "+=", "-=", "*=", "/=", "%=", "&=", "|=", "^=", "<<", ">>", "++", "--",
  "(", ")", "{", "}", "[", "]", "<", ">", ",", ";", ":", ".", "=", "+", "-", "*", "/", "%", "!", "&", "|", "^", "~", "@",
];

export const HOLE_PREFIX = "loft_hole_";

export class TokenError extends Error {
  constructor(readonly line: number, readonly col: number, readonly what: string) { super(what); }
}

const isIdStart = (c: string): boolean => /[A-Za-z_]/.test(c);
const isId = (c: string): boolean => /[A-Za-z0-9_]/.test(c);
const isDigit = (c: string): boolean => c >= "0" && c <= "9";

export function tokenize(text: string): Tok[] {
  const out: Tok[] = [];
  let i = 0, line = 0, lineStart = 0;
  const n = text.length;
  while (i < n) {
    const c = text[i]!;
    if (c === "\n") { i++; line++; lineStart = i; continue; }
    if (c === " " || c === "\t" || c === "\r") { i++; continue; }
    if (c === "/" && text[i + 1] === "/") { while (i < n && text[i] !== "\n") i++; continue; }
    if (c === "/" && text[i + 1] === "*") {
      // WGSL block comments nest.
      let depth = 0;
      const l0 = line, c0 = i - lineStart;
      do {
        if (i >= n) throw new TokenError(l0, c0, "a block comment that never closes");
        if (text[i] === "/" && text[i + 1] === "*") { depth++; i += 2; continue; }
        if (text[i] === "*" && text[i + 1] === "/") { depth--; i += 2; continue; }
        if (text[i] === "\n") { line++; lineStart = i + 1; }
        i++;
      } while (depth > 0);
      continue;
    }
    const start = i, col = i - lineStart;
    if (isIdStart(c)) {
      while (i < n && isId(text[i]!)) i++;
      out.push({ kind: "ident", s: text.slice(start, i), line, col, start, end: i });
      continue;
    }
    if (isDigit(c) || (c === "." && isDigit(text[i + 1] ?? ""))) {
      if (c === "0" && (text[i + 1] === "x" || text[i + 1] === "X")) {
        i += 2;
        while (i < n && /[0-9A-Fa-f.pP]/.test(text[i]!)) { if ((text[i] === "p" || text[i] === "P") && /[+-]/.test(text[i + 1] ?? "")) i++; i++; }
      } else {
        while (i < n && isDigit(text[i]!)) i++;
        if (text[i] === ".") { i++; while (i < n && isDigit(text[i]!)) i++; }
        if ((text[i] === "e" || text[i] === "E") && (isDigit(text[i + 1] ?? "") || (/[+-]/.test(text[i + 1] ?? "") && isDigit(text[i + 2] ?? "")))) {
          i += 2;
          while (i < n && isDigit(text[i]!)) i++;
        }
      }
      if (i < n && /[fhiu]/.test(text[i]!)) i++;
      out.push({ kind: "num", s: text.slice(start, i), line, col, start, end: i });
      continue;
    }
    const p = PUNCT.find((q) => text.startsWith(q, i));
    if (p === undefined) throw new TokenError(line, col, `the character ${JSON.stringify(c)}, which WGSL does not use`);
    i += p.length;
    out.push({ kind: "punct", s: p, line, col, start, end: i });
  }
  return out;
}

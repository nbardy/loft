// How a command ends: every failure classified once, here, into one of a closed list of verdicts, and each verdict
// mapped to one exit code and one last line on stderr that a script can match (`loft: <kind>: ...`). README.md
// "Exit codes" is the table.
import { EnvironmentError, LoadError, SceneError, siteOf } from "../errors.ts";
import { DesignError } from "../design.ts";
import { UsageError, USAGE } from "./args.ts";
import { NativeError } from "../core/native.ts";

export type Verdict =
  /** The command line asked for something wrong or absent (a flag, a camera, a variant, an input, a style). */
  | { readonly kind: "usage"; readonly message: string }
  /** The scene module did not load as a scene: a syntax or import error, a throw at its top level, the wrong exports. */
  | { readonly kind: "load"; readonly message: string }
  /** The scene was refused by a constructor or the compiler with a typed error: the scene must change. */
  | { readonly kind: "scene"; readonly message: string; readonly site: string | undefined }
  /** The scene drew or built, and one of its checks failed. */
  | { readonly kind: "check"; readonly message: string }
  /** Not the scene: the GPU, the WebGPU binding, a missing tool. Already retried where retrying can help. */
  | { readonly kind: "environment"; readonly message: string }
  /**
   * A stated limit ran out before an answer: a deadline, a CPU limit, a full queue. The message names the limit. Not a
   * verdict on the scene either way; the caller decides whether running out is a penalty.
   */
  | { readonly kind: "budget"; readonly message: string }
  /** An untyped error inside Loft: a bug in Loft, or a scene mistake no constructor catches yet. */
  | { readonly kind: "internal"; readonly message: string; readonly stack: string };

/** `loft check` found failing checks (and every variant built). */
export class ChecksFailed extends Error {
  override readonly name = "ChecksFailed";
}

/**
 * The author's line a scene error names itself, where no stack frame can: the GPU refused the author's WGSL, raised by
 * the host (src/host/host.ts `ShaderTextError`), whose stack is all Loft's.
 */
async function authorSite(e: SceneError): Promise<string | undefined> {
  const { ShaderTextError } = await import("../host/host.ts");
  return e instanceof ShaderTextError ? e.site : undefined;
}

/** A refusal the library raised on purpose, with a message meant for the scene's writer. */
export const isTypedRefusal = (e: unknown): boolean => e instanceof SceneError || e instanceof DesignError || e instanceof LoadError;

/** The boundary: whatever was thrown, as a verdict. */
export async function verdictOf(e: unknown): Promise<Verdict> {
  if (!(e instanceof Error)) return { kind: "internal", message: String(e), stack: "" };
  if (e instanceof UsageError) return { kind: "usage", message: e.message };
  if (e instanceof DesignError) {
    // Asking for a variant, slot or option the design lacks is the command's mistake; a design that contradicts
    // itself is the scene's.
    switch (e.kind) {
      case "syntax": case "unknown-variant": case "unknown-slot": case "unknown-option": return { kind: "usage", message: e.message };
      case "unknown-number": case "conflict": return { kind: "scene", message: e.message, site: siteOf(e.stack) };
    }
  }
  if (e instanceof LoadError) return { kind: "load", message: e.message };
  // The Rust engine already classified its refusal (rust/crates/loft-ir Error::kind); its exit codes equal CODE's.
  if (e instanceof NativeError) {
    switch (e.kind) {
      case "scene": return { kind: "scene", message: e.message, site: undefined };
      case "internal": return { kind: "internal", message: e.message, stack: e.stack ?? "" };
      case "usage": case "load": case "budget": case "environment": return { kind: e.kind, message: e.message };
    }
  }
  if (e instanceof SceneError) return { kind: "scene", message: e.message, site: await authorSite(e) ?? siteOf(e.stack) };
  if (e instanceof EnvironmentError) return { kind: "environment", message: e.message };
  if (e instanceof ChecksFailed) return { kind: "check", message: e.message };
  return { kind: "internal", message: `${e.name === "Error" ? "" : `${e.name}: `}${e.message}`, stack: e.stack ?? "" };
}

const CODE: Readonly<Record<Verdict["kind"], number>> = { internal: 1, usage: 2, load: 3, scene: 4, check: 5, environment: 6, budget: 7 };
const LINE: Readonly<Record<Verdict["kind"], string>> = {
  internal: "internal (a bug in Loft, or a scene mistake no constructor catches yet)",
  usage: "usage",
  load: "load",
  scene: "scene",
  check: "check",
  environment: "environment (not the scene's fault)",
  budget: "budget",
};

/**
 * A message as its last line and the rest. A first line ending in ":" only introduces its reason (the GPU's
 * "pipeline creation failed:" then Dawn's text), so the reason is pulled up onto it: the last line must say why.
 */
export function headed(message: string): [string, ...string[]] {
  const [first, ...rest] = message.split("\n");
  const k = rest.findIndex((l) => l.trim() !== "");
  if (!first!.trimEnd().endsWith(":") || k < 0) return [first!, ...rest];
  return [`${first!.trimEnd()} ${rest[k]!.trim()}`, ...rest.slice(k + 1)];
}

/** A verdict as a command ends with it: the exit code, the last line (`loft: <kind>: ...`), and the lines above it. */
export function ending(v: Verdict): { readonly code: number; readonly line: string; readonly head: string; readonly detail: readonly string[] } {
  const [head, ...rest] = headed(v.message);
  const site = v.kind === "scene" ? v.site : undefined;
  if (site !== undefined) rest.push(`  at ${site}`);
  return { code: CODE[v.kind], line: `loft: ${LINE[v.kind]}: ${head}${site !== undefined ? ` (at ${site})` : ""}`, head, detail: rest };
}

/** Prints the verdict (the detail, then one last line) and exits with its code. */
export function exitWith(v: Verdict): never {
  const e = ending(v);
  switch (v.kind) {
    case "usage": console.error(USAGE); break;
    case "internal": console.error(v.stack); break;
    case "scene": case "load": case "check": case "environment": case "budget": break;
  }
  if (e.detail.length > 0) console.error(`${e.head}\n${e.detail.join("\n")}`);
  console.error(e.line);
  process.exit(e.code);
}

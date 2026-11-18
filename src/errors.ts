// The kinds of failure a caller must tell apart, as types: every refusal that is the scene's fault is a SceneError
// (exit 4), whatever constructor raised it. A leaf module: it imports nothing of Loft's, so any
// constructor can raise these without an import cycle. The CLI maps them to exit codes in src/cli/verdict.ts.

/**
 * A mistake in the scene as written, raised by the constructor where it was made: what was given, what was expected,
 * and the smallest change that fixes it, with the user's own thing named in `where` (`draw.surface "ball"`).
 */
export class SceneError extends Error {
  override readonly name: string = "SceneError";
  // Written either as its four parts or, by a constructor whose message already names the value and the fix, whole.
  constructor(...said: [message: string] | [where: string, given: string, expected: string, fix: string]) {
    super(said.length === 1 ? said[0] : `${said[0]}: given ${said[1]}; expected ${said[2]}. Fix: ${said[3]}`);
  }
}

/** The scene module could not be loaded as a scene: it failed to import, or exports the wrong shape. */
export class LoadError extends Error {
  override readonly name = "LoadError";
}

/**
 * Not the scene's fault: the GPU, the WebGPU binding, or a tool the command needs (cwebp, ffmpeg). `transient` says
 * whether trying again on a fresh device can help (a shader or pipeline that failed to build; not a missing tool).
 */
export class EnvironmentError extends Error {
  override readonly name = "EnvironmentError";
  constructor(message: string, readonly transient: boolean) {
    super(message);
  }
}

/** A short, one-line rendering of a value a user passed, for "given ..." (strings quoted, arrays bracketed). */
export function shown(x: unknown): string {
  if (x === undefined) return "undefined";
  if (typeof x === "string") return JSON.stringify(x);
  if (Array.isArray(x)) return `[${x.map(shown).join(", ")}]`;
  if (typeof x === "function") return "a function";
  if (typeof x === "object" && x !== null) return "an object";
  return String(x);
}

const SRC = new URL(".", import.meta.url).pathname;
// The library's root (the parent of src/). A site inside it prints relative to it; any other prints absolute. Pure
// string work, not node:path and process.cwd(): this module is reachable from scene code, which creative's confined
// compile admits only when it touches no host module or global.
const ROOT = new URL("..", import.meta.url).pathname;

/**
 * The first stack frame outside Loft's own source: the line of the user's file a constructor was called from
 * (`examples/leap.ts:42`), or undefined when every frame is Loft's (a call from inside the library).
 */
export function siteOf(stack: string | undefined): string | undefined {
  return sitesOf(stack)[0];
}

/** Every stack frame outside Loft's own source, innermost first: the author's line, then the lines that called it. */
export function sitesOf(stack: string | undefined): string[] {
  const out: string[] = [];
  for (const line of (stack ?? "").split("\n").slice(1)) {
    const m = line.match(/\(?((?:\/|[A-Za-z]:)[^()]*?):(\d+):(\d+)\)?\s*$/);
    if (m && !m[1]!.startsWith(SRC) && !m[1]!.includes("/node_modules/")) out.push(`${m[1]!.startsWith(ROOT) ? m[1]!.slice(ROOT.length) : m[1]!}:${m[2]}`);
  }
  return out;
}

/** A CPU tool reached a resource whose contents come from the author's WGSL. */
export class RawComputeError extends Error {
  override readonly name = "RawComputeError";
  constructor(readonly resource: string) { super(`undecided: raw compute "${resource}"`); }
}

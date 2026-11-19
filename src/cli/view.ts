// loft-view (rust/crates/loft-loader), Loft's default executor: the client of `loft-view serve`. `loft serve`,
// `render`, `frames` and `clip` draw through it unless LOFT_ENGINE says otherwise (src/cli/args.ts `Engine`). It reads
// the artefact folders the TypeScript compiler writes (docs/artefact_format.md); nothing that crosses the pipe is code.
//
// The child's lines (rust/crates/loft-loader/src/bin/loft-view/serve.rs): `ready` once, then one reply per request, in
// order. Requests go one at a time. Frame k of a request is written to `out/k.rgba`.
import { existsSync } from "node:fs";
import { fileURLToPath } from "node:url";
import type { Camera } from "../artefact.ts";
import { EnvironmentError, LoadError } from "../errors.ts";
import type { Bundle, FramePrecision, Precision } from "../host/manifest.ts";
import type { StillSpec } from "../host/still.ts";
import { canonical, type Identity } from "../identity.ts";
import { UsageError } from "./args.ts";
import type { Verdict } from "./verdict.ts";

/** Where `tools/dev.sh build loft-loader --fast` puts loft-view, and the protocol version this checkout speaks. */
export const LOFT_VIEW = fileURLToPath(new URL("../../rust/target/lab/loft-view", import.meta.url));
const VIEW_WIRE = 1;
const FIX = "tools/dev.sh build loft-loader --fast";

/** One request: everything resolved (the camera a Camera, inputs as lists), as the child takes it. */
export type ViewAsk = {
  readonly dir: string; readonly camera: Camera; readonly times: readonly number[]; readonly size: readonly [number, number];
  readonly params: Readonly<Record<string, number>>; readonly inputs: Readonly<Record<string, readonly number[]>>;
  readonly precision: Precision; readonly construction: boolean; readonly still: StillSpec; readonly out: string;
};
export type ViewFrame = { readonly file: string; readonly sha256: string; readonly triangles: number; readonly draws: number };
export type ViewReply =
  | { readonly ok: true; readonly precision: FramePrecision; readonly frames: readonly ViewFrame[]; readonly ms: { readonly load: number; readonly frames: number } }
  | { readonly ok: false; readonly verdict: Exclude<Verdict, { kind: "internal" | "scene" | "check" }> | { readonly kind: "internal"; readonly message: string } };
export type ViewReady = {
  readonly wire: number;
  readonly adapter: { readonly vendor: string; readonly architecture: string; readonly device: string; readonly description: string } | null;
  readonly device: Identity["device"];
};

export type ViewChild = {
  /** The child's `ready` line, or the reason it never came (a missing or stale binary, an exit). */
  readonly ready: Promise<ViewReady>;
  draw(ask: ViewAsk): Promise<ViewReply>;
  kill(): Promise<void>;
};

/** A reply's verdict as the CLI's own (src/cli/verdict.ts): the child classified it already. */
export function verdictOfView(v: Extract<ViewReply, { ok: false }>["verdict"]): Verdict {
  return v.kind === "internal" ? { kind: "internal", message: v.message, stack: "" } : v;
}

/**
 * A refusal of a built scene as the Dawn path classifies it: a shader module the GPU refused at a line the author wrote
 * (Naga names `wgsl:line:column` within the module) is the scene's, at that line (src/host/host.ts `ShaderTextError`);
 * one at a line the library wrote is the environment's (WGSL Loft wrote that the GPU refused). Anything else is
 * `errorOfView`'s.
 */
export async function refusalOfView(bundle: Bundle, v: Extract<ViewReply, { ok: false }>["verdict"]): Promise<Error> {
  const head = /^(.+) failed to compile:/.exec(v.message), at = /wgsl:(\d+):\d+/.exec(v.message);
  const mod = head === null ? undefined : bundle.manifest.modules.find((m) => m.label === head[1]);
  if (v.kind !== "load" || mod === undefined) return errorOfView(v);
  const error = /(?:parsing|validation) error: ([^\n]*)/i.exec(v.message)?.[1] ?? "refused";
  const { authorRefusalAt } = await import("../host/host.ts");
  return (at === null ? undefined : authorRefusalAt(bundle, mod, Number(at[1]), error, v.message))
    ?? new EnvironmentError(`loft-view: the GPU refused WGSL Loft wrote: ${v.message}`, false);
}

/** The same verdict as an error a command throws, for `verdictOf` to classify back into it. */
export function errorOfView(v: Extract<ViewReply, { ok: false }>["verdict"]): Error {
  switch (v.kind) {
    case "usage": return new UsageError(`loft-view: ${v.message}`);
    case "load": return new LoadError(`loft-view: ${v.message}`);
    case "environment": return new EnvironmentError(`loft-view: ${v.message}`, false);
    case "budget": case "internal": return new Error(`loft-view (${v.kind}): ${v.message}`);
  }
}

/**
 * Starts `loft-view serve`. `ended` is told when the child exits on its own (a device loss is exit 6), with a message
 * naming how; a `kill()` says nothing. Draws queue: each waits for the reply before it.
 */
export function startView(ended: (message: string) => void): ViewChild {
  if (!existsSync(LOFT_VIEW)) {
    const missing = Promise.reject(new EnvironmentError(`${LOFT_VIEW} is missing. Fix: ${FIX}`, false));
    missing.catch(() => {});
    return { ready: missing, draw: () => missing as Promise<never>, kill: async () => {} };
  }
  let killed = false;
  const child = Bun.spawn([LOFT_VIEW, "serve"], {
    stdin: "pipe", stdout: "pipe", stderr: "inherit",
    onExit(c, code, signal) { if (!killed) ended(`loft-view ${c.pid} ended (${signal ?? `exit ${code}`})`); },
  });
  const reader = child.stdout.getReader(), decoder = new TextDecoder();
  let buffered = "";
  const line = async <T,>(): Promise<T> => {
    for (let at = buffered.indexOf("\n"); at < 0; at = buffered.indexOf("\n")) {
      const { value, done } = await reader.read();
      if (done) throw new EnvironmentError(`loft-view ${child.pid} closed its output${killed ? "" : " (it ended)"}`, false);
      buffered += decoder.decode(value, { stream: true });
    }
    const at = buffered.indexOf("\n"), head = buffered.slice(0, at);
    buffered = buffered.slice(at + 1);
    return JSON.parse(head) as T;
  };
  const ready = line<ViewReady>().then((r) => {
    if (r.wire !== VIEW_WIRE) throw new EnvironmentError(`${LOFT_VIEW} speaks wire ${r.wire}, this checkout ${VIEW_WIRE}: it is stale. Fix: ${FIX}`, false);
    return r;
  });
  ready.catch(() => {});
  let turn: Promise<unknown> = ready;
  return {
    ready,
    draw(ask) {
      const still = ask.still.kind === "frame" ? null : { samples: ask.still.samples, shutter: ask.still.shutter, aperture: ask.still.aperture };
      const mine = turn.then(() => {
        // Canonical JSON keeps -0 (a time or input of -0 is drawn as given).
        child.stdin.write(canonical({ ...ask, still }) + "\n");
        child.stdin.flush();
        return line<ViewReply>();
      });
      turn = mine.catch(() => {});
      return mine;
    },
    async kill() {
      killed = true;
      child.kill("SIGKILL");
      await child.exited;
    },
  };
}

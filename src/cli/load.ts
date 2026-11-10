// The only place the CLI touches the other agents' modules. They are imported dynamically, by URL, so this file
// compiles before they exist and fails with a message naming the missing export if the contract drifts.
import { resolve } from "node:path";
import type { Artefact, Host } from "../artefact.ts";
import type { Source } from "./args.ts";

async function importFrom<T>(rel: string): Promise<T> {
  return (await import(new URL(rel, import.meta.url).href)) as T;
}

function need<T>(mod: Record<string, unknown>, name: string, from: string): T {
  const v = mod[name];
  if (v === undefined) throw new Error(`${from} does not export "${name}" (the CLI expects it)`);
  return v as T;
}

export async function loadArtefact(source: Source): Promise<Artefact> {
  switch (source.kind) {
    case "fixture": {
      const mod = await importFrom<Record<string, unknown>>("../host/fixture.ts");
      return need<Artefact>(mod, "fixture", "src/host/fixture.ts");
    }
    case "file": {
      const scene = ((await import(resolve(source.path))) as { default?: unknown }).default;
      if (scene === undefined) throw new Error(`${source.path} has no default export (it must export a scene)`);
      const index = await importFrom<Record<string, unknown>>("../index.ts");
      return need<(s: unknown) => Artefact>(index, "compile", "src/index.ts")(scene);
    }
  }
}

/** A host drawing into an offscreen texture on bun's headless WebGPU. */
export async function headlessHost(): Promise<Host> {
  const headless = await importFrom<Record<string, unknown>>("../host/headless.ts");
  const device = await need<() => Promise<GPUDevice>>(headless, "headlessDevice", "src/host/headless.ts")();
  const index = await importFrom<Record<string, unknown>>("../host/index.ts");
  return need<(d: GPUDevice) => Host>(index, "createHost", "src/host/index.ts")(device);
}

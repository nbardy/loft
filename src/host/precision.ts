import { LoadError } from "../errors.ts";
import type { Bundle, FramePrecision, Precision } from "./manifest.ts";

/** Pick the format-4 alternatives as a unit before allocating buffers or compiling shaders. */
export function selectPrecision(bundle: Bundle, shaderF16: boolean, ask?: Precision): { bundle: Bundle; precision: FramePrecision } {
  const m = bundle.manifest;
  const variant = m.precision;
  const fail = (message: string): never => {
    throw new LoadError(`artefact precision: ${message}. Fix: rebuild with \`loft build --precision mixed\``);
  };
  if (variant === undefined && m.version !== 4) {
    if (ask === "mixed") fail("the file contains only f32");
    return { bundle, precision: "f32" };
  }
  const record = (v: unknown) => typeof v === "object" && v !== null && !Array.isArray(v);
  if (m.version !== 4 || variant?.kind !== "mixed" || variant.fallback !== "f32-fallback" || !record(variant.modules) || !record(variant.buffers)) {
    return fail("expected the mixed variant and its f32 fallback in format 4");
  }
  const id = (k: string, count: number) => /^(0|[1-9]\d*)$/.test(k) && Number(k) < count;
  for (const [key, chunks] of Object.entries(variant.modules)) {
    if (!id(key, m.modules.length) || !Array.isArray(chunks) || chunks.length === 0
      || chunks.some((c) => !Number.isInteger(c) || c < 0 || c >= m.chunks.length)) fail(`invalid mixed shader module ${key}`);
  }
  for (const [key, buffer] of Object.entries(variant.buffers)) {
    if (!id(key, m.buffers.length) || !buffer || typeof buffer.init !== "string"
      || !Number.isSafeInteger(buffer.size) || buffer.size < 4 || buffer.size % 4 !== 0
      || buffer.size > m.buffers[Number(key)]!.size || !bundle.blobs.has(buffer.init)
      || bundle.blobs.get(buffer.init)!.byteLength > buffer.size
      || bundle.blobs.get(buffer.init)!.byteLength % 4 !== 0) fail(`invalid mixed buffer ${key}`);
  }
  if (ask === "f32") return { bundle, precision: "f32" };
  if (!shaderF16) return { bundle, precision: variant.fallback };
  return {
    precision: "mixed",
    bundle: { ...bundle, manifest: {
      ...m,
      modules: m.modules.map((mod, i) => variant.modules[i] === undefined ? mod : { ...mod, chunks: variant.modules[i]! }),
      buffers: m.buffers.map((buf, i) => {
        const mixed = variant.buffers[i];
        return mixed === undefined ? buf : { ...buf, size: mixed.size, init: mixed.init };
      }),
    } },
  };
}

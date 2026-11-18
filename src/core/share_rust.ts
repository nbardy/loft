// Shader sharing in the Rust core (rust/loft-core/src/share.rs): the twin of src/host/share.ts, one call per artefact.
// A shader it does not read exactly as share.ts does is a named refusal for the whole artefact, and share.ts shares.

import type { Artefact } from "../artefact.ts";
import type { Sharing } from "../host/share.ts";
import { call, KERNEL } from "./ffi.ts";
import { refuse } from "./switch.ts";
import { Writer } from "./wire.ts";

/** The override that names a draw's level-of-detail slot (share.ts PER_DRAW). */
const PER_DRAW = "LOFT_DETAIL_SLOT";
/** Instance bits for the copy (share.ts COPY_BITS). */
const COPY_BITS = 16;

/** `share(a)` computed by the Rust core, or undefined when refused (and reported). */
export function shareRust(a: Artefact): Sharing | undefined {
  const used = [...new Set(a.draws.map((d) => d.shader))];
  const index = new Map(used.map((s, i) => [s, i]));
  const w = new Writer();
  w.str(a.sceneWgsl);
  w.u32(used.length);
  for (const s of used) {
    w.str(s);
    // share.ts: a precision-specific module keeps its own shader; a draw with more instances than the copy bits hold too.
    const eligible = a.mixed?.shaders[s] === undefined && !a.draws.some((d) => d.shader === s && d.instances > 1 << COPY_BITS);
    w.u32(eligible ? 1 : 0);
    w.str(eligible ? a.shaders[s]! : "");
  }
  w.u32(a.draws.length);
  for (const d of a.draws) { w.u32(index.get(d.shader)!); w.f64(d.constants[PER_DRAW] ?? 0); }
  const o = call(KERNEL.share, w.finish());
  const r = o.r;
  if (o.kind === "failed") {
    const kind = r.u32();
    if (kind === 1) throw new Error(`share: ${r.u32()} rows in one group overflow the instance index`);
    refuse("share", r.str());
    return undefined;
  }
  const module = new Map<string, { key: string; wgsl: string }>();
  const groups = r.u32();
  for (let g = 0; g < groups; g++) {
    const key = r.str(), wgsl = r.str();
    for (const m of r.u32s()) module.set(used[m]!, { key, wgsl });
  }
  const draws = a.draws.map((d) => {
    const drop = r.u32() === 1, instanceBase = r.f64();
    if (!drop) return { constants: d.constants, instanceBase };
    const { [PER_DRAW]: _, ...constants } = d.constants;
    return { constants, instanceBase };
  });
  const n = r.u32();
  const rows = new Float32Array(n);
  for (let i = 0; i < n; i++) rows[i] = r.f32();
  return {
    moduleOf: (shader) => module.get(shader) ?? { key: shader, wgsl: a.shaders[shader]! },
    drawOf: (k) => draws[k]!,
    rows,
  };
}

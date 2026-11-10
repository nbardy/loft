// Attachment (DESIGN.md 2.3): a part whose root rides a host's chart. The part is authored in the local frame of the
// host at body coordinates (m, v, rho): x along the host's spine (toward increasing m), y out of the host's skin at v,
// z = x cross y. The pose pass evaluates the host's chart there every frame (the same chart the shaders call) and
// carries the part with it, so a horn stays on its head however the neck moves, at every time slice.

import type { Placement, Strand } from "./strand.ts";

export type AttachSpec = {
  readonly to: Strand;
  /** Along the host, in [0, 1]. */
  readonly m: number;
  /** Round the host: 0.25 the dorsal line, 0.75 the belly. */
  readonly v: number;
  /** Out from the host's spine as a fraction of its thickness: 0 on the spine, 1 on the skin. */
  readonly rho: number;
};

/** Anything that can be re-placed onto a host. */
export interface Attachable<T> {
  placed(placement: Placement): T;
}

export function attach<T>(thing: Attachable<T>, spec: AttachSpec): T {
  if (!(spec.m >= 0 && spec.m <= 1)) throw new Error(`attach: m must be in [0, 1], got ${spec.m}`);
  if (!(spec.rho >= 0) || !Number.isFinite(spec.rho)) throw new Error(`attach: rho must be a finite number >= 0, got ${spec.rho}`);
  if (!Number.isFinite(spec.v)) throw new Error(`attach: v must be finite, got ${spec.v}`);
  return thing.placed({ kind: "on", host: spec.to, m: spec.m, v: spec.v, rho: spec.rho });
}

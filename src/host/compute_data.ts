import { LoadError } from "../errors.ts";
import { COMPUTE_TEXEL_BYTES, computeHistorySize, type Bundle, type Dispatch, type RawDispatch } from "./manifest.ts";

const MIB = 1024 ** 2;
const int = (x: unknown, min: number, max: number): x is number =>
  typeof x === "number" && Number.isSafeInteger(x) && x >= min && x <= max;

/** Version-4 resource bounds also apply to bundles from a GPU-less child, before the GPU owner allocates or loops. */
export function checkComputeData(b: Bundle): void {
  const m = b.manifest;
  const bad = (label: string, what: string): never => { throw new LoadError(`artefact: ${label}: ${what}; rebuild with loft build`); };
  if (m.version < 4) {
    if (m.build !== undefined || m.load !== undefined || m.frame.compute !== undefined || m.textures.some((t) => t.extent !== undefined || t.init !== undefined))
      bad("compute", "author compute requires format version 4");
    return;
  }
  const list = <T>(x: readonly T[], label: string, max: number): readonly T[] => {
    if (!Array.isArray(x) || x.length > max) bad(label, `expected a list of at most ${max} items`);
    return x;
  };
  const at = <T>(xs: readonly T[], id: unknown, label: string): T => {
    if (!int(id, 0, xs.length - 1)) return bad(label, `resource id ${String(id)} is outside its list`);
    return xs[id]!;
  };
  let bytes = 0;
  for (const t of m.textures) {
    if (t.extent === undefined) {
      if (t.init !== undefined) bad(t.label, "view-sized texture has an init blob");
      continue;
    }
    const side = Array.isArray(t.extent) && t.extent.length === 3 ? 256 : 8192;
    if (t.size !== undefined || t.sampleCount !== 1 || !Array.isArray(t.extent) || ![2, 3].includes(t.extent.length) || t.extent.some((n) => !int(n, 1, side)))
      bad(t.label, "invalid fixed texture extent");
    if (!Object.hasOwn(COMPUTE_TEXEL_BYTES, t.format)) bad(t.label, "invalid compute texture format");
    const size = t.extent.reduce((n, s) => n * s, COMPUTE_TEXEL_BYTES[t.format as keyof typeof COMPUTE_TEXEL_BYTES]);
    if (size > 64 * MIB) bad(t.label, "compute texture exceeds 64 MiB");
    if (t.init !== undefined && b.blobs.get(t.init)?.byteLength !== size) bad(t.label, `init blob must have ${size} bytes`);
    bytes += size;
  }
  const buffers = new Set<number>();
  const buffer = (id: number, label: string) => {
    const desc = at(m.buffers, id, label);
    if (!int(desc.size, 16, 64 * MIB) || desc.size % 4 !== 0) bad(label, `invalid compute buffer size ${desc.size}`);
    if (!buffers.has(id)) { buffers.add(id); bytes += desc.size; }
    return desc;
  };
  const same = (a: readonly number[] | undefined, b: readonly number[] | undefined) =>
    a !== undefined && b !== undefined && a.length === b.length && a.every((x, i) => x === b[i]);
  const dispatch = (d: Dispatch): number => {
    if (typeof d?.label !== "string") bad("compute dispatch", "missing dispatch label");
    at(m.computePipelines, d.pipeline, d.label);
    const xyz = d.workgroups;
    if (!Array.isArray(xyz) || xyz.length !== 3 || xyz.some((x) => !int(x, 1, 65535))) return bad(d.label, "invalid workgroup dimensions");
    const groups = xyz[0] * xyz[1] * xyz[2];
    if (groups > 2 ** 20) bad(d.label, "dispatch exceeds 1,048,576 workgroups");
    for (const id of list(d.bindGroups, d.label, 4)) for (const e of at(m.bindGroups, id, d.label).entries) {
      if ("buffer" in e) buffer(e.buffer, d.label);
      if ("texture" in e && at(m.textures, e.texture, d.label).extent === undefined) bad(d.label, "compute binds a view-sized texture");
    }
    return groups;
  };
  const run = (cmd: RawDispatch): number => {
    const groups = dispatch(cmd?.run);
    const label = cmd.run.label;
    for (const id of list(cmd.clearBuffers ?? [], label, 8)) buffer(id, label);
    for (const c of list(cmd.clearTextures ?? [], label, 4)) {
      const a = at(m.textures, c.from, label), b = at(m.textures, c.to, label);
      if (c.from === c.to || !same(a.extent, b.extent) || a.format !== b.format) bad(label, "invalid texture clear copy");
    }
    return groups;
  };
  for (const d of list(m.frame.compute ?? [], "frame.compute", 64)) run(d);
  for (const job of list(m.load ?? [], "load", 64)) {
    switch (job?.kind) {
      case "once": run(job); break;
      case "steps": {
        const label = job.start?.run?.label ?? "steps";
        if (!int(job.count, 1, 4096) || !int(job.keep, 1, job.count)) bad(label, "invalid count or keep");
        const seed = run(job.start), odd = run(job.odd), even = run(job.even);
        if (seed + Math.ceil(job.count / 2) * odd + Math.floor(job.count / 2) * even > 2 ** 22) bad(label, "step job exceeds 4,194,304 workgroups");
        const states = 1 + Math.ceil(job.count / job.keep);
        for (const c of list(job.history, label, 8)) {
          if (c.a === c.b || c.a === c.to || c.b === c.to) bad(label, "step copy aliases its source and destination");
          switch (c.kind) {
            case "buffer": {
              const a = buffer(c.a, label), b = buffer(c.b, label), to = buffer(c.to, label);
              if (!int(c.bytes, 16, Math.min(a.size, b.size)) || c.bytes % 16 !== 0 || c.bytes * states > to.size) bad(label, "invalid step buffer size");
              break;
            }
            case "texture": {
              const a = at(m.textures, c.a, label), b = at(m.textures, c.b, label), to = at(m.textures, c.to, label);
              if (a.extent === undefined || !same(a.extent, b.extent) || !same(computeHistorySize(a.extent, states), to.extent)
                || a.format !== b.format || a.format !== to.format) bad(label, "invalid step texture extent");
              break;
            }
            default: bad(label, "invalid step resource kind");
          }
        }
        break;
      }
      default: bad("load", "invalid compute job kind");
    }
  }
  if (m.build !== undefined) {
    for (const d of list(m.build.runs, "build", 64)) run(d);
    for (const out of list(m.build.outputs, "build outputs", 64 * 8)) {
      switch (out?.kind) {
        case "buffer": if (!int(out.bytes, 16, buffer(out.id, out.name).size) || out.bytes % 16 !== 0) bad(out.name, "invalid bake buffer size"); break;
        case "texture": if (at(m.textures, out.id, out.name).extent === undefined) bad(out.name, "build output is a view-sized texture"); break;
        default: bad("build", "invalid output kind");
      }
    }
  }
  // Frame input and parameter buffers can contribute two minimum-size allocations in addition to the declared outputs.
  if (bytes > 256 * MIB + 2 * 64 * 1024) bad("compute", "compute resources exceed 256 MiB");
}

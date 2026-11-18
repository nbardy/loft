import { keptStates, orderCompute, resourceError, type ComputePass, type ComputeResource } from "../compute.ts";
import { emitWgsl, rawComputeOf, time, WGSL_TY, type LibFn } from "../expr.ts";
import { computeHistorySize } from "../host/manifest.ts";
import type { Slots } from "./bind.ts";

export type CompiledCompute = {
  readonly passes: readonly Omit<ComputePass, "values">[];
  readonly resources: readonly { readonly binding: number; readonly resource: ComputeResource }[];
  readonly wgsl: string;
};

const array = "array<vec4<f32>>";

function binding(group: number, at: number, r: ComputeResource, name: string, access: "read" | "write"): string {
  const slot = `@group(${group}) @binding(${at})`;
  switch (r.kind) {
    case "buffer": return `${slot} var<storage, ${access === "read" ? "read" : "read_write"}> ${name}: ${array};`;
    case "texture": {
      const d = `${r.size.length}d`;
      return `${slot} var ${name}: ${access === "read" ? `texture_${d}<f32>` : `texture_storage_${d}<${r.format}, write>`};`;
    }
  }
}

function reader(group: number, at: number, r: ComputeResource, name: string): string {
  if (r.steps === undefined) return binding(group, at, r, name, "read");
  const data = `loft_kept_${name}`, { count, rate, keep } = r.steps;
  const clip = `let step = u32(clamp(floor(seconds * f32(${rate})), 0.0, f32(${count})));
  let kept = select(step / ${keep}u, ${Math.ceil(count / keep)}u, step == ${count}u);`;
  switch (r.kind) {
    case "buffer": return `${binding(group, at, r, data, "read")}
fn ${name}(i: u32, lane: u32, seconds: f32) -> vec4<f32> {
  ${clip}
  return ${data}[(kept * ${r.entries}u + i) * ${r.lanes}u + lane];
}`;
    case "texture": {
      const [, h, d = 1] = r.size;
      const is3d = r.size.length === 3;
      const dim = is3d ? "vec3" : "vec2";
      const offset = is3d ? `vec3<i32>(q.xy, q.z + i32(kept) * ${d})` : `vec2<i32>(q.x, q.y + i32(kept) * ${h})`;
      return `${binding(group, at, { ...r, size: computeHistorySize(r.size, keptStates(r.steps)) }, data, "read")}
fn ${name}(p: ${dim}<i32>, seconds: f32) -> vec4<f32> {
  ${clip}
  let q = clamp(p, ${dim}<i32>(0), ${dim}<i32>(${r.size.map((n) => n - 1).join(", ")}));
  return textureLoad(${data}, ${offset}, 0);
}`;
    }
  }
}

function shader(p: ComputePass, slots: Slots): string {
  const q = p.writes.length, stepped = p.when.kind === "steps";
  const declarations = [
    ...p.writes.map(([name, r], i) => binding(1, i, r, name, "write")),
    ...(stepped ? p.writes.map(([name, r], i) => binding(1, q + i, r, `prev_${name}`, "read")) : []),
    ...p.reads.map(([name, r], i) => reader(1, (stepped ? 2 : 1) * q + i, r, name)),
  ];
  if (p.when.kind === "steps") declarations.push(`const loft_dt: f32 = 1.0 / f32(${p.when.rate});`);
  if (p.when.kind === "frame") {
    declarations.push(
      "@group(0) @binding(0) var<storage, read> inputs: array<f32>;",
      "@group(0) @binding(1) var<storage, read> params: array<f32>;",
    );
    for (const [name, e] of [["loft_time", time], ...Object.entries(p.values ?? {})] as const) {
      const { lines, out } = emitWgsl([e], slots.wgsl({}), "value");
      declarations.push(`fn ${name}() -> ${WGSL_TY[e.ty]} { ${lines.join(" ")} return ${out[0]}; }`);
    }
  }
  return [p.wgsl, ...declarations].join("\n");
}

/** Registers frame inputs before slots are sealed and gives every consumer the same generated accessors. */
export function compileCompute(given: readonly object[] | undefined, slots: Slots) {
  const passes = orderCompute(given).map(({ values, ...p }) => ({ ...p, wgsl: shader({ ...p, values }, slots) }));
  const outputs = passes.flatMap((p) => p.writes.map(([, r]) => r));
  const byName = new Set(outputs.map((r) => r.key));
  const check = (libs: readonly LibFn[]): void => {
    for (const fn of libs) {
      const raw = rawComputeOf(fn);
      if (raw !== undefined && !byName.has(raw))
        resourceError(raw, "a read of a pass outside this scene", "the producer in scene.compute", "add its compute record to scene.compute");
    }
  };
  return (start: number, libs: readonly LibFn[], texts: readonly string[]): CompiledCompute | undefined => {
    check(libs);
    if (passes.length === 0) return undefined;
    // Intermediate outputs count against a kernel's bindings only. Render and pose layouts bind the resources their
    // WGSL can read, including uses in raw shader code and in expression library functions.
    const drawn = new Set<string>();
    for (const source of [...texts, ...libs.map((fn) => fn.wgsl)]) {
      for (const m of source.matchAll(/\bloft_compute_[A-Za-z0-9_]+\b/g)) drawn.add(m[0]);
    }
    const resources = outputs.filter((r) => drawn.has(r.symbol)).map((resource, i) => ({ binding: start + i, resource }));
    return { passes, resources, wgsl: resources.map(({ binding, resource: r }) => reader(1, binding, r, r.symbol)).join("\n") };
  };
}

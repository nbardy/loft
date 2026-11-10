// Slots: where each leaf of the expression graph lives at run time. Assigned lazily, in order of first use, as the
// compiler emits code (so there is one pass, and the order is deterministic). CPU-only expressions register too, so
// a param used only by the pose pass still gets a slot the host can set.

import type { InputSlot, ParamSlot } from "../artefact.ts";
import { CURVE_N, emitWgsl, leaves, type Bind, type Env, type Expr, type InputSpec, type NumTy, type ParamSpec, type Row } from "../expr.ts";

const FLOATS: Record<NumTy, number> = { f32: 1, vec2: 2, vec3: 3, vec4: 4 };

export class Slots {
  private readonly inputMap = new Map<string, InputSlot>();
  private readonly paramMap = new Map<string, ParamSlot>();
  private readonly rowMap = new Map<number, { readonly row: Row; readonly slot: number }>();
  private inputFloats = 0;

  input(spec: InputSpec): InputSlot {
    const seen = this.inputMap.get(spec.name);
    if (seen) return seen;
    const made: InputSlot = { name: spec.name, ty: spec.ty, slot: this.inputFloats, def: spec.def };
    this.inputFloats += FLOATS[spec.ty];
    this.inputMap.set(spec.name, made);
    return made;
  }

  param(spec: ParamSpec): ParamSlot {
    const seen = this.paramMap.get(spec.name);
    if (seen) return seen;
    const made: ParamSlot = { name: spec.name, slot: this.paramMap.size, def: spec.def, min: spec.min, max: spec.max };
    this.paramMap.set(spec.name, made);
    return made;
  }

  row(r: Row): number {
    const seen = this.rowMap.get(r.id);
    if (seen) return seen.slot;
    const slot = this.rowMap.size;
    this.rowMap.set(r.id, { row: r, slot });
    return slot;
  }

  /** Give every leaf of these (CPU-evaluated) expressions a slot. */
  register(roots: readonly Expr[]): void {
    const l = leaves(roots);
    for (const s of l.inputs) this.input(s);
    for (const p of l.params) this.param(p);
    for (const r of l.rows) this.row(r);
  }

  /** A WGSL binding of leaves; free variables map through `vars` (name -> WGSL source), and an unknown one throws. */
  wgsl(vars: Readonly<Record<string, string>>): Bind {
    return {
      variable: (name) => {
        const src = vars[name];
        if (src === undefined) throw new Error(`free variable "${name}" is not bound here (bound: ${Object.keys(vars).join(", ") || "none"})`);
        return src;
      },
      input: (spec) => {
        const s = this.input(spec).slot;
        switch (spec.ty) {
          case "f32": return `inputs[${s}]`;
          case "vec2": return `vec2<f32>(inputs[${s}], inputs[${s + 1}])`;
          case "vec3": return `vec3<f32>(inputs[${s}], inputs[${s + 1}], inputs[${s + 2}])`;
          case "vec4": return `vec4<f32>(inputs[${s}], inputs[${s + 1}], inputs[${s + 2}], inputs[${s + 3}])`;
        }
      },
      param: (spec) => `params[${this.param(spec).slot}]`,
      row: (r, x) => `curve(${this.row(r)}u, ${x})`,
    };
  }

  /** The leaf tables, final once emission is done. */
  tables(): { inputs: InputSlot[]; inputFloats: number; params: ParamSlot[]; curves: Float32Array; rows: number } {
    const rows = [...this.rowMap.values()];
    const curves = new Float32Array(rows.length * CURVE_N * 4);
    for (const { row, slot } of rows) curves.set(row.samples, slot * CURVE_N * 4);
    return { inputs: [...this.inputMap.values()], inputFloats: this.inputFloats, params: [...this.paramMap.values()], curves, rows: rows.length };
  }

  /** Read a host's slot-laid-out state back into the by-name environment the JavaScript backend evaluates in. */
  envReader(): (inputs: Float32Array, params: Float32Array) => { inputs: Record<string, number | number[]>; params: Record<string, number> } {
    const ins = [...this.inputMap.values()], ps = [...this.paramMap.values()];
    return (inputs, params) => {
      const i: Record<string, number | number[]> = {};
      for (const s of ins) i[s.name] = s.ty === "f32" ? inputs[s.slot] : Array.from(inputs.subarray(s.slot, s.slot + FLOATS[s.ty]));
      const p: Record<string, number> = {};
      for (const s of ps) p[s.name] = params[s.slot];
      return { inputs: i, params: p };
    };
  }
}

export type { Env };

const WGSL_TY: Record<string, string> = { f32: "f32", vec2: "vec2<f32>", vec3: "vec3<f32>", vec4: "vec4<f32>", bool: "bool" };

/**
 * A WGSL function whose body is straight-line code for `roots`. `args` are (name, type) pairs and are bound as free
 * variables of the same name; `extra` binds more variables (for example the fields of a station sample) and `pre` is
 * hand-written code that runs first and defines them. `ret` combines the root values into the return expression.
 */
export function wgslFn(
  slots: Slots,
  name: string,
  args: readonly (readonly [string, NumTy])[],
  out: string,
  roots: readonly Expr[],
  ret: (values: readonly string[]) => string,
  opts: { readonly pre?: readonly string[]; readonly extra?: Readonly<Record<string, string>> } = {},
): string {
  const vars: Record<string, string> = { ...(opts.extra ?? {}) };
  for (const [a] of args) vars[a] = a;
  const { lines, out: values } = emitWgsl(roots, slots.wgsl(vars));
  const sig = args.map(([a, t]) => `${a}: ${WGSL_TY[t]}`).join(", ");
  return [`fn ${name}(${sig}) -> ${out} {`, ...(opts.pre ?? []).map((l) => `  ${l}`), ...lines.map((l) => `  ${l}`), `  return ${ret(values)};`, `}`].join("\n");
}

// A WGSL struct and its readings. Constructor arguments live in a densely packed word table.
// The generated fetch constructs the native WGSL struct, so vec3's storage padding never leaks to the author.
import { SceneError } from "../errors.ts";
import type { Open } from "../record/passes.ts";
type Arg = number | readonly number[];
type Field = { name: string; type: string; size: number };
export type Value = { kind: Kind; words: readonly number[] };
export type Kind = { name: string; tag: number; source: string; functions: ReadonlySet<string>; fields: Field[] };

export const bits = (x: number) => new Uint32Array(Float32Array.of(x).buffer)[0]!;
export function hash(text: string) {
  let h = 2166136261;
  for (const c of new TextEncoder().encode(text)) h = Math.imul(h ^ c, 16777619);
  return h >>> 0;
}

export function kind(source: string) {
  const code = source.replace(/\/\*[\s\S]*?\*\/|\/\/[^\n]*/g, " ");
  const struct = /\bstruct\s+(\w+)\s*\{([^{}]*)\}/.exec(code);
  if (!struct) throw new SceneError("kind: supply a WGSL struct followed by its readings");
  const [, name, body] = struct;
  const fields = body.split(",").map(x => x.trim()).filter(Boolean).map(x => {
    const m = /^(\w+)\s*:\s*(f32|u32|vec([234])([fu]))$/.exec(x);
    if (!m) throw new SceneError(`kind ${name}: unsupported field ${x}. Use f32, u32 or their vector types`);
    return { name: m[1], type: m[2], size: Number(m[3] ?? 1) };
  });
  const functions = new Set(Array.from(code.matchAll(/\bfn\s+(\w+)\s*\(/g), m => m[1]));
  const tag = hash(source);
  const scoped = source.replace(/\b[A-Za-z_]\w*\b/g, s => functions.has(s) ? `${name}_${s}` : s);
  let offset = 0;
  const args = fields.map(f => {
    const read = Array.from({ length: f.size }, () => `words[at + ${offset++}u]`);
    return f.type === "u32" ? read[0] : f.type === "f32" ? `bitcast<f32>(${read[0]})` : `bitcast<${f.type}>(vec${f.size}u(${read.join(", ")}))`;
  });
  const k: Kind = { name, tag, fields, functions, source: scoped + `\nfn load_${name}(at: u32) -> ${name} { return ${name}(${args.join(", ")}); }\n` };
  return (...values: Arg[]): Value => {
    if (values.length !== fields.length) throw new SceneError(`${name}: supply ${fields.map(f => f.name).join(", ")}`);
    const words = fields.flatMap((f,i) => {
      const arg = values[i], xs = typeof arg === "number" ? [arg] : arg;
      const uint = f.type.endsWith("u") || f.type === "u32";
      if (xs.length !== f.size || xs.some(x => !Number.isFinite(Math.fround(x)) || uint && x !== (x >>> 0))) {
        throw new SceneError(`${name}.${f.name}: supply a finite ${f.type}`);
      }
      return xs.map(x => uint ? x : bits(x));
    });
    return { kind: k, words };
  };
}

export function reading(kinds: Kind[], name: string, params: string, args: string, ret: string, otherwise: string, adapter?: (k: Kind) => string): Open {
  return { signature: `fn read_${name}(tag: u32, at: u32${params && ", " + params}) -> ${ret}`, selector: "tag", otherwise,
    cases: kinds.map(k => {
      if (k.functions.has(name)) return [k.tag, `${k.name}_${name}(load_${k.name}(at)${args && ", " + args})`];
      if (adapter) return [k.tag, adapter(k)];
      throw new SceneError(`${k.name} has no ${name} reading. Define fn ${name}(s: ${k.name}${params && ", " + params}) -> ${ret}`);
    }) };
}

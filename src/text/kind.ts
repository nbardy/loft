// A WGSL struct and its readings. Constructor arguments live in a densely packed word table.
// The generated fetch constructs the native WGSL struct, so vec3's storage padding never leaks to the author.
import { parseModule, ParseError, type Module } from "./parse.ts";
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
  let mod: Module;
  try { mod = parseModule(source); } catch (e) {
    if (e instanceof ParseError) throw new SceneError(`kind line ${e.at.line+1}: ${e.message}. ${e.fix}`);
    throw e;
  }
  if (mod.structs.length !== 1) throw new SceneError("kind: declare exactly one row struct followed by its readings");
  const [{ name, fields: members }] = mod.structs;
  const fields = members.map(f => {
    const type = f.type.args.length === 1 && /^vec[234]$/.test(f.type.name)
      ? f.type.name + ({f32:"f",u32:"u"} as Record<string,string>)[f.type.args[0].name] : f.type.name;
    const m = /^(f32|u32|vec([234])([fu]))$/.exec(type);
    if (!m) throw new SceneError(`${name}.${f.name}: use f32, u32 or their two- to four-component vector types`);
    return {name:f.name,type,size:Number(m[2] ?? 1)};
  });
  const functions = new Set(mod.fns.map(f=>f.sig.name));
  const tag = hash(source);
  // Only function declarations and calls are scoped. Members and comments can use the same word.
  let scoped="",last=0;
  mod.toks.forEach((t,i)=>{
    if (functions.has(t.s) && mod.toks[i+1]?.s==="(" && mod.toks[i-1]?.s!==".") {
      scoped+=source.slice(last,t.start)+name+"_"+t.s; last=t.end;
    }
  });
  scoped+=source.slice(last);
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

export function reading(kinds: Kind[], name: string, params: string, args: string, ret: string, otherwise: string, adapter?: (k: Kind, receiver: string) => string, receiver = (k: Kind) => `load_${k.name}(at)`): Open {
  return { signature: `fn read_${name}(tag: u32, at: u32${params && ", " + params}) -> ${ret}`, selector: "tag", otherwise,
    cases: kinds.map(k => {
      if (k.functions.has(name)) return [k.tag, `${k.name}_${name}(${receiver(k)}${args && ", " + args})`];
      if (adapter) return [k.tag, adapter(k, receiver(k))];
      throw new SceneError(`${k.name} has no ${name} reading. Define fn ${name}(s: ${k.name}${params && ", " + params}) -> ${ret}`);
    }) };
}

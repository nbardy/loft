// A WGSL struct and its readings. Constructor arguments live in a densely packed word table.
// The generated fetch constructs the native WGSL struct, so vec3's storage padding never leaks to the author.
import { parseModule, ParseError, type Module } from "./parse.ts";
import { SceneError } from "../errors.ts";
import type { Open,RowType } from "../record/passes.ts";
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

/** Numeric holes travel with the row; changing their values leaves the linked law identical. */
export function kind(text: string | TemplateStringsArray, ...holes: Arg[]) {
  let source = typeof text==="string" ? text : text.reduce((s,t,i)=>s+(i?`loft_capture_${i-1}`:"")+t,"");
  if(holes.length) {
    const parsed=parseModule(source), row=parsed.structs[0];
    if(parsed.structs.length!==1) throw new SceneError("kind holes: declare the row struct that carries the captured fields");
    const types=holes.map(v=>{
      if(typeof v==="number") return "f32";
      if(v.length>=2 && v.length<=4) return `vec${v.length}f`;
      throw new SceneError("kind hole: supply a scalar or a vector of two to four components");
    });
    if(row.fields.some(f=>f.name.startsWith("loft_capture_"))) throw new SceneError("kind: rename row fields beginning with loft_capture_");
    const edits:Array<[number,number,string]>=[];
    const end=source.lastIndexOf("}",row.span[1]-1);
    const separator=row.fields.length && !source.slice(row.span[0],end).trimEnd().endsWith(",")?",":"";
    edits.push([end,end,separator+types.map((ty,i)=>`loft_capture_${i}: ${ty}`).join(", ")]);
    for(const tok of parsed.toks.filter(t=>/^loft_capture_\d+$/.test(t.s))) {
      const fn=parsed.fns.find(f=>f.span[0]<=tok.start && tok.end<=f.span[1]);
      const receiver=fn?.sig.params[0];
      if(!receiver || receiver.type.name!==row.name) throw new SceneError("kind hole: use the capture inside a reading whose first argument is the row");
      edits.push([tok.start,tok.end,`${receiver.name}.${tok.s}`]);
    }
    for(const [a,b,v] of edits.sort((x,y)=>y[0]-x[0])) source=source.slice(0,a)+v+source.slice(b);
  }
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
  const k: Kind = { name, tag, fields, functions, source:scoped };
  return (...values: Arg[]): Value => {
    values = [...values,...holes];
    if (values.length !== fields.length) throw new SceneError(`${name}: supply ${fields.map(f => f.name).join(", ")}`);
    const words = fields.flatMap((f,i) => {
      const arg = values[i], xs = typeof arg === "number" ? [arg] : arg;
      const uint = f.type.endsWith("u") || f.type === "u32";
      if (xs.length !== f.size || xs.some(x => !Number.isFinite(Math.fround(x)) || uint && x !== (x >>> 0))) {
        throw new SceneError(`${name}.${f.name}: supply a finite ${f.type}`);
      }
      return Array.from(xs,x => uint ? x : bits(x));
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


export function layout(k:Kind,storage:string):RowType {
  return {name:k.name,storage,fields:k.fields.map(f=>({ty:f.type==="u32" || f.type.endsWith("u") ? "U32" : "F32",width:f.size}))};
}

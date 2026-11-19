import type { Camera } from "../artefact.ts";
import { SceneError } from "../errors.ts";
import type { Declarations, Open, Source, Pass, Write, Draw } from "../record/passes.ts";
import { bits, reading, type Value } from "../text/kind.ts";
import base from "./interface.wgsl" with { type: "text" };
import queries from "./world.wgsl" with { type: "text" };
import screen from "./covers/screen.wgsl" with { type: "text" };
export type V3 = readonly [number,number,number];
export type Thing = { shape: Value; material: Value; at: V3; id?: number; ink?: number };
export const thing = (shape: Value, material: Value, at: V3 = [0,0,0]): Thing => ({ shape, material, at });
export type World = { things: readonly Thing[]; shadows?: readonly Thing[] };
export type Scene = World & { cameras: Record<string,Camera>; engine: (s: Scene) => Declarations; sky?: string };
export const scene = (s: Scene) => ({ kind: "ordered" as const, cameras: s.cameras, record: () => s.engine(s) });
export type OrderedScene = ReturnType<typeof scene>;

export function world(s: World) {
  const shadows = new Set(s.shadows ?? s.things), primary = new Set(s.things);
  const things = [...new Set([...primary, ...shadows])];
  if (!things.length || things.length >= 0x80000000) throw new SceneError("world: supply 1..2³¹−1 rows");
  const rows: number[] = [], words: number[] = [];
  const kinds = (values: Value[]) => {
    const byName = new Map(values.map(v => [v.kind.name,v.kind]));
    if (values.some(v => byName.get(v.kind.name)!.source !== v.kind.source)) throw new SceneError("two kind definitions share a name; rename one struct");
    const all = [...byName.values()].sort((a,b)=>a.tag-b.tag);
    if (new Set(all.map(k => k.tag)).size !== all.length) throw new SceneError("two kinds share a tag; rename one struct");
    return all;
  };
  const shapes=kinds(things.map(t=>t.shape)), materials=kinds(things.map(t=>t.material));
  for (const [i,t] of things.entries()) {
    const ref = (v:Value) => { const at=words.length; words.push(...v.words); return [v.kind.tag,at]; };
    const id=t.id ?? i+1;
    if (id !== (id>>>0) || t.at.some(x=>!Number.isFinite(Math.fround(x))) || !Number.isFinite(t.ink ?? 0)) throw new SceneError("thing: use a u32 id, finite position and ink width");
    rows.push(...ref(t.shape),...ref(t.material),...t.at.map(bits),id,(primary.has(t)?1:0)|(shadows.has(t)?2:0),bits(t.ink??0),0,0);
  }
  while (!words.length || words.length%4) words.push(0);
  const opens = [
    reading(shapes,"hit","r: Ray","r","Hit","miss"),
    reading(shapes,"bound","","","Box","Box(vec3f(0),vec3f(0))"),
  ];
  const geometry = base+"\n"+shapes.map(k=>k.source).join("\n");
  return {
    tables:[{lanes:3,words:rows},{lanes:1,words}],
    cover(body:string):Source { return {text:geometry+body,opens:[...opens,reading(shapes,"gap","r: Ray","r","vec2f","vec2f(-1,1e30)",()=> "vec2f(-1,1e30)")]}; },
    query(body:string, extra:Open[]=[]):Source { return {text:geometry+queries+screen+body,opens:[...opens,...extra]}; },
    materials,
  };
}
export function write(target: Write["target"],clear=0,blend:Write["blend"]="Opaque"):Write {
  return {target,clear:{Color:[bits(clear),0,0,0]},blend};
}
export function raster(name:string, module:number, draws:Draw[], writes:Write[], reads:Pass["reads"]=[], repeat=1):Pass {
  return { name,module,run:{Raster:{draws}},reads,writes,repeat };
}
export const screenDraw = (fragment:string):Draw => ({ vertex:"screen",fragment,vertices:3,instances:{Fixed:1},cull:"None" });

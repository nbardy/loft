import { SceneError } from "../errors.ts";
import type { DataTable, Expr } from "../expr.ts";
import type { RowStep } from "../record/decls.ts";
import type { body } from "./body.ts";

type Body = ReturnType<typeof body>;
export function move(shape: Body | readonly Body[], source: string, inputs: {
  go: Expr<"vec2">; ticks?: number; hz?: number; script?: (tick: number) => readonly [number,number]
}) {
  const bodies: readonly Body[] = Array.isArray(shape) ? shape : [shape as Body];
  const { hz=120, ticks=480 } = inputs;
  if (!bodies.length || !Number.isSafeInteger(ticks) || ticks<1 || ticks>100000 || !Number.isInteger(hz) || hz<1 || hz>100000)
    throw new SceneError("move", "empty batch or invalid clock", "finite positive counts", "supply bodies, hz and ticks (at most 100000)");
  const node=inputs.go.node;
  if(node.op!=="input") throw new SceneError("move",node.op,"a named level or sampled script","bind go to input(name, 'vec2', default)");
  const table=(name:string,entries:number,lanes:number,data:Float32Array):DataTable=>({name,role:"data",entries,lanes,data});
  const joined=(key:"plan"|"state")=>Float32Array.from(bodies.flatMap(b=>Array.from(b[key])));
  const pose=table("posed",58*bodies.length,1,new Float32Array(58*4*bodies.length));
  const script=inputs.script;
  const samples=new Float32Array(script ? (ticks+1)*4 : 0);
  if(script) for(let k=0;k<=ticks;k++){
    const v=script(k);
    if(v.length!==2 || !v.every(Number.isFinite)) throw new SceneError(`move: invalid stick sample at tick ${k}. Fix: return two finite velocity components`);
    samples.set(v,4*k);
  }
  const steps: RowStep = { source, plan:table("plans",bodies.length,26,joined("plan")), seed:table("initial",bodies.length,42,joined("state")),
    output:pose, input:node.spec, hz,ticks, home:script ? {kind:"script",samples:table("script",ticks+1,1,samples)} : {kind:"level"} };
  const holds=bodies.flatMap((b,row)=>b.chains.map((_,i)=>{
    const word=4*(58*row+50+i);
    return {label:`row ${row}, planted toe ${i}`,word,negative:word+1};
  }));
  return {bodies,steps,pose,holds};
}

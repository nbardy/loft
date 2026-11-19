// Record the same explicit scene as loft build, with one controlled change per comparison.
import { mkdirSync } from "node:fs";
import { join } from "node:path";
import { crab } from "../examples/motion/crab_live.ts";
import { stage, stick } from "../examples/motion/stage.ts";
import { body, move, roam } from "../src/motion/index.ts";
import { input } from "../src/expr.ts";
import { native, UNLIMITED } from "../src/core/native.ts";
import { rec } from "../src/record/recorder.ts";
import { pack, Program } from "../src/record/wire.ts";
import type { Scene } from "../src/record/decls.ts";

const out=process.argv[2] ?? "out/motion-score";
mkdirSync(out,{recursive:true});
const go=input("stick","vec2",[0,0]), normal=roam();
function save(name:string,scene:Scene){
  const job=pack(rec.job(Program.Scene,[],scene));
  native().run(job,{nodes:UNLIMITED,tapeOps:UNLIMITED,boxes:UNLIMITED,gpuNanos:UNLIMITED},join(out,name+".loft"),{naga:"emit",precision:"f32"});
}
for(const [name,source] of [
  ["one",normal],
  ["low",roam({arch:"fn arch(u:f32,urgency:f32)->f32 {return (1.0-0.5*urgency)*16.0*u*u*(1.0-u)*(1.0-u);}"})],
  ["broken",normal.replace("p.joints[k+2]=distal;","if i==3u { distal.t.x+=0.020; }\n    p.joints[k+2]=distal;")]
]){
  save(name,stage(move(crab,source,{go,script:stick})).scene);
}
for(const n of [5,256,1000]){
  const group=Array.from({length:n},(_,i)=>i<5 ? body({...crab,trunk:{...crab.trunk,height:2.5+i*0.04}}):crab);
  const m=move(group,normal,{go});
  save(String(n),n===5?stage(m).scene:{cameras:[],passes:[],checks:[],solves:[],rowSteps:[m.steps]});
}

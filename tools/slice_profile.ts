// bun tools/slice_profile.ts. Large row counts are loaded at 16²; only the requested frame budgets use full resolution.
import {createHash,randomInt} from "node:crypto";
import {writeFileSync} from "node:fs";
import {build,sections,run,out,draw} from "./slice_measure.ts";
import {slice} from "../examples/slice/slice.ts";
import {ring,card} from "../examples/slice/washer.ts";
import {washer} from "../src/kinds/shapes.ts";
import {reference} from "../src/engines/reference.ts";
import {realtime} from "../src/engines/realtime.ts";
const hash=(s:string)=>createHash("sha256").update(s).digest("hex");
function profile(file:string,size:string,frames:number,options:string[]=[]) {
  return JSON.parse(run([draw,"profile",file,"--view","front","--size",size,"--frames",String(frames),...options]).trim().split("\n").at(-1)!).profile;
}
function measureEdits() {
  const camera=slice(10).cameras;
  const washers=[0.5,0.25].map(r=>sections(build("edit"+r,{things:[{...ring,shape:washer(r,1)},card],cameras:camera,engine:realtime()})));
  const edits=washers[0].texts.filter((t,i)=>t!==washers[1].texts[i]).length;
  const wordsChanged=washers[0].bundle.resources.flatMap((r:{init:{Bytes:number}},i:number)=>{
    const a=washers[0].data[r.init.Bytes],b=washers[1].data[washers[1].bundle.resources[i].init.Bytes];
    return Array.from({length:a.length/4},(_,j)=>j).filter(j=>a.readUInt32LE(4*j)!==b.readUInt32LE(4*j)).map(w=>[i,w]);
  });
  const changed=[];
  for(const [name,engine] of [["realtime",realtime()],["reference",reference()]] as const) {
    const scene=slice(10);
    // Isolate the shape edit: copper is also present on a box, even when the bead is absent.
    const things=scene.things.map((t,i)=>i===5?{...t,material:scene.things[7].material}:t);
    const [a,b]=[false,true].map(withBead=>sections(build("bead-"+name+withBead,{...scene,things:things.filter((_,i)=>withBead||i!==7),engine})));
    changed.push(...b.texts.flatMap((t,i)=>t===a.texts[i]?[]:[name+"/"+i]));
  }
  return {edits,wordsChanged,changed,pass:edits===0 && wordsChanged.length===1 && changed.length===3};
}
const edits=measureEdits();
if(process.argv.includes("--edits")) {console.log(JSON.stringify(edits));process.exit(edits.pass?0:1);}
const files=[10,100,1000].map(n=>build("size"+n,slice(n)));
const sets=files.map(f=>sections(f).texts.map(hash));
const equal=sets.every(s=>JSON.stringify(s)===JSON.stringify(sets[0]));
const lines=sections(files[0]).texts.reduce((n,t)=>n+t.trimEnd().split("\n").length,0);
const load=files.map(f=>profile(f,"16x16",1,["--loads","4","--cold-nonce",String(randomInt(1<<24,0x7fffffff))]));
const p=load[0];
const counts={pipelines:p.render_pipelines+p.compute_pipelines,modules:p.modules,bundleLines:lines,equal,hashes:sets[0]};
const warm=load.map(p=>[...p.loads_ms.slice(1)].sort((a,b)=>a-b)[1]),cold=load.map(p=>p.loads_ms[0]);
const spread=(x:number[])=>Math.max(...x)/Math.min(...x)-1;
const view=[10,30].map(n=>profile(build("timed"+n,slice(n)),"1280x800",7));
const train=profile(build("training",{...slice(30),engine:realtime({samples:16})}),"512x512",16);
const y=view.map(v=>v.pass_ms[1][1]);const slope=(y[1]-y[0])/20,offset=y[0]-10*slope;
const close=(a:number,b:number)=>Math.abs(a/b-1)<=0.3;
const results={
  "1":{...counts,pass:equal&&counts.pipelines<=8&&lines<=2000},
  "2":edits,
  "3":{cold,warm,coldSpread:spread(cold),warmSpread:spread(warm),pass:Math.max(...cold)<=350&&Math.max(...warm)<=60&&Math.max(spread(cold),spread(warm))<=0.1},
  "12":{shade:y,slope,offset,frames:view.map(v=>v.gpu_ms),pass:close(slope,0.481)&&close(offset,11)&&view.every((v,i)=>v.gpu_ms[1]<=[34.6,53.8][i])},
  "13":{times:train.gpu_ms,pass:train.gpu_ms[2]<=50},load,view,train};
writeFileSync(out+"/profile.json",JSON.stringify(results,null,2));
console.log(JSON.stringify(Object.fromEntries(Object.entries(results).filter(([k])=>!isNaN(Number(k)))),null,2));

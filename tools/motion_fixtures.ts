// Record the same explicit scene as loft build, with one controlled change per comparison.
import { mkdirSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { crab } from "../examples/motion/crab_live.ts";
import { stage, stick } from "../examples/motion/stage.ts";
import { body, foot, gaze, hop, move, roam } from "../src/motion/index.ts";
import { cameraMatrices } from "../src/host/camera.ts";
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

// Freeze the same camera in both timing arms. Metal exposes pass boundaries, and a
// named GPU lens would otherwise sit between the live span and the first render pass.
const camera = cameraMatrices({kind:"perspective", eye:[17,14,15], target:[0,1.8,-0.2], fovY:0.63}, 256/192);
writeFileSync(join(out,"game.camera"),Float32Array.from([
  ...camera.viewProj, ...camera.invViewProj, ...camera.eye, 1, ...camera.forward, 0
]));

const headless = (m:ReturnType<typeof move>):Scene => ({cameras:[],passes:[],checks:[],solves:[],rowSteps:[m.steps]});
const parked = {go, script:(): [number,number] => [0,0]};
const movingDeck = "fn deck_at(t:f32)->Frame {return Frame(vec4f(0.5*t,0,0,1),vec4f(0,0,0,1));}";
save("deck", headless(move(crab,roam({deck:movingDeck}),parked)));
const pads = body({trunk:{size:[0.3,0.1,0.2],height:0.3},chains:
  [-1,1].flatMap(x=>[-1,1].map(z=>({root:[0.1*x,0,0.1*z] as const,lengths:[0.2,0.2,0.2] as const,end:foot([0.3*x,0,0.2*z])})))});
const loads = roam({present:`fn present(s:ptr<function,State>,b:ptr<function,Plan>,f:f32,dt:f32)->Pose {
  refresh(s,15u); let q=vec2f(0.15,0.0); let interval=share_range(s,15u,q,3u);
  var p:Pose; p.margins[0]=vec4f(interval,shares(s,15u,q)[3],0); return p;
}`});
save("pads",headless(move(pads,loads,parked)));

// Two numbered impulses on one tick exercise capacity, sequence order and a zero-valued event.
const impulse = move([crab,crab],roam({step:`fn step(s:ptr<function,State>,b:ptr<function,Plan>,e:TickEvents,dt:f32){
  for(var i=0u;i<e.count;i++){(*s).drive.x=10.0*(*s).drive.x+e.values[i].x;(*s).drive.y+=1.0;}
}`}),{go});
save("events",{...headless(impulse),rowSteps:[{...impulse.steps,home:{kind:"events",slots:2}}]});

// A host can replace the complete law and present its own observations in normal rows.
const follow = gaze + `
fn step(s:ptr<function,State>,b:ptr<function,Plan>,look:vec4f,dt:f32) {
  let a=(*s).tilt; let g=follow_gaze(Gaze(a.x,a.y,a.z,a.w),look.x,dt,1.9,sqrt(1.9/9.81),look.y>0.0);
  (*s).tilt=vec4f(g.bearing,g.unwrapped,g.yaw,g.neck); (*s).stamps.x+=1.0;
}`;
const measure = hop + `
fn present(s:ptr<function,State>,b:ptr<function,Plan>,f:f32,dt:f32)->Pose {
  let t=((*s).stamps.x+f)*dt;
  let a=(*s).tilt; let g=Gaze(a.x,a.y,a.z,a.w);
  var p:Pose; p.margins[0]=vec4f(hop_at(t,3.876,9.81,3.0),hop_stance(3.876,9.81,3.0));
  p.margins[1]=vec4f(g.neck,g.yaw,gaze_error(g),g.unwrapped); return p;
}`;
const law=roam({step:follow,present:measure});
for(const [name,rate,ground] of [["tracking",1,true],["saturated",2,true],["flight",1,false]] as const) {
  save(name,headless(move(pads,law,{go,script:k=>[Math.atan2(Math.sin(rate*k/120),Math.cos(rate*k/120)),Number(ground)]})));
}

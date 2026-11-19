// A CPU observer runs the linked WGSL over the same packed word tables as the device.
import { world, type World, type V3 } from "./record.ts";
import { SceneError } from "../errors.ts";

export type Pick = {kind:"miss"} | {kind:"hit";id:number;t:number;normal:V3;uv:readonly [number,number]};
export async function twin(scene:World) {
  const [{WgslExec,WgslParser},{native}]=await Promise.all([import("wgsl_reflect"),import("../core/native.ts")]);
  const w=world(scene);
  const source=w.query(`
@group(2) @binding(0) var<storage,read> probe:array<vec4f,2>;
@group(2) @binding(1) var<storage,read_write> answer:array<vec4u,2>;
@compute @workgroup_size(1) fn pick_probe() {
  let x=trace(Ray(probe[0].xyz,probe[1].xyz,probe[0].w,probe[1].w,vec2f(0)));
  answer[0]=bitcast<vec4u>(vec4f(x.n,x.t));
  answer[1]=vec4u(rows[x.thing].id,bitcast<vec2u>(x.uv),0u);
}
@compute @workgroup_size(1) fn visibility_probe() {
  answer[0]=bitcast<vec4u>(vec4f(visible(probe[0].xyz,probe[1].xyz),1));
}`);
  const interpreter=new WgslExec(new WgslParser().parse(native().link(source)));
  const input=new Float32Array(8),words=new Uint32Array(8),values=new Float32Array(words.buffer);
  const groups={1:{0:Uint32Array.from(w.tables[0].words),1:Uint32Array.from(w.tables[1].words)},2:{0:input,1:words}};
  return {
    trace(o:V3,d:V3,t0=0,t1=1e30):Pick {
      if (![...o,...d,t0,t1].every(x=>Number.isFinite(Math.fround(x))) || t1<t0 || Math.abs(Math.hypot(...d)-1)>1e-5)
        throw new SceneError("trace: use finite coordinates, a unit direction and an ordered distance interval");
      input.set([...o,t0,...d,t1]);interpreter.dispatchWorkgroups("pick_probe",1,groups);
      return values[3]<0 ? {kind:"miss"} : {kind:"hit",t:values[3],id:words[4],normal:[values[0],values[1],values[2]],uv:[values[5],values[6]]};
    },
    visible(a:V3,b:V3):V3 {
      if (![...a,...b].every(x=>Number.isFinite(Math.fround(x)))) throw new SceneError("visible: use two finite points");
      input.set([...a,0,...b,0]);interpreter.dispatchWorkgroups("visibility_probe",1,groups);
      return [values[0],values[1],values[2]];
    },
  };
}

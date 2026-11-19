// bun tools/slice_oracle.ts   Integer masks, explicit axis/inside rays and the two-camera record check.
import {WgslExec,WgslParser} from "wgsl_reflect";
import {writeFileSync} from "node:fs";
import {build,picture,out} from "./slice_measure.ts";
import {ring,card} from "../examples/slice/washer.ts";
import {bead,washer} from "../src/kinds/shapes.ts";
import {thing,world,raster,screenDraw,write,type Scene} from "../src/render/record.ts";
import {inspect} from "../src/engines/inspect.ts";
import {print} from "../src/engines/print.ts";
import {find} from "../src/render/find.ts";
import base from "../src/render/interface.wgsl" with {type:"text"};
const cameras:Scene["cameras"]={front:{kind:"orthographic",eye:[0,0,3],target:[0,0,0],height:8},side:{kind:"perspective",eye:[2,3,4],target:[0,0,0],fovY:1}};
const report:Record<string,unknown>={};
let ok=true;
const score=(name:string,data:unknown,passed:boolean)=>{report[name]={data,passed};ok&&=passed;console.log(name,data,passed);};
for (const [inner,solid] of [[0.5,false],[0.25,false],[0.5,true]] as const) {
  const plate={...ring,shape:washer(inner,1)};
  const things=[plate,card];
  const shadows=solid?[{...plate,shape:washer(0,1)},card]:things;
  const shade=`fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f {
    let p=vec3f((px.x/64)-4,4-px.y/64,0);
    let blocked=1-visible(p,p+vec3f(20,0,10)).r;
    return vec4f(f32(rows[x.thing].id)/255,blocked,0,1);
  }`;
  const file=build("washer-"+inner+"-"+solid,{things,shadows,cameras,engine:print({shade,samples:1})});
  const {rgba}=picture(file,"512x512");
  let primary=0,shadow=0,badIDs=0,badShadow=0;
  for(let y=0;y<512;y++) for(let x=0;x<512;x++) {
    const dx=2*x-511,dy=2*y-511,i=4*(x+512*y);
    const ringAt=(dx:number,r:number)=>{const q=dx*dx+dy*dy;return q<=128**2 && q>=(128*r)**2;};
    const id=rgba[i],blocked=rgba[i+1]===255;
    primary+=Number(id===7); shadow+=Number(blocked);
    badIDs+=Number(id!==(ringAt(dx,inner)?7:9));badShadow+=Number(blocked!==ringAt(dx+256,solid?0:inner));
  }
  score("4-5 "+inner+" solid "+solid,{primary,shadow,badIDs,badShadow},badIDs===0 && badShadow===0);
  if(!solid) {
    const probe=inspect(`fn pixel(px:vec2f)->vec4f {
      let k=u32(px.x);let z=select(3.0,2.0,k>=2u);
      let x=trace(Ray(vec3f(0,select(0.0,0.375,k%2u==1u),z),vec3f(0,0,-1),0,10,vec2f(0)));
      return vec4f(f32(rows[x.thing].id)/255,x.t/255,0,1);
    }`);
    const {rgba:r}=picture(build("pick"+inner,{things,cameras,engine:probe}),"4x1");
    const got=Array.from({length:4},(_,k)=>[r[4*k],r[4*k+1]]);
    score("6 "+inner,got,JSON.stringify(got)===JSON.stringify(inner===0.5?[[9,3],[9,3],[9,2],[9,2]]:[[9,3],[7,2],[9,2],[7,1]]));
  }
}
const value=bead(1,0.35,0.06);
const rays=`fn probe_ray(k:u32)->Ray {
  let origins=array<vec3f,4>(vec3f(0,3,0),vec3f(0.6,3,0),vec3f(0,0,0),vec3f(0.6,0,0));
  return Ray(origins[k],select(vec3f(0,-1,0),vec3f(1,0,0),k>=2u),0,10,vec2f(0));
}`;
const probe=`fn pixel(px:vec2f)->vec4f {let h=intersect(0,probe_ray(u32(px.x)));return unpack4x8unorm(bitcast<u32>(select(h.t,h.n.x,px.y>1)));}`;
const {rgba}=picture(build("bore",{things:[thing(value,ring.material)],cameras,engine:inspect(rays+probe)}),"4x2");
const gpu=Array.from(new Float32Array(rgba.buffer,rgba.byteOffset,8));
const cpu=new Float32Array(8).fill(NaN);
const shader=base+value.kind.source+rays+`
@group(2) @binding(0) var<storage,read_write> res:array<f32,8>;
@compute @workgroup_size(1) fn probe(@builtin(global_invocation_id) gid:vec3u) {
  let k=gid.x;let h=Bead_hit(load_Bead(0u),probe_ray(k));res[k]=h.t;res[k+4u]=h.n.x;
}`;
const exec=new WgslExec(new WgslParser().parse(shader));
exec.dispatchWorkgroups("probe",4,{1:{1:Uint32Array.from(value.words)},2:{0:cpu}});
const exact=[-1,2.2,0.35,0.4,0,0.6,-1,1];
score("7",{cpu:Array.from(cpu),gpu},[...cpu,...gpu].every((n,i)=>Math.abs(n-exact[i%8])<1e-5));

const engine=(s:Scene)=>{
  const w=world(s),f=find(w,4);
  const diagnostic=`
@group(2) @binding(0) var ids:texture_multisampled_2d<u32>;
@group(2) @binding(1) var depths:texture_depth_multisampled_2d;
@fragment fn compare(@builtin(position) px:vec4f)->@location(0) vec4f {
  let p=vec2i(px.xy);let id=textureLoad(ids,p,0);let r=camera_ray(px.xy);let x=trace(r);
  for(var j=1;j<4;j++){if any(textureLoad(ids,p,j)!=id){return vec4f(0,0,1,1);}}
  let t=dot(unproject(px.xy,textureLoad(depths,p,0))-r.o,r.d);
  let bad=select(id.x!=x.thing+1u || abs(t-x.t)>1e-4*x.t,id.x!=0u,x.t<0);
  return vec4f(f32(bad),0,0,1);
}`;
  return {tables:w.tables, modules:[f.module,w.query(diagnostic)],schedule:{images:f.images,passes:[f.pass,raster("compare",1,[screenDraw("compare")],[write("Output")],[{Now:0},{Now:1}])]}};
};
const file=build("record-trace",{things:[ring,card],cameras,engine});
for(const camera of ["front","side"]) {
  const {rgba}=picture(file,"512x512",camera);let bad=0,edge=0;
  for(let i=0;i<rgba.length;i+=4) {bad+=Number(rgba[i]>0);edge+=Number(rgba[i+2]>0);}
  score("8 "+camera,{bad,edge},bad<=26);
}
writeFileSync(out+"/geometry.json",JSON.stringify(report,null,2));
process.exitCode=ok?0:1;

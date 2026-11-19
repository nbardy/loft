import {writeFileSync} from "node:fs";
import {build,picture,run,draw,out} from "./slice_measure.ts";
import {closed,white} from "../examples/slice/furnace.ts";
import {slice} from "../examples/slice/slice.ts";
import {reference} from "../src/engines/reference.ts";
import {realtime} from "../src/engines/realtime.ts";
import {world,raster,screenDraw,write,type Scene} from "../src/render/record.ts";
import {find} from "../src/render/find.ts";
import {readPng} from "./picdiff.ts";
const float="fn present(c:vec3f)->vec4f {return unpack4x8unorm(bitcast<u32>(c.r));}";
const report:Record<string,unknown>={};let ok=true;
function score(name:string,data:unknown,pass:boolean) {ok&&=pass;report[name]={data,pass};console.log(name,data,pass);}
for(const depth of [1,2,16]){
  const f=build("furnace"+depth,{...closed,engine:reference({samples:1024,depth,present:float})});
  const a=picture(f,"2x2").rgba;
  const values=Array.from(new Float32Array(a.buffer,a.byteOffset,4));
  const expected=2*(1-Math.pow(0.5,depth+1));
  score("9 D="+depth,values,values.every(x=>Math.abs(x/expected-1)<=0.01));
}
const whiteFile=build("white-furnace",{...white,engine:reference({samples:1024,depth:1,present:float})});
const a=picture(whiteFile,"32x32").rgba;
const fv=new Float32Array(a.buffer,a.byteOffset,1024);
score("10",{centre:fv[16*32+16],rim:fv[16*32+29]},[fv[528],fv[541]].every(x=>Math.abs(x/0.5-1)<=0.02));
const s=slice(10),size="48x48";
// Each colour channel is measured as f32 before display. The mask is read back independently of either colour.
const interior=(s:Scene)=>{
 const w=world(s),f=find(w,4);
 const mask=w.query(`@group(2) @binding(0) var ids:texture_multisampled_2d<u32>;
 @fragment fn mask(@builtin(position) p:vec4f)->@location(0) vec4f {
   let q=vec2i(p.xy);let id=textureLoad(ids,q,0);
   for(var k=1;k<4;k++){if any(id!=textureLoad(ids,q,k)) {return vec4f(0);}}
   return vec4f(1);
 }`);
 return {tables:w.tables,modules:[f.module,mask],schedule:{images:f.images,passes:[f.pass,raster("mask",1,[screenDraw("mask")],[write("Output")],[{Now:0}])]}};
};
const mask=picture(build("interior",{...s,engine:interior}),size).rgba;
const biases=[];
for(const c of ["r","g","b"]) {
 const sampling={samples:256,depth:1,present:float.replace("c.r","c."+c)};
 const render=(engine:Scene["engine"],name:string)=>{
   const a=picture(build(name+c,{...s,engine}),size).rgba;
   return new Float32Array(a.buffer,a.byteOffset,a.length/4);
 };
 const rt=render(realtime(sampling),"rt"),ref=render(reference({...sampling,primary:"fn primary(px:vec2f,k:u32)->Ray{return camera_ray(px);}"}),"ref-centre");
 let sum=0,total=0,n=0,relative=0;
 for(let i=0;i<rt.length;i++) if(mask[4*i] && ref[i]>1e-8) {sum+=rt[i]-ref[i];total+=ref[i];relative+=(rt[i]-ref[i])/ref[i];n++;}
 biases.push({channel:c,interior:n,relativeBias:sum/total,meanRelative:relative/n});
}
score("11",biases,biases.every(x=>Math.abs(x.meanRelative)<=0.01 && Math.abs(x.relativeBias)<=0.01));
const one=build("isolated",{...s,engine:realtime({samples:4})});
const alone=picture(one,size).rgba;
const times=[0,0.5,1,1.37];
run([draw,"still",one,"--view","front","--size",size,"--time","0",...times.slice(1).flatMap(t=>["--shot",`front@${t}@${size}=${out}/clip-${t}.png`])]);
const clip=readPng(out+"/clip-1.37.png").rgba;
const same=alone.every((v,i)=>v===clip[i]);
const historyErrors=[];
for(const channel of ["r","g","b"]) {
  const present=float.replace("c.r","c."+channel);
  const viewer=build("viewer"+channel,{...s,engine:realtime({history:true,present})});
  const history=out+"/history-"+channel+".png";
  run([draw,"still",viewer,"--view","front","--size",size,...Array.from({length:63},(_,i)=>["--shot",`front@${(i+1)/60}@${size}=${history}`]).flat()]);
  const pic=readPng(history).rgba;const hist=new Float32Array(pic.buffer,pic.byteOffset,pic.length/4);
  const still=picture(build("still64"+channel,{...s,engine:realtime({samples:64,present})}),size).rgba;
  const actual=new Float32Array(still.buffer,still.byteOffset,still.length/4);
  historyErrors.push(actual.reduce((sum,x,i)=>sum+Math.abs(x-hist[i])/Math.max(x,1e-5),0)/hist.length);
}
const mean=historyErrors.reduce((sum,x)=>sum+x,0)/3;
score("14",{same,channelErrors:historyErrors,meanRelative:mean},same && mean<=0.01);
writeFileSync(out+"/light.json",JSON.stringify(report,null,2));process.exitCode=ok?0:1;

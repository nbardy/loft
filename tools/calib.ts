// GPU time per scalar operation, coherent load, and extra draw at the rates used by the slice.
import {writeFileSync} from "node:fs";
import {build,run,draw,out} from "./slice_measure.ts";
import {slice} from "../examples/slice/slice.ts";
import {bits} from "../src/text/kind.ts";
import {raster,write,screenDraw} from "../src/render/record.ts";
import screen from "../src/render/covers/screen.wgsl" with {type:"text"};
const rows=Array.from({length:65536},(_,i)=>bits((i+1)/65536));
function sample(name:string,body:string,repeat:number,size:string) {
  const text=screen+`
@group(1) @binding(0) var<storage,read> data:array<f32>;
@fragment fn bench(@builtin(position) p:vec4f)->@location(0) vec4f {
  let idx=u32(p.y)*256u+u32(p.x);var x=data[idx&65535u];
  ${body}
  return vec4f(x,x,x,1);
}`;
  const f=build("cal-"+name,{...slice(10),engine:()=>({tables:[{lanes:1,words:rows}],modules:[{text,opens:[]}],schedule:{images:[],passes:[
    raster(name,0,[screenDraw("bench")],[write("Output")],[],repeat)]}})});
  const report=run([draw,"profile",f,"--view","front","--size",size,"--frames","9"]);
  return JSON.parse(report.trim().split("\n").at(-1)!).profile.pass_ms[0][1] as number;
}
const base=sample("base","",1,"256x256");
const alu=sample("alu","for(var k=0u;k<256u;k++){x=fma(x,0.99823,0.00091);}",1,"256x256");
const load=sample("load","for(var k=0u;k<128u;k++){x+=data[(idx+1507u*k)&65535u];} x/=129;",1,"256x256");
const one=sample("one","",1,"8x8"),many=sample("draws","",257,"8x8");
const c=(alu-base)*1e9/(256**2*512),l=((load-base)*1e9/(256**2*128))-c,d=(many-one)*1000/256;
const result={base,alu,load,one,many,scalar_ps:c,coherent_ps:l,draw_us:d};
console.log(result);writeFileSync(out+"/calibration.json",JSON.stringify(result,null,2));

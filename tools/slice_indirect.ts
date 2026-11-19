// A frame changes copy levels. Compare two indexed indirect draws to the CPU slot oracle after a single load.
import {writeFileSync} from "node:fs";
import {grid,compact,raster,write, type Scene} from "loft/render";
import type {Declarations} from "../src/record/passes.ts";
import {slice} from "../examples/slice/slice.ts";
import {build,out,draw,run} from "./slice_measure.ts";
import {readPng} from "./picdiff.ts";

const levels=[[1,0,2,1],[0,2,1,0],[0,0,0,0]];
const cells=[grid(1,1),grid(2,2)];
const law=`
struct Frame { vp:mat4x4f,inv:mat4x4f,eye:vec4f,forward:vec4f,viewport:vec4f,time:vec4f }
@group(0) @binding(0) var<uniform> frame:Frame;
@group(1) @binding(0) var<storage,read> choices:array<vec4u>;
fn copies()->u32 {return 4;}
fn level(i:u32)->u32 {return choices[min(u32(frame.time.x),2u)][i];}`;
const shaders=cells.map((_,i)=>`
@group(1) @binding(${3+i}) var<storage,read> chosen:array<u32>;
@vertex fn place(@builtin(vertex_index) v:u32,@builtin(instance_index) k:u32)->@builtin(position) vec4f {
  let uv=vec2f(f32(v%${i+2}u),f32(v/${i+2}u))/${i+1}.0;
  return vec4f(-1+(f32(chosen[8u+k])+uv.x)*0.5,1-2*uv.y,0,1);
}
@fragment fn color()->@location(0) vec4f {return vec4f(${i===0?"1,0":"0,1"},0,1);}`);
const engine=():Declarations=>({
  tables:[{lanes:1,words:levels.flat()},...cells.map(c=>c.table),...cells.map((_,i)=>({
    lanes:1,rows:3,module:0,entry:"level_"+(i+1),extent:[1,1,1] as [number,number,number],when:"Frame" as const
  }))],
  modules:[compact(law,cells.map(c=>c.indices)),...shaders.map(text=>({text,opens:[]}))],
  schedule:{images:[],passes:cells.map((c,i)=>raster("level"+(i+1),i+1,[{
    vertex:"place",fragment:"color",vertices:c.indices,indices:i+1,instances:{Indirect:3+i},cull:"None"
  }],[i===0?write("Output"):{target:"Output",clear:"Load",blend:"Opaque"}]))},
});
const file=build("compact-grid",{...slice(10),engine} satisfies Scene);
run([draw,"still",file,"--view","front","--size","64x16","--time","0","--out",out+"/grid0.png",
    "--shot",`front@1@64x16=${out}/grid1.png`,"--shot",`front@2@64x16=${out}/grid2.png`]);
const measurements=levels.map((slots,t)=>{
  const {rgba}=readPng(`${out}/grid${t}.png`);let coarse=0,fine=0,wrong=0;
  for(let i=0;i<1024;i++){
    const r=rgba[4*i],g=rgba[4*i+1],expected=slots[Math.floor(i%64/16)];
    coarse+=Number(r===255);fine+=Number(g===255);
    wrong+=Number(r!==(expected===1?255:0)||g!==(expected===2?255:0)||rgba[4*i+2]!==0);
  }
  return {t,coarse,fine,wrong};
});
console.log(measurements);
writeFileSync(out+"/indirect.json",JSON.stringify(measurements,null,2));
process.exitCode=Number(measurements.some(s=>s.wrong));

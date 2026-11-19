// Native build, GPU readback and bundle inspection shared by the slice instruments.
import {mkdirSync, readFileSync} from "node:fs";
import {resolve} from "node:path";
import {native, UNLIMITED} from "../src/core/native.ts";
import {cameraOf} from "../src/draw/record.ts";
import {rec} from "../src/record/recorder.ts";
import {pack,Program} from "../src/record/wire.ts";
import type { Scene } from "../src/render/record.ts";
import {readPng} from "./picdiff.ts";
export const out=resolve("out/slice");
export const draw=resolve("rust/target/lab/loft-draw");
export function run(args:string[]) {
  const r=Bun.spawnSync(args);
  if(r.exitCode!==0) throw Error(`${args.join(" ")}\n${r.stderr.toString()}\n${r.stdout.toString()}`);
  return r.stdout.toString();
}
export function build(name:string, s:Scene) {
  mkdirSync(out,{recursive:true});
  const decl=s.engine(s);
  const cameras=Object.entries(s.cameras).map(([n,c])=>cameraOf(n,c));
  const job=pack(rec.job(Program.Scene,[],{cameras,passes:[],checks:[],solves:[]}),decl);
  const file=resolve(out,name+".loft");
  native().run(job,{nodes:UNLIMITED,tapeOps:UNLIMITED,boxes:UNLIMITED,gpuNanos:UNLIMITED},file,{naga:"emit",precision:"f32"});
  return file;
}
export function picture(file:string,size:string,view="front",time=1.37) {
  const png=file+".png";
  run([draw,"still",file,"--view",view,"--time",String(time),"--size",size,"--out",png]);
  return readPng(png);
}
export function sections(file:string) {
  const bytes=readFileSync(file),len=Number(bytes.readBigUInt64LE(8));
  const b=JSON.parse(bytes.subarray(16,16+len).toString());
  let at=16+len;
  const data:Buffer[]=b.sections.map((n:number)=>{at=Math.ceil(at/64)*64;const s=bytes.subarray(at,at+n);at+=n;return s;});
  return {bundle:b,texts:b.modules.map((m:{wgsl:number})=>data[m.wgsl].toString()) as string[],data};
}

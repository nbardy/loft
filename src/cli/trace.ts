import { mkdirSync, mkdtempSync, rmSync } from "node:fs";
import { resolve } from "node:path";
import { native, UNLIMITED, type Verdict } from "../core/native.ts";
import { rec } from "../record/recorder.ts";
import { pack, Program } from "../record/wire.ts";
import type { Scene } from "../record/decls.ts";
import { LOFT_DRAW } from "./serve.ts";
import { EnvironmentError } from "../errors.ts";
import { UsageError } from "./args.ts";
import { ChecksFailed } from "./verdict.ts";

export async function checkRecorded(scene:Scene){
  if(!scene.traces?.length) throw new UsageError("this recording declares no observations: include trace sites from the presented rows");
  mkdirSync("out",{recursive:true});
  const dir=mkdtempSync(resolve("out/trace-"));
  try {
    native().run(pack(rec.job(Program.Scene,[],scene)),{nodes:UNLIMITED,tapeOps:UNLIMITED,boxes:UNLIMITED,gpuNanos:UNLIMITED},
      dir+"/scene.loft",{naga:"emit",precision:"f32"});
    let failed=false;
    for(const group of scene.traces){
      const r=Bun.spawnSync([LOFT_DRAW,"trace",dir,String(group.step),JSON.stringify(group.sites)]);
      if(r.exitCode!==0 && r.exitCode!==5) throw new EnvironmentError(r.stderr.toString(),false);
      for(const line of r.stdout.toString().trim().split("\n")){
        const {label,observations,verdict:v}=JSON.parse(line) as {label:string;observations:number;verdict:Verdict};
        if(typeof v.scope!=="object" || !("Trace" in v.scope)) throw new Error("expected a GPU trace verdict");
        const scope=v.scope.Trace;
        console.log(`${label}: ${v.truth}, margin ${v.margin.toPrecision(5)} at tick ${v.witness[0][1]} · ${observations}/${scope.ticks} samples · ${scope.by} · input ${Buffer.from(scope.inputs).toString("hex").slice(0,12)} · law ${Buffer.from(scope.law).toString("hex").slice(0,12)}`);
      }
      failed ||= r.exitCode===5;
    }
    if(failed) throw new ChecksFailed("a signed margin failed on the recorded GPU take");
  } finally {rmSync(dir,{recursive:true,force:true});}
}

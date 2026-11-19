import { world, write, raster, screenDraw, type Scene } from "../render/record.ts";
import { find, shadePixel, identityFinish } from "../render/find.ts";
import { lighting, albedoFill } from "../light/queries.ts";
import { resolve,film,sampling,type Sampling } from "./reference.ts";
import screen from "../render/covers/screen.wgsl" with {type:"text"};
import accumulate from "../light/history.wgsl" with {type:"text"};
import type { Declarations, Image } from "../record/passes.ts";

export const realtime=(o:Sampling & {history?:boolean;shade?:string}={})=>(s:Scene):Declarations=>{
  const {samples,depth}=sampling(o),w=world(s),f=find(w,4);
  const hist=o.history??false,l=lighting(s,w,hist?4:3);
  const shade=o.shade??`fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f {
    var L=vec3f(0);let first=${hist?`bitcast<u32>(frame.time.y)*${samples}u`:"0u"};
    for(var s=0u;s<${samples}u;s++){L+=transport(x,wo,vec2u(px),first+s,${depth}u);}
    return vec4f(L/${samples}.0,1);
  }`;
  const body=shadePixel(4,shade,"fn background(r:Ray)->vec4f {return vec4f(sky(r.d),1);}")+identityFinish;
  const modules=[f.module,l.module(body),resolve(o.present??film,false)];
  const images:Image[]=[...f.images,{format:"Rgba16Float",samples:1}];
  const passes=[f.pass,raster("shade",1,[screenDraw("shade_pixel")],[write({Image:2})],[{Now:0},{Now:1}])];
  if(hist) {
    images.push({format:"Rgba16Float",samples:1},{format:"Rgba16Uint",samples:1});
    modules.push({text:screen+accumulate,opens:[]});
    passes.push(raster("history",3,[screenDraw("accumulate")],[write({Image:3}),write({Image:4})],[{Now:2},{Now:0},{Previous:3},{Previous:4}]));
  }
  if(l.hasMetal) modules.push(albedoFill);
  passes.push(raster("film",2,[screenDraw("finish")],[write("Output")],[{Now:hist?3:2}]));
  return {tables:l.tables,modules,schedule:{images,passes}};
};

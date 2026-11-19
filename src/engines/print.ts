import { find, shadePixel, identityFinish } from "../render/find.ts";
import { world, raster, write, screenDraw, type Scene } from "../render/record.ts";

export const flat = `fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f {
  let id=rows[x.thing].id;
  let c=vec3f(vec3u(id>>16u,id>>8u,id)&vec3u(255))/255;
  return vec4f(select(c,vec3f(0.1125,0.0525,0.1125),ink),1);
}`;

export const print = (look: { shade:string; background?:string; finish?:string; samples?:1|4 }) => (s:Scene) => {
  const w=world(s), samples=look.samples??4, f=find(w,samples);
  const body=shadePixel(samples,look.shade,look.background??"fn background(r:Ray)->vec4f {return vec4f(0,0,0,1);}");
  return { tables:w.tables,modules:[f.module,w.query(body+(look.finish??identityFinish))],
    schedule:{images:f.images,passes:[f.pass,raster("shade",1,[screenDraw("shade_pixel")],[write("Output")],[{Now:0},{Now:1}])]} };
};

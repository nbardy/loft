import { world, raster, screenDraw, write, type Scene } from "../render/record.ts";

/** An engine is a function returning an ordinary pass list. Pixel coordinates have a top-left origin. */
export const inspect = (pixel = `fn pixel(px:vec2f)->vec4f {
  let x=trace(camera_ray(px));
  if x.t<0 {return vec4f(0.18,0.18,0.18,1);}
  return vec4f(0.5*x.n+0.5,1);
}`) => (s:Scene) => {
  const w=world(s);
  return { tables:w.tables, modules:[w.query(pixel+`
@fragment fn inspect(@builtin(position) p:vec4f)->@location(0) vec4f {return pixel(p.xy);}`)],
    schedule:{images:[],passes:[raster("inspect",0,[screenDraw("inspect")],[write("Output")])]} };
};

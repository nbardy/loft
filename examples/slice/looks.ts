import { kind, scene, thing, sphere, washer, print, toon, type Scene } from "loft/render";
import { ring, card } from "./washer.ts";

const shell = kind(`
struct Shell { base:vec3f, turn:vec3f, rough:f32 }
fn eval(m:Shell,wo:vec3f,wi:vec3f)->vec3f {
  let h=normalize(wo+wi);
  let hue=mix(m.base,m.turn,pow(1-dot(wo,h),2));
  return hue*ggx(m.rough,wo,wi);
}
fn surface(m:Shell,x:Point)->Shell {
  return Shell(m.base,m.turn,mix(m.rough,0.5,step(0.5,fract(8*x.uv.x))));
}`);
export const striped = thing(sphere(0.5), shell([0.1, 0.4, 0.2], [0.5, 0.1, 0.6], 0.15), [0, 0.5, 0]);

const things = [ring, card, striped];
export const shadows = things.map((t) => (t === ring ? thing(washer(0, 1), ring.material, ring.at) : t));

const cameras: Scene["cameras"] = { front: { kind: "perspective", eye: [2, 1.5, 4], target: [0, 0, 0], fovY: 0.8 } };
export default scene({ things, shadows, cameras, engine: print({ shade: toon }) });

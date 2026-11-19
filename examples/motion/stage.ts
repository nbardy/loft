// Explicit instanced grid draws. Each ornament names a rigid frame and its own local site.
import { f, fetch, add, mul, lib, vec2, vec3, type DataTable, type Expr, type LibFn } from "../../src/expr.ts";
import { recorded, role, type Camera, type Draw } from "../../src/record/decls.ts";
import type { move } from "loft/motion";

export const stick=(k:number):[number,number]=>[0,k<24 ? 0 : k<204 ? 1.2 : k<384 ? -2.1 : 0];
type Motion=ReturnType<typeof move>;
type V3=readonly [number,number,number];
const raw=(name:string,ret:LibFn["ret"],body:string):LibFn=>({name,ret,wgsl:body,js:"gpu-only"});
const shade=raw("carapace_paint","vec4",`fn carapace_paint(n:vec3f,p:vec3f,c:vec4f,uv:vec2f)->vec4f {
  if c.w<0.0 {return vec4f(0.61,0.51,0.44,1);}
  let sun=normalize(vec3f(-0.45,0.82,-0.35)); let light=max(0.0,dot(n,sun));
  let gloss=pow(max(dot(n,normalize(sun+normalize(frame.eye.xyz-p))),0.0),24.0);
  let edge=0.76+0.24*smoothstep(0.02,0.28,abs(dot(n,normalize(frame.eye.xyz-p))));
  return vec4f(c.rgb*(0.43+0.57*light)*edge+vec3f(0.3,0.22,0.12)*gloss,1);
}`);
const surface=raw("body_surface","vec3",`fn body_surface(uv:vec2f,at:vec4f,q:vec4f,site:vec4f,dim:vec4f,flag:f32,capsule:f32,normal:f32)->vec3f{
  let a=-6.2831853*uv.x;let b=3.14159265*(uv.y-0.5);
  var axis=vec3f(cos(a)*cos(b),sin(b),sin(a)*cos(b));var local=axis*dim.xyz+site.xyz;
  if capsule>0.5 {
    let t=uv.y*3.0;let lat=clamp(t-1.0,-1.0,0.0)*1.5707963+clamp(t-2.0,0.0,1.0)*1.5707963;
    axis=vec3f(cos(a)*cos(lat),sin(lat),sin(a)*cos(lat));
    local=axis*dim.x+vec3f(0.0,clamp(t-1.0,0.0,1.0)*dim.y,0.0);
  } else {axis=normalize(axis/dim.xyz);}
  var v=select(local,axis,normal>0.5);
  v=v+2.0*cross(q.xyz,cross(q.xyz,v)+q.w*v);
  if normal<0.5 {v+=at.xyz+vec3f(dim.w,0,0);}
  if flag<0.0 && normal<0.5 {v=vec3f(v.x+v.y*0.42,0.006,v.z+v.y*0.28);}
  return v;
}`);
function solids(name:string,rows:number[],motion:Motion,capsule:boolean):Draw{
  const count=rows.length/12;const table:DataTable={name,entries:count,lanes:3,data:Float32Array.from(rows),role:"static"};
  const geom=(uv:Expr<"vec2">,id:Expr<"f32">,normal:number)=>{
    const site=fetch(table,id,0),dim=fetch(table,id,1),colour=fetch(table,id,2),k=site.w;
    return lib<"vec3">(surface,uv,fetch(motion.pose,k,0),fetch(motion.pose,add(k,1),0),site,dim,colour.w,Number(capsule),normal);
  };
  const [u,v]=capsule ? [12,9] as const : [24,16] as const;
  return {name,layer:"subject",raster:{cull:"back",depthWrite:true,blend:"opaque"},
    cover:{kind:"grid",u,v,count,place:role(name+"Place",["vec2","f32"],(uv,id)=>geom(uv,id,0)),
      normal:role(name+"Normal",["vec2","f32"],(uv,id)=>geom(uv,id,1)),cells:role(name+"Cells",["f32"],()=>vec2(u,v))},
    paint:role(name+"Paint",["vec3","vec3","vec2","f32"],(p,n,uv,id)=>lib<"vec4">(shade,n,p,fetch(table,id,2),uv))};
}
const sand=raw("sand_lines","vec4",`fn sand_lines(p:vec3f)->vec4f {
  let d=abs(fract(p.xz*0.5+0.5)-0.5);
  let w=max(fwidth(p.xz)*0.5,vec2f(0.001));
  let lines=1.0-smoothstep(vec2f(0.0),w+0.009,d);let line=max(lines.x,lines.y);
  let fleck=0.004*sin(dot(p.xz,vec2f(18.1,12.9)))*sin(p.x*8.1);
  return vec4f(mix(vec3f(0.92,0.81,0.64)+fleck,vec3f(0.75,0.65,0.51),line),1);
}`);
export function stage(motion:Motion){
  const capsules:number[]=[], ellipsoids:number[]=[];
  motion.bodies.forEach((b,j)=>{
    const offset=(j-(motion.bodies.length-1)/2)*12;
    const addPart=(rows:number[],joint:number,site:V3,dim:V3,col:V3)=>{
      for(const shadow of [-1,1]) rows.push(...site,j*58+2*joint,...dim,offset,...col,shadow);
    };
    const red:V3=[0.94,0.24,0.088],dark:V3=[0.26,0.11,0.095];
    const [x,y,z]=b.trunk.size;
    addPart(ellipsoids,0,[0,0,0],[0.85*x,y,0.67*z],red);
    for(const side of [-1,1]){
      addPart(ellipsoids,0,[3.45,0.64,1.65*side],[0.50,0.67,0.50],[0.97,0.70,0.28]);
      addPart(ellipsoids,0,[3.77,0.88,1.70*side],[0.28,0.33,0.28],dark);
      addPart(ellipsoids,0,[3.4,-0.24,3.0*side],[1.65,0.55,0.76],red);
      addPart(ellipsoids,0,[4.65,-0.16,3.36*side],[0.89,0.25,0.24],red);
      addPart(ellipsoids,0,[4.75,-0.12,2.76*side],[0.74,0.22,0.22],red);
    }
    b.chains.forEach((chain,i)=>{
      chain.lengths.forEach((len,k)=>addPart(capsules,1+3*i+k,[0,0,0],[[0.26,0.20,0.12][k]!,len,1],k===2?dark:red));
      addPart(ellipsoids,3+3*i,[0,chain.lengths[2],0],[0.20,0.16,0.2],i%2 ? [0.26,0.67,0.66] : [0.99,0.72,0.24]);
    });
  });
  const ground:Draw={name:"deck",layer:"surroundings",raster:{cull:"none",depthWrite:true,blend:"opaque"},
    cover:{kind:"grid",u:1,v:1,count:1,place:role("deckPlace",["vec2","f32"],uv=>vec3(mul(add(uv.x,-0.5),300),0,mul(add(uv.y,-0.5),300))),
      normal:role("deckNormal",["vec2","f32"],()=>vec3(0,1,0)),cells:role("deckCells",["f32"],()=>vec2(1,1))},
    paint:role("deckPaint",["vec3","vec3","vec2","f32"],p=>lib<"vec4">(sand,p))};
  const camera:Camera={name:"game",kind:"look",eye:vec3(17,14,15),target:vec3(0,1.8,-0.2),up:vec3(0,1,0),fovy:f(0.63)};
  return recorded({cameras:[camera],passes:[{camera,draws:[ground,solids("limbs",capsules,motion,true),solids("shell",ellipsoids,motion,false)],
    clear:{kind:"colour",rgba:[0.9,0.8,0.66,1]},ground:{kind:"none"},outline:{kind:"none"}}],rowSteps:[motion.steps],traces:[{step:0,sites:motion.holds}],checks:[],solves:[]});
}

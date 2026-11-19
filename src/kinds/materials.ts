import { kind } from "../text/kind.ts";
export const matte = kind(`
struct Matte { base:vec3f }
fn eval(m:Matte,wo:vec3f,wi:vec3f)->vec3f {return m.base*(max(wi.z,0)/PI);}
fn albedo(m:Matte)->vec3f {return m.base;}`);
const lampKind = kind(`
struct Lamp { base:vec3f, power:vec3f }
fn eval(m:Lamp,wo:vec3f,wi:vec3f)->vec3f {return m.base*(max(wi.z,0)/PI);}
fn emit(m:Lamp,wo:vec3f)->vec3f {return m.power;}
fn albedo(m:Lamp)->vec3f {return m.base;}`);
export const lamp = (base: readonly number[], emission: readonly number[]) => ({
  ...lampKind(base, emission),
  emission,
});
export const glow = (power: readonly number[]) => lamp([0, 0, 0], power);

export const metal = kind(`
struct Metal { base:vec3f, rough:f32 }
fn eval(m:Metal,wo:vec3f,wi:vec3f)->vec3f {
  if wo.z<=0 || wi.z<=0 {return vec3f(0);}
  let h=normalize(wo+wi);
  let F=m.base+(1-m.base)*pow(1-max(dot(wi,h),0),5);
  let single=F*ggx(m.rough,wo,wi);
  let eo=directional_albedo(wo.z,m.rough);let ei=directional_albedo(wi.z,m.rough);
  let favg=m.base+(1-m.base)/21;
  let damp=favg*favg*eo.y/max(vec3f(1e-6),1-favg*(1-eo.y));
  return single+damp*((1-eo.x)*(1-ei.x)*wi.z/(PI*max(1e-6,1-eo.y)));
}
fn albedo(m:Metal)->vec3f {return m.base;}`);

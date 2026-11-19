import { kind } from "../text/kind.ts";
export const matte = kind(`
struct Matte { base:vec3f }
fn eval(m:Matte,wo:vec3f,wi:vec3f)->vec3f {return m.base*(max(wi.z,0)/PI);}
fn albedo(m:Matte)->vec3f {return m.base;}`);
export const lamp = kind(`
struct Lamp { base:vec3f, power:vec3f }
fn eval(m:Lamp,wo:vec3f,wi:vec3f)->vec3f {return m.base*(max(wi.z,0)/PI);}
fn emit(m:Lamp,wo:vec3f)->vec3f {return m.power;}
fn albedo(m:Lamp)->vec3f {return m.base;}`);
export const glow = (power: readonly number[]) => lamp([0,0,0],power);

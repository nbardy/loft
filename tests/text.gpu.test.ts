// L1 check 2: a paint written in WGSL text, using each operator text added to the graph (WGSL's round, %, ceil, trunc,
// inverseSqrt, exp2, log2, asin, acos, atan, sinh, cosh, tanh) and each builtin text lowers to existing nodes (fma,
// radians, degrees, reflect, refract, faceForward, a matrix product), agrees between the CPU twin and the GPU within
// 1/255. The bug this catches: an operator whose JavaScript and WGSL meanings differ (round's ties, the sign of %), or
// a lowering that is wrong in one backend only.
import { expect, test } from "bun:test";
import { variable, wgsl } from "../src/index.ts";
import { compare } from "./lib.harness.ts";

const paint = wgsl`
fn unit(x: f32) -> f32 { return 0.5 + 0.5 * sin(x * 0.7); }   // continuous: a colour from any number
fn ops(u: f32, v: f32) -> array<f32, 20> {
  let x = u * 8.0 - 4.0;               // halves land on whole and half numbers: round's ties
  let y = v * 1.8 - 0.9;               // inside [-1, 1] for asin and acos
  let n = normalize(vec3(0.3, -1.0, 0.2));
  let i = normalize(vec3(u - 0.5, -0.7, v - 0.5));
  let m = mat2x2f(cos(y), sin(y), -sin(y), cos(y));
  return array(
    unit(round(x)), unit(x % 1.3), unit(ceil(x)), unit(trunc(x)),
    unit(inverseSqrt(u + 0.1)), unit(exp2(y * 3.0)), unit(log2(u + 0.05)), unit(asin(y)), unit(acos(y)), unit(atan(x)),
    unit(sinh(y)), unit(cosh(y)), unit(tanh(x)),
    unit(fma(x, y, 0.25)), unit(radians(x * 40.0)), unit(degrees(y)),
    unit(reflect(i, n).x), unit(refract(i, n, 0.75).y), unit(faceForward(n, i, n).z), unit((m * vec2(x, y)).x));
}`;

test("WGSL text's own operators agree between the CPU twin and the GPU within 1/255", async () => {
  const u = variable("u", "f32"), v = variable("v", "f32");
  const exprs = paint.ops(u, v) as ReturnType<typeof u.add>[];
  expect(exprs.length).toBe(20);
  const samples = Array.from({ length: 17 * 9 }, (_, k) => ({ u: (k % 17) / 16, v: Math.floor(k / 17) / 8 }));
  const cmp = await compare(exprs, samples, "text ops");
  console.log(`text ops: worst CPU-GPU difference ${cmp.worst.toExponential(2)} over ${samples.length} samples x ${exprs.length} operators (bound ${(1 / 255).toExponential(2)})`);
  expect(cmp.worst).toBeLessThan(1 / 255);
  // round is WGSL's (ties to even), not Loft's floor(x + 0.5): at x = 0.5 (u = 0.5625) it gives 0, so unit(0) = 0.5.
  const k = samples.findIndex((s) => s.u === 0.5625 && s.v === 0);
  expect(cmp.cpu[k]![0]![0]).toBeCloseTo(0.5, 6);
  expect(cmp.gpu[k]![0]![0]).toBeCloseTo(0.5, 6);
}, 60_000);

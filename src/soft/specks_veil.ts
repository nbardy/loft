// Many specks amortise the cloud integral over a small view-dependent grid in the cloud's world box.
import type { CompileContext, Shader } from "../compile/context.ts";
import { fetch, leaves, lib, lift, mapOf, type Expr, type LibFn, type TableSource } from "../expr.ts";
import { foldAt, sunDir, sunOnly } from "../light.ts";
import { densityIntegral, type DensityDrawable } from "./density.ts";

const EDGE = 32;
const CELLS = EDGE ** 3;
const caches = new WeakMap<CompileContext, Map<DensityDrawable, TableSource>>();
const samplers = new Map<string, LibFn>();

const RAY_TO_GRAIN = /* wgsl */ `
fn grain_front(o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32, zenith: vec3<f32>, distant: vec3<f32>) -> vec4<f32> {
  let v = soft_integral(o, d, t0, t1, zenith, distant);
  return vec4<f32>(v.rgb * v.a, v.a);
}`;

export function specksVeil(ctx: CompileContext, cloud: DensityDrawable, count: number): Shader & { cached: boolean } {
  const integral = densityIntegral(ctx, cloud);
  const coat = lift<"vec3">([0, 1, 0]);
  const zenith = ctx.style.sky({ dir: coat, sun: lift<"vec3">(sunDir(ctx.sunAt)), lights: foldAt(sunOnly(ctx.lighting), { kind: "point", at: coat }) });
  // The normal render integral can read fragment-only sky code, or evaluate distance haze per ray.
  const cached = count >= CELLS && cloud.o.haze === 0 && !leaves([zenith]).libs.some((l) => l.js === "gpu-only");
  if (count >= CELLS) {
    ctx.note(`specks veil "${cloud.name}": ${cached ? "32³ samples for the main camera" : "direct ray integral (distance haze or shader-only sky)"}`);
  }
  const direct = frontRay(cached ? "grain_veil_direct" : "grain_veil", "",
    `grain_front(o, d, t0, t1, sky(vec3<f32>(0.0, 1.0, 0.0)), ${cloud.o.haze > 0 ? "sky(d)" : "vec3<f32>(0.0)"})`);
  const grid = cached ? lookup(ctx.table(veilTable(ctx, cloud, integral, zenith)).wgsl) : "";
  return { code: [integral.code, RAY_TO_GRAIN, direct, grid].join("\n"), libs: integral.libs, cached };
}

function frontRay(name: string, inside: string, colour: string): string {
  return /* wgsl */ `
fn ${name}(p: vec3<f32>, d: vec3<f32>) -> vec4<f32> {
  ${inside}
  let t = dot(p - frame.eye.xyz, d);
  let o = p - d * t;
  let box = soft_box(o, d);
  let t0 = max(0.0, box.x);
  let t1 = min(box.y, t);
  if (t1 <= t0) { return vec4<f32>(0.0); }
  return ${colour};
}`;
}

function lookup(read: string): string {
  return /* wgsl */ `
fn grain_grid(p: vec3<f32>) -> vec4<f32> {
  let xyz = clamp((p - BOX_MIN) / (BOX_MAX - BOX_MIN), vec3<f32>(0.0), vec3<f32>(1.0)) * ${EDGE - 1}.0;
  let lo = min(vec3<u32>(xyz), vec3<u32>(${EDGE - 2}u));
  let w = xyz - vec3<f32>(lo);
  let i = lo.x + ${EDGE}u * lo.y + ${EDGE * EDGE}u * lo.z;
  let a = mix(${read}(i), ${read}(i + 1u), w.x);
  let b = mix(${read}(i + ${EDGE}u), ${read}(i + ${EDGE + 1}u), w.x);
  let c = mix(${read}(i + ${EDGE * EDGE}u), ${read}(i + ${EDGE * EDGE + 1}u), w.x);
  let e = mix(${read}(i + ${EDGE * EDGE + EDGE}u), ${read}(i + ${EDGE * EDGE + EDGE + 1}u), w.x);
  return mix(mix(a, b, w.y), mix(c, e, w.y), w.z);
}${frontRay("grain_veil", "if (all(p >= BOX_MIN) && all(p <= BOX_MAX)) { return grain_grid(p); }", "grain_grid(o + t1 * d)")}`;
}

function veilTable(ctx: CompileContext, cloud: DensityDrawable, integral: Shader, zenith: Expr<"vec3">): TableSource {
  let tables = caches.get(ctx);
  if (tables === undefined) { tables = new Map(); caches.set(ctx, tables); }
  const seen = tables.get(cloud);
  if (seen !== undefined) return seen;
  const { blobs, clusters, layers, banks } = cloud.medium.tables(ctx.sun, ctx.lighting.lights);
  const sources = [blobs, clusters, ...[layers, banks].flatMap((o) => o.kind === "some" ? [o.table] : [])];
  const name = `${cloud.name}_specksVeil`;
  // The accessors are in the integral's raw WGSL. These arguments tell the pose scheduler which tables it reads.
  const reads = sources.map((source) => fetch(source, 0, 0));
  const sample = sampler(name, integral, reads.length);
  const table: TableSource = { name, entries: CELLS, lanes: 1, role: "data", program: mapOf(CELLS, (i) => [lib<"vec4">(sample, i, zenith, ...reads)]) };
  tables.set(cloud, table);
  return table;
}

function sampler(label: string, integral: Shader, reads: number): LibFn {
  const key = `${label} ${reads}\n${integral.code}`;
  const seen = samplers.get(key);
  if (seen !== undefined) return seen;
  // The expression arena interns library functions process-wide. Reuse identical code across builds and keep
  // differing options or bindings distinct, even when variants call their clouds by the same name.
  let hash = 0xcbf29ce484222325n;
  for (let i = 0; i < key.length; i++) hash = BigInt.asUintN(64, (hash ^ BigInt(key.charCodeAt(i))) * 0x100000001b3n);
  const name = `${label}_${hash.toString(16)}`;
  const sample: LibFn = {
    name, ret: "vec4", js: "gpu-only", deps: integral.libs,
    wgsl: /* wgsl */ `${namespace(integral.code + RAY_TO_GRAIN, name)}
fn ${name}(entry: f32, zenith: vec3<f32>${Array.from({ length: reads }, (_, i) => `, read${i}: vec4<f32>`).join("")}) -> vec4<f32> {
  let i = u32(entry);
  let xyz = vec3<f32>(f32(i % ${EDGE}u), f32(i / ${EDGE}u % ${EDGE}u), f32(i / ${EDGE * EDGE}u)) / ${EDGE - 1}.0;
  let p = mix(${name}_BOX_MIN, ${name}_BOX_MAX, clamp(xyz, vec3<f32>(1e-5), vec3<f32>(0.99999)));
  let ray = ${name}_soft_ray(p);
  let box = ${name}_soft_box(ray[0], ray[1]);
  return ${name}_grain_front(ray[0], ray[1], max(0.0, box.x), min(box.y, dot(p - ray[0], ray[1])), zenith, vec3<f32>(0.0));
}`,
  };
  samplers.set(key, sample);
  return sample;
}

// Several veils can share one pose stage. Only the integral's private declarations need prefixes.
function namespace(code: string, prefix: string): string {
  const names = /\b(?:fn|const|var<private>) ([A-Za-z_]\w*)/g;
  const declarations = [...code.matchAll(names)].map((m) => m[1]);
  return code.replace(new RegExp(`\\b(${declarations.join("|")})\\b`, "g"), `${prefix}_$1`);
}

// Many specks amortise the cloud integral over a small view-dependent grid in the cloud's world box.
import type { CompileContext, Drawable, Shader } from "../compile/context.ts";
import { fetch, leaves, lib, lift, mapOf, type Expr, type LibFn, type TableSource } from "../expr.ts";
import { foldAt, sunDir, sunOnly } from "../light.ts";
import { TRANSMITTANCE, type VolumeDrawable } from "../draw/volume.ts";
import { densityIntegral, softRayWgsl, type DensityDrawable } from "./density.ts";

const EDGE = 32;
const CELLS = EDGE ** 3;
const caches = new WeakMap<CompileContext, Map<Drawable, TableSource>>();
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

/**
 * A volume's light in front of a speck and its opacity there: the volume's own march (its density and source
 * functions, at the frame's time) from where the view ray enters its box to the speck, sampled at stratum midpoints,
 * composited by the speck's fragment as a density cloud's front is. Many specks read it from the same 32³ grid of the
 * main camera's front values (one march per cell per frame, in a pose stage) when the volume's functions read only
 * their arguments; a march per fragment costs the volume's steps for every speck pixel (116 ms for 100,000 specks at
 * 1280x800 and 64 steps, against 1 ms without a veil, measured on dust_veil.ts).
 */
export function specksVolumeVeil(ctx: CompileContext, v: VolumeDrawable, count: number): Shader & { cached: boolean } {
  const { fns, closed } = v.sampling(ctx);
  const cached = count >= CELLS && closed;
  if (count >= CELLS) ctx.note(`specks veil "${v.name}": ${cached ? "32³ samples for the main camera" : "direct march per fragment (the volume reads tables, inputs, params or shader-only functions)"}`);
  const march = volumeMarch(v.steps);
  const code = [softRayWgsl(v.box), ...fns.map((f) => f.code), `const VOLUME_SHADOW_STEPS = ${v.shadowSteps}u;`, march,
    frontRay(cached ? "grain_veil_direct" : "grain_veil", "", "volume_front(o, d, t0, t1)")].join("\n");
  const libs = [...new Set(fns.flatMap((f) => f.libs))];
  if (!cached) return { code, libs, cached };
  // The pose stage's copy: the same text, its declarations prefixed, the shadow march inlined so it is prefixed too.
  const own = [code, TRANSMITTANCE.wgsl].join("\n");
  const table = tableOnce(ctx, v, () => {
    const name = `${v.name}_specksVeil`;
    const sample = samplerOf(name, `${name}\n${own}`, own, "", libs.filter((l) => l !== TRANSMITTANCE), (n) => `
  let ray = ${n}_soft_ray(p);
  let box = ${n}_soft_box(ray[0], ray[1]);
  return ${n}_volume_front(ray[0], ray[1], max(0.0, box.x), min(box.y, dot(p - ray[0], ray[1])));`);
    return { name, entries: CELLS, lanes: 1, role: "data", program: mapOf(CELLS, (i) => [lib<"vec4">(sample, i)]) };
  });
  return { code: [code, lookup(ctx.table(table).wgsl)].join("\n"), libs, cached };
}

/** The volume's march along o + d s for s in [t0, t1]: front colour (premultiplied) and opacity. */
function volumeMarch(steps: number): string {
  return /* wgsl */ `
fn volume_front(o: vec3<f32>, d: vec3<f32>, t0: f32, t1: f32) -> vec4<f32> {
  let ds = (t1 - t0) / ${steps}.0;
  let time = frame.time.x;
  var colour = vec3<f32>(0.0);
  var opacity = 0.0;
  for (var i = 0u; i < ${steps}u; i++) {
    let q = o + d * (t0 + (f32(i) + 0.5) * ds);
    let sigma = volume_density(q, time);
    if (!(sigma > 0.0)) { continue; }
    let tau = sigma * ds;
    var alpha = 1.0 - exp(-tau);
    if (tau < 1e-3) { alpha = tau * (1.0 - 0.5 * tau); }
    let weight = (1.0 - opacity) * alpha;
    colour += weight * volume_source(q, time, d, sigma);
    opacity += weight;
    if (opacity >= (1.0 - 1.0 / 1024.0)) { break; }
  }
  return vec4<f32>(colour, opacity);
}`;
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
  return tableOnce(ctx, cloud, () => {
    const { blobs, clusters, layers, banks } = cloud.medium.tables(ctx.sun, ctx.lighting.lights);
    const sources = [blobs, clusters, ...[layers, banks].flatMap((o) => o.kind === "some" ? [o.table] : [])];
    const name = `${cloud.name}_specksVeil`;
    // The accessors are in the integral's raw WGSL. These arguments tell the pose scheduler which tables it reads.
    const reads = sources.map((source) => fetch(source, 0, 0));
    const params = `, zenith: vec3<f32>${reads.map((_, i) => `, read${i}: vec4<f32>`).join("")}`;
    const sample = samplerOf(name, `${name} ${reads.length}\n${integral.code}`, integral.code + RAY_TO_GRAIN, params, integral.libs, (n) => `
  let ray = ${n}_soft_ray(p);
  let box = ${n}_soft_box(ray[0], ray[1]);
  return ${n}_grain_front(ray[0], ray[1], max(0.0, box.x), min(box.y, dot(p - ray[0], ray[1])), zenith, vec3<f32>(0.0));`, sources);
    return { name, entries: CELLS, lanes: 1, role: "data", program: mapOf(CELLS, (i) => [lib<"vec4">(sample, i, zenith, ...reads)]) };
  });
}

/** A cloud's veil grid, made once per compile. */
function tableOnce(ctx: CompileContext, cloud: Drawable, make: () => TableSource): TableSource {
  let tables = caches.get(ctx);
  if (tables === undefined) { tables = new Map(); caches.set(ctx, tables); }
  const seen = tables.get(cloud);
  if (seen !== undefined) return seen;
  const table = make();
  tables.set(cloud, table);
  return table;
}

/**
 * A grid cell's sampler, `fn <label>_<hash>(entry: f32<params>)`: `code` with its declarations prefixed, then the
 * cell's point p and `body` (given the prefix). The expression arena interns library functions process-wide: `key`
 * reuses identical code across builds and keeps differing options or bindings distinct, even when variants call their
 * clouds by the same name.
 */
function samplerOf(label: string, key: string, code: string, params: string, deps: readonly LibFn[], body: (prefix: string) => string, reads: readonly TableSource[] = []): LibFn {
  const seen = samplers.get(key);
  if (seen !== undefined) return seen;
  let hash = 0xcbf29ce484222325n;
  for (let i = 0; i < key.length; i++) hash = BigInt.asUintN(64, (hash ^ BigInt(key.charCodeAt(i))) * 0x100000001b3n);
  const name = `${label}_${hash.toString(16)}`;
  const sample: LibFn = {
    name, ret: "vec4", js: "gpu-only", deps, reads: reads.map((table) => ({ kind: "table", table }) as const),
    wgsl: /* wgsl */ `${namespace(code, name)}
fn ${name}(entry: f32${params}) -> vec4<f32> {
  let i = u32(entry);
  let xyz = vec3<f32>(f32(i % ${EDGE}u), f32(i / ${EDGE}u % ${EDGE}u), f32(i / ${EDGE * EDGE}u)) / ${EDGE - 1}.0;
  let p = mix(${name}_BOX_MIN, ${name}_BOX_MAX, clamp(xyz, vec3<f32>(1e-5), vec3<f32>(0.99999)));${body(name)}
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

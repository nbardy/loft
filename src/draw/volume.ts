// A bounded march of the author's extinction field. Both rays sample the same field and time.
import { emitShaderFn } from "../compile/bind.ts";
import { unported, type Recorded } from "./record.ts";
import { drawSpec, type CompileContext, type Drawable, type Emitted, type Form, type Shader } from "../compile/context.ts";
import { SceneError, shown } from "../errors.ts";
import { add, clamp, div, dot, Expr, gt, lib, lift, max, mul, neg, pow, leaves, select, sub, substitute, time, variable, withIdentities, type In, type LibFn } from "../expr.ts";
import { hash2 } from "../lib/noise.ts";
import { foldAt, type LightFold } from "../light.ts";
import type { F, P3 } from "../map/types.ts";
import { boxVertexWgsl, softRayWgsl } from "../soft/density.ts";
import type { VolumeCtx } from "../style.ts";
import type { Bounds } from "./index.ts";

type Sample<T extends "f32" | "vec3"> = (p: P3) => In<T>;

/** What a raymarch integrates, sampled in world coordinates (read `time` for motion). */
export type VolumeOptions = {
  readonly bounds: Bounds;
  /** Maximum number of view-ray samples per pixel inside the bound. */
  readonly steps: number;
  /** Single-scattering albedo, clamped to [0, 1] by channel. Default white. */
  readonly albedo?: In<"vec3"> | ((p: P3) => In<"vec3">);
  /** Emitted radiance per unit extinction, default black. Values above 1 are allowed. */
  readonly emission?: In<"vec3"> | ((p: P3) => In<"vec3">);
  /** Henyey–Greenstein anisotropy in (-1, 1), default 0. Positive is forward scattering. */
  readonly g?: number;
  /** Self-shadow rays toward the sun and each lamp, or unshadowed scattering (the default). */
  readonly shadow?: { readonly kind: "unshadowed" } | { readonly kind: "march"; readonly steps: number };
};

const SAMPLE_ARGS = [["p", "vec3"], ["t", "f32"]] as const;

function stepsOf(where: string, field: string, steps: number): number {
  // The loop's integer index becomes f32 to locate the sample within its stratum.
  if (!Number.isInteger(steps) || steps < 1 || steps >= 2 ** 24) {
    throw new SceneError(where, `${field} ${shown(steps)}`, "a whole number of samples from 1 to 16777215", `set ${field} to a positive count such as ${field === "steps" ? 64 : 6}`);
  }
  return steps;
}

const finiteF32 = (x: unknown): x is number => typeof x === "number" && Number.isFinite(Math.fround(x));
const finiteVec3 = (x: unknown): x is Bounds["min"] => Array.isArray(x) && x.length === 3 && finiteF32(x[0]) && finiteF32(x[1]) && finiteF32(x[2]);

/**
 * A slot read at p and at a time t: the author's expression reads `time`, which is replaced by t (the shadow march and
 * the specks' veil sample the field at the frame's time through a parameter, as the view march does).
 */
function fieldOf<T extends "f32" | "vec3">(where: string, name: string, ty: T, value: Sample<T> | In<T>): (p: P3, t: F) => Expr<T> {
  return (p, t) => {
    const x = typeof value === "function" ? value(p) : value;
    const e = x instanceof Expr && x.ty === ty ? x : (ty === "f32" ? finiteF32(x) : finiteVec3(x)) ? lift<T>(x as In<T>) : undefined;
    if (e === undefined) throw new SceneError(where, `${name} as ${x instanceof Expr ? x.ty : shown(x)}`, `a ${ty} expression for ${name}`, ty === "f32" ? "return a number or an f32 expression from density(p)" : `give ${name} as [r, g, b] or return vec3(r, g, b) from its function`);
    return t === time ? e : substitute([e], time, t)[0] as Expr<T>;
  };
}

/** March extinction (inverse world units) from the eye to opaque depth inside a box, with single scattering and emission. */
export function drawVolume(name: string, density: (p: P3) => In<"f32">, o: VolumeOptions): VolumeDrawable {
  const where = typeof name === "string" ? `draw.volume ${shown(name)}` : "draw.volume";
  if (typeof name !== "string" || !/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
    throw new SceneError(where, `name ${shown(name)}`, "an identifier beginning with a letter", 'name the volume e.g. "dust"');
  }
  if (typeof density !== "function") {
    throw new SceneError(where, `density ${shown(density)}`, "a function of position", "write (p) => expression (read time for motion), or () => 0.4 for constant density");
  }
  const bounds = o?.bounds;
  if (!finiteVec3(bounds?.min) || !finiteVec3(bounds.max) || bounds.min.some((x, i) => !(Math.fround(x) < Math.fround(bounds.max[i]!)) || !finiteF32(bounds.max[i]! - x))) {
    throw new SceneError(where, `bounds from ${shown(bounds?.min)} to ${shown(bounds?.max)}`, "finite f32 corners and a positive finite span on every axis", "give bounds: { min: [x0, y0, z0], max: [x1, y1, z1] } with each max above its min");
  }
  const box: Bounds = { min: [...bounds.min], max: [...bounds.max] };
  const steps = stepsOf(where, "steps", o.steps);
  const g = o.g === undefined ? 0 : o.g;
  if (typeof g !== "number" || !(Math.abs(Math.fround(g)) < 1)) {
    throw new SceneError(where, `g ${shown(g)}`, "an anisotropy in (-1, 1) at f32 precision", "use g: 0 for isotropic or g: 0.6 for forward scattering");
  }
  const shadow = o.shadow === undefined ? { kind: "unshadowed" } as const : o.shadow;
  const shadowKind = shadow?.kind;
  const shadowSteps = (() => {
    switch (shadowKind) {
      case "unshadowed": return 0;
      case "march": return stepsOf(where, "shadow.steps", shadow.steps);
      default: throw new SceneError(where, `shadow ${shown(shadowKind ?? shadow)}`, 'a shadow with kind "march" or "unshadowed"', 'use shadow: { kind: "march", steps: 6 }, or omit shadow');
    }
  })();

  return new VolumeDrawable(name, box, steps, Math.fround(g), shadowSteps, {
    density: fieldOf(where, "density", "f32", density),
    albedo: fieldOf(where, "albedo", "vec3", o.albedo === undefined ? [1, 1, 1] : o.albedo),
    emission: fieldOf(where, "emission", "vec3", o.emission === undefined ? [0, 0, 0] : o.emission),
  });
}

type Fields = { readonly [K in "density" | "albedo" | "emission"]: (p: P3, t: F) => Expr<K extends "density" ? "f32" : "vec3"> };

/**
 * The `Drawable` that `draw.volume` returns: a march of the author's extinction field in a box. `draw.points` reads it
 * as a veil (its `veil` option), so grains inside the volume are dimmed and tinted by the same field.
 */
export class VolumeDrawable implements Drawable {
  readonly skeletons = [];
  readonly forms: readonly Form[];

  constructor(readonly name: string, readonly box: Bounds, readonly steps: number, readonly g: number, readonly shadowSteps: number, private readonly f: Fields) {
    this.forms = [{
      kind: "medium", name, bound: { kind: "box", min: [...box.min], max: [...box.max] },
      density: (p) => this.extinction(p, time), colour: (p) => this.albedo(p, time),
    }];
  }

  private extinction(p: P3, t: F): F { return max(this.f.density(p, t), 0); }
  private albedo(p: P3, t: F): P3 { return clamp(this.f.albedo(p, t), 0, 1); }
  private emission(p: P3, t: F): P3 { return max(this.f.emission(p, t), 0); }

  /**
   * `volume_density(p, t)` and `volume_source(p, t, ray, sigma)`: what the view march and a grain's veil sample.
   * `closed`: they read nothing but their arguments and library functions a compute stage can run (no table, row,
   * input or param; no derivative, texture sample or scene depth), so a pose stage can sample them (the specks' veil grid).
   */
  sampling(ctx: CompileContext): { readonly fns: readonly Shader[]; readonly closed: boolean } {
    // Allocate expression identities only when a volume is emitted, so unused volume code leaves other shaders alone.
    const p = variable("p", "vec3"), t = variable("t", "f32"), ray = variable("ray", "vec3");
    const source: VolumeCtx = {
      world: p, time: t, view: neg(ray), density: variable("sigma", "f32"), albedo: this.albedo(p, t), emission: this.emission(p, t),
      lights: scatteredLights(ctx, p, t, ray, this.g, this.shadowSteps > 0),
    };
    const shaded = withIdentities(() => ctx.style.volume === undefined ? linearVolume(source) : ctx.style.volume(source));
    if (!(shaded instanceof Expr) || shaded.ty !== "vec3") {
      throw new SceneError(`style ${JSON.stringify(ctx.style.name)} volume`, shown(shaded), "a vec3 expression for the sample's source colour", "return vec3(r, g, b) or an expression of the volume context");
    }
    const density = this.extinction(p, t);
    const read = leaves([density, shaded]);
    const closed = read.tables.length + read.rows.length + read.inputs.length + read.params.length === 0 && !read.libs.some(fragmentOnly);
    return { fns: [emit(ctx, "volume_density", SAMPLE_ARGS, density), emit(ctx, "volume_source", [...SAMPLE_ARGS, ["ray", "vec3"], ["sigma", "f32"]], shaded)], closed };
  }

  record(): Recorded { return unported("volume: a march medium, not written yet"); }
  draws(ctx: CompileContext): Emitted {
    const key = `volume.${this.name}`;
    const fns = [...this.sampling(ctx).fns, emit(ctx, "volume_jitter", [["px", "vec2"]], hash2(variable("px", "vec2")))];
    const code = [boxVertexWgsl(this.box), softRayWgsl(this.box), ...fns.map((f) => f.code), fragment(ctx.ground.kind, this.steps, this.shadowSteps)].join("\n");
    return {
      shaders: { [key]: { code, libs: [...new Set(fns.flatMap((f) => f.libs))] } },
      draws: [drawSpec({ label: this.name, shader: key, vertices: 36, phase: "transparent", cull: "front", depthWrite: false, blend: "premultiplied", sceneDepth: true })],
    };
  }
}

function linearVolume(ctx: VolumeCtx): P3 {
  return add(ctx.emission, mul(ctx.albedo, ctx.lights("sum", (l) => lift<"vec3">(l.amount))));
}

function emit(ctx: CompileContext, name: string, args: Parameters<typeof emitShaderFn>[1]["args"], root: F | P3): Shader {
  return emitShaderFn(ctx.slots, { name, args, out: root.ty === "f32" ? "f32" : "vec3<f32>", roots: [root], ret: (v) => v[0]! });
}

/** The same fold as on surfaces, with a point receiver and a phase-weighted arrival. */
function scatteredLights(ctx: CompileContext, p: P3, t: F, ray: P3, g: number, march: boolean): LightFold {
  const lights = foldAt(ctx.lighting, { kind: "point", at: p });
  const iso = 1 / (4 * Math.PI), a = Math.abs(g);
  return (combine, term) => lights(combine, (l, which) => {
    // This form of the HG denominator stays positive even for strong forward/backward lobes near cos = ±1.
    const cosine = clamp(dot(ray, l.toward), -1, 1);
    // At a lamp's exact centre its direction is undefined; use the phase at 90 degrees there.
    const angle = l.distance === undefined ? cosine : select(gt(l.distance, 0), cosine, 0);
    const phase = g === 0 ? iso : div(iso * (1 - g * g), pow(add((1 - a) ** 2, mul(2 * a, sub(1, mul(Math.sign(g), angle)))), 1.5));
    const self = march ? lib<"f32">(TRANSMITTANCE, p, t, l.toward, l.distance ?? 1e30) : 1;
    const unshadowed = mul(l.unshadowed, phase);
    const visibility = mul(l.visibility, self);
    return term({ ...l, unshadowed, visibility, amount: mul(unshadowed, visibility) }, which);
  });
}

/** A library function only a fragment stage can run: it (or a function it calls) takes derivatives or reads the screen. */
const fragmentOnly = (l: LibFn): boolean => /\b(fwidth|dpdx|dpdy|textureSample|scene_depth|scene_position|scene_clearance)\w*\s*\(/.test(l.wgsl) || (l.deps ?? []).some(fragmentOnly);

export const TRANSMITTANCE: LibFn = {
  name: "loft_volume_transmittance", ret: "f32", js: "gpu-only",
  wgsl: /* wgsl */ `
fn loft_volume_transmittance(p: vec3<f32>, t: f32, toward: vec3<f32>, distance: f32) -> f32 {
  if (!(distance > 0.0)) { return 1.0; }
  let span = min(distance, soft_box(p, toward).y);
  if (!(span > 0.0)) { return 1.0; }
  let ds = span / f32(VOLUME_SHADOW_STEPS);
  var tau = 0.0;
  for (var i = 0u; i < VOLUME_SHADOW_STEPS; i++) {
    tau += volume_density(p + toward * ((f32(i) + 0.5) * ds), t) * ds;
  }
  return exp(-tau);
}`,
};

function groundEnd(ground: CompileContext["ground"]["kind"]): string {
  switch (ground) {
    case "nothing": return "";
    case "sea": return "far = min(far, soft_sea(o, d));";
    case "plane": return "if (abs(d.y) > 1e-12) { let t = -o.y / d.y; if (t > 0.0) { far = min(far, t); } }";
  }
}

function fragment(ground: CompileContext["ground"]["kind"], steps: number, shadowSteps: number): string {
  return /* wgsl */ `
const VOLUME_SHADOW_STEPS = ${shadowSteps}u;
@fragment fn fs(in: VOut) -> @location(0) vec4<f32> {
  let ray = soft_ray(in.world);
  let o = ray[0];
  let d = ray[1];
  let box = soft_box(o, d);
  let near = max(box.x, 0.0);
  var far = min(box.y, dot(scene_position(in.clip.xy) - o, d));
  ${groundEnd(ground)}
  if (!(far > near)) { discard; }

  let ds = (far - near) / ${steps}.0;
  let jitter = volume_jitter(in.clip.xy);
  let time = frame.time.x;
  var colour = vec3<f32>(0.0);
  var opacity = 0.0;
  for (var i = 0u; i < ${steps}u; i++) {
    let p = o + d * (near + (f32(i) + jitter) * ds);
    let sigma = volume_density(p, time);
    if (!(sigma > 0.0)) { continue; }
    let tau = sigma * ds;
    // A small-tau expansion preserves faint extinction when exp(-tau) rounds to one.
    var alpha = 1.0 - exp(-tau);
    if (tau < 1e-3) { alpha = tau * (1.0 - 0.5 * tau); }
    let weight = (1.0 - opacity) * alpha;
    colour += weight * volume_source(p, time, d, sigma);
    opacity += weight;
    if (opacity >= (1.0 - 1.0 / 1024.0)) { break; }
  }
  return vec4<f32>(colour, opacity);
}`;
}

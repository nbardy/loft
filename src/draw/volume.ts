// A bounded march of the author's extinction field. Both rays sample the same field and time.
import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type CompileContext, type Drawable, type Form, type Shader } from "../compile/context.ts";
import { SceneError, shown } from "../errors.ts";
import { add, clamp, div, dot, Expr, gt, lib, lift, max, mul, neg, pow, select, sub, time, variable, withIdentities, type In, type LibFn } from "../expr.ts";
import { hash2 } from "../lib/noise.ts";
import { foldAt, type LightFold } from "../light.ts";
import type { F, P3 } from "../map/types.ts";
import { boxVertexWgsl } from "../soft/density.ts";
import type { VolumeCtx } from "../style.ts";
import type { Bounds } from "./index.ts";

type Sample<T extends "f32" | "vec3"> = (p: P3, t: F) => In<T>;

/** What a raymarch integrates, sampled in world coordinates and at the frame's time in seconds. */
export type VolumeOptions = {
  readonly bounds: Bounds;
  /** Maximum number of view-ray samples per pixel inside the bound. */
  readonly steps: number;
  /** Single-scattering albedo, clamped to [0, 1] by channel. Default white. */
  readonly albedo?: In<"vec3"> | ((p: P3, t: F) => In<"vec3">);
  /** Emitted radiance per unit extinction, default black. Values above 1 are allowed. */
  readonly emission?: In<"vec3"> | ((p: P3, t: F) => In<"vec3">);
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

function fieldOf<T extends "f32" | "vec3">(where: string, name: string, ty: T, value: Sample<T> | In<T>): (p: P3, t: F) => Expr<T> {
  return (p, t) => {
    const x = typeof value === "function" ? value(p, t) : value;
    if (x instanceof Expr && x.ty === ty) return x;
    if (ty === "f32" ? finiteF32(x) : finiteVec3(x)) return lift<T>(x);
    throw new SceneError(where, `${name} as ${x instanceof Expr ? x.ty : shown(x)}`, `a ${ty} expression for ${name}`, ty === "f32" ? "return a number or an f32 expression from density(p, t)" : `give ${name} as [r, g, b] or return vec3(r, g, b) from its function`);
  };
}

/** March extinction (inverse world units) from the eye to opaque depth inside a box, with single scattering and emission. */
export function drawVolume(name: string, density: (p: P3, t: F) => In<"f32">, o: VolumeOptions): Drawable {
  const where = typeof name === "string" ? `draw.volume ${shown(name)}` : "draw.volume";
  if (typeof name !== "string" || !/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
    throw new SceneError(where, `name ${shown(name)}`, "an identifier beginning with a letter", 'name the volume e.g. "dust"');
  }
  if (typeof density !== "function") {
    throw new SceneError(where, `density ${shown(density)}`, "a function of position and time", "write (p, t) => expression, or () => 0.4 for constant density");
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

  const field = fieldOf(where, "density", "f32", density);
  const colour = fieldOf(where, "albedo", "vec3", o.albedo === undefined ? [1, 1, 1] : o.albedo);
  const emits = fieldOf(where, "emission", "vec3", o.emission === undefined ? [0, 0, 0] : o.emission);
  const extinction = (p: P3, t: F) => max(field(p, t), 0);
  const albedo = (p: P3, t: F) => clamp(colour(p, t), 0, 1);
  const emission = (p: P3, t: F) => max(emits(p, t), 0);
  const forms: readonly Form[] = [{
    kind: "medium", name, bound: { kind: "box", min: [...box.min], max: [...box.max] },
    density: (p) => extinction(p, time), colour: (p) => albedo(p, time),
  }];

  return {
    name, skeletons: [], forms,
    draws(ctx) {
      const key = `volume.${name}`;
      // Allocate expression identities only when a volume is emitted, so unused volume code leaves other shaders alone.
      const p = variable("p", "vec3"), t = variable("t", "f32"), ray = variable("ray", "vec3");
      const source: VolumeCtx = {
        world: p, time: t, view: neg(ray), density: variable("sigma", "f32"), albedo: albedo(p, t), emission: emission(p, t),
        lights: scatteredLights(ctx, p, t, ray, Math.fround(g), shadowSteps > 0),
      };
      const shaded = withIdentities(() => ctx.style.volume === undefined ? linearVolume(source) : ctx.style.volume(source));
      if (!(shaded instanceof Expr) || shaded.ty !== "vec3") {
        throw new SceneError(`style ${JSON.stringify(ctx.style.name)} volume`, shown(shaded), "a vec3 expression for the sample's source colour", "return vec3(r, g, b) or an expression of the volume context");
      }
      const fns = [
        emit(ctx, "volume_density", SAMPLE_ARGS, extinction(p, t)),
        emit(ctx, "volume_source", [...SAMPLE_ARGS, ["ray", "vec3"], ["sigma", "f32"]], shaded),
        emit(ctx, "volume_jitter", [["px", "vec2"]], hash2(variable("px", "vec2"))),
      ];
      const code = [boxVertexWgsl(box), ...fns.map((f) => f.code), fragment(ctx.ground.kind, steps, shadowSteps)].join("\n");
      return {
        shaders: { [key]: { code, libs: [...new Set(fns.flatMap((f) => f.libs))] } },
        draws: [drawSpec({ label: name, shader: key, vertices: 36, phase: "transparent", cull: "front", depthWrite: false, blend: "premultiplied", sceneDepth: true })],
      };
    },
  };
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

const TRANSMITTANCE: LibFn = {
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

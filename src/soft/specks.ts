// `draw.specks`: camera-facing specks generated from (index, time) in one instanced draw. Nothing is stored per speck.
import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type CompileContext, type Drawable, type Shader } from "../compile/context.ts";
import { lit } from "../compile/scene_wgsl.ts";
import { SceneError, shown } from "../errors.ts";
import { add, dot, Expr, lift, mul, saturate, variable, type In } from "../expr.ts";
import { foldAt } from "../light.ts";
import type { F } from "../map/types.ts";
import { DensityDrawable } from "./density.ts";
import { specksVeil } from "./specks_veil.ts";

/** One speck: world centre and radius, colour and opacity, as expressions of its index and the time. */
export type Speck = {
  readonly at: In<"vec3">;
  readonly radius: In<"f32">;
  readonly colour: In<"vec3">;
  readonly opacity: In<"f32">;
};

export type SpecksOptions = {
  /** Seconds of projected travel to smear across the centre. Default 0 (round specks). */
  readonly stretch?: number;
  /** A cloud from `draw.density`, listed before the specks in `things`, whose front light and extinction veil them. */
  readonly veil?: DensityDrawable;
};

type Veil = { readonly kind: "clear" } | { readonly kind: "density"; readonly draw: DensityDrawable };

/**
 * `count` camera-facing specks in one instanced draw, each placed, sized and coloured by your own expression of
 * (index, seconds); nothing is stored per speck. Hash the index (`hash22(vec2(i, k))`) for stable variation; any path
 * (ballistics, drag, orbits, a fan) is your own equation of the seconds. Sub-pixel specks keep a pixel-sized footprint
 * and fade by projected area, so they do not shimmer. `stretch` samples each path a short time earlier to elongate it
 * along its projected motion. The sun and scene lights light each speck; opaque bodies hide it; a `veil` attenuates
 * and tints it by the cloud's own integral to its centre.
 *
 * Cannot sort overlapping specks by depth (they blend in index order), cast shadows, or keep state between frames.
 * Give a finished speck opacity 0.
 */
export function specks(name: string, count: number, speck: (index: F, seconds: F) => Speck, o: SpecksOptions = {}): Drawable {
  const where = `draw.specks ${shown(name)}`;
  if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) {
    throw new SceneError(where, `name ${shown(name)}`, "an identifier starting with a letter", 'use a name like "dust"');
  }
  if (!Number.isInteger(count) || count < 1 || count > 2 ** 24) {
    throw new SceneError(where, `count ${shown(count)}`, "an integer from 1 to 16777216 (the index is an exact f32)", "give a fixed count in that range");
  }
  const stretch = o.stretch === undefined ? 0 : o.stretch;
  if (typeof stretch !== "number" || !Number.isFinite(Math.fround(stretch)) || stretch < 0) {
    throw new SceneError(where, `stretch ${shown(stretch)}`, "a finite nonnegative duration in seconds", "give a small duration such as 0.01, or omit stretch");
  }
  if (typeof speck !== "function") {
    throw new SceneError(where, `speck ${shown(speck)}`, "a function of (index, seconds)", "write (i, t) => ({ at, radius, colour, opacity })");
  }
  const density = o.veil;
  if (density !== undefined && !(density instanceof DensityDrawable)) {
    throw new SceneError(where, `veil ${shown(density)}`, "the drawable returned by draw.density", "pass your cloud drawable and list it before the specks in things, or omit veil");
  }
  const veil: Veil = density === undefined ? { kind: "clear" } : { kind: "density", draw: density };
  const i = variable("i", "f32"), t = variable("t", "f32");
  const q = speck(i, t);
  if (q === null || typeof q !== "object") {
    throw new SceneError(where, `speck returns ${shown(q)}`, "an object with at, radius, colour and opacity", "return all four fields");
  }
  for (const key of ["at", "radius", "colour", "opacity"] as const) {
    const value = q[key];
    const ty = key === "at" || key === "colour" ? "vec3" : "f32";
    const correct = value instanceof Expr ? value.ty === ty
      : ty === "vec3" ? Array.isArray(value) && value.length === 3 && value.every((v) => typeof v === "number")
        : typeof value === "number";
    if (!correct) throw new SceneError(where, `speck.${key} ${shown(value)}`, `a ${ty} expression`, ty === "vec3" ? `give ${key} as vec3(x, y, z)` : `give ${key} as a scalar`);
  }
  return {
    name, forms: [], skeletons: [],
    draws(ctx) {
      const key = `specks.${name}`;
      const sample = emitShaderFn(ctx.slots, {
        name: "grain", args: [["i", "f32"], ["t", "f32"]], out: "Grain",
        roots: [lift<"vec3">(q.at), lift<"f32">(q.radius), lift<"vec3">(q.colour), lift<"f32">(q.opacity)],
        ret: (v) => `Grain(${v.join(", ")})`,
      });
      const past = stretch === 0 ? { code: "", libs: [] } : emitShaderFn(ctx.slots, {
        name: "grain_position", args: [["i", "f32"], ["t", "f32"]], out: "vec3<f32>",
        roots: [lift<"vec3">(q.at)], ret: (v) => v[0]!,
      });
      const light = grainLight(ctx);
      const front = grainVeil(ctx, veil, name, count);
      return {
        shaders: { [key]: { code: [GRAIN, sample.code, past.code, light.code, front.code, billboard(stretch, ctx.ground.kind !== "nothing"),
          fragment("fs", "grain_veil"), front.cached ? fragment("fs_scene_pass", "grain_veil_direct") : ""].join("\n"),
          libs: [...new Set([...sample.libs, ...past.libs, ...light.libs, ...front.libs])] } },
        draws: [drawSpec({ label: name, shader: key, vertices: 6, instances: count, phase: "transparent", cull: "none", depthWrite: false, blend: "alpha",
          scenePassFragment: front.cached ? "fs_scene_pass" : undefined })],
      };
    },
  };
}

function grainLight(ctx: CompileContext): Shader {
  const p = variable("p", "vec3"), ray = variable("ray", "vec3");
  const light = foldAt(ctx.lighting, { kind: "point", at: p })("sum", (l) => {
    const forward = saturate(add(0.5, mul(0.5, dot(l.toward, ray))));
    return mul(l.amount, add(0.35, mul(0.65, mul(forward, forward))));
  });
  return emitShaderFn(ctx.slots, { name: "grain_light", args: [["p", "vec3"], ["ray", "vec3"]], out: "vec3<f32>", roots: [light], ret: (v) => v[0]! });
}

function grainVeil(ctx: CompileContext, veil: Veil, name: string, count: number): Shader & { cached: boolean } {
  switch (veil.kind) {
    case "clear": return { code: "fn grain_veil(p: vec3<f32>, d: vec3<f32>) -> vec4<f32> { return vec4<f32>(0.0); }", libs: [], cached: false };
    case "density": {
      const d = veil.draw;
      const at = ctx.things.indexOf(d);
      if (at < 0 || at >= ctx.things.findIndex((t) => t.name === name)) {
        throw new SceneError(`draw.specks ${shown(name)}`, `veil ${shown(d.name)}`, "a cloud drawn before the specks", `put ${d.name} before ${name} in scene things`);
      }
      return specksVeil(ctx, d, count);
    }
  }
}

const GRAIN = /* wgsl */ `
struct Grain { centre: vec3<f32>, radius: f32, colour: vec3<f32>, opacity: f32 };
struct GrainVertex {
  @builtin(position) clip: vec4<f32>,
  @location(0) @interpolate(linear) uv: vec2<f32>,
  @location(1) @interpolate(flat) centre: vec3<f32>,
  @location(2) @interpolate(flat) rgba: vec4<f32>,
};
`;

function billboard(stretch: number, ground: boolean): string {
  const axis = stretch === 0 ? /* wgsl */ `
  let along = vec2<f32>(1.0, 0.0);
  let trail = 0.0;` : /* wgsl */ `
  let past = frame.view_proj * vec4<f32>(grain_position(f32(i), frame.time.x - ${lit(stretch)}), 1.0);
  let delta = (clip.xy / clip.w - past.xy / max(past.w, 1e-9)) * (0.5 * frame.viewport.xy);
  let speed = length(delta);
  // Sub-pixel travel stays aligned to the pixel grid, so a changing angle cannot change the four-tap energy.
  let along = select(vec2<f32>(1.0, 0.0), delta / max(speed, 1e-9), speed > 1.0 && past.w > 0.0);
  let trail = select(0.0, 0.5 * (speed - 1.0), past.w > 0.0 && speed > 1.0);`;
  return /* wgsl */ `
@vertex fn vs(@builtin(vertex_index) v: u32, @builtin(instance_index) i: u32) -> GrainVertex {
  let g = grain(f32(i), frame.time.x);
  let clip = frame.view_proj * vec4<f32>(g.centre, 1.0);
  if (!(g.radius > 0.0 && g.opacity > 0.0 && clip.w > 0.0)${ground ? " || g.centre.y < sea_height(g.centre.xz)" : ""}) {
    return GrainVertex(vec4<f32>(0.0, 0.0, 2.0, 1.0), vec2<f32>(0.0), vec3<f32>(0.0), vec4<f32>(0.0));
  }
  let fy = length(vec3<f32>(frame.view_proj[0][1], frame.view_proj[1][1], frame.view_proj[2][1]));
  let radius = g.radius * (0.5 * frame.viewport.y * fy) / clip.w;${axis}
  let side = vec2<f32>(-along.y, along.x);
  let ext = vec2<f32>(max(radius, 1.0) + trail, max(radius, 1.0));
  let corner = vec2<f32>(quad_corner(v));
  var xy: vec2<f32>;
  var uv: vec2<f32>;
  if (radius <= 1.0 && trail == 0.0) {
    let centre = (clip.xy / clip.w * 0.5 + 0.5) * frame.viewport.xy;
    // Cover the four pixels whose centres the tent reaches. Snapping the *quad* to their edges leaves the tent
    // at the true centre, and MSAA cannot trim the coverage a second time as a grain crosses a pixel boundary.
    xy = (floor(centre - 0.5) - centre) + 2.0 * corner;
    uv = xy;
  } else {
    // Pad the raster bound by the projection of a half-pixel square. Only the fragment tent controls coverage.
    let pad = 0.5 * (abs(along.x) + abs(along.y));
    let offset = (ext + pad) * (2.0 * corner - 1.0);
    xy = along * offset.x + side * offset.y;
    uv = offset / ext;
  }
  // Area compensation: the separable tent partitions unity on the pixel grid at unit width, even during translation.
  let fade = (radius / ext.x) * (radius / ext.y);
  return GrainVertex(vec4<f32>(clip.xy + xy * (2.0 * frame.viewport.zw * clip.w), clip.zw), uv,
    g.centre, vec4<f32>(g.colour, min(g.opacity, 1.0) * fade));
}`;
}

function fragment(entry: string, veil: string): string {
  // Separate entry points keep the full cloud integrator out of the fast pipeline's register allocation.
  return /* wgsl */ `
@fragment fn ${entry}(in: GrainVertex) -> @location(0) vec4<f32> {
  let edge = max(vec2<f32>(0.0), 1.0 - abs(in.uv));
  let alpha = in.rgba.a * edge.x * edge.y;
  if (alpha <= 0.0) { discard; }
  let ray = view_dir(in.centre);
  let light = grain_light(in.centre, ray);
  let colour = clamp(in.rgba.rgb * light, vec3<f32>(0.0), vec3<f32>(1.0));
  let front = ${veil}(in.centre, ray);
  // The cloud already covers the background. In straight-alpha over, replacing its rear with a grain
  // needs the source to include the same front scattering: C_front + T_front * grain.
  return vec4<f32>(colour * (1.0 - front.a) + front.rgb, alpha);
}`;
}

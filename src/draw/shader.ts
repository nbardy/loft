// `draw.shader`: a raw drawable, rung 3 of the escape ladder (docs/proposals/2026-11-18_authoring_api.md section 4). The
// author writes `@vertex fn vs` and `@fragment fn fs` in a `wgsl` module; Loft supplies the frame (`frame.view_proj`,
// `frame.viewport`, `camera_ray(px)`), `time`, and whatever holes bring in (params, inputs, compute outputs), each
// through a generated accessor, so no slot index is ever written. The draw is unstyled and has no `forms`: checks cannot
// see it, and say nothing about it.

import { emitShaderFn } from "../compile/bind.ts";
import { drawSpec, type Drawable, type DrawSpec } from "../compile/context.ts";
import { SceneError, shown } from "../errors.ts";
import { WGSL_TY } from "../expr.ts";
import { WGSL_STATE, type WgslModule } from "../text/index.ts";

/** The draw's state: how many vertices (and instances) `vs` is run for, its phase, culling, depth and blending. */
export type ShaderOptions = Pick<DrawSpec, "vertices"> & Partial<Pick<DrawSpec, "instances" | "phase" | "cull" | "depthWrite" | "blend">>;

const IDENT = /^[A-Za-z][A-Za-z0-9_]*$/;

/**
 * A drawable written in WGSL: `module` defines `@vertex fn vs` and `@fragment fn fs`, drawn `o.vertices` times per
 * instance. A fragment stage that calls `scene_depth` or `scene_position` reads the opaque scene behind it (a
 * transparent phase only).
 */
export function drawShader(name: string, module: WgslModule, o: ShaderOptions): Drawable {
  const where = `draw.shader "${name}"`;
  if (typeof name !== "string" || !IDENT.test(name) || name.includes("__") || name.startsWith("loft_"))
    throw new SceneError("draw.shader", `name ${shown(name)}`, "an identifier without double underscores or a loft_ prefix", "name it like sparks");
  const state = typeof module === "object" && module !== null ? module[WGSL_STATE] : undefined;
  if (state === undefined)
    throw new SceneError(where, typeof module === "string" ? "a string" : shown(module), "a wgsl module", "write draw.shader(name, wgsl`...`, { vertices })");
  for (const [entry, stage] of [["vs", "vertex"], ["fs", "fragment"]] as const) {
    const f = state.module.fns.find((x) => x.sig.name === entry);
    if (f?.kind !== "entry" || f.stage !== stage)
      throw new SceneError(where, f === undefined ? `no fn ${entry}` : `fn ${entry} as a ${f.kind === "entry" ? `@${f.stage} entry` : "plain function"}`, `@${stage} fn ${entry}`, `write @${stage} fn ${entry}(...) in the module`);
  }
  // Written now, so a mistake in the text is reported where the drawable is made.
  const code = state.entries(["vs", "fs"]);
  const sceneDepth = [code.text, ...code.libs.map((l) => l.wgsl)].some((t) => /\bscene_(depth|position)\s*\(/.test(t));
  return {
    name, skeletons: [], forms: [],
    draws(ctx) {
      const accessors = code.values.map((e, k) => emitShaderFn(ctx.slots, { name: `loft_value_${k}`, args: [], out: WGSL_TY[e.ty], roots: [e], ret: ([v]) => v! }));
      return {
        shaders: { [name]: { code: [code.text, ...accessors.map((a) => a.code)].join("\n"), libs: [...code.libs, ...accessors.flatMap((a) => a.libs)] } },
        draws: [drawSpec({ ...o, label: name, shader: name, ...(sceneDepth ? { sceneDepth } : {}) })],
      };
    },
  };
}

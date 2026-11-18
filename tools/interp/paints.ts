// Isolate the compiler's actual functions, including its folding and library calls.
import { PRELUDE_WGSL, type View } from "../../src/artefact.ts";
import { compileScene } from "../../src/compile/index.ts";
import { CORE_WGSL } from "../../src/compile/scene_wgsl.ts";
import { plan } from "../../src/compile/rates.ts";
import { fold, libsOf } from "../../src/expr.ts";
import { frameUniform, viewState } from "../../src/host/host.ts";
import titan from "../../examples/bosses/reefback_titan.ts";
import wyrm from "../../examples/bosses/maelstrom_wyrm.ts";
import { UnsupportedPaintOp } from "./bytecode.ts";

export const common = PRELUDE_WGSL + CORE_WGSL + `
fn outline_world() -> f32 { return 0.; }
@vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
  return vec4f(vec2f(f32(i & 1u) * 4. - 1., f32(i >> 1u) * 4. - 1.), 0., 1.);
}
fn world_at(uv: vec2f) -> vec3f {
  return vec3f((uv.x - .5) * 20., 0., (uv.y - .5) * 20.);
}
`;

export function paints() {
  const view: View = { camera: { kind: "orthographic", eye: [0, 50, 0], target: [0, 0, 0], height: 20 },
    width: 512, height: 512, time: 0.91 };
  return [
    { scene: titan, name: "paint_titan", call: "vec4f(paint_titan(uv.x, uv.y, world_at(uv), vec3f(0,1,0), 0.), 1.)" },
    { scene: wyrm, name: "colour_sea_2", call: "colour_sea_2(world_at(uv), pixel.xy, uv.x, uv.y)" },
  ].map(({ scene, name, call }) => {
    const { artefact, programs } = compileScene(scene);
    const program = programs.flat().find(p => p.name === name);
    if (!program || program.roots.length !== 1) throw new Error(`missing single-root program ${name}`);
    const roots = fold(program.roots);
    if (plan([{ ...program, roots }]).lifted[0].length) throw new UnsupportedPaintOp("lifted frame-table fetch");
    const pattern = new RegExp(`^fn ${name}\\([^]*?^}`, "m");
    const fn = Object.values(artefact.shaders).map(s => s.match(pattern)?.[0]).find(Boolean);
    if (!fn) throw new Error(`missing compiler WGSL for ${name}`);
    return { name, root: roots[0], slots: new Map(artefact.inputs.map(i => [i.name, i.slot])),
      frame: frameUniform(view), inputs: viewState(artefact, view).inputs,
      shader: common + libsOf(roots).map(f => f.wgsl).join("\n") + "\n" + fn + `
@fragment fn fs(@builtin(position) pixel: vec4f) -> @location(0) vec4f {
  let uv = pixel.xy / 512.;
  return ${call};
}` };
  });
}

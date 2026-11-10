// compile(scene) -> Artefact. One pass: every drawable emits its shaders and draws through the context (slots for
// inputs, params, curve rows and per-frame tables are registered as expressions are emitted, and a table is prepared
// the first time anything reads it); then the tables are packed, the shared WGSL is assembled, each shader module gets
// the library functions `sceneWgsl` does not already hold, the draws are ordered by phase and the pose function built.

import type { Artefact, Draw, FrameState, PostPass } from "../artefact.ts";
import { compileJs, type Env, type Expr, type RowSource, type TableFrame, type TablePrep, type TableSource, type V } from "../expr.ts";
import type { Scene } from "../scene.ts";
import { Slots } from "./bind.ts";
import { PASS_OF, PHASES, type CompileContext, type DrawSpec, type PostSpec, type Shader } from "./context.ts";
import { CORE_WGSL, styleWgsl } from "./scene_wgsl.ts";
import { poseFrame, poseFunction, Tables } from "./tables.ts";

/** CPU evaluation of expressions over a posed frame; free variables bound per call. */
export type CpuFn = (frame: TableFrame, vars?: Env["vars"]) => (V | boolean)[];

/** The artefact, and the compiler's own view of it, for checks, tests and the baker. */
export type Compiled = {
  readonly artefact: Artefact;
  /** The pose pass's frame for a state: every table as the shaders will read it. */
  frame(state: FrameState): TableFrame;
  /** Expressions on the CPU over a frame. Their leaves must already be in the artefact (anything drawn reads them). */
  cpu(roots: readonly Expr[]): CpuFn;
  /** A per-frame table of the artefact, by name (for raw-WGSL tables, which no expression names). */
  table(name: string): TableSource;
};

export function compile(scene: Scene): Artefact {
  return compileScene(scene).artefact;
}

export function compileScene(scene: Scene): Compiled {
  const tables = new Tables();
  // The slots and the table registry need each other: a fetched table is declared on first sight, and preparing it
  // compiles CPU expressions that register their own leaves.
  const slots: Slots = new Slots((t) => tables.declare(t, prep));
  const rows: RowSource = { kind: "table", slot: (r) => slots.row(r) };
  const cpu = (roots: readonly Expr[]): CpuFn => {
    slots.register(roots);
    const run = compileJs(roots, rows);
    return (frame, vars = {}) => run({ ...frame.env, vars });
  };
  const prep: TablePrep = { compile: cpu };
  const posts: PostSpec[] = [...scene.style.post];

  const ctx: CompileContext = {
    slots, style: scene.style, sea: scene.sea, sun: scene.sun, things: scene.things,
    table: (t) => tables.declare(t, prep),
    post: (p) => { posts.push(p); },
  };

  const modules: Record<string, Shader> = {};
  const specs: DrawSpec[] = [];
  for (const t of scene.things) {
    const e = t.draws(ctx);
    for (const [k, shader] of Object.entries(e.shaders)) {
      if (k in modules) throw new Error(`compile: two drawables emit a shader named "${k}"`);
      modules[k] = shader;
    }
    for (const d of e.draws) {
      if (d.sceneDepth === true && PASS_OF[d.phase] !== "transparent") throw new Error(`compile: draw "${d.label}" reads scene depth but its phase "${d.phase}" is opaque`);
      if (!(d.shader in modules)) throw new Error(`compile: draw "${d.label}" names the shader "${d.shader}", which no drawable emitted`);
    }
    specs.push(...e.draws);
  }
  const style = styleWgsl(slots, scene.style, scene.sea, scene.sun);
  const layout = tables.layout();
  slots.seal();
  const shared = slots.libs();
  const sceneWgsl = [layout.wgsl, CORE_WGSL, style, ...shared.map((l) => l.wgsl)].join("\n");
  const shaders: Record<string, string> = {};
  for (const [k, m] of Object.entries(modules)) {
    const own = m.libs.filter((l) => !shared.includes(l));
    shaders[k] = [...own.map((l) => l.wgsl), m.code].join("\n");
  }
  const post: PostPass[] = posts.map((p, i) => {
    const key = `post.${i}`;
    shaders[key] = p.wgsl;
    return { label: p.label, shader: key };
  });

  const leaves = slots.tables();
  const readEnv = slots.envReader();
  const envOf = (state: FrameState): Omit<Env, "tables"> => ({ vars: {}, ...readEnv(state.inputs, state.params), curves: state.curves });

  // Stable sort by phase: within a phase, the order drawables emitted.
  const draws: Draw[] = PHASES.flatMap((phase) => specs.filter((d) => d.phase === phase)).map(({ phase, ...d }) => ({ ...d, pass: PASS_OF[phase] }));
  const triangles = draws.filter((d) => d.layer === "scene").reduce((n, d) => n + (d.vertices / 3) * d.instances, 0);
  const artefact: Artefact = {
    version: 1,
    clear: scene.style.clear,
    inputs: leaves.inputs,
    inputFloats: leaves.inputFloats,
    params: leaves.params,
    curves: leaves.curves,
    dynamic: layout.dynamic,
    ...(layout.statics.length > 0 ? { statics: layout.statics } : {}),
    sceneWgsl,
    shaders,
    draws,
    ...(post.length > 0 ? { post } : {}),
    pose: poseFunction(layout, envOf),
    cameras: scene.cameras,
    stats: { triangles, draws: draws.length, curveRows: leaves.rows, stations: layout.stations },
  };
  const byName = new Map([...layout.placed.keys()].map((t) => [t.name, t]));
  const table = (name: string): TableSource => {
    const t = byName.get(name);
    if (!t) throw new Error(`compile: no table named "${name}" (tables: ${[...byName.keys()].join(", ")})`);
    return t;
  };
  return { artefact, frame: (state) => poseFrame(layout, envOf, state).frame, cpu, table };
}

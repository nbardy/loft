// compile(scene) -> Artefact. One pass: every drawable emits its shaders and draws through the context (slots for
// inputs, params and curve rows, tables, station slices and charts are registered as code is emitted); then the
// tables are packed, the shared WGSL is assembled, the draws are ordered by phase and the pose function is built.

import type { Artefact, Draw, FrameState, PostPass } from "../artefact.ts";
import { compileJs, time, variable, vec2, type Env, type Expr, type RowSource } from "../expr.ts";
import type { Strand } from "../geom/strand.ts";
import { STATIONS_TABLE } from "../geom/stations.ts";
import type { Scene } from "../scene.ts";
import { Slots } from "./bind.ts";
import { chartTwin, chartWgsl } from "./charts.ts";
import { PASS_OF, PHASES, type ChartRef, type CompileContext, type DrawSpec, type PoseFrame, type PostSpec } from "./context.ts";
import { StationPose } from "./pose.ts";
import { CORE_WGSL, styleWgsl } from "./scene_wgsl.ts";
import { CpuProgram, poseFunction, Tables } from "./tables.ts";

/** The artefact, and the compiler's own view of it: charts and pose frames, for checks and tests. */
export type Compiled = {
  readonly artefact: Artefact;
  /** The chart of a strand slice the drawables asked for (registering a new one after compilation is an error). */
  chart(strand: Strand, at?: Expr<"f32">): ChartRef;
  /** The pose pass's frame for a state: tables and CPU values, as the fills see them. */
  frame(state: FrameState): PoseFrame;
};

export function compile(scene: Scene): Artefact {
  return compileScene(scene).artefact;
}

export function compileScene(scene: Scene): Compiled {
  const slots = new Slots();
  const tables = new Tables();
  const program = new CpuProgram();
  const stationsRef = { name: STATIONS_TABLE, wgsl: `t_${STATIONS_TABLE}` };
  const pose = new StationPose(slots, program, stationsRef);
  const shared = new Map<string, string>();
  const posts: PostSpec[] = [...scene.style.post];
  const charts = new Map<Strand, Map<Expr<"f32">, ChartRef>>();
  let sealed = false;

  const chart = (strand: Strand, at: Expr<"f32"> = time): ChartRef => {
    const seen = charts.get(strand)?.get(at);
    if (seen) return seen;
    if (sealed) throw new Error(`chart "${strand.name}": no drawable asked for this slice while compiling`);
    const range = pose.range(strand, at);
    const slice = charts.get(strand)?.size ?? 0;
    const name = slice === 0 ? `chart_${strand.name}` : `chart_${strand.name}_${slice}`;
    ctx.shared(name, chartWgsl(slots, name, strand, range, at));
    const twin = chartTwin(slots, strand, range, at);
    const made: ChartRef = { range, wgsl: name, cpu: (frame) => twin(frame.table(range.table), frame.env) };
    charts.set(strand, (charts.get(strand) ?? new Map()).set(at, made));
    return made;
  };

  const ctx: CompileContext = {
    slots, style: scene.style, sea: scene.sea, sun: scene.sun, things: scene.things,
    table: (spec) => tables.declare(spec),
    evaluate: (roots) => program.add(roots),
    stations: (strand, at = time) => pose.range(strand, at),
    chart,
    shared: (key, wgsl) => {
      const seen = shared.get(key);
      if (seen !== undefined && seen !== wgsl) throw new Error(`compile: two drawables share WGSL under the key "${key}" with different code`);
      shared.set(key, wgsl);
    },
    post: (p) => { posts.push(p); },
  };

  const shaders: Record<string, string> = {};
  const specs: DrawSpec[] = [];
  for (const t of scene.things) {
    const e = t.draws(ctx);
    for (const [k, code] of Object.entries(e.shaders)) {
      if (k in shaders) throw new Error(`compile: two drawables emit a shader named "${k}"`);
      shaders[k] = code;
    }
    for (const d of e.draws) {
      if (d.sceneDepth === true && PASS_OF[d.phase] !== "transparent") throw new Error(`compile: draw "${d.label}" reads scene depth but its phase "${d.phase}" is opaque`);
    }
    specs.push(...e.draws);
  }
  const style = styleWgsl(slots, scene.style, scene.sea, scene.sun);
  const post: PostPass[] = posts.map((p, i) => {
    const key = `post.${i}`;
    shaders[key] = p.wgsl;
    return { label: p.label, shader: key };
  });

  // Stations last: its size is known only once every drawable has asked for its slices.
  tables.declare(pose.spec());
  sealed = true;
  const layout = tables.layout();
  const sceneWgsl = [layout.wgsl, CORE_WGSL, style, ...slots.libs().map((l) => l.wgsl), ...shared.values()].join("\n");

  // Every CPU expression registers its leaves, so a param only the pose pass reads still gets a slot.
  const seaHeightExpr = scene.sea.height(vec2(variable("x", "f32"), variable("z", "f32")));
  slots.register([...program.all(), seaHeightExpr]);
  const rows: RowSource = { kind: "table", slot: (r) => slots.row(r) };
  const seaHeightJs = compileJs([seaHeightExpr], rows);
  const compiled = program.compile(rows);
  const leaves = slots.tables();
  const readEnv = slots.envReader();
  const envOf = (state: FrameState): Env => ({ vars: {}, ...readEnv(state.inputs, state.params), curves: state.curves });
  const seaHeight = (env: Env) => {
    const vars = { x: 0, z: 0 };
    const at: Env = { ...env, vars };
    return (x: number, z: number): number => { vars.x = x; vars.z = z; return seaHeightJs(at)[0] as number; };
  };
  const poseFn = poseFunction(layout, compiled, envOf, seaHeight);

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
    sceneWgsl,
    shaders,
    draws,
    ...(post.length > 0 ? { post } : {}),
    pose: poseFn,
    cameras: scene.cameras,
    stats: { triangles, draws: draws.length, curveRows: leaves.rows, stations: pose.stations },
  };
  return {
    artefact,
    chart,
    frame: (state) => framesFor(layout, compiled, envOf, seaHeight, state),
  };
}

/** A pose frame over one state (the tables the pose function would return, readable one by one). */
function framesFor(...args: [...Parameters<typeof poseFunction>, FrameState]): PoseFrame {
  const [layout, program, env, seaHeight, state] = args;
  // The pose function fills every table; keep its buffers and serve reads from them.
  const buffers = poseFunction(layout, program, env, seaHeight)(state);
  const e = env(state);
  const byName = new Map(layout.placed.map((p) => [p.spec.name, p]));
  let values: ReturnType<typeof program.run> | undefined;
  return {
    env: e,
    seaHeight: seaHeight(e),
    values: (r) => program.slice((values ??= program.run(e)), r),
    table: (t) => {
      const p = byName.get(t.name);
      if (!p) throw new Error(`frame: no table named "${t.name}"`);
      return buffers[layout.dynamic[p.buffer].name].subarray(p.offset * 4, (p.offset + p.spec.vec4s) * 4);
    },
  };
}

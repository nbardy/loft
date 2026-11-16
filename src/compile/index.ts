// compile(scene) -> Artefact. One pass: every drawable emits its shaders and draws through the context (slots for
// inputs, params, curve rows and per-frame tables are registered as expressions are emitted, and a table is prepared
// the first time anything reads it); then the tables are packed, the shared WGSL is assembled, each shader module gets
// the library functions `sceneWgsl` does not already hold, the draws are ordered by phase and the pose function built.

import { DETAIL_SLOTS, POST_WGSL, PRELUDE_WGSL, SCENE_CLEARANCE_WGSL, SCENE_DEPTH_WGSL, type Artefact, type ArtefactPass, type Detail, type Draw, type FrameState, type Loop, type PostPass, type Recurrence } from "../artefact.ts";
import { compileJs, type Env, type Expr, type LibFn, type RowSource, type TableFrame, type TablePrep, type TableSource, type V } from "../expr.ts";
import type { Scene } from "../scene.ts";
import { Slots } from "./bind.ts";
import type { Program } from "./rates.ts";
import { PASS_OF, PHASES, type CompileContext, type DetailSpec, type DrawSpec, type PostSpec, type Shader } from "./context.ts";
import { CORE_WGSL, styleWgsl } from "./scene_wgsl.ts";
import { INK_POST, MARK_WGSL } from "../draw/ink.ts";
import { measureRest } from "../draw/grid_detail.ts";
import { castPlan } from "../draw/cast.ts";
import { lampShadows, refuseSpread } from "../frame/lamp_shadow.ts";
import { cpuPoseFunction, poseFrame, poseFunction, poseProfile, Tables, bufferName, type TableLayout } from "./tables.ts";
import { discover, planPasses } from "./passes.ts";
import { SceneError } from "../errors.ts";

/** CPU evaluation of expressions over a posed frame; free variables bound per call. */
export type CpuFn = (frame: TableFrame, vars?: Env["vars"]) => (V | boolean)[];

/** The artefact, and the compiler's own view of it, for checks, tests and the baker. */
export type Compiled = {
  readonly artefact: Artefact;
  /** The pose pass's frame for a state: every table as the shaders will read it. */
  frame(state: FrameState): TableFrame;
  /** Expressions on the CPU over a frame. Their leaves must already be in the artefact (anything drawn reads them). */
  cpu(roots: readonly Expr[]): CpuFn;
  /** CPU milliseconds of one frame per `cpu` table, each without the tables it read (for `loft profile`). */
  poseProfile(state: FrameState): ReadonlyMap<string, number>;
  /** A per-frame table of the artefact, by name (for raw-WGSL tables, which no expression names). */
  table(name: string): TableSource;
  /** Every program the compiler lowered (src/compile/rates.ts), by group: for tools/rates_census.ts. */
  readonly programs: readonly (readonly Program[])[];
};

/**
 * Compiles a `Scene` to an `Artefact`: the WGSL shaders, draw list, and tables a WebGPU host draws, plus the CPU
 * functions that pose the per-frame tables. Called by a host or tool, once per scene; a scene author builds the
 * `Scene` and does not need to look inside the result.
 */
export function compile(scene: Scene): Artefact {
  return compileScene(scene).artefact;
}

export function compileScene(scene: Scene): Compiled {
  // The slots and the table registry need each other: a fetched table is declared on first sight, and classifying it
  // registers the leaves of its program (or compiles the CPU expressions of a hand-written fill).
  const slots: Slots = new Slots((t) => tables.declare(t, prep), (p) => tables.lower(p));
  const rows: RowSource = { kind: "table", slot: (r) => slots.row(r) };
  const tables = new Tables({ register: (roots) => slots.register(roots), cpu: (roots) => compileJs(roots, rows), wgsl: (vars) => slots.wgsl(vars), input: (spec) => slots.input(spec).slot });
  const cpu = (roots: readonly Expr[]): CpuFn => {
    slots.register(roots);
    const run = compileJs(roots, rows);
    return (frame, vars = {}) => run({ ...frame.env, vars });
  };
  const prep: TablePrep = { compile: cpu };
  const posts: PostSpec[] = [...scene.style.post];
  let screenInk = false;
  const notes: string[] = [];
  const markOut = (mark: string, inked: boolean): string => {
    switch (scene.outline) {
      case "hull": return mark;
      case "screen": screenInk ||= inked; return `loft_mark_ink(${mark}, ${inked})`;
    }
  };

  // A lamp that asked for a shadow gets its depth pass and test here, before anything folds the lights (DESIGN.md 17).
  const lamps = lampShadows(scene.lighting, scene.things);
  const lighting = lamps.lighting;
  const ctx: CompileContext = {
    slots, style: scene.style, ground: scene.ground, sun: scene.sun, sunAt: scene.sunAt, lighting, things: scene.things,
    table: (t) => tables.declare(t, prep),
    post: (p) => { posts.push(p); },
    outline: scene.outline, markOut,
    note: (line) => { if (!notes.includes(line)) notes.push(line); },
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
  // The ink pass runs first: it restores the plain mark in alpha for the style's own passes.
  if (screenInk) posts.unshift({ label: "ink outline", wgsl: INK_POST });
  const style = styleWgsl(slots, scene.style, scene.ground, scene.sun, lighting);
  // Passes are found from what the shaders call (src/compile/passes.ts); a scene that reads none finds none here, and
  // every step below that is about passes then does nothing.
  const found = discover(slots, [...slots.libs(), ...Object.values(modules).flatMap((m) => m.libs)]);
  for (const d of found.defs) if (d.what.kind === "scene") tables.declare(d.what.camera, prep);
  const depthPasses = found.defs.some((d) => d.what.kind === "scene" && d.what.keep === "depth");
  // castRay: its tables and functions exist only in a scene whose code calls it (src/draw/cast.ts).
  const casting = castPlan(ctx, [...Object.values(modules).flatMap((m) => m.libs), ...slots.libs()]);
  const layout = tables.layout();
  if (found.defs.length > 0) {
    const reader = layout.stages.find((st) => /\bloft_passtex_/.test(st.wgsl));
    if (reader !== undefined) throw new SceneError(`table stage "${reader.label}"`, "a table that reads a pass", "tables that are closed forms of the frame (a table filled from a picture would carry pictures from frame to frame)", "read the pass in a shader's paint or style, not in a table");
  }
  slots.seal();
  const shared = slots.libs();
  const leaves = slots.tables();
  const readEnv = slots.envReader();
  const envOf = (state: FrameState): Omit<Env, "tables"> => ({ vars: {}, ...readEnv(state.inputs, state.params), curves: state.curves });
  // Rest: time 0, every input and param at its default (as a static `tabulate` is filled).
  const restState: FrameState = {
    time: 0, curves: leaves.curves,
    inputs: (() => { const x = new Float32Array(leaves.inputFloats); for (const s of leaves.inputs) x.set(s.def, s.slot); return x; })(),
    params: (() => { const x = new Float32Array(leaves.params.reduce((n, q) => Math.max(n, q.slot + 1), 0)); for (const q of leaves.params) x[q.slot] = q.def; return x; })(),
  };
  // Before any bake: a recurrence reads as its start state here (`Unbaked`), the only reading that may.
  const restFrame = poseFrame(layout, envOf, restState, undefined, "start").frame;
  // Only a map something reads is in the artefact (a style that never folds the lights reads none), and only its
  // tables were laid out.
  for (const s of lamps.spreads.filter((x) => found.defs.some((d) => d.name === x.pass))) {
    const [at, half] = cpu([s.at, s.half])(restFrame) as [number[], number];
    refuseSpread(s, at, half);
  }
  const castText = casting === undefined ? "" : casting.wgsl(cpu, restFrame);
  /** A library function's text: its own, or for the cast functions the scene's (written once, with the first of them). */
  const textOf = (l: LibFn): string => (casting !== undefined && casting.libs.includes(l) ? (l === casting.libs[0] ? castText : "") : l.wgsl);
  const sceneWgsl = [layout.wgsl, CORE_WGSL, MARK_WGSL, style, ...shared.map(textOf)].join("\n");
  const shaders: Record<string, string> = {};
  const moduleOf = (m: Shader, depth: boolean): string => {
    const own = m.libs.filter((l) => !shared.includes(l));
    // A depth entry is written only into a scene with a depth pass (Shader.depthEntry), so others keep their bytes.
    return [...own.map(textOf), m.code, ...(depth && m.depthEntry !== undefined ? [m.depthEntry] : [])].join("\n");
  };
  for (const [k, m] of Object.entries(modules)) shaders[k] = moduleOf(m, depthPasses);
  for (const [name, m] of found.screens) shaders[`pass.${name}`] = moduleOf(m, false);
  const post: PostPass[] = posts.map((p, i) => {
    const key = `post.${i}`;
    shaders[key] = p.wgsl;
    return { label: p.label, shader: key };
  });

  // Stable sort by phase: within a phase, the order drawables emitted.
  const detailOf = detailSlots(layout.placed, (roots) => cpu(roots)(restFrame));
  const draws: Draw[] = PHASES.flatMap((phase) => specs.filter((d) => d.phase === phase)).map(({ phase, detail, depth, ...d }) => {
    const resolved = detailOf(detail, d);
    // The shader reads its level from `loft_detail[LOFT_DETAIL_SLOT + instance]` (an override, so one module serves
    // every slot it is drawn with).
    const constants = resolved.kind === "fixed" ? d.constants : { ...d.constants, LOFT_DETAIL_SLOT: resolved.slot };
    return { ...d, constants, detail: resolved, pass: PASS_OF[phase], ...(depthPasses && depth !== undefined ? { depth } : {}) };
  });
  const passes: readonly ArtefactPass[] = found.defs.length === 0 ? [] : planPasses({
    defs: found.defs,
    shared: [PRELUDE_WGSL, SCENE_DEPTH_WGSL, SCENE_CLEARANCE_WGSL, POST_WGSL, sceneWgsl].join("\n"),
    shaders, draws,
    camera: (def) => {
      if (def.what.kind !== "scene") throw new Error(`compile: pass "${def.name}" has no camera`);
      const placed = layout.placed.get(def.what.camera)!;
      return { buffer: bufferName(placed.buffer), offset: placed.offset };
    },
  });
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
    ...(notes.length > 0 ? { notes } : {}),
    shaders,
    draws,
    ...(post.length > 0 ? { post } : {}),
    ...(passes.length > 0 ? { passes } : {}),
    stages: layout.stages,
    tables: layout.info,
    cpuPose: cpuPoseFunction(layout, envOf),
    pose: poseFunction(layout, envOf),
    cameras: scene.cameras,
    loop: scene.loop,
    ...(layout.recurrences.length > 0 ? { recurrences: refuseSpans(layout.recurrences, scene.loop) } : {}),
    stats: { maxTriangles: triangles, draws: draws.length, curveRows: leaves.rows, stations: layout.stations },
  };
  const byName = new Map([...layout.placed.keys()].map((t) => [t.name, t]));
  const table = (name: string): TableSource => {
    const t = byName.get(name);
    if (!t) throw new Error(`compile: no table named "${name}" (tables: ${[...byName.keys()].join(", ")})`);
    return t;
  };
  return { artefact, frame: (state) => poseFrame(layout, envOf, state).frame, poseProfile: (state) => poseProfile(layout, envOf, state), cpu, table, programs: tables.programs };
}

/**
 * A scene that comes round every P seconds must keep each recurrence over all of [0, P], or keep it still there (until
 * at or before 0): otherwise its seam is not a measurement but a jump (docs/proposals/state.md 4.5).
 */
function refuseSpans(rs: readonly Recurrence[], loop: Loop): readonly Recurrence[] {
  if (loop.kind === "open") return rs;
  const P = loop.seconds;
  for (const r of rs) {
    const until = r.from + r.total / r.rate;
    if (r.from > 0 || (until > 0 && until < P)) {
      throw new SceneError(`scene() with period ${P}`, `the recurrence "${r.name}" kept from ${Math.max(r.from, 0)} to ${until} s`, `a span covering [0, ${P}] (from at or before 0, until at or after ${P}), or one that is still by then (until at or before 0)`, until > 0 && until < P ? `until: ${P} (and settle before 0 with a negative from, so the loop closes by physics)` : "from: 0 or earlier");
    }
  }
  return rs;
}

/**
 * Slots in `loft_detail` for each draw's level of detail, its bound table's place in the dynamic buffers, and a grid's
 * shape measured at rest (`rest` evaluates expressions in the rest frame). One choice per bound table (a surface and its
 * ink outline name the same one), one per projected spec.
 */
function detailSlots(placed: TableLayout["placed"], rest: (roots: readonly Expr[]) => (V | boolean)[]): (spec: DetailSpec, d: Pick<Draw, "label" | "instances">) => Detail {
  const slots = new Map<object, number>();
  const measured = new Map<object, ReturnType<typeof measureRest>>();
  let next = 0;
  const slotFor = (key: object, n: number, label: string): number => {
    const known = slots.get(key);
    if (known !== undefined) return known;
    if (next + n > DETAIL_SLOTS) throw new Error(`compile: draw "${label}" needs ${n} level-of-detail slots; the scene has used ${next} of ${DETAIL_SLOTS}`);
    slots.set(key, next);
    next += n;
    return next - n;
  };
  return (spec, d) => {
    switch (spec.kind) {
      case "fixed": return spec;
      case "grid": {
        const p = placed.get(spec.bound);
        if (!p) throw new Error(`compile: draw "${d.label}" states bounds in table "${spec.bound.name}", which was never declared (ctx.table)`);
        if (spec.bound.entries !== d.instances || spec.bound.lanes !== 1) throw new Error(`compile: draw "${d.label}": its bound table must have 1 lane and one entry per instance (${d.instances}), got ${spec.bound.lanes} x ${spec.bound.entries}`);
        const m = measured.get(spec.bound) ?? (() => {
          const values = rest([...spec.rest.samples, spec.rest.anchor]) as number[][];
          const r = measureRest(values.slice(0, -1), values[values.length - 1]!);
          measured.set(spec.bound, r);
          return r;
        })();
        const levels = spec.counts.map((cells) => ({ cells, vertices: 6 * cells[0] * cells[1], cell: Math.max(m.lengths[0] / cells[0], m.lengths[1] / cells[1]) }));
        return { kind: "grid", slot: slotFor(spec.bound, d.instances, d.label), bound: { buffer: bufferName(p.buffer), offset: p.offset }, radius: m.radius, levels };
      }
      case "projected": return { ...spec, slot: slotFor(spec, 2, d.label) };
    }
  };
}

/**
 * The CPU reading of tables outside any scene (tests and tools): every table they read is declared and filled from its
 * program at time 0 with every input and param at its default, curve rows as authored.
 */
export function readTablesAlone(roots: readonly TableSource[]): (t: TableSource) => Float32Array {
  const slots: Slots = new Slots((t) => tables.declare(t, prep), (p) => tables.lower(p));
  const prep: TablePrep = { compile: (r) => { slots.register(r); const run = compileJs(r); return (frame, vars = {}) => run({ ...frame.env, vars }); } };
  const tables = new Tables({ register: (r) => slots.register(r), cpu: (r) => compileJs(r), wgsl: (v) => slots.wgsl(v), input: (spec) => slots.input(spec).slot });
  for (const t of roots) tables.declare(t, prep);
  const layout = tables.layout();
  const state: FrameState = { time: 0, inputs: new Float32Array(0), params: new Float32Array(0), curves: new Float32Array(0) };
  const { frame } = poseFrame(layout, () => ({ vars: {}, inputs: {}, params: {} }), state);
  return (t) => frame.table(t);
}

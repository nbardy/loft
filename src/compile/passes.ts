// Passes, found from what is drawn (src/frame/pass.ts). Nothing registers a pass: `discover` collects the passes named
// by the library functions the scene's shaders call, `plan` decides each pass's draws and its place in the frame.
//
// Which draws read a pass is decided on the final WGSL, by static use: a draw reads pass X when its entry points reach,
// through the functions they call, the texture X is bound as. That is the rule WebGPU itself applies to bindings, so
// the answer is exact for expressions and raw WGSL alike (a style's `shade` lives in the shared scene code, and only
// the entry points that call it read what it reads).
//
// The rule about readers: a draw whose entry points read X is not drawn into X (a mirror is left out of its own
// reflection). A depth entry that reads X is a true cycle (a shape whose depth depends on its own shadow): refused.

import { SceneError } from "../errors.ts";
import { libsOf, variable, type Expr, type LibFn, type ProgramTable } from "../expr.ts";
import { passOf, type PassDef } from "../frame/pass.ts";
import type { ArtefactPass, Draw } from "../artefact.ts";
import { emitShaderFn, type Slots } from "./bind.ts";
import type { Shader } from "./context.ts";

/** Every pass the given library functions read (and their dependencies), with the passes those passes read, in order of discovery. */
export function discover(slots: Slots, roots: readonly LibFn[]): { readonly defs: readonly PassDef[]; readonly screens: ReadonlyMap<string, Shader> } {
  const defs: PassDef[] = [];
  const screens = new Map<string, Shader>();
  const seen = new Set<LibFn>();
  const visit = (fns: readonly LibFn[]): void => {
    for (const fn of fns) {
      if (seen.has(fn)) continue;
      seen.add(fn);
      visit(fn.deps ?? []);
      const d = passOf(fn);
      if (d === undefined || defs.includes(d)) continue;
      defs.push(d);
      switch (d.what.kind) {
        case "scene": {
          const read = passesIn(libsOf(laneRoots(d.what.camera)));
          if (read.length > 0) throw new SceneError(`pass "${d.name}"`, `a camera that reads pass "${read[0]}"`, "a camera that is a closed form of the frame (cameras are computed before any pass is drawn)", "compute the camera from the scene's own numbers, not from a picture");
          break;
        }
        case "screen": {
          const s = emitShaderFn(slots, { name: `loft_screen_${d.name}`, args: [[d.what.uv, "vec2"]], out: "vec4<f32>", roots: [d.what.colour], ret: (o) => o[0]! });
          screens.set(d.name, { code: `${s.code}\n${SCREEN_ENTRIES(d.name)}`, libs: s.libs });
          visit(s.libs);
          break;
        }
      }
    }
  };
  visit(roots);
  return { defs, screens };
}

const laneRoots = (t: ProgramTable): Expr[] => {
  switch (t.program.kind) {
    case "map": return t.program.runs.flatMap((r) => [...r.lanes(variable("loft_pass_i", "f32"))]);
    case "scan": throw new Error(`pass camera table "${t.name}" is not a map (src/frame/pass.ts builds one)`);
  }
};
const passesIn = (fns: readonly LibFn[]): string[] => fns.flatMap((fn) => { const d = passOf(fn); return d === undefined ? [] : [d.name]; });

/** A screen pass's entry points: one triangle over the target, its uv (0..1, v down) to the pass's colour function. */
const SCREEN_ENTRIES = (name: string): string => /* wgsl */ `
struct LoftScreenIn { @builtin(position) px: vec4<f32>, @location(0) uv: vec2<f32> };
@vertex fn vs(@builtin(vertex_index) vi: u32) -> LoftScreenIn {
  let uv = vec2<f32>(f32((vi << 1u) & 2u), f32(vi & 2u));
  return LoftScreenIn(vec4<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0, 0.0, 1.0), uv);
}
@fragment fn fs(in: LoftScreenIn) -> @location(0) vec4<f32> { return loft_screen_${name}(in.uv); }`;

// ---- static use ------------------------------------------------------------------------------------------------

/** The functions of some WGSL: each name, and the identifiers its body uses. */
type Graph = ReadonlyMap<string, ReadonlySet<string>>;

const FN = /\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*\(/g;
const IDENT = /[A-Za-z_][A-Za-z0-9_]*/g;

/** Parse function bodies by brace matching (comments removed first; WGSL has no string literals). */
export function graphOf(text: string): Graph {
  const code = text.replace(/\/\/[^\n]*/g, "");
  const out = new Map<string, Set<string>>();
  for (const m of code.matchAll(FN)) {
    const open = code.indexOf("{", m.index! + m[0].length);
    if (open < 0) continue;
    let depth = 0, end = open;
    for (; end < code.length; end++) {
      const ch = code[end];
      if (ch === "{") depth++;
      else if (ch === "}" && --depth === 0) break;
    }
    out.set(m[1]!, new Set(code.slice(open, end).match(IDENT) ?? []));
  }
  return out;
}

/** The passes (by texture name -> pass name) that the entry points reach through the graphs (searched in order). */
function reads(graphs: readonly Graph[], entries: readonly string[], textures: ReadonlyMap<string, string>): Set<string> {
  const found = new Set<string>();
  const seen = new Set<string>();
  const stack = [...entries];
  while (stack.length > 0) {
    const name = stack.pop()!;
    if (seen.has(name)) continue;
    seen.add(name);
    const body = graphs.find((g) => g.has(name))?.get(name);
    if (body === undefined) continue;
    for (const id of body) {
      const pass = textures.get(id);
      if (pass !== undefined) found.add(pass);
      else if (!seen.has(id)) stack.push(id);
    }
  }
  return found;
}

// ---- the plan --------------------------------------------------------------------------------------------------

export type PlanInput = {
  readonly defs: readonly PassDef[];
  /** Every function a draw's or post pass's module holds besides its own shader: the prelude, the host's group 2 WGSL, the scene code. */
  readonly shared: string;
  readonly shaders: Readonly<Record<string, string>>;
  readonly draws: readonly Draw[];
  /** Where each scene pass's camera table was placed. */
  readonly camera: (d: PassDef) => { readonly buffer: string; readonly offset: number };
};

/**
 * Each pass's draws and what it reads, in drawing order (a pass after every pass it reads). Throws a `SceneError`
 * naming the passes of a cycle, or the draw whose depth entry reads the depth pass it would be drawn into.
 */
export function planPasses(p: PlanInput): readonly ArtefactPass[] {
  const textures = new Map(p.defs.map((d) => [d.texture, d.name]));
  const base = graphOf(p.shared);
  const graphs = new Map<string, Graph>();
  const graph = (shader: string): Graph => {
    const known = graphs.get(shader);
    if (known !== undefined) return known;
    const g = graphOf(p.shaders[shader]!);
    graphs.set(shader, g);
    return g;
  };
  const readsOf = (shader: string, entries: readonly string[]) => reads([graph(shader), base], entries, textures);
  const drawReads = p.draws.map((d) => readsOf(d.shader, ["vs", "fs"]));
  const depthReads = p.draws.map((d) => {
    const depth = d.depth;
    if (depth === undefined) return new Set<string>();
    switch (depth.kind) {
      case "vertex": return readsOf(d.shader, ["vs"]);
      case "entry": return readsOf(d.shader, ["vs", depth.entry]);
    }
  });

  const made: ArtefactPass[] = p.defs.map((def) => {
    switch (def.what.kind) {
      case "screen": return { name: def.name, size: def.size, reads: [...readsOf(`pass.${def.name}`, ["vs", "fs"])], what: { kind: "screen", shader: `pass.${def.name}` } };
      case "scene": {
        const keep = def.what.keep;
        const leftOut: { label: string; why: string }[] = [];
        const draws: number[] = [];
        const read = new Set<string>();
        p.draws.forEach((d, k) => {
          const why = excluded(def.name, keep, d, drawReads[k]!, depthReads[k]!);
          if (why !== undefined) { leftOut.push({ label: d.label, why }); return; }
          draws.push(k);
          for (const r of keep === "depth" ? depthReads[k]! : drawReads[k]!) read.add(r);
        });
        return { name: def.name, size: def.size, reads: [...read], what: { kind: "scene", keep, camera: p.camera(def), draws, leftOut } };
      }
    }
  });
  return ordered(made);
}

/** Why draw d is not drawn into pass `name`, or undefined when it is. */
function excluded(name: string, keep: "colour" | "depth" | "light", d: Draw, readsAll: ReadonlySet<string>, readsDepth: ReadonlySet<string>): string | undefined {
  // The sea's grid is laid over the view's own image of its plane (`Detail` "projected"): from another camera it would
  // cover the wrong rows.
  if (d.detail.kind === "projected") return "laid over the view's own image of the plane";
  switch (keep) {
    case "colour": case "light": return readsAll.has(name) ? "reads this pass" : undefined;
    case "depth": {
      if (d.pass !== "opaque" || !d.depthWrite) return "transparent or writes no depth";
      if (d.depth === undefined) return "no depth entry";
      if (readsDepth.has(name)) throw new SceneError(`pass "${name}"`, `draw "${d.label}", whose depth reads this pass`, "a depth that does not depend on the picture it is drawn into", "read the pass only where the colour is shaded, not where the shape is placed");
      return undefined;
    }
  }
}

/** Passes in an order where each comes after every pass it reads; among those free to go, the order of discovery. */
function ordered(passes: readonly ArtefactPass[]): ArtefactPass[] {
  const out: ArtefactPass[] = [];
  const placed = new Set<string>();
  const left = [...passes];
  while (left.length > 0) {
    const k = left.findIndex((q) => q.reads.every((r) => placed.has(r)));
    if (k < 0) {
      const names = left.map((q) => `"${q.name}" reads ${q.reads.filter((r) => !placed.has(r)).map((r) => `"${r}"`).join(", ")}`);
      throw new SceneError("passes", `a cycle: ${names.join("; ")}`, "passes that read each other in one direction only (each picture holds the draws that read the other)", "let one of those draws stop reading its pass, so one picture can be drawn first (of two mirrors that face each other, make one plain)");
    }
    const [q] = left.splice(k, 1);
    out.push(q!);
    placed.add(q!.name);
  }
  return out;
}

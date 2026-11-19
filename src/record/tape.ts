// The cut between author closures and pure lowering. Callbacks save references during the compile; graph construction
// and serialization happen after the TS artefact is written.
import { authoredTexts } from "../authored.ts";
import { PHASES, type Drawable, type DrawSpec, type Emitted, type PostSpec, type Shader } from "../compile/context.ts";
import type { CompileObserver, CompileStage } from "../compile/index.ts";
import type { FnSpec } from "../compile/bind.ts";
import type { Program } from "../compile/rates.ts";
import { orderCompute, type ComputePass, type ComputeResource } from "../compute.ts";
import { light } from "../draw/light.ts";
import { shadows } from "../draw/shadows.ts";
import { sky } from "../draw/sky.ts";
import { lift, rawComputeOf, variable, withIdentities, type Expr, type LibFn, type TableSource } from "../expr.ts";
import type { PassDef } from "../frame/pass.ts";
import type { Spread } from "../frame/lamp_shadow.ts";
import type { Lighting } from "../light.ts";
import type { RecurState } from "../map/recur.ts";
import type { Scene } from "../scene.ts";
import { printGraph } from "./print.ts";
import { Unrecorded } from "./recorder.ts";
import type { Role } from "./decls.ts";
import { Decl, TYS } from "./wire.ts";

export type PortTag = Decl.PortScene | Decl.Thing | Decl.Effect | Decl.StyleEmit | Decl.Lighting | Decl.LampPass
  | Decl.Posts | Decl.ComputePass | Decl.Recurrence | Decl.PortPass | Decl.Authored | Decl.Cast | Decl.RawFlags | Decl.NumericCamera;

export type PortRecord = {
  words: number[];
  lits: number[];
  syms: string[];
  fns: (LibFn | Role)[];
  kernels: (TableSource | ComputePass)[];
  blobs: Uint8Array[];
  roots: Expr[];
};

type Emission = { thing: Drawable; emitted: Emitted; posts: PostSpec[]; notes: string[] };
type Effect = { stage: CompileStage } & (
  | { kind: "register"; roots: readonly Expr[] }
  | { kind: "declare"; table: TableSource }
  | { kind: "lower"; program: Program }
);
type Lamps = { lighting: Lighting; spreads: readonly Spread[] };
type Passes = { defs: readonly PassDef[]; screens: ReadonlyMap<string, Shader> };

export class Tape {
  private stage: CompileStage = "compute";
  private readonly effects: Effect[] = [];
  private readonly things: Emission[] = [];
  private readonly style: FnSpec[] = [];
  private pendingPosts: PostSpec[] = [];
  private pendingNotes: string[] = [];
  private posts: { passes: readonly PostSpec[]; screenInk: boolean } | undefined;
  private lamps: Lamps | undefined;
  private passes: Passes | undefined;
  private cast: { wgsl: string; libs: readonly LibFn[] } | undefined;
  // The final writer visits pass defs named by raw functions as well as those found by discover (including screen deps).
  private readonly seenPasses = new Map<PassDef, number>();

  readonly observe: CompileObserver = {
    stage: (stage) => { this.stage = stage; },
    register: (roots) => { this.effects.push({ kind: "register", stage: this.stage, roots }); },
    declare: (table) => { this.effects.push({ kind: "declare", stage: this.stage, table }); },
    lower: (program) => { this.effects.push({ kind: "lower", stage: this.stage, program }); },
    thing: (thing, emitted) => {
      this.things.push({ thing, emitted, posts: this.pendingPosts, notes: this.pendingNotes });
      this.pendingPosts = [];
      this.pendingNotes = [];
    },
    post: (p) => { this.pendingPosts.push(p); },
    note: (n) => { this.pendingNotes.push(n); },
    style: (f) => { this.style.push(f); },
    lamps: (lighting, spreads) => { this.lamps = { lighting, spreads }; },
    passes: (defs, screens) => {
      this.passes = { defs, screens };
      defs.forEach((def, i) => this.seenPasses.set(def, i));
    },
    posts: (passes, screenInk) => { this.posts = { passes: [...passes], screenInk }; },
    cast: (wgsl, libs) => { this.cast = { wgsl, libs }; },
  };

  passDefs(): readonly PassDef[] { return this.passes?.defs ?? []; }

  passRecord(def: PassDef, ordinal: number): PortRecord {
    const r = record(), order = this.seenPasses.get(def), screen = this.passes?.screens.get(def.name);
    r.words.push(ordinal, Number(order !== undefined));
    if (order !== undefined) r.words.push(order);
    r.syms.push(def.texture);
    r.words.push(Number(screen !== undefined));
    if (screen !== undefined) putShader(r, `pass.${def.name}`, screen);
    return r;
  }

  rawRecord(f: LibFn, ordinal: number): PortRecord {
    const r = record();
    r.words.push(ordinal, Number(f.js === "gpu-only"));
    putText(r, rawComputeOf(f));
    return r;
  }

  recurRecord(state: RecurState): PortRecord {
    const r = record();
    r.syms.push(state.name);
    r.words.push(state.total, state.after.length, Number(state.replay !== undefined));
    r.lits.push(state.start);
    r.kernels.push(state.table, ...state.after.map((s) => s.table));
    if (state.replay !== undefined) {
      r.kernels.push(state.replay.even, state.replay.odd);
      r.roots.push(state.replay.step);
    }
    return r;
  }

  *records(scene: Scene, compiled: boolean): Iterable<[PortTag, PortRecord]> {
    // Take the registry before reopening any closure to write lighting or a table body.
    const authored = [...authoredTexts()];
    const sceneDecl = record();
    sceneDecl.words.push(["nothing", "plane", "sea"].indexOf(scene.ground.kind), ["hull", "screen"].indexOf(scene.outline),
      Number(this.posts?.screenInk === true), Number(scene.loop.kind === "every"));
    sceneDecl.lits.push(...scene.sun, ...scene.style.clear);
    if (scene.loop.kind === "every") sceneDecl.lits.push(scene.loop.seconds);
    yield [Decl.PortScene, sceneDecl];

    for (const [key, camera] of Object.entries(scene.cameras)) {
      const r = record();
      r.words.push(camera.kind === "perspective" ? 0 : 1);
      r.syms.push(key);
      r.lits.push(...camera.eye, ...camera.target, camera.kind === "perspective" ? camera.fovY : camera.height);
      yield [Decl.NumericCamera, r];
    }

    for (const emission of this.things) {
      const r = record();
      r.syms.push(kindOf(scene, emission), emission.thing.name);
      // The four empty columns are the per-kind payload. The draw lanes fill them in at step 2.
      r.words.push(0, 0, 0, 0);
      putEmitted(r, emission);
      yield [Decl.Thing, r];
    }
    for (const e of this.effects) yield [Decl.Effect, effectRecord(e)];
    for (const spec of this.style) yield [Decl.StyleEmit, styleRecord(spec)];
    if (this.posts !== undefined) {
      const r = record();
      putPosts(r, this.posts.passes);
      yield [Decl.Posts, r];
    }
    if (this.lamps !== undefined) {
      const { lighting, spreads } = this.lamps;
      // Reopening closures is only needed on a completed compile. A refusal may itself come from one of these.
      if (compiled) yield [Decl.Lighting, withIdentities(() => lightingRecord(lighting))];
      for (const s of spreads) {
        const r = record();
        r.words.push(s.light);
        r.syms.push(s.pass);
        r.roots.push(s.at, s.half);
        yield [Decl.LampPass, r];
      }
    }
    if (compiled || this.stage !== "compute") {
      const passes = orderCompute(scene.compute);
      const outputs = new Map<ComputeResource, readonly [ComputePass, number]>();
      for (const pass of passes) pass.writes.forEach(([, resource], i) => outputs.set(resource, [pass, i]));
      for (const pass of passes) {
        const r = record();
        r.kernels.push(pass);
        for (const [symbol, resource] of pass.reads) {
          // orderCompute has already checked that every read has a producer in the scene.
          const [producer, output] = outputs.get(resource)!;
          r.syms.push(symbol);
          r.kernels.push(producer);
          r.words.push(output);
        }
        yield [Decl.ComputePass, r];
      }
    }
    if (this.cast !== undefined) {
      const r = record();
      r.syms.push(this.cast.wgsl);
      r.fns.push(...this.cast.libs);
      yield [Decl.Cast, r];
    }
    for (const t of authored) {
      const r = record();
      r.words.push(t.line);
      r.syms.push(t.text, t.file, t.what);
      yield [Decl.Authored, r];
    }
  }

  probes(scene: Scene): readonly (readonly [string, string])[] {
    return this.things.map((e, k) => {
      const shaders = Object.entries(e.emitted.shaders).map(([key, s]) => ({ key, code: s.code, depthEntry: s.depthEntry, mixedCode: s.mixedCode, libs: s.libs.map((l) => l.name) }));
      const draws = e.emitted.draws.map((d) => {
        const detail = d.detail;
        return detail.kind !== "grid" ? d : { ...d, detail: {
          kind: detail.kind, bound: detail.bound.name, counts: detail.counts, rest: printGraph([detail.rest.point, detail.rest.anchor]),
        } };
      });
      return [`tape.${k}.${kindOf(scene, e)}`, JSON.stringify({ shaders, draws, posts: e.posts, notes: e.notes }) + "\n"];
    });
  }
}

export function record(): PortRecord { return { words: [], lits: [], syms: [], fns: [], kernels: [], blobs: [], roots: [] }; }

function kindOf(scene: Scene, e: Emission): string {
  const { thing } = e;
  if (thing === sky) return "sky";
  if (thing === scene.ground) return scene.ground.kind === "sea" ? "sea" : "ground";
  if (thing === shadows) return "shadows";
  if (thing === light) return "light";
  switch (thing.constructor.name) {
    case "SurfaceDrawable": return "surface";
    case "ReliefDrawable": return "relief";
    case "Strand": return "strand";
    case "Telegraph": return "telegraph";
    case "Smear": return "smear";
    case "FieldDrawable": return "field";
    case "TracedDrawable": return "traced";
    case "MeshDrawable": return "mesh";
    case "Copies": return "copies";
    case "Impostors": return "impostors";
    case "VolumeDrawable": return "volume";
    case "DensityDrawable": return "density";
    case "GlowDrawable": return "glow";
    case "RainDrawable": case "SplashDrawable": return "rain";
    case "Wake": return "wake";
  }
  // Some factories and wrappers return plain objects. The key identifies the module that actually emitted the draw,
  // including the Titan's posed thorns, whose wrapper drops the skeleton but forwards surface.draws unchanged.
  const keys = Object.keys(e.emitted.shaders);
  if (keys.length === 1) {
    switch (keys[0]) {
      case `flash.${thing.name}`: return "lightning";
      case `specks.${thing.name}`: return "specks";
      case `surface.${thing.name}`: return "surface";
      case thing.name: if (thing.record !== undefined) return "shader"; break;
    }
  }
  return thing.constructor.name === "Object" ? "extension" : thing.constructor.name;
}

function putText(r: PortRecord, s: string | undefined): void {
  r.words.push(Number(s !== undefined));
  if (s !== undefined) r.syms.push(s);
}

export function putShader(r: PortRecord, key: string, shader: Shader): void {
  r.syms.push(key, shader.code);
  putText(r, shader.depthEntry);
  putText(r, shader.mixedCode);
  r.words.push(shader.libs.length);
  r.fns.push(...shader.libs);
}

function putPosts(r: PortRecord, posts: readonly PostSpec[]): void {
  r.words.push(posts.length);
  for (const p of posts) {
    r.syms.push(p.label, p.wgsl);
    putText(r, p.mixedWgsl);
  }
}

function putEmitted(r: PortRecord, e: Emission): void {
  const shaders = Object.entries(e.emitted.shaders);
  r.words.push(shaders.length);
  for (const [key, s] of shaders) putShader(r, key, s);
  r.words.push(e.emitted.draws.length);
  for (const d of e.emitted.draws) putDraw(r, d);
  putPosts(r, e.posts);
  r.words.push(e.notes.length);
  r.syms.push(...e.notes);
}

function putDraw(r: PortRecord, d: DrawSpec): void {
  r.syms.push(d.label, d.shader);
  putText(r, d.scenePassFragment);
  r.lits.push(d.vertices, d.instances);
  r.words.push(PHASES.indexOf(d.phase), ["none", "back", "front"].indexOf(d.cull), Number(d.depthWrite),
    ["none", "alpha", "premultiplied", "add", "alpha-min", "alpha-pull"].indexOf(d.blend),
    ["scene", "construction"].indexOf(d.layer), d.sceneDepth === undefined ? 0 : d.sceneDepth ? 2 : 1);
  const constants = Object.entries(d.constants);
  r.words.push(constants.length);
  for (const [k, v] of constants) { r.syms.push(k); r.lits.push(v); }
  if (d.depth === undefined) r.words.push(0);
  else if (d.depth.kind === "vertex") r.words.push(1);
  else { r.words.push(2); r.syms.push(d.depth.entry); }
  switch (d.detail.kind) {
    case "fixed": r.words.push(0); break;
    case "grid":
      r.words.push(1, d.detail.counts.length);
      for (const counts of d.detail.counts) r.lits.push(...counts);
      r.kernels.push(d.detail.bound);
      r.roots.push(d.detail.rest.point, d.detail.rest.anchor);
      break;
    case "projected":
      r.words.push(2);
      r.lits.push(d.detail.cell, d.detail.plane, d.detail.reach, d.detail.lift);
      break;
  }
}

const stages: readonly CompileStage[] = ["compute", "things", "style", "passes", "cast", "layout", "rest", "details"];
const rates = ["build", "frame", "copy", "item"];

function effectRecord(e: Effect): PortRecord {
  const r = record();
  r.words.push(stages.indexOf(e.stage));
  switch (e.kind) {
    case "register": r.words.push(0); r.roots.push(...e.roots); break;
    case "declare": r.words.push(1); r.kernels.push(e.table); break;
    case "lower":
      r.words.push(2, rates.indexOf(e.program.runs));
      r.syms.push(e.program.name);
      for (const [name, rate] of e.program.vars) { r.syms.push(name); r.words.push(rates.indexOf(rate)); }
      r.roots.push(...e.program.roots);
      break;
  }
  return r;
}

function styleRecord(spec: FnSpec): PortRecord {
  const r = record();
  const values = spec.roots.map((_, k) => String(k));
  const pick = values.indexOf(spec.ret(values));
  if (pick < 0) throw new Unrecorded(`style emit ${spec.name} combines its roots rather than picking one`);
  r.words.push(pick, ...spec.args.map(([, ty]) => TYS.indexOf(ty)));
  r.syms.push(spec.name, spec.out, ...spec.args.map(([name]) => name));
  r.roots.push(...spec.roots);
  return r;
}

function lightingRecord(l: Lighting): PortRecord {
  const r = record();
  r.words.push(l.sampling.kind === "point" ? 0 : 1);
  switch (l.sun.kind) {
    case "fixed": r.words.push(0); r.lits.push(...l.sun.dir); break;
    case "input": r.words.push(1); r.syms.push(l.sun.name); r.lits.push(...l.sun.dir); break;
    case "expression": r.words.push(2); r.roots.push(l.sun.direction); break;
  }
  r.words.push(Number(l.amount.kind === "given"));
  if (l.amount.kind === "given") r.roots.push(lift<"vec3">(l.amount.amount));
  const p = variable("port_light_at", "vec3"), xi = variable("port_light_xi", "vec2"), n = variable("port_light_normal", "vec3");
  r.roots.push(p, xi, n);
  r.words.push(l.lights.length);
  for (const light of l.lights) {
    const a = light(p, l.sampling.kind === "sampled" ? xi : undefined), test = l.shadows.get(light);
    r.words.push(Number(a.distance !== undefined), Number(a.size !== undefined), Number(test !== undefined));
    r.roots.push(a.toward, lift<"vec3">(a.amount));
    if (a.distance !== undefined) r.roots.push(lift<"f32">(a.distance));
    if (a.size !== undefined) r.roots.push(lift<"f32">(a.size));
    if (test !== undefined) r.roots.push(test({ kind: "point", at: p }, a), test({ kind: "surface", at: p, normal: n }, a));
  }
  return r;
}

// bakeScene: evaluate a scene at one time into plain triangle meshes, one per surface it draws.
//
// Any `Surface` bakes the same way: its expressions (point, normal, paint) are sampled on the CPU on the grid its
// drawable draws it on (the same spacing, so baked rings sit where drawn ones do), over the tables the pose pass
// fills at that time. What to bake is found through the scene's own structure (`Drawable.forms`, one switch); the surface's
// topology says how the grid closes: periodic edges are seams, poles collapse a row to a point, caps get a flat disc.
// Triangles wind counter-clockwise seen from the side d/du x d/dv points to, which is the side its normal faces.

import { classify } from "../draw/painted.ts";
import { compileScene, type Compiled, type CpuFn } from "../compile/index.ts";
import type { Form } from "../compile/context.ts";
import { f, libsOf, variable, type Expr } from "../expr.ts";
import { fieldMesh, FieldScheduleError } from "../draw/field.ts";
import { fromNets, meshPoint, MESH_PAINT_VARS, type Mesh, type MeshOptions, type MeshPaint } from "../draw/mesh.ts";
import type { Poses } from "../map/types.ts";
import type { End, Topology } from "../map/types.ts";
import type { Scene } from "../scene.ts";

export type Density = { readonly along: number; readonly around: number };
export type BakeOptions = { readonly time: number; readonly density: Density };

export type BodyMesh = {
  readonly name: string;
  /** A closed solid (every edge shared by two triangles), or a sheet (two-sided in the file's material). */
  readonly closed: boolean;
  readonly doubleSided: boolean;
  readonly positions: Float32Array; // xyz
  readonly normals: Float32Array; // xyz, unit
  readonly uvs: Float32Array; // the surface's own (u, v): body coordinates (m, v) for a body
  readonly paint: Colours;
  readonly indices: Uint32Array; // counter-clockwise seen from the normal side
};
/**
 * A body's vertex colours, linear rgb in [0, 1] (glTF COLOR_0 is linear); or none, and why: a paint that reads a screen
 * derivative (`fwidth`, an ink edge a pixel wide) has no value off the screen, so the file carries no COLOR_0 for it.
 */
export type Colours = { readonly kind: "rgb"; readonly rgb: Float32Array } | { readonly kind: "none"; readonly why: string };
export type Skipped = { readonly name: string; readonly reason: string };
export type Baked = { readonly time: number; readonly bodies: readonly BodyMesh[]; readonly skipped: readonly Skipped[] };

export const vertexCount = (b: BodyMesh): number => b.positions.length / 3;
export const triangleCount = (b: BodyMesh): number => b.indices.length / 3;

/** sRGB (display-referred, as Loft authors colour) to linear, per channel (IEC 61966-2-1). */
export const toLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

type Vertex = { readonly pos: readonly number[]; readonly normal: readonly number[]; readonly uv: readonly [number, number]; readonly colour: readonly number[] | undefined };

/** Why a paint cannot be evaluated on the CPU, or undefined when it can. */
function unpaintable(base: Expr): string | undefined {
  const screen = libsOf([base]).find((l) => l.js === "gpu-only");
  return screen === undefined ? undefined : `its paint reads ${screen.name}, which has no value off the screen`;
}

class Builder {
  private readonly pos: number[] = [];
  private readonly nrm: number[] = [];
  private readonly uv: number[] = [];
  private readonly col: number[] = [];
  private readonly idx: number[] = [];
  constructor(private readonly name: string, private readonly unpainted: string | undefined) {}

  /** Vertices whose surface has no normal at that point (a degenerate point: a hull's stern closing to a line). */
  private readonly unnormal: number[] = [];

  vertex(v: Vertex): number {
    const all = [...v.pos, ...v.uv, ...(v.colour ?? [])];
    // A NaN here would reach the file and fail in someone else's viewer; fail where the surface is known instead.
    if (all.some((x) => !Number.isFinite(x))) throw new Error(`bake: surface "${this.name}" evaluated to a non-finite vertex (${[...v.pos, ...v.normal, ...v.uv].join(", ")})`);
    const n = this.pos.length / 3;
    if (!v.normal.every(Number.isFinite)) this.unnormal.push(n);
    this.pos.push(...v.pos);
    this.nrm.push(...v.normal);
    this.uv.push(...v.uv);
    if (v.colour !== undefined) this.col.push(...v.colour.map((c) => toLinear(Math.min(1, Math.max(0, c)))));
    return n;
  }
  triangle(a: number, b: number, c: number): void { this.idx.push(a, b, c); }

  /**
   * A point where the surface's normal is undefined (its map degenerates: d/du x d/dv is zero there, and a finite
   * difference divides 0 by 0) takes the limit of the normals round it: the area-weighted sum of the triangles that
   * touch that point, over every vertex at the same position (the pole's duplicates each touch one triangle).
   */
  private fillNormals(): void {
    if (this.unnormal.length === 0) return;
    const key = (i: number) => `${this.pos[3 * i]},${this.pos[3 * i + 1]},${this.pos[3 * i + 2]}`;
    const sums = new Map(this.unnormal.map((i): [string, number[]] => [key(i), [0, 0, 0]]));
    for (let t = 0; t < this.idx.length; t += 3) {
      const [a, b, c] = [this.idx[t]!, this.idx[t + 1]!, this.idx[t + 2]!];
      const P = (i: number, k: number) => this.pos[3 * i + k]!;
      const u = [0, 1, 2].map((k) => P(b, k) - P(a, k)), w = [0, 1, 2].map((k) => P(c, k) - P(a, k));
      const face = [u[1]! * w[2]! - u[2]! * w[1]!, u[2]! * w[0]! - u[0]! * w[2]!, u[0]! * w[1]! - u[1]! * w[0]!];
      for (const i of [a, b, c]) { const s = sums.get(key(i)); if (s) for (let k = 0; k < 3; k++) s[k]! += face[k]!; }
    }
    for (const i of this.unnormal) {
      const s = sums.get(key(i))!, l = Math.hypot(...s);
      if (!(l > 0)) throw new Error(`bake: surface "${this.name}" has no normal at vertex ${i} and no triangle of area round it`);
      for (let k = 0; k < 3; k++) this.nrm[3 * i + k] = s[k]! / l;
    }
  }

  finish(closed: boolean, doubleSided: boolean): BodyMesh {
    this.fillNormals();
    return {
      name: this.name, closed, doubleSided,
      positions: Float32Array.from(this.pos), normals: Float32Array.from(this.nrm), uvs: Float32Array.from(this.uv),
      paint: this.unpainted === undefined ? { kind: "rgb", rgb: Float32Array.from(this.col) } : { kind: "none", why: this.unpainted },
      indices: Uint32Array.from(this.idx),
    };
  }
}

const isClosed = (t: Topology): boolean => t.v === "periodic" && (t.u === "periodic" || t.u.every((e) => e !== "free"));
const endsOf = (t: Topology): readonly [End, End] => (t.u === "periodic" ? ["free", "free"] : t.u);

const U = variable("u", "f32"), W = variable("v", "f32");

type Frame = Parameters<CpuFn>[0];
type Cpu = (roots: readonly Expr[]) => CpuFn;

/**
 * One baked body, compiled once and evaluated at any pose: `mesh` is the whole body (normals, paint) at a frame;
 * `positions` its points alone, vertex for vertex the same order (what the animated export samples at every key).
 * `form` and `copy` say where it came from.
 */
export type Sampler = {
  readonly name: string;
  readonly form: Form;
  readonly copy: number;
  mesh(frame: Frame): BodyMesh;
  positions(frame: Frame): Float32Array;
};

/** One surface: a (rows + 1) x (cols + 1) grid of samples, its pole rows collapsed and its cap ends closed. */
function surfaceSampler(s: Extract<Form, { kind: "surface" }>, cpu: Cpu, d: Density): Sampler {
  const p = s.surface.point(U, W), n = s.surface.normal(U, W);
  // glTF takes the colour; channels and a thing's own answers are a style's to print.
  const base = classify(s.name, s.paint(U, W, { world: p, normal: n, copy: f(s.copy) })).base;
  const unpainted = unpaintable(base);
  const run = cpu(unpainted === undefined ? [p, n, base] : [p, n]), runPoint = cpu([p]);
  const uAt = cpu([s.grid.u.at(U)]), vAt = cpu([s.grid.v.at(W)]);
  const rows = d.along, cols = d.around;
  const build = (frame: Frame, sample: (u: number, v: number) => Vertex, unpainted: string | undefined): BodyMesh => {
    const us = Array.from({ length: rows + 1 }, (_, i) => uAt(frame, { u: i / rows })[0] as number);
    const vs = Array.from({ length: cols + 1 }, (_, j) => vAt(frame, { v: j / cols })[0] as number);
    const b = new Builder(s.name, unpainted);
    const at: number[][] = us.map((u) => vs.map((v) => b.vertex(sample(u, v))));
    const [first, last] = endsOf(s.surface.topology);
    for (let i = 0; i < rows; i++) {
      for (let j = 0; j < cols; j++) {
        const a = at[i][j], bb = at[i + 1][j], c = at[i + 1][j + 1], e = at[i][j + 1];
        // A pole at u = 0 makes a = e, so (a, c, e) is the degenerate one; at u = 1, bb = c, so (a, bb, c) is.
        if (!(last === "pole" && i === rows - 1)) b.triangle(a, bb, c);
        if (!(first === "pole" && i === 0)) b.triangle(a, c, e);
      }
    }
    const cap = (end: 0 | 1): void => {
      const u = us[end === 0 ? 0 : rows];
      // The ring without its seam duplicate when v is periodic.
      const ring = vs.slice(0, s.surface.topology.v === "periodic" ? cols : cols + 1).map((v) => sample(u, v));
      const centre = [0, 1, 2].map((k) => ring.reduce((t, r) => t + r.pos[k], 0) / ring.length);
      // Newell's normal of the ring, turned to face out of the solid: along +d/du at u = 1, -d/du at u = 0.
      const nw = [0, 0, 0];
      ring.forEach((p, j) => {
        const q = ring[(j + 1) % ring.length];
        nw[0] += (p.pos[1] - q.pos[1]) * (p.pos[2] + q.pos[2]);
        nw[1] += (p.pos[2] - q.pos[2]) * (p.pos[0] + q.pos[0]);
        nw[2] += (p.pos[0] - q.pos[0]) * (p.pos[1] + q.pos[1]);
      });
      const inner = sample(us[end === 0 ? 1 : rows - 1], vs[0]).pos;
      const outward = [0, 1, 2].map((k) => ring[0].pos[k] - inner[k]);
      const flip = nw[0] * outward[0] + nw[1] * outward[1] + nw[2] * outward[2] < 0;
      // A ring that has closed to a point (a cap whose girth reaches 0 there: the crab's "atoll") has no Newell normal;
      // its disc has no area, so its normal is the way out of the solid. The vertices stay, so the topology is the
      // same at every time (the animated export keys one vertex list).
      const l = Math.hypot(...nw), lo = Math.hypot(...outward);
      const normal = l > 1e-12 ? nw.map((x) => x / (flip ? -l : l)) : lo > 0 ? outward.map((x) => x / lo) : [0, 1, 0];
      const rim = ring.map((r) => b.vertex({ ...r, normal }));
      const mid = b.vertex({ pos: centre, normal, uv: [u, 0], colour: ring[0].colour });
      rim.forEach((p, j) => {
        const q = rim[(j + 1) % rim.length];
        // Newell's normal follows the ring's order: (mid, p, q) faces it, so flipping the normal flips the fan.
        if (flip) b.triangle(mid, q, p); else b.triangle(mid, p, q);
      });
    };
    if (first === "cap") cap(0);
    if (last === "cap") cap(1);
    return b.finish(isClosed(s.surface.topology), s.sides === "two");
  };
  return {
    name: s.name, form: s, copy: 0,
    mesh: (frame) => build(frame, (u, v) => {
      const [pos, normal, colour] = run(frame, { u, v }) as number[][];
      return { pos, normal, uv: [u, v], colour };
    }, unpainted),
    // The same walk with the point alone: a cap's normal is still found from its ring, so it is never degenerate.
    positions: (frame) => build(frame, (u, v) => ({ pos: runPoint(frame, { u, v })[0] as number[], normal: [0, 0, 1], uv: [u, v], colour: undefined }), "positions only").positions,
  };
}

/**
 * A mesh form: each point moved by `at` (and its copy's pose) at the bake's time, with its carried normal and its paint
 * evaluated there. A mesh whose normals are its faces' (a count, or an `at` that does not read the rest position) is
 * written unshared, one normal per triangle, as it draws. One sampler per copy.
 */
function meshSamplers(form: Form, name: string, mesh: Mesh, at: MeshOptions["at"], copies: Poses | undefined, paint: MeshPaint, cpu: Cpu): Sampler[] {
  const placed = meshPoint(mesh, at, copies);
  const move = cpu([placed.world, placed.normal]);
  const base = classify(name, paint(MESH_PAINT_VARS.row(mesh.width), MESH_PAINT_VARS.at())).base;
  const unpainted = unpaintable(base);
  const colour = unpainted === undefined ? cpu([base]) : undefined;
  const lanes = (vars: Record<string, number | number[]>) => Object.fromEntries(Object.entries(vars).filter(([k]) => /^mesh_r\d+$/.test(k)));
  const one = (copy: number, label: string): Sampler => {
    const points = (frame: Frame) => Array.from({ length: mesh.count }, (_, i) => {
      const vars: Record<string, number | number[]> = { ...placed.rowVars(i), mesh_i: i, mesh_copy: copy };
      const [pos, n] = move(frame, vars) as number[][];
      return { pos: pos!, n: n!, vars };
    });
    const build = (frame: Frame): BodyMesh => {
      const b = new Builder(label, unpainted);
      const pts = points(frame);
      const paintAt = (k: number, normal: readonly number[]) => colour?.(frame, { ...lanes(pts[k]!.vars), p_world: pts[k]!.pos, p_normal: [...normal], p_copy: copy })[0] as number[] | undefined;
      const T = mesh.triangles;
      switch (mesh.normals.kind) {
        case "carried": {
          const ids = pts.map((q, k) => { const l = Math.hypot(...q.n) || 1; const normal = q.n.map((x) => x / l); return b.vertex({ pos: q.pos, normal, uv: [0, 0], colour: paintAt(k, normal) }); });
          for (let t = 0; t < T.length; t += 3) b.triangle(ids[T[t]!]!, ids[T[t + 1]!]!, ids[T[t + 2]!]!);
          break;
        }
        case "faces":
          for (let t = 0; t < T.length; t += 3) {
            const [a, c, d] = [T[t]!, T[t + 1]!, T[t + 2]!].map((k) => pts[k]!.pos) as [number[], number[], number[]];
            const u = [c[0]! - a[0]!, c[1]! - a[1]!, c[2]! - a[2]!], v = [d[0]! - a[0]!, d[1]! - a[1]!, d[2]! - a[2]!];
            const nn = [u[1]! * v[2]! - u[2]! * v[1]!, u[2]! * v[0]! - u[0]! * v[2]!, u[0]! * v[1]! - u[1]! * v[0]!];
            const l = Math.hypot(...nn) || 1, normal = nn.map((x) => x / l);
            const ids = [T[t]!, T[t + 1]!, T[t + 2]!].map((k) => b.vertex({ pos: pts[k]!.pos, normal, uv: [0, 0], colour: paintAt(k, normal) }));
            b.triangle(ids[0]!, ids[1]!, ids[2]!);
          }
          break;
      }
      return b.finish(false, false);
    };
    // Vertex order as `build` writes it: the points themselves (carried), or three per triangle (faces).
    const positions = (frame: Frame): Float32Array => {
      const pts = points(frame);
      switch (mesh.normals.kind) {
        case "carried": return Float32Array.from(pts.flatMap((q) => q.pos));
        case "faces": return Float32Array.from(Array.from(mesh.triangles).flatMap((k) => pts[k]!.pos));
      }
    };
    return { name: label, form, copy, mesh: build, positions };
  };
  return copies === undefined ? [one(0, name)] : Array.from({ length: copies.count }, (_, i) => one(i, `${name}_${i}`));
}

/** What the baker makes of each form: samplers, or the reason it cannot (reported by name, never dropped silently). */
function samplersOf(form: Form, cpu: Cpu, d: Density): Sampler[] | string {
  switch (form.kind) {
    case "surface": return [surfaceSampler(form, cpu, d)];
    case "mesh": return meshSamplers(form, form.name, form.mesh, form.at, form.copies, form.paint, cpu);
    case "field": {
      // Meshed by surface nets at full detail, as the `mesh` schedule draws it; a field that changes cannot be sampled once.
      const nets = (() => { try { return fieldMesh(form.name, form.field, form.bound); } catch (e) { if (e instanceof FieldScheduleError) return e.message; throw e; } })();
      if (typeof nets === "string") return nets;
      return meshSamplers(form, form.name, fromNets(nets), undefined, undefined, (_row, at) => classify(form.name, form.paint({ p: at.world, normal: at.normal })).base, cpu)
        .map((s) => ({ ...s, mesh: (frame: Frame) => ({ ...s.mesh(frame), closed: true }) }));
    }
    case "medium": return "a medium has no surface to bake";
    case "hit": return "a traced solid has no surface to bake (only its hit function)";
  }
}

/** A scene compiled for baking: its samplers, what it skipped, and the pose pass's frame at any time. */
export type Prepared = {
  readonly compiled: Compiled;
  readonly samplers: readonly Sampler[];
  readonly skipped: readonly Skipped[];
  frameAt(time: number): Frame;
};

export function prepareBake(scene: Scene, density: Density): Prepared {
  if (!(Number.isInteger(density.along) && density.along >= 2)) throw new Error(`bake: density.along must be an integer >= 2, got ${density.along}`);
  if (!(Number.isInteger(density.around) && density.around >= 3)) throw new Error(`bake: density.around must be an integer >= 3, got ${density.around}`);
  const c = compileScene(scene);
  const a = c.artefact;
  const params = Float32Array.from(a.params.map((p) => p.def));
  // The frame state the host would build: time, and every other input and parameter at its default.
  const frameAt = (time: number): Frame => {
    if (!Number.isFinite(time)) throw new Error(`bake: time must be a finite number, got ${time}`);
    const inputs = new Float32Array(Math.max(1, a.inputFloats));
    for (const s of a.inputs) inputs.set(s.name === "time" ? [time] : s.def, s.slot);
    return c.frame({ time, inputs, params, curves: a.curves });
  };
  const samplers: Sampler[] = [];
  const skipped: Skipped[] = [];
  for (const t of scene.things) {
    if (t.forms.length === 0) skipped.push({ name: t.name, reason: "draws no form a reader can use" });
    for (const form of t.forms) {
      const made = samplersOf(form, c.cpu, density);
      if (typeof made === "string") skipped.push({ name: form.name, reason: made }); else samplers.push(...made);
    }
  }
  return { compiled: c, samplers, skipped, frameAt };
}

export function bakeScene(scene: Scene, options: BakeOptions): Baked {
  const p = prepareBake(scene, options.density);
  const frame = p.frameAt(options.time);
  return { time: options.time, bodies: p.samplers.map((s) => s.mesh(frame)), skipped: p.skipped };
}

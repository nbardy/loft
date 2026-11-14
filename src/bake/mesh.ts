// bakeScene: evaluate a scene at one time into plain triangle meshes, one per surface it draws.
//
// Any `Surface` bakes the same way: its expressions (point, normal, paint) are sampled on the CPU on the grid its
// drawable draws it on (the same spacing, so baked rings sit where drawn ones do), over the tables the pose pass
// fills at that time. What to bake is found through the scene's own structure (`Drawable.forms`, one switch); the surface's
// topology says how the grid closes: periodic edges are seams, poles collapse a row to a point, caps get a flat disc.
// Triangles wind counter-clockwise seen from the side d/du x d/dv points to, which is the side its normal faces.

import { compileScene, type CpuFn } from "../compile/index.ts";
import type { Form } from "../compile/context.ts";
import { f, variable, type Expr } from "../expr.ts";
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
  readonly colors: Float32Array; // linear rgb in [0, 1] (glTF COLOR_0 is linear)
  readonly indices: Uint32Array; // counter-clockwise seen from the normal side
};
export type Skipped = { readonly name: string; readonly reason: string };
export type Baked = { readonly time: number; readonly bodies: readonly BodyMesh[]; readonly skipped: readonly Skipped[] };

export const vertexCount = (b: BodyMesh): number => b.positions.length / 3;
export const triangleCount = (b: BodyMesh): number => b.indices.length / 3;

/** sRGB (display-referred, as Loft authors colour) to linear, per channel (IEC 61966-2-1). */
export const toLinear = (c: number): number => (c <= 0.04045 ? c / 12.92 : ((c + 0.055) / 1.055) ** 2.4);

type Vertex = { readonly pos: readonly number[]; readonly normal: readonly number[]; readonly uv: readonly [number, number]; readonly colour: readonly number[] };

class Builder {
  private readonly pos: number[] = [];
  private readonly nrm: number[] = [];
  private readonly uv: number[] = [];
  private readonly col: number[] = [];
  private readonly idx: number[] = [];
  constructor(private readonly name: string) {}

  vertex(v: Vertex): number {
    const all = [...v.pos, ...v.normal, ...v.uv, ...v.colour];
    // A NaN here would reach the file and fail in someone else's viewer; fail where the surface is known instead.
    if (all.some((x) => !Number.isFinite(x))) throw new Error(`bake: surface "${this.name}" evaluated to a non-finite vertex (${all.join(", ")})`);
    const n = this.pos.length / 3;
    this.pos.push(...v.pos);
    this.nrm.push(...v.normal);
    this.uv.push(...v.uv);
    this.col.push(...v.colour.map((c) => toLinear(Math.min(1, Math.max(0, c)))));
    return n;
  }
  triangle(a: number, b: number, c: number): void { this.idx.push(a, b, c); }

  finish(closed: boolean, doubleSided: boolean): BodyMesh {
    return {
      name: this.name, closed, doubleSided,
      positions: Float32Array.from(this.pos), normals: Float32Array.from(this.nrm), uvs: Float32Array.from(this.uv),
      colors: Float32Array.from(this.col), indices: Uint32Array.from(this.idx),
    };
  }
}

const isClosed = (t: Topology): boolean => t.v === "periodic" && (t.u === "periodic" || t.u.every((e) => e !== "free"));
const endsOf = (t: Topology): readonly [End, End] => (t.u === "periodic" ? ["free", "free"] : t.u);

const U = variable("u", "f32"), W = variable("v", "f32");

/** One surface: a (rows + 1) x (cols + 1) grid of samples, its pole rows collapsed and its cap ends closed. */
function bakeSurface(s: Extract<Form, { kind: "surface" }>, cpu: (roots: readonly Expr[]) => CpuFn, frame: Parameters<CpuFn>[0], d: Density): BodyMesh {
  const p = s.surface.point(U, W), n = s.surface.normal(U, W);
  const run = cpu([p, n, s.paint(U, W, { world: p, normal: n, copy: f(s.copy) })]);
  const uAt = cpu([s.grid.u.at(U)]), vAt = cpu([s.grid.v.at(W)]);
  const rows = d.along, cols = d.around;
  const us = Array.from({ length: rows + 1 }, (_, i) => uAt(frame, { u: i / rows })[0] as number);
  const vs = Array.from({ length: cols + 1 }, (_, j) => vAt(frame, { v: j / cols })[0] as number);
  const b = new Builder(s.name);
  const sample = (u: number, v: number): Vertex => {
    const [pos, normal, colour] = run(frame, { u, v }) as number[][];
    return { pos, normal, uv: [u, v], colour };
  };
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
    const l = Math.hypot(...nw) * (flip ? -1 : 1);
    const normal = nw.map((x) => x / l);
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
}

/**
 * A mesh form: each point moved by `at` (and its copy's pose) at the bake's time, with its carried normal and its paint
 * evaluated there. A mesh whose normals are its faces' (a count, or an `at` that does not read the rest position) is
 * written unshared, one normal per triangle, as it draws.
 */
function bakeMesh(name: string, mesh: Mesh, at: MeshOptions["at"], copies: Poses | undefined, paint: MeshPaint, cpu: (roots: readonly Expr[]) => CpuFn, frame: Parameters<CpuFn>[0]): BodyMesh[] {
  const placed = meshPoint(mesh, at, copies);
  const move = cpu([placed.world, placed.normal]);
  const colour = cpu([paint(MESH_PAINT_VARS.row(mesh.width), MESH_PAINT_VARS.at())]);
  const lanes = (vars: Record<string, number | number[]>) => Object.fromEntries(Object.entries(vars).filter(([k]) => /^mesh_r\d+$/.test(k)));
  const one = (copy: number, label: string): BodyMesh => {
    const b = new Builder(label);
    const pts = Array.from({ length: mesh.count }, (_, i) => {
      const vars: Record<string, number | number[]> = { ...placed.rowVars(i), mesh_i: i, mesh_copy: copy };
      const [pos, n] = move(frame, vars) as number[][];
      return { pos: pos!, n: n!, vars };
    });
    const paintAt = (k: number, normal: readonly number[]) => colour(frame, { ...lanes(pts[k]!.vars), p_world: pts[k]!.pos, p_normal: [...normal], p_copy: copy })[0] as number[];
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
  return copies === undefined ? [one(0, name)] : Array.from({ length: copies.count }, (_, i) => one(i, `${name}_${i}`));
}

/** What the baker makes of each form: triangles, or the reason it cannot (reported by name, never dropped silently). */
function bakeForm(form: Form, cpu: (roots: readonly Expr[]) => CpuFn, frame: Parameters<CpuFn>[0], d: Density): BodyMesh[] | string {
  switch (form.kind) {
    case "surface": return [bakeSurface(form, cpu, frame, d)];
    case "mesh": return bakeMesh(form.name, form.mesh, form.at, form.copies, form.paint, cpu, frame);
    case "field": {
      // Meshed by surface nets at full detail, as the `mesh` schedule draws it; a field that changes cannot be sampled once.
      const nets = (() => { try { return fieldMesh(form.name, form.field, form.bound); } catch (e) { if (e instanceof FieldScheduleError) return e.message; throw e; } })();
      if (typeof nets === "string") return nets;
      return bakeMesh(form.name, fromNets(nets), undefined, undefined, (_row, at) => form.paint({ p: at.world, normal: at.normal }), cpu, frame).map((m) => ({ ...m, closed: true }));
    }
    case "medium": return "a medium has no surface to bake";
    case "hit": return "a traced solid has no surface to bake (only its hit function)";
  }
}

export function bakeScene(scene: Scene, options: BakeOptions): Baked {
  const { time, density } = options;
  if (!Number.isFinite(time)) throw new Error(`bake: time must be a finite number, got ${time}`);
  if (!(Number.isInteger(density.along) && density.along >= 2)) throw new Error(`bake: density.along must be an integer >= 2, got ${density.along}`);
  if (!(Number.isInteger(density.around) && density.around >= 3)) throw new Error(`bake: density.around must be an integer >= 3, got ${density.around}`);
  const c = compileScene(scene);
  const a = c.artefact;
  // The frame state the host would build: time, and every other input and parameter at its default.
  const inputs = new Float32Array(Math.max(1, a.inputFloats));
  for (const s of a.inputs) inputs.set(s.name === "time" ? [time] : s.def, s.slot);
  const frame = c.frame({ time, inputs, params: Float32Array.from(a.params.map((p) => p.def)), curves: a.curves });
  const bodies: BodyMesh[] = [];
  const skipped: Skipped[] = [];
  for (const t of scene.things) {
    if (t.forms.length === 0) skipped.push({ name: t.name, reason: "draws no form a reader can use" });
    for (const form of t.forms) {
      const made = bakeForm(form, c.cpu, frame, density);
      if (typeof made === "string") skipped.push({ name: form.name, reason: made }); else bodies.push(...made);
    }
  }
  return { time, bodies, skipped };
}

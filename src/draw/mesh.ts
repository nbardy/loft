// draw.mesh: triangles made by the author's own code (a hull, a shatter, a subdivision, a growth), drawn and lit like
// any body (docs/proposals/geometry_and_rays.md section 4.1).
//
// The discrete and the continuous are split. The algorithm that decides topology runs once, at build, in plain
// TypeScript, and hands over rows of numbers (one per point) and triangles. The motion that must answer for every frame
// is `at`, an expression of a point's row, its index and `time`, evaluated in the vertex stage, so any frame still draws
// alone. Drawn by vertex pulling from two static tables (rows, then four triangle corners per vec4); no vertex buffer.
//
// Normals are not an option. The mesh is classified once (`Normals`):
//  - carried: the rows carry a rest position (row[0..2]) and `at`, if given, reads it. At build each point's rest
//    normal is the area-weighted sum of the normals of the triangles that share its index, and its welded normal the
//    same sum over the triangles that share its position (so a hull outline stays closed across the hard edges of an
//    unshared mesh). Both are carried through `at` by the cofactor of its Jacobian with respect to the rest position:
//    n' = n.x (dat/dy x dat/dz) + n.y (dat/dz x dat/dx) + n.z (dat/dx x dat/dy). DERIVATION: tangents map by J, so
//    normals map by cof(J) = det(J) J^-T; J = I gives n and a rotation R gives R n. The Jacobian is symbolic (grad.ts).
//  - faces: `points` is a count, or `at` does not read the rest position: the moved triangle's own face, per pixel,
//    from the screen derivatives of the world point, turned to the eye. Exact and flat for any motion.
//
// Where nothing moves the mesh (no `at`, one copy) the rest normals are used as stored, unnormalised in the vertex
// stage, so a field's surface-nets mesh (src/draw/field.ts, the `mesh` schedule) draws as it did before it moved here.

import { add, cross, f, leaves, libsOf, mul, swizzle, variable, vec3, compileJs, type Expr, scopedName } from "../expr.ts";
import { jacobian } from "../grad.ts";
import { emitShaderFn } from "../compile/bind.ts";
import { lit } from "../compile/scene_wgsl.ts";
import { drawSpec, type CompileContext, type Drawable, type DrawSpec, type Emitted, type Form, type TableRef } from "../compile/context.ts";
import type { TableSource } from "../expr.ts";
import { posesOf, rotate } from "../map/chart.ts";
import type { F, P3, PaintAt, Points, Posed, Poses, Skeleton } from "../map/types.ts";
import { SceneError } from "../errors.ts";
import { nameId, posedJoint } from "./surface.ts";
import { fitBlob, shadowSkeleton } from "./field_shadow.ts";
import { traceShadeWgsl } from "./trace.ts";
import type { TriangleMesh } from "../map/nets.ts";

/** Triangles made by the author's code. */
export type MeshData = {
  /**
   * One row of numbers per point, every row the same width (3 to 16): the first three are its rest position unless
   * `at` says otherwise, the rest are the author's own (a body coordinate, a piece's index, a birth time). Or a count,
   * when `at` makes each point from its index alone.
   */
  readonly points: readonly (readonly number[])[] | number;
  /** Three indices into `points` per triangle, counter-clockwise seen from the front (the outside of a solid). */
  readonly triangles: readonly (readonly [number, number, number])[];
};

export type MeshOptions = {
  /** Where point i is now: an expression of its row (one F per number), its index and `time`. Default: row[0..2]. */
  readonly at?: (row: readonly F[], i: F) => P3;
  /**
   * Base colour, lit by the style. `row` is interpolated across the triangle, as (u, v) is on a surface; numbers equal
   * at a triangle's three corners stay exact. `at.copy` is the copy's index under `draw.posed`.
   */
  readonly paint: (row: readonly F[], at: PaintAt) => P3;
  readonly sides: "one" | "two";
  /** `hull`: the triangles pushed out along the welded normal and drawn back faces only, in ink. */
  readonly outline: "hull" | "none";
  /** What casts shadows. Absent: one blob fitted to the rest points (it does not follow `at`). */
  readonly skeletons?: readonly Skeleton[];
};

/** How a mesh's normals are found, classified once from its points and `at` (see the header). */
export type Normals =
  /** Rows carry a rest position: rest normals at build (xyz per point), carried through `at` in the vertex stage. */
  | { readonly kind: "carried"; readonly rest: Float32Array; readonly welded: Float32Array }
  /** `points` is a count, or `at` does not read row[0..2]: the moved triangle's own face, per pixel. */
  | { readonly kind: "faces" };

/** A mesh as drawn: its rows (count x width numbers, width 0 for a count), its triangles and its normals. */
export type Mesh = {
  readonly count: number;
  readonly width: number;
  readonly rows: Float32Array;
  readonly triangles: Uint32Array;
  readonly normals: Normals;
};

/** Where the points are each frame: at rest (the rows' first three numbers, nothing moves), or moved by `at` and poses. */
type Motion = { readonly kind: "rest" } | { readonly kind: "moved"; readonly at: (row: readonly F[], i: F) => P3 };

/** One copy, or one per pose. */
type Copies = { readonly kind: "one" } | { readonly kind: "poses"; readonly poses: Poses };

/** The fragment's paint as the drawable states it (the internal field mesh binds the pixel size; see field.ts). */
export type MeshPaint = (row: readonly F[], at: PaintAt) => P3;

/** Exactly representable as f32 indices. */
const MAX_POINTS = 2 ** 24;
const MAX_WIDTH = 16;

// The variables a mesh's expressions are written over, made on first use: an expression made when this module loads
// would take an expression id and renumber every later `let` in every scene's WGSL (the same program in other bytes).
let made: { REST: Expr<"vec3">; LANES: Expr<"vec4">[]; N0: Expr<"vec3">; W0: Expr<"vec3">; I: F; COPY: F; AT: PaintAt } | undefined;
const vars = () => made ??= {
  REST: variable("mesh_rest", "vec3"), LANES: Array.from({ length: 4 }, (_, k) => variable(`mesh_r${k}`, "vec4")),
  N0: variable("mesh_n0", "vec3"), W0: variable("mesh_w0", "vec3"), I: variable("mesh_i", "f32"), COPY: variable("mesh_copy", "f32"),
  AT: { world: variable("p_world", "vec3"), normal: variable("p_normal", "vec3"), copy: variable("p_copy", "f32") },
};

const lanesOf = (width: number): number => Math.ceil(width / 4);

/** A row as expressions: the rest position's three numbers from REST, the others from the lanes. */
function rowOf(width: number): F[] {
  const { REST, LANES } = vars();
  return Array.from({ length: width }, (_, j) => (j < 3 ? swizzle<"f32">(REST, "xyz"[j]!) : swizzle<"f32">(LANES[j >> 2]!, "xyzw"[j & 3]!)));
}
/** The row as the fragment stage sees it (interpolated lanes, rest position included). */
function paintRowOf(width: number): F[] {
  const { LANES } = vars();
  return Array.from({ length: width }, (_, j) => swizzle<"f32">(LANES[j >> 2]!, "xyzw"[j & 3]!));
}

const where = (name: string) => `draw.mesh "${name}"`;

/** Area-weighted normals at each point, summed over the triangles that share its index (or its position). */
function restNormals(rows: Float32Array, width: number, count: number, tris: Uint32Array, key: (i: number) => number): Float32Array {
  const sum = new Float64Array(count * 3);
  const p = (i: number, a: number) => rows[i * width + a]!;
  for (let t = 0; t < tris.length; t += 3) {
    const a = tris[t]!, b = tris[t + 1]!, c = tris[t + 2]!;
    const u = [p(b, 0) - p(a, 0), p(b, 1) - p(a, 1), p(b, 2) - p(a, 2)], v = [p(c, 0) - p(a, 0), p(c, 1) - p(a, 1), p(c, 2) - p(a, 2)];
    const n = [u[1]! * v[2]! - u[2]! * v[1]!, u[2]! * v[0]! - u[0]! * v[2]!, u[0]! * v[1]! - u[1]! * v[0]!];
    for (const k of [a, b, c]) for (let x = 0; x < 3; x++) sum[key(k) * 3 + x] += n[x]!;
  }
  const out = new Float32Array(count * 3);
  for (let i = 0; i < count; i++) {
    const k = key(i), l = Math.hypot(sum[k * 3]!, sum[k * 3 + 1]!, sum[k * 3 + 2]!);
    // A point in no triangle (or whose faces cancel) has no normal; it is never drawn, so 0 is never read.
    if (l > 0) for (let x = 0; x < 3; x++) out[i * 3 + x] = sum[k * 3 + x]! / l;
  }
  return out;
}

/**
 * The author's mesh classified once (the boundary): numbers checked, indices in range, normals by variant. Throws a
 * `SceneError` naming what is wrong and the fix.
 */
export function classifyMesh(name: string, data: MeshData, at: MeshOptions["at"]): { readonly mesh: Mesh; readonly motion: Motion } {
  const w = where(name);
  const motion: Motion = at === undefined ? { kind: "rest" } : { kind: "moved", at };
  const pts = data.points;
  let count: number, width: number, rows: Float32Array;
  if (typeof pts === "number") {
    if (!(Number.isInteger(pts) && pts >= 1 && pts < MAX_POINTS)) throw new SceneError(w, `points ${pts}`, `a whole count from 1 to ${MAX_POINTS - 1}, or rows of numbers`, "points: rows.length");
    if (at === undefined) throw new SceneError(w, "points as a count with no `at`", "an `at` that makes each point from its index", "at: (row, i) => vec3(...)");
    count = pts; width = 0; rows = new Float32Array(0);
  } else {
    count = pts.length;
    if (count < 3 || count >= MAX_POINTS) throw new SceneError(w, `${count} points`, `3 to ${MAX_POINTS - 1} points`, "check the algorithm that made them");
    width = pts[0]!.length;
    if (!(width >= 3 && width <= MAX_WIDTH)) throw new SceneError(w, `rows of ${width} numbers`, `3 to ${MAX_WIDTH} numbers per row (x, y, z first)`, "move what does not fit into a table that `at` fetches");
    rows = new Float32Array(count * width);
    pts.forEach((r, i) => {
      if (r.length !== width) throw new SceneError(w, `row ${i} has ${r.length} numbers`, `every row ${width} numbers, as row 0`, "pad the short rows");
      r.forEach((x, j) => {
        if (!Number.isFinite(x)) throw new SceneError(w, `row ${i} number ${j} is ${x}`, "finite numbers", "check the algorithm that made the row");
        rows[i * width + j] = x;
      });
    });
  }
  if (data.triangles.length === 0) throw new SceneError(w, "no triangles", "at least one triangle", "check the algorithm that made them");
  const triangles = new Uint32Array(data.triangles.length * 3);
  data.triangles.forEach((t, k) => t.forEach((i, c) => {
    if (!(Number.isInteger(i) && i >= 0 && i < count)) throw new SceneError(w, `triangle ${k} corner ${c} is ${i}`, `an index from 0 to ${count - 1}`, "check the indices the algorithm made");
    triangles[k * 3 + c] = i;
  }));
  return { mesh: { count, width, rows, triangles, normals: normalsOf(rows, width, count, triangles, motion) }, motion };
}

/** The variant: carried when the rows hold a rest position that `at` (if any) reads; faces otherwise. */
function normalsOf(rows: Float32Array, width: number, count: number, tris: Uint32Array, motion: Motion): Normals {
  if (width === 0) return { kind: "faces" };
  if (motion.kind === "moved" && !leaves([motion.at(rowOf(width), vars().I)]).vars.has("mesh_rest")) return { kind: "faces" };
  const byPosition = new Map<string, number>();
  const pos = new Int32Array(count);
  for (let i = 0; i < count; i++) {
    const k = `${rows[i * width]},${rows[i * width + 1]},${rows[i * width + 2]}`;
    pos[i] = byPosition.get(k) ?? (byPosition.set(k, i), i);
  }
  return { kind: "carried", rest: restNormals(rows, width, count, tris, (i) => i), welded: restNormals(rows, width, count, tris, (i) => pos[i]!) };
}

/** A field's surface-nets mesh as a mesh with rows of width 3 and its own (gradient) normals, welded already. */
export function fromNets(m: TriangleMesh): Mesh {
  const count = m.positions.length / 3;
  if (count >= MAX_POINTS) throw new Error(`mesh: ${count} vertices; indices are stored as f32, exact below 2^24`);
  return { count, width: 3, rows: m.positions, triangles: m.indices, normals: { kind: "carried", rest: m.normals, welded: m.normals } };
}

/** The point a pose puts a copy's point at (a size per axis scales before turning). */
function posedPoint(p: P3, at: Posed): P3 {
  const z = at.size;
  switch (z.kind) {
    case "even": return add(at.frame.o, mul(rotate(at.frame, p), z.k));
    case "axes": return add(at.frame.o, rotate(at.frame, mul(p, z.k)));
  }
}

/** n carried by the cofactor of J (columns dx, dy, dz): n.x (dy x dz) + n.y (dz x dx) + n.z (dx x dy). */
function cofactor(J: readonly P3[], n: P3): P3 {
  const [dx, dy, dz] = J as [P3, P3, P3];
  return add(add(mul(cross(dy, dz), n.x), mul(cross(dz, dx), n.y)), mul(cross(dx, dy), n.z));
}

/** The world point of point i of copy COPY, and its two normals carried (zero for faces). */
function placedOf(name: string, mesh: Mesh, motion: Motion, copies: Copies): { readonly world: P3; readonly n: P3; readonly w: P3; readonly still: boolean } {
  const { REST, N0, W0, I, COPY } = vars();
  const row = rowOf(mesh.width);
  const moved = motion.kind === "moved" ? motion.at(row, I) : REST;
  const world = copies.kind === "poses" ? posedPoint(moved, copies.poses.at(COPY)) : moved;
  const zero = vec3(0, 0, 0);
  switch (mesh.normals.kind) {
    case "faces": return { world, n: zero, w: zero, still: false };
    case "carried": {
      if (motion.kind === "rest" && copies.kind === "one") return { world, n: N0, w: W0, still: true };
      const gpu = libsOf([world]).find((l) => l.js === "gpu-only");
      if (gpu) throw new SceneError(where(name), `an \`at\` that calls the GPU-only function "${gpu.name}"`, "an `at` written in expressions (its Jacobian carries the normals)", "compute the motion in expressions, or give `points` as a count (the faces' own normals)");
      const J = jacobian(world, REST);
      return { world, n: cofactor(J, N0), w: cofactor(J, W0), still: false };
    }
  }
}

/** A static table of the rows, then (carried) the rest and welded normals: lanes(width) + 2 vec4s per point. */
function rowsTable(name: string, m: Mesh): TableSource {
  const L = lanesOf(m.width), extra = m.normals.kind === "carried" ? 2 : 0, S = L + extra;
  const data = new Float32Array(Math.max(1, m.count) * S * 4);
  for (let i = 0; i < m.count; i++) {
    for (let j = 0; j < m.width; j++) data[i * S * 4 + j] = m.rows[i * m.width + j]!;
    if (m.normals.kind === "carried") {
      data.set(m.normals.rest.subarray(i * 3, i * 3 + 3), (i * S + L) * 4);
      data.set(m.normals.welded.subarray(i * 3, i * 3 + 3), (i * S + L + 1) * 4);
    }
  }
  return { name, lanes: Math.max(1, S), entries: Math.max(1, m.count), role: "static", prepare: () => (_frame, out) => out.set(data) };
}

/** A static table of the triangle corners, four point indices per vec4 (exact as f32 below 2^24). */
function trianglesTable(name: string, m: Mesh): TableSource {
  const data = new Float32Array(Math.max(4, Math.ceil(m.triangles.length / 4) * 4));
  data.set(m.triangles);
  return { name, lanes: 1, entries: data.length / 4, role: "static", prepare: () => (_frame, out) => out.set(data) };
}

/**
 * What a mesh draw needs beyond the mesh: its shader key, a WGSL bool per vertex that shows it (`true`, or a field's
 * per-frame choice between its mesh and its trace) and the code that bool calls.
 */
export type MeshEmit = { readonly key: string; readonly show: string; readonly gate: string };

/** The shaders and draws of a mesh: one draw (two with a hull outline), `copies` instances. */
export function emitMesh(ctx: CompileContext, name: string, mesh: Mesh, motion: Motion, copies: Copies, o: { readonly paint: MeshPaint; readonly sides: "one" | "two"; readonly outline: "hull" | "none" }, e: MeshEmit): Emitted {
  const L = lanesOf(mesh.width), S = L + (mesh.normals.kind === "carried" ? 2 : 0);
  const rows: TableRef | undefined = S > 0 ? ctx.table(rowsTable(`${name}_rows`, mesh)) : undefined;
  const tris = ctx.table(trianglesTable(`${name}_tris`, mesh));
  const placed = placedOf(name, mesh, motion, copies);
  const lanes = Array.from({ length: L }, (_, k) => k);
  const atFn = emitShaderFn(ctx.slots, {
    name: "mesh_at",
    args: [["mesh_rest", "vec3"], ...lanes.map((k) => [`mesh_r${k}`, "vec4"] as const), ["mesh_i", "f32"], ["mesh_copy", "f32"], ["mesh_n0", "vec3"], ["mesh_w0", "vec3"]],
    out: "MeshV", roots: [placed.world, placed.n, placed.w], ret: (v) => `MeshV(${v.join(", ")})`,
  });
  const paint = emitShaderFn(ctx.slots, {
    name: "mesh_paint", args: [...lanes.map((k) => [`mesh_r${k}`, "vec4"] as const), ["p_world", "vec3"], ["p_normal", "vec3"], ["p_copy", "f32"]], out: "vec3<f32>",
    roots: [o.paint(paintRowOf(mesh.width), vars().AT)], ret: (v) => v[0]!,
  });
  const fetchLane = (k: number) => `${rows!.wgsl}(k * ${S}u + ${k}u)`;
  const rest = mesh.width > 0 ? "r0.xyz" : "vec3<f32>(0.0)";
  const n0 = mesh.normals.kind === "carried" ? `${fetchLane(L)}.xyz` : "vec3<f32>(0.0)";
  const w0 = mesh.normals.kind === "carried" ? `${fetchLane(L + 1)}.xyz` : "vec3<f32>(0.0)";
  // Still: the stored unit normals as they are (normalising them again would move a field's mesh by an ulp).
  const hullN = placed.still ? "m.w" : "normalize(m.w)";
  const faceN = mesh.normals.kind === "faces"
    ? "let fn0 = normalize(cross(dpdx(in.world), dpdy(in.world)));\n  let n = select(-fn0, fn0, dot(fn0, view_dir(in.world)) < 0.0);"
    : o.sides === "two" ? "let nn = normalize(in.normal);\n  let n = select(-nn, nn, front);" : "let n = normalize(in.normal);";
  const varyings = lanes.map((k) => `  @location(${2 + k}) r${k}: vec4<f32>,`).join("\n");
  const code = [traceShadeWgsl(), e.gate, `const MESH_ID: f32 = ${lit(nameId(name))};`, atFn.code, paint.code, /* wgsl */ `
struct MeshV { p: vec3<f32>, n: vec3<f32>, w: vec3<f32> };
override INK_HULL: u32 = 0u;
struct MeshIn {
  @builtin(position) clip: vec4<f32>,
  @location(0) world: vec3<f32>,
  @location(1) normal: vec3<f32>,
${varyings}
  @location(${2 + L}) @interpolate(flat) copy: f32,
};
@vertex fn vs(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> MeshIn {
  let k = u32(${tris.wgsl}(vi >> 2u)[vi & 3u]);
${lanes.map((j) => `  let r${j} = ${fetchLane(j)};`).join("\n")}
  let m = mesh_at(${[rest, ...lanes.map((j) => `r${j}`), "f32(k)", "f32(ii)", n0, w0].join(", ")});
  var p = m.p;
  if (INK_HULL == 1u) { let fp = pixel_world(p); p += ${hullN} * ink_px(fp) * fp; }
  return MeshIn(select(HIDDEN_VERTEX, frame.view_proj * vec4<f32>(p, 1.0), ${e.show}), p, m.n, ${[...lanes.map((j) => `r${j}`), "f32(ii)"].join(", ")});
}
// The paint may run a loop whose length differs per pixel before the style's derivatives.
@diagnostic(off, derivative_uniformity)
@fragment fn fs(in: MeshIn, @builtin(front_facing) front: bool) -> @location(0) vec4<f32> {
  ${faceN}
  let d = view_dir(in.world);
  let id = copy_id(MESH_ID, u32(in.copy));
  let base = mesh_paint(${[...lanes.map((j) => `in.r${j}`), "in.world", "n", "in.copy"].join(", ")});
  let colour = select(trace_shade(base, n, in.world, d, id), trace_ink(base, n, in.world, d, id), INK_HULL == 1u);
  return vec4<f32>(colour, ${ctx.markOut("trace_mark(base, n, in.world, d, id)", o.outline === "hull")});
}`].join("\n");
  const instances = copies.kind === "poses" ? copies.poses.count : 1;
  const n = mesh.triangles.length;
  const ink: DrawSpec[] = o.outline === "hull" && ctx.outline === "hull" ? [drawSpec({ label: `${name}_ink`, shader: e.key, vertices: n, instances, cull: "front", constants: { INK_HULL: 1 } })] : [];
  return {
    shaders: { [e.key]: { code, libs: [...new Set([...atFn.libs, ...paint.libs])] } },
    draws: [drawSpec({ label: name, shader: e.key, vertices: n, instances, cull: o.sides === "two" ? "none" : "back", constants: { INK_HULL: 0 } }), ...ink],
  };
}

/**
 * The `Drawable` that `draw.mesh` returns: the author's triangles, moved by `at`, painted over rows, lit by the style.
 * A scene author gets it from `draw.mesh`, not by constructing it.
 */
export class MeshDrawable implements Drawable {
  readonly skeletons: readonly Skeleton[];
  readonly forms: readonly Form[];
  private readonly mesh: Mesh;
  private readonly motion: Motion;

  constructor(readonly name: string, readonly data: MeshData, readonly o: MeshOptions, private readonly copies: Copies = { kind: "one" }) {
    if (!/^[A-Za-z][A-Za-z0-9_]*$/.test(name)) throw new SceneError(where(name), `the name "${name}"`, "an identifier (it names shaders)", "letters, digits and _ only");
    this.name = scopedName(name);
    if (o.sides === "two" && o.outline === "hull") throw new SceneError(where(name), `sides "two" with outline "hull"`, "a hull outline only on a one-sided mesh (it draws the back faces)", `outline: "none"`);
    const c = classifyMesh(name, data, o.at);
    this.mesh = c.mesh;
    this.motion = c.motion;
    this.skeletons = o.skeletons ?? defaultShadow(name, this.mesh, this.motion, copies);
    this.forms = [{ kind: "mesh", name: this.name, mesh: this.mesh, at: this.motion.kind === "moved" ? this.motion.at : undefined, paint: o.paint, copies: copies.kind === "poses" ? copies.poses : undefined }];
  }

  /** The same mesh, one copy per point. */
  instanced(name: string, points: Points): MeshDrawable {
    return new MeshDrawable(name, this.data, this.o, { kind: "poses", poses: posesOf(points) });
  }

  /** The same mesh, one copy per pose: turned, and scaled along each axis (normals carried by the Jacobian). */
  posed(name: string, poses: Poses): MeshDrawable {
    return new MeshDrawable(name, this.data, this.o, { kind: "poses", poses });
  }

  draws(ctx: CompileContext): Emitted {
    return emitMesh(ctx, this.name, this.mesh, this.motion, this.copies, this.o, { key: `mesh.${this.name}`, show: "true", gate: "" });
  }
}

/** One blob fitted to the rest points (or to `at` at time 0 for a count), carried by each copy's pose. */
function defaultShadow(name: string, mesh: Mesh, motion: Motion, copies: Copies): readonly Skeleton[] {
  const pts: { p: [number, number, number]; w: number }[] = [];
  if (mesh.width > 0) {
    for (let i = 0; i < mesh.count; i++) pts.push({ p: [mesh.rows[i * mesh.width]!, mesh.rows[i * mesh.width + 1]!, mesh.rows[i * mesh.width + 2]!], w: 1 });
  } else if (motion.kind === "moved") {
    const e = motion.at([], vars().I);
    const l = leaves([e]);
    if (l.tables.length > 0 || libsOf([e]).some((x) => x.js === "gpu-only")) throw new SceneError(where(name), "points as a count whose `at` the CPU cannot evaluate (a table or a GPU-only function)", "skeletons for the shadow", "skeletons: [] for none, or chains that follow the mesh");
    const run = compileJs([e]);
    for (let i = 0; i < mesh.count; i++) pts.push({ p: run({ vars: { mesh_i: i }, inputs: { time: [0] }, params: {} })[0] as unknown as [number, number, number], w: 1 });
  }
  const above = pts.filter((q) => q.p[1] > 0);
  const cast = above.length >= 2 ? above : pts;
  let lo = Infinity, hi = -Infinity;
  for (const q of cast) { lo = Math.min(lo, q.p[0], q.p[2]); hi = Math.max(hi, q.p[0], q.p[2]); }
  const blob = shadowSkeleton({ kind: "sampled", inside: cast.length, chain: fitBlob(cast, Math.max(1e-3, 0.05 * (hi - lo))) });
  switch (copies.kind) {
    case "one": return [blob];
    case "poses": return Array.from({ length: copies.poses.count }, (_, i) => {
      const at = copies.poses.at(f(i));
      return { joints: blob.joints, joint: (k: number) => posedJoint(blob.joint(k), at) };
    });
  }
}

/** Triangles made by the author's code: `draw.mesh(name, { points, triangles }, { at?, paint, sides, outline, skeletons? })`. */
export function drawMesh(name: string, mesh: MeshData, o: MeshOptions): MeshDrawable {
  return new MeshDrawable(name, mesh, o);
}

/** For the baker: point i of copy `copy` moved (or at rest), and its normal; built on the CPU from the same expressions. */
export function meshPoint(mesh: Mesh, at: MeshOptions["at"], copies: Poses | undefined): { readonly world: P3; readonly normal: P3; readonly rowVars: (i: number) => Record<string, number[]> } {
  const motion: Motion = at === undefined ? { kind: "rest" } : { kind: "moved", at };
  const c: Copies = copies === undefined ? { kind: "one" } : { kind: "poses", poses: copies };
  const placed = placedOf("bake", mesh, motion, c);
  const L = lanesOf(mesh.width);
  const rowVars = (i: number): Record<string, number[]> => {
    const v: Record<string, number[]> = { mesh_rest: [0, 0, 0] };
    for (let k = 0; k < L; k++) v[`mesh_r${k}`] = Array.from({ length: 4 }, (_, j) => (k * 4 + j < mesh.width ? mesh.rows[i * mesh.width + k * 4 + j]! : 0));
    if (mesh.width > 0) v.mesh_rest = [mesh.rows[i * mesh.width]!, mesh.rows[i * mesh.width + 1]!, mesh.rows[i * mesh.width + 2]!];
    if (mesh.normals.kind === "carried") { v.mesh_n0 = [...mesh.normals.rest.subarray(i * 3, i * 3 + 3)]; v.mesh_w0 = [...mesh.normals.welded.subarray(i * 3, i * 3 + 3)]; }
    return v;
  };
  return { world: placed.world, normal: placed.n, rowVars };
}

/** The fragment's row and PaintAt variables, for the baker's evaluation of paint at a point. */
export const MESH_PAINT_VARS = { row: (width: number) => paintRowOf(width), at: (): PaintAt => vars().AT };

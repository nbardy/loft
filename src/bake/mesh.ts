// bakeScene: evaluate a scene at one time into plain triangle meshes, one per body.
//
// A strand is a grid of (m, v) samples of `Strand.surface` over the pose table, with its tips closed; a crest is a grid
// of (x, f) samples of `Crest.point`, a single sheet (two-sided in the file's material, not by duplicated triangles).
// Expressions run through the CPU backend (`compileJs`), the same expressions the GPU evaluates.

import { compile } from "../compile/index.ts";
import { compileJs, variable, type Env } from "../expr.ts";
import type { Crest } from "../geom/crest.ts";
import { stationAt, STATION_VEC4S } from "../geom/stations.ts";
import { FRAME_VARS, type Strand, type Tip } from "../geom/strand.ts";
import type { Scene } from "../scene.ts";
import { discover, type Skipped, type Source } from "./discover.ts";

export type Density = { readonly along: number; readonly around: number };
export type BakeOptions = { readonly time: number; readonly density: Density };

export type BodyMesh = {
  readonly name: string;
  readonly kind: "strand" | "crest";
  /** Crests are sheets: the material is double-sided. Strands are closed solids. */
  readonly doubleSided: boolean;
  readonly positions: Float32Array; // xyz
  readonly normals: Float32Array; // xyz, unit
  readonly uvs: Float32Array; // strand: (m, v); crest: (x, f)
  readonly colors: Float32Array; // rgb in [0, 1]
  readonly indices: Uint32Array; // counter-clockwise seen from the normal side
};
export type Baked = { readonly time: number; readonly bodies: readonly BodyMesh[]; readonly skipped: readonly Skipped[] };

export const vertexCount = (b: BodyMesh): number => b.positions.length / 3;
export const triangleCount = (b: BodyMesh): number => b.indices.length / 3;

// ---- a mesh under construction -------------------------------------------------------------------------------------

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
    // A NaN here would reach the file and fail in someone else's viewer; fail where the body is known instead.
    if (all.some((x) => !Number.isFinite(x))) throw new Error(`bake: body "${this.name}" evaluated to a non-finite vertex (${all.join(", ")})`);
    const n = this.pos.length / 3;
    this.pos.push(...v.pos);
    this.nrm.push(...v.normal);
    this.uv.push(...v.uv);
    this.col.push(...v.colour.map((c) => Math.min(1, Math.max(0, c))));
    return n;
  }
  triangle(a: number, b: number, c: number): void { this.idx.push(a, b, c); }

  finish(kind: BodyMesh["kind"], doubleSided: boolean): BodyMesh {
    return {
      name: this.name, kind, doubleSided,
      positions: Float32Array.from(this.pos), normals: Float32Array.from(this.nrm), uvs: Float32Array.from(this.uv),
      colors: Float32Array.from(this.col), indices: Uint32Array.from(this.idx),
    };
  }
}

/** Which rows of a grid have collapsed to a point (a closing tip): the quad next to one has a single real triangle. */
type Collapse = { readonly first: boolean; readonly last: boolean };

/**
 * A (rows + 1) x (cols + 1) grid of vertices, sample(i, j) at row i, column j; quads split into two triangles that run
 * counter-clockwise when row direction x column direction is the outward normal. A collapsed first or last row keeps
 * only the quad's non-degenerate triangle.
 */
function grid(b: Builder, rows: number, cols: number, collapse: Collapse, sample: (i: number, j: number) => Vertex): void {
  const at: number[][] = [];
  for (let i = 0; i <= rows; i++) {
    const ring: number[] = [];
    for (let j = 0; j <= cols; j++) ring.push(b.vertex(sample(i, j)));
    at.push(ring);
  }
  for (let i = 0; i < rows; i++) {
    for (let j = 0; j < cols; j++) {
      const a = at[i][j], bb = at[i + 1][j], c = at[i + 1][j + 1], d = at[i][j + 1];
      // First row collapsed: a = d, so (a, c, d) is the degenerate one. Last row collapsed: bb = c, so (a, bb, c) is.
      if (!(collapse.last && i === rows - 1)) b.triangle(a, bb, c);
      if (!(collapse.first && i === 0)) b.triangle(a, c, d);
    }
  }
}

// ---- strands ---------------------------------------------------------------------------------------------------------

/**
 * Copied from `BODY_M_WGSL` in src/compile/shaders.ts: the vertex stage spaces rings along the body by this map
 * (denser at the ends, where the tips close), so the baked rings sit where the drawn ones do.
 */
const bodyM = (u: number): number => u + 0.6 * ((0.5 - 0.5 * Math.cos(Math.PI * u)) - u);

type EndClosure = "collapsed" | "capped";
function closureOf(tip: Tip): EndClosure {
  switch (tip.kind) {
    case "round": case "point": return "collapsed"; // the radius reaches 0 at the end: the ring is a point
    case "open": return "capped"; // the radius stays: close it with a flat disc
  }
}

type FrameEnv = Env["vars"];
const frameVars = (st: ReturnType<typeof stationAt>): FrameEnv => ({
  st_p: st.p, st_der: st.der, st_t: st.t, st_d: st.d, st_s: st.s, st_stretch: st.stretch,
});

function bakeStrand(strand: Strand, base: number, table: Float32Array, env: Pick<Env, "inputs" | "params">, d: Density): BodyMesh {
  const m = variable("m", "f32"), v = variable("v", "f32");
  const surf = strand.surface(FRAME_VARS, m, v);
  const run = compileJs([surf.pos, surf.normal, strand.paint(m, v)]);
  const ends = [closureOf(strand.tips[0]), closureOf(strand.tips[1])] as const;
  const b = new Builder(strand.name);
  const sampleAt = (mm: number, vv: number): Vertex => {
    const st = stationAt(table, base, strand.stations, mm);
    const [pos, normal, colour] = run({ vars: { m: mm, v: vv, ...frameVars(st) }, ...env }) as [number[], number[], number[]];
    return { pos, normal, uv: [mm, vv], colour };
  };
  grid(b, d.along, d.around, { first: ends[0] === "collapsed", last: ends[1] === "collapsed" },
    (i, j) => sampleAt(bodyM(i / d.along), j / d.around));

  // Open ends get a flat disc: a ring with its own normals (hard edge) and a centre, fanned.
  const cap = (end: 0 | 1) => {
    const mm = end, outward = end === 0 ? -1 : 1;
    const st = stationAt(table, base, strand.stations, mm);
    const ring: number[] = [];
    for (let j = 0; j < d.around; j++) {
      const s = sampleAt(mm, j / d.around);
      ring.push(b.vertex({ ...s, normal: st.t.map((x) => x * outward) }));
    }
    const centre = b.vertex({ pos: st.p, normal: st.t.map((x) => x * outward), uv: [mm, 0], colour: sampleAt(mm, 0).colour });
    for (let j = 0; j < d.around; j++) {
      const p = ring[j], q = ring[(j + 1) % d.around];
      // v runs from the side toward the dorsal line, i.e. clockwise seen from +tangent: so the fan reads CCW from outside
      // at the m = 1 end with (centre, q, p), and at the m = 0 end with (centre, p, q).
      if (end === 0) b.triangle(centre, p, q); else b.triangle(centre, q, p);
    }
  };
  ends.forEach((e, end) => { if (e === "capped") cap(end as 0 | 1); });
  return b.finish("strand", false);
}

// ---- crests ----------------------------------------------------------------------------------------------------------

/** `Strand`'s default `detail.along`; a crest's authored `rows` is scaled by density.along over this. */
const AUTHORED_ALONG = 96;

function bakeCrest(crest: Crest, base: number, table: Float32Array, env: Pick<Env, "inputs" | "params">, d: Density): BodyMesh {
  const x = variable("x", "f32"), f = variable("f", "f32");
  const pt = crest.point(FRAME_VARS, x, f);
  const run = compileJs([pt.pos, pt.normal, crest.paint(x, f)]);
  const rows = Math.max(2, Math.ceil((crest.rows * d.along) / AUTHORED_ALONG)), cols = Math.max(3, Math.ceil(d.around / 4));
  const host = crest.host;
  const b = new Builder(crest.name);
  grid(b, rows, cols, { first: false, last: false }, (i, j) => {
    const xx = i / rows, ff = j / cols;
    const st = stationAt(table, base, host.stations, crest.from + (crest.to - crest.from) * xx);
    const [pos, normal, colour] = run({ vars: { x: xx, f: ff, ...frameVars(st) }, ...env }) as [number[], number[], number[]];
    return { pos, normal, uv: [xx, ff], colour };
  });
  return b.finish("crest", true);
}

// ---- the scene -------------------------------------------------------------------------------------------------------

function bakeSource(s: Source, table: Float32Array, env: Pick<Env, "inputs" | "params">, d: Density): BodyMesh {
  switch (s.kind) {
    case "strand": return bakeStrand(s.strand, s.range.base, table, env, d);
    case "crest": return bakeCrest(s.crest, s.range.base, table, env, d);
  }
}

export function bakeScene(scene: Scene, options: BakeOptions): Baked {
  const { time, density } = options;
  if (!Number.isFinite(time)) throw new Error(`bake: time must be a finite number, got ${time}`);
  if (!(Number.isInteger(density.along) && density.along >= 2)) throw new Error(`bake: density.along must be an integer >= 2, got ${density.along}`);
  if (!(Number.isInteger(density.around) && density.around >= 3)) throw new Error(`bake: density.around must be an integer >= 3, got ${density.around}`);

  const found = discover(scene);
  const artefact = compile(scene);
  if (artefact.stats.stations !== found.stations) {
    throw new Error(`bake: the compiler posed ${artefact.stats.stations} stations but the scene's bodies account for ${found.stations}; the station layout has changed (src/bake/discover.ts)`);
  }
  // The frame state the host would build: time, and every other input and parameter at its default.
  const inputs = new Float32Array(Math.max(1, artefact.inputFloats));
  for (const s of artefact.inputs) inputs.set(s.name === "time" ? [time] : s.def, s.slot);
  const params = Float32Array.from(artefact.params.map((p) => p.def));
  const table = artefact.pose({ time, inputs, params }).stations;
  if (table.length < found.stations * STATION_VEC4S * 4) throw new Error("bake: the pose returned a short station table");
  const env = { inputs: { time } as Env["inputs"], params: Object.fromEntries(artefact.params.map((p) => [p.name, p.def])) };

  const bodies = found.sources.map((s) => bakeSource(s, table, env, density));
  return { time, bodies, skipped: found.skipped };
}

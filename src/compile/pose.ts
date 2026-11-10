// The stations table: every strand slice the drawables asked for, posed once per frame on the CPU.
//
// A slice is a strand at a time: `time` itself, `time + offset` (a smear, a wake), or an input (a telegraph at an
// attack's arming time). Its expressions are the strand's with `time` substituted, registered with the shared CPU
// program, so hash-consing still shares whatever does not depend on m across all stations, strands and slices.
// (Evaluating each station by binding a free m recomputed all of it per call and cost about 40 ms a frame for the
// serpent.) Attached strands are placed on their host's chart, evaluated by the chart's CPU twin over the part of the
// table already posed: a host is always registered, and so posed, before the parts that ride it.

import { compileJs, lift, substitute, time, type Env, type Expr } from "../expr.ts";
import { poseStrand, STATION_VEC4S, STATIONS_TABLE, v3, type V3 } from "../geom/stations.ts";
import type { Strand } from "../geom/strand.ts";
import type { Slots } from "./bind.ts";
import { chartTwin, type ChartTwin } from "./charts.ts";
import type { CpuRef, PoseFrame, StationRange, TableRef, TableSpec } from "./context.ts";
import type { CpuProgram } from "./tables.ts";

/** How a slice's spine reaches the world. */
type Place =
  | { readonly kind: "world" }
  | { readonly kind: "on"; readonly host: ChartTwin; readonly m: number; readonly v: number; readonly rho: number };

type Slice = { readonly strand: Strand; readonly at: Expr<"f32">; readonly range: StationRange; readonly now: CpuRef; readonly place: Place };

/** Roots per station: spine, twist, half-width, aspect; then the strand's `up`. */
const PER_STATION = 4;

export class StationPose {
  private readonly slices: Slice[] = [];
  private readonly restRoots: Expr[] = [];
  private readonly restAt = new Map<Strand, number>();
  private total = 0;

  constructor(private readonly slots: Slots, private readonly program: CpuProgram, readonly table: TableRef) {}

  get stations(): number { return this.total; }

  range(strand: Strand, at: Expr<"f32">): StationRange {
    const seen = this.slices.find((s) => s.strand === strand && s.at === at);
    if (seen) return seen.range;
    const clash = this.slices.find((s) => s.strand !== strand && s.strand.name === strand.name);
    if (clash) throw new Error(`compile: two different strands are named "${strand.name}" (names key shaders and charts)`);
    const place = this.placeOf(strand, at);
    const range: StationRange = { table: this.table, base: this.total, count: strand.stations };
    this.total += strand.stations;
    const roots: Expr[] = [];
    for (let k = 0; k < strand.stations; k++) {
      const m = lift<"f32">(k / (strand.stations - 1));
      roots.push(strand.spine(m), strand.twist(m), strand.halfWidth(m), strand.aspect(m));
    }
    roots.push(strand.up);
    const now = this.program.add(at === time ? roots : substitute(roots, time, at));
    if (!this.restAt.has(strand)) {
      this.restAt.set(strand, this.restRoots.length);
      for (let k = 0; k < strand.stations; k++) this.restRoots.push(strand.spine(lift<"f32">(k / (strand.stations - 1))));
    }
    this.slices.push({ strand, at, range, now, place });
    return range;
  }

  private placeOf(strand: Strand, at: Expr<"f32">): Place {
    const p = strand.placement;
    switch (p.kind) {
      case "world": return { kind: "world" };
      case "on": return { kind: "on", host: chartTwin(this.slots, p.host, this.range(p.host, at), at), m: p.m, v: p.v, rho: p.rho };
    }
  }

  /** The table, once every drawable has registered its slices. */
  spec(): TableSpec {
    // The rest pose (stretch is measured against it) is the scene at time 0. It depends only on params, non-time
    // inputs and curves, which rarely change between frames, so the last one is kept. (A memo keyed on all of those,
    // so the function stays pure: the same state always gives the same output.)
    const rest = compileJs(this.restRoots, { kind: "table", slot: (r) => this.slots.row(r) });
    let restKey = "", restCurves = new Float32Array(0), restValues: V3[] = [];
    const restOf = (env: Env): V3[] => {
      const { time: _, ...inputs } = env.inputs;
      const key = JSON.stringify([inputs, env.params]);
      const curves = env.curves!;
      if (key !== restKey || !sameFloats(curves, restCurves)) {
        restValues = rest({ ...env, inputs: { ...env.inputs, time: 0 } }) as V3[];
        restKey = key;
        restCurves = curves.slice();
      }
      return restValues;
    };
    const slices = this.slices;
    return {
      name: STATIONS_TABLE,
      vec4s: Math.max(1, this.total * STATION_VEC4S),
      fill: (frame: PoseFrame, out: Float32Array) => {
        const restPos = restOf(frame.env);
        for (const s of slices) {
          const v = frame.values(s.now);
          const n = s.strand.stations, r0 = this.restAt.get(s.strand)!;
          const pos: V3[] = [], rest: V3[] = [], twist: number[] = [], halfWidth: number[] = [], aspect: number[] = [];
          for (let k = 0; k < n; k++) {
            const o = k * PER_STATION;
            pos.push(v[o] as V3); twist.push(v[o + 1] as number); halfWidth.push(v[o + 2] as number); aspect.push(v[o + 3] as number);
            rest.push(restPos[r0 + k]);
          }
          const placed = place(s.place, pos, v[n * PER_STATION] as V3, out, frame.env);
          poseStrand({ pos: placed.pos, rest, twist, halfWidth, aspect, up: placed.up, level: s.strand.level }, out, s.range.base * STATION_VEC4S * 4);
        }
      },
    };
  }
}

/** Local spine and `up` to world. `table` already holds every host (hosts are registered first). */
function place(p: Place, pos: V3[], up: V3, table: Float32Array, env: Env): { pos: V3[]; up: V3 } {
  switch (p.kind) {
    case "world": return { pos, up };
    case "on": {
      const b = p.host(table, env)(p.m, p.v, p.rho);
      const rot = (l: V3): V3 => v3.add(v3.add(v3.scale(b.x, l[0]), v3.scale(b.y, l[1])), v3.scale(b.z, l[2]));
      return { pos: pos.map((l) => v3.add(b.pos, rot(l))), up: rot(up) };
    }
  }
}

function sameFloats(a: Float32Array, b: Float32Array): boolean {
  if (a.length !== b.length) return false;
  for (let i = 0; i < a.length; i++) if (a[i] !== b[i]) return false;
  return true;
}

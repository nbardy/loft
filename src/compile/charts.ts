// A strand is a chart (DESIGN.md 2.3): body coordinates (m, v, rho) to a world point, normal and frame, over its
// posed stations. One expression graph, two backends: the WGSL function every shader may call, and its CPU twin
// (which the pose pass uses to place attached parts, and which checks and tests use). They agree because they are the
// same expressions evaluated over the same table through mirrored `station_at` readers.

import { compileJs, cross, mix, substitute, time, variable, type Env, type Expr } from "../expr.ts";
import { FRAME_VARS, FRAME_WGSL } from "../geom/frame.ts";
import { stationAt, type V3 } from "../geom/stations.ts";
import type { Strand } from "../geom/strand.ts";
import { emitFn, type Slots } from "./bind.ts";
import type { BodyPoint, StationRange } from "./context.ts";

const M = variable("m", "f32"), V = variable("v", "f32"), RHO = variable("rho", "f32");

export const BODY_POINT_WGSL = /* wgsl */ `
struct BodyPoint { pos: vec3<f32>, normal: vec3<f32>, x: vec3<f32>, y: vec3<f32>, z: vec3<f32> };`;

/** pos, normal, x, y, z at (m, v, rho), over the frame-sample variables, with `time` replaced by `at`. */
function chartRoots(strand: Strand, at: Expr<"f32">): Expr[] {
  const sp = strand.surface(FRAME_VARS, M, V);
  const roots: Expr[] = [mix(FRAME_VARS.p, sp.pos, RHO), sp.normal, FRAME_VARS.t, sp.radial, cross(FRAME_VARS.t, sp.radial)];
  return at === time ? roots : substitute(roots, time, at);
}

/** `fn <name>(m, v, rho) -> BodyPoint` over the strand's range. */
export function chartWgsl(slots: Slots, name: string, strand: Strand, range: StationRange, at: Expr<"f32">): string {
  return emitFn(slots, name, [["m", "f32"], ["v", "f32"], ["rho", "f32"]], "BodyPoint", chartRoots(strand, at), (o) => `BodyPoint(${o.join(", ")})`, {
    pre: [`let st = station_at(${range.base}u, ${range.count}u, m);`],
    extra: FRAME_WGSL,
  });
}

export type ChartTwin = (stations: Float32Array, env: Env) => (m: number, v: number, rho: number) => BodyPoint;

/** The CPU twin, reading curve rows from the frame's curve table (so `setCurves` edits reach it). */
export function chartTwin(slots: Slots, strand: Strand, range: StationRange, at: Expr<"f32">): ChartTwin {
  const run = compileJs(chartRoots(strand, at), { kind: "table", slot: (r) => slots.row(r) });
  return (stations, env) => (m, v, rho) => {
    const st = stationAt(stations, range.base, range.count, m);
    const vars = { m, v, rho, st_p: st.p, st_der: st.der, st_t: st.t, st_d: st.d, st_s: st.s, st_stretch: st.stretch };
    const [pos, normal, x, y, z] = run({ vars, inputs: env.inputs, params: env.params, curves: env.curves }) as V3[];
    return { pos, normal, x, y, z };
  };
}

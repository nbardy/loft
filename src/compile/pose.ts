// The pose program: everything the CPU evaluates once per frame, as ONE compiled JavaScript function.
//
// Every station of every strand is a separate root at a constant m, so hash-consing shares whatever does not depend on
// m (control points, a head's frame built from the body's spine, a horn's base built from the head) across all
// stations and strands: it is computed once per frame, not once per station. (Evaluating each station by binding a
// free m recomputed all of it per call and cost about 40 ms a frame for the serpent; this costs well under one.)

import { compileJs, lift, type Env, type Expr } from "../expr.ts";
import type { Strand } from "../geom/strand.ts";
import { poseStrand, STATION_VEC4S, type V3 } from "../geom/stations.ts";
import type { StationRange } from "./context.ts";
import { foldConstants } from "./fold.ts";

type Posed = { readonly strand: Strand; readonly range: StationRange };

/** Roots per station: spine, twist, half-width, aspect; then one `up` per strand. */
const PER_STATION = 4;

export function poseProgram(posed: readonly Posed[]): { roots: Expr[]; run: (now: Env, rest: Env, out: Float32Array) => void } {
  const roots: Expr[] = [], restRoots: Expr[] = [];
  for (const { strand } of posed) {
    for (let k = 0; k < strand.stations; k++) {
      const m = lift<"f32">(k / (strand.stations - 1));
      const spine = strand.spine(m);
      roots.push(spine, strand.twist(m), strand.halfWidth(m), strand.aspect(m));
      restRoots.push(spine);
    }
  }
  for (const { strand } of posed) roots.push(strand.up);
  const now = compileJs(foldConstants(roots)), rest = compileJs(foldConstants(restRoots));
  // The rest pose depends only on params and non-time inputs, which rarely change between frames: keep the last one.
  // (A memo, so the function stays pure: the same state always gives the same output.)
  let restKey = "", restValues: ReturnType<typeof rest> = [];
  const run = (envNow: Env, envRest: Env, out: Float32Array): void => {
    const key = JSON.stringify([envRest.inputs, envRest.params]);
    if (key !== restKey) { restValues = rest(envRest); restKey = key; }
    const v = now(envNow), r = restValues;
    let at = 0, restAt = 0;
    posed.forEach(({ strand, range }, s) => {
      const n = strand.stations;
      const pos: V3[] = [], restPos: V3[] = [], twist: number[] = [], halfWidth: number[] = [], aspect: number[] = [];
      for (let k = 0; k < n; k++, at += PER_STATION) {
        pos.push(v[at] as V3); twist.push(v[at + 1] as number); halfWidth.push(v[at + 2] as number); aspect.push(v[at + 3] as number);
        restPos.push(r[restAt++] as V3);
      }
      const up = v[roots.length - posed.length + s] as V3;
      poseStrand({ pos, rest: restPos, twist, halfWidth, aspect, up, level: strand.level }, out, range.base * STATION_VEC4S * 4);
    });
  };
  return { roots: [...roots, ...restRoots], run };
}

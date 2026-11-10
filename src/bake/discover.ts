// Finding the bakeable bodies in a scene.
//
// `Scene.things` holds `Drawable`s (a name and a `draws(ctx)` closure): the Strand or Crest each one wraps is not kept.
// Baking needs the objects, so each drawable is run against a probe context that stops it at the first thing that
// reveals what it is:
//   - a strand registers itself with `ctx.stations(strand)` and then touches `ctx.slots`;
//   - a crest registers its host, then calls `crest.point(...)` (we catch `this`) before touching `ctx.slots`.
// This leans on the order of calls inside `strandDraws` / `crestDraws` (src/compile/shaders.ts). The proper fix is for
// `Scene` to keep its sources; when it does, this file shrinks to a lookup. Until then a drawable that is neither is
// reported as skipped, by name, never dropped silently.
//
// The probe also rebuilds the compiler's station table layout: `compile` gives each strand a range in the order the
// drawables first register it, and `artefact.pose` returns the table in that layout, so we register in the same order.

import type { StationRange } from "../compile/context.ts";
import type { Drawable } from "../compile/context.ts";
import { Crest } from "../geom/crest.ts";
import type { Strand } from "../geom/strand.ts";
import type { Scene } from "../scene.ts";

export type Posed = { readonly strand: Strand; readonly range: StationRange };
export type Source =
  | { readonly kind: "strand"; readonly name: string; readonly strand: Strand; readonly range: StationRange }
  | { readonly kind: "crest"; readonly name: string; readonly crest: Crest; readonly range: StationRange };
export type Skipped = { readonly name: string; readonly reason: string };
export type Discovered = { readonly sources: readonly Source[]; readonly skipped: readonly Skipped[]; readonly posed: readonly Posed[]; readonly stations: number };

/** Thrown by the probe to stop a drawable once it has said what it is. */
class Stop {
  constructor(readonly crest: Crest | undefined) {}
}

export function discover(scene: Scene): Discovered {
  const posed: Posed[] = [];
  let total = 0;
  const register = (strand: Strand): StationRange => {
    const seen = posed.find((p) => p.strand === strand);
    if (seen) return seen.range;
    const range = { base: total, count: strand.stations };
    total += strand.stations;
    posed.push({ strand, range });
    return range;
  };

  const sources: Source[] = [], skipped: Skipped[] = [];
  const original = Crest.prototype.point;
  try {
    for (const thing of scene.things) {
      const touched: Strand[] = [];
      let crestSeen: Crest | undefined;
      Crest.prototype.point = function (this: Crest) { crestSeen = this; throw new Stop(this); };
      const refuse = () => { throw new Stop(undefined); };
      const ctx = {
        stations: (s: Strand) => { touched.push(s); return register(s); },
        slots: new Proxy({}, { get: refuse }),
        get style() { return refuse(); },
        get sea() { return refuse(); },
      };
      try {
        (thing as Drawable).draws(ctx as never);
        skipped.push({ name: thing.name, reason: "drew nothing the baker recognises" });
      } catch (e) {
        if (!(e instanceof Stop)) {
          skipped.push({ name: thing.name, reason: `unknown kind of drawable (${e instanceof Error ? e.message : String(e)})` });
          continue;
        }
        const strand = touched[0];
        if (crestSeen !== undefined && crestSeen.name === thing.name && touched.length === 1) {
          sources.push({ kind: "crest", name: thing.name, crest: crestSeen, range: register(crestSeen.host) });
        } else if (crestSeen === undefined && touched.length === 1 && strand.name === thing.name) {
          sources.push({ kind: "strand", name: thing.name, strand, range: register(strand) });
        } else {
          skipped.push({ name: thing.name, reason: "unknown kind of drawable (not a plain strand or crest)" });
        }
      }
    }
  } finally {
    Crest.prototype.point = original;
  }
  return { sources, skipped, posed, stations: total };
}

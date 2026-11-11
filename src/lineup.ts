// Several subjects in one scene (docs/lineup.md): four crabs side by side. Each is a design's variant, built in a
// namespace of its own (so the names of its things, tables and inputs do not meet another's: src/expr.ts) and in two
// stages (so all of them stand in one sea, with every one's ring waves on it: `Staged`, src/design.ts).

import { namespaced } from "./expr.ts";
import { parseVariantAsk, type Member, type Overrides, type Resolved, type Staged, type VariantAsk } from "./design.ts";
import type { Check } from "./check.ts";
import type { Drawable } from "./compile/context.ts";
import { looping } from "./geom/lively.ts";
import { sea, type Sea, type SeaSpec } from "./geom/sea.ts";

/** One member of a lineup: a design's variant under a name, with what the scene changes of its numbers and how it stands. */
export type Standing<N, H, M extends Member> = {
  /** Its namespace: letters and digits. Its inputs are `name.<input>`, its things `name__<thing>`. */
  readonly name: string;
  /** A design with a staged build (its slots and what it builds alone are its own business). */
  readonly design: Stageable<N, H, M>;
  /** A named variant, or picks over one (`islander`, `stilt,face=mask`). */
  readonly variant: string;
  readonly numbers: Overrides<N>;
  readonly how: H;
};

/** What a lineup needs of a design: a choice made concrete, and its staged build. `Design` is one. */
export type Stageable<N, H, M extends Member> = {
  resolve(ask: VariantAsk, also?: Overrides<N>): Resolved<N>;
  stage(r: Resolved<N>, how: H): Staged<M>;
};

export type Lineup<M extends Member> = {
  readonly sea: Sea;
  /** Each member's product, in order, under its name. */
  readonly members: readonly { readonly name: string; readonly made: M }[];
  /** Every member's things, in order: with the sea, the scene. */
  readonly things: readonly Drawable[];
  readonly checks: readonly Check[];
};

/** Whether the lineup must loop (every idle swing of every member then takes whole turns in the period), or not. */
export type LineupLoop = { readonly kind: "open" } | { readonly kind: "every"; readonly seconds: number };

/**
 * Stand several subjects in one sea. `water` is the sea without its deformations (the swell and the foam width): every
 * member's are added. Each member is built inside its own namespace, both stages.
 */
export function lineup<N, H, M extends Member>(o: { readonly members: readonly Standing<N, H, M>[]; readonly water: Omit<SeaSpec, "deform">; readonly loop: LineupLoop }): Lineup<M> {
  if (o.members.length < 1) throw new Error("lineup: no members");
  const names = new Set<string>();
  for (const m of o.members) { if (names.has(m.name)) throw new Error(`lineup: two members are named "${m.name}"`); names.add(m.name); }
  const inLoop = <T>(build: () => T): T => { switch (o.loop.kind) { case "open": return build(); case "every": return looping(o.loop.seconds, build); } };
  const staged = o.members.map((m) => ({ name: m.name, stage: namespaced(m.name, () => inLoop(() => m.design.stage(m.design.resolve(parseVariantAsk(m.variant), m.numbers), m.how))) }));
  const water = sea({ ...o.water, deform: staged.flatMap((s) => s.stage.deforms) });
  const members = staged.map((s) => ({ name: s.name, made: namespaced(s.name, () => inLoop(() => s.stage.finish(water))) }));
  return { sea: water, members, things: members.flatMap((m) => m.made.things), checks: members.flatMap((m) => m.made.checks) };
}

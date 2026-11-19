// The scene-level decls a job can carry (docs/proposals/2026-11-19_lean_rust_loft.md sections 1.5-1.7, wire 3.2):
// draws, passes, media and cameras (what is drawn and how), checks and solves (what is asked of the motion). The
// recorder writes them (`Recorder.job`'s `scene`); loft-ir decodes each into its own array of `Graph`.
//
// A draw's roles are graph functions recorded once over binders (`role`), with loft-core `draw`'s signatures:
//   paint (p: vec3, n: vec3, uv: vec2, id: f32) -> vec4   hit (o: vec3, d: vec3, span: vec2, width: vec2, id: f32) -> vec4 (normal, t)
//   grid place and normal (uv: vec2, id: f32) -> vec3     quads place (id: f32) -> vec4 (centre, radius)
//   grid cells (id: f32) -> vec2: a copy's cell counts this frame (its level of detail; 0 hides it)
//   density (p: vec3) -> f32                        outline (colour: vec3, q: vec3) -> vec4 (ink, width)

import { variable, type DataTable, type InputSpec, type Expr, type NumTy, type Row, type TableSource } from "../expr.ts";
import { rec } from "./recorder.ts";

/** A graph function recorded for a decl to name (`role`). */
export type Role = { readonly fn: number };
type V3 = readonly [number, number, number];
type Args<P extends readonly NumTy[]> = { [K in keyof P]: Expr<P[K]> };

let roles = 0;
/** `body` recorded once as the graph function `name` over parameters of types `params`, for a decl to name. */
export function role<const P extends readonly NumTy[], R extends NumTy>(name: string, params: P, body: (...a: Args<P>) => Expr<R>): Role {
  return {
    fn: rec.fn(name, () => {
      const id = roles++, binders = params.map((t, j) => variable(`loft_role${id}_${j}`, t)) as unknown as Args<P>;
      return { kind: "body", binders: binders as readonly Expr[], body: body(...binders) };
    }),
  };
}

/** Which faces a raster draw drops: `front` is a hull outline's (the surface pushed out, drawn from behind). */
export type Raster = { readonly cull: "none" | "back" | "front"; readonly depthWrite: boolean; readonly blend: "opaque" | "alpha" | "add" };
export type Draw = {
  readonly name: string;
  readonly layer: "subject" | "surroundings";
  readonly raster: Raster;
  readonly paint: Role;
  readonly cover: Cover;
};
export type Cover =
  | { readonly kind: "grid"; readonly u: number; readonly v: number; readonly place: Role; readonly normal: Role; readonly cells: Role; readonly count: number }
  | { readonly kind: "mesh"; readonly table: TableSource | Row; readonly count: number }
  | { readonly kind: "quads"; readonly place: Role; readonly count: number }
  | { readonly kind: "ray"; readonly proxy: Proxy; readonly find: Find }
  | { readonly kind: "screen" };
export type Proxy =
  | { readonly kind: "box"; readonly min: V3; readonly max: V3 }
  | { readonly kind: "instances"; readonly table: TableSource | Row; readonly local: readonly [V3, V3] };
export type Find = { readonly kind: "hit"; readonly hit: Role } | { readonly kind: "medium"; readonly medium: Medium };
export type Medium =
  | { readonly kind: "march"; readonly density: Role; readonly within: readonly [V3, V3]; readonly steps: number; readonly shadowSteps: number }
  | { readonly kind: "gaussians"; readonly table: TableSource | Row };
export type Camera =
  | { readonly name: string; readonly kind: "look"; readonly eye: Expr<"vec3">; readonly target: Expr<"vec3">; readonly up: Expr<"vec3">; readonly fovy: Expr<"f32"> }
  | { readonly name: string; readonly kind: "matrix"; readonly cols: readonly [Expr<"vec4">, Expr<"vec4">, Expr<"vec4">, Expr<"vec4">] };
/** A view pass: its draws in order, seen by its camera. */
export type Pass = {
  readonly camera: Camera;
  readonly draws: readonly Draw[];
  readonly clear: { readonly kind: "load" } | { readonly kind: "colour"; readonly rgba: readonly [number, number, number, number] };
  readonly ground: { readonly kind: "none" } | { readonly kind: "traced"; readonly hit: Role; readonly paint: Role };
  readonly outline: { readonly kind: "none" } | { readonly kind: "ink"; readonly outline: Role };
};

export type Check = { readonly label: string; readonly how: { readonly kind: "prove"; readonly boxes: number } | { readonly kind: "sample"; readonly n: number }; readonly body: Formula };
export type Formula =
  | { readonly kind: "atom"; readonly margin: Expr<"f32">; readonly test: Expr<"bool"> }
  | { readonly kind: "always" | "eventually"; readonly leaves: readonly Leaf[]; readonly body: Formula }
  | { readonly kind: "and" | "or"; readonly parts: readonly Formula[] }
  | { readonly kind: "not"; readonly part: Formula };
/** A leaf (time, a param, an input or a free variable) and its range. */
export type Leaf = { readonly leaf: Expr<"f32">; readonly lo: Expr<"f32">; readonly hi: Expr<"f32"> };

/** src/geom/solve.ts `SolveSpec`, as the solver reads it. An unknown is read through its four jets: its value and first three time derivatives. */
export type Solve = {
  readonly label: string;
  readonly over: readonly [number, number];
  readonly delta: number;
  readonly jerk: number;
  readonly acceleration: number;
  readonly nearWeight: number;
  readonly unknowns: readonly { readonly name: string; readonly numbers: number; readonly from: Expr<"f32">; readonly jets: readonly [Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">] }[];
  readonly near: readonly { readonly unknown: number; readonly to: Expr<"f32"> }[];
  readonly holds: readonly Hold[];
};
export type Times = { readonly kind: "at"; readonly t: number } | { readonly kind: "during"; readonly from: number; readonly to: number } | { readonly kind: "span" };
export type Hold = { readonly times: Times; readonly said: string; readonly site: string } & (
  | { readonly kind: "equal"; readonly margin: Expr<NumTy>; readonly within: number }
  | { readonly kind: "bound"; readonly margin: Expr<"f32"> }
);

/** Everything scene-level a job carries: the named cameras a host may view by (each written once, passes' too), the
 * passes (their media and draws are reached from them), checks and solves. */
export type RowStep = {
  readonly source: string; readonly plan: DataTable; readonly seed: DataTable; readonly output: DataTable;
  readonly input: InputSpec; readonly hz: number; readonly ticks: number;
  readonly home: { kind: "level" } | { kind: "script"; samples: DataTable } | { kind: "events"; slots: number };
};
export type Scene = { readonly rowSteps?: readonly RowStep[]; readonly cameras: readonly Camera[]; readonly passes: readonly Pass[]; readonly checks: readonly Check[]; readonly solves: readonly Solve[] };
export const NO_SCENE: Scene = { cameras: [], passes: [], checks: [], solves: [] };

/** A scene given as explicit passes. The Rust build consumes these declarations directly. */
export function recorded(scene: Scene) { return { kind: "recorded" as const, scene }; }

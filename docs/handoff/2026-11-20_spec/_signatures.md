# Current Loft TS API: every exported signature, first line (generated 2026-11-20 from main)

Engine internals (cli, host, compile, bake, record, core, frame, text) are omitted; this is the author-facing library.

## src/artefact.ts
```ts
export const CURVE_N = 64;
export const DETAIL_SLOTS = 4096;
export const PRELUDE_WGSL = /* wgsl */ `
export const SCENE_DEPTH_WGSL = /* wgsl */ `
export const CLEARANCE_TILE = 8;
export const CLEARANCE_MAX = 49;
export const CLEARANCE_SCALE = 64;
export const SCENE_CLEARANCE_WGSL = /* wgsl */ `
export const POST_WGSL = /* wgsl */ `${SCENE_DEPTH_WGSL}
export type Camera =
export type InputSlot = { readonly name: string; readonly ty: NumTy; readonly slot: number; readonly def: readonly number[] };
export type ParamSlot = { readonly name: string; readonly slot: number; readonly def: number; readonly min: number; readonly max: number };
export type DynamicBuffer = { readonly name: string; readonly binding: number; readonly vec4s: number };
export type StaticBuffer = { readonly name: string; readonly binding: number; readonly data: Float32Array };
export const POSE_WORKGROUP = 64;
export type PoseStage = { readonly label: string; readonly wgsl: string; readonly invocations: number };
export type TableFillKind =
export type BakeRecord =
export class Baked
export type Recurrence =
export type ReplayClock = { readonly slot: number; readonly rate: number; readonly start: number; readonly steps: number; readonly every: number };
export function replayClocks(rs: readonly Recurrence[] | undefined): ReplayClock[]
export function replayStep(r: ReplayClock, t: number): { readonly n: number; readonly k: number }
export type TableInfo = { readonly name: string; readonly buffer: string; readonly offset: number; readonly vec4s: number; readonly lanes: number; readonly fill: TableFillKind };
export type CpuUpload = { readonly buffer: string; readonly offset: number; readonly data: Float32Array };
export type Detail =
export const HYSTERESIS = 1.25;
export type Draw =
export type DrawDepth =
export type TextureSize = { readonly kind: "view"; readonly scale: number } | { readonly kind: "fixed"; readonly width: number; readonly height: number };
export type ArtefactPass =
export const PASS_CAMERA_VEC4S = 12;
export function passBindingsWgsl(passes: readonly ArtefactPass[]): string
export type PostPass = { readonly label: string; readonly shader: string };
export type AuthorLines = { readonly from: number; readonly lines: number; readonly file: string; readonly line: number; readonly what: string };
export function authorLinesProblem(x: unknown): string | undefined
export function authorLineAt(spans: readonly AuthorLines[], n: number): { readonly file: string; readonly line: number; readonly what: string } | undefined
export type FrameState = { readonly time: number; readonly inputs: Float32Array; readonly params: Float32Array; readonly curves: Float32Array };
export type Artefact =
export const LIGHT_SAMPLE_INPUT = "loft_xi";
export type View =
export const CELL_PIXELS = 7;
export type FrameStats = { readonly triangles: number; readonly draws: number };
export interface Host
export interface Loaded
export function handPose(dynamic: readonly DynamicBuffer[], pose: (state: FrameState) => Readonly<Record<string, Float32Array>>, why: string): Pick<Artefact, "stages" | "tables" | "cpuPose" | "pose">
export type Loop = { readonly kind: "open" } | { readonly kind: "every"; readonly seconds: number };
export const OPEN: Loop = { kind: "open" };
export function loopTime(l: Loop, t: number): number
```

## src/authored.ts
```ts
export type AuthorText = { readonly text: string; readonly file: string; readonly line: number; readonly what: string };
export function authored(t: AuthorText): void
export function authorLines(code: string): AuthorLines[]
```

## src/check.ts
```ts
export type Verdict =
export function checkOutcome(v: Verdict): "pass" | "fail" | "undecided"
export class Probe
export interface Check
export type Measured = Body | Part;
export function footprintShare(chain: readonly Joint[], outside: (x: number, z: number) => boolean, step: number): { readonly share: number; readonly area: number }
export function clearance(body: Measured, target: Target): Expr<"f32">
export type CheckTime = In<"f32"> | Arming;
export function landsInside(strike: Strike, o: { readonly tolerance?: number; readonly step?: number } = {}): Check
export function litBy(light: Light, feature: Measured, o: { readonly least: number; readonly from?: number; readonly to?: number; readonly at?: CheckTime }): Check
export function minScreenSize(feature: Measured, camera: string, pixels: number, o: { readonly at?: CheckTime; readonly size?: readonly [number, number] } = {}): Check
export type Mark = { readonly from: In<"vec3">; readonly to: In<"vec3"> };
export function expressionsRead<K extends string, N extends string>(score: Score<K, N>, marks: Readonly<Record<string, Mark>>, o:
export function spansOnScreen(what: string, mark: Mark, camera: string, pixels: number, o: { readonly at: CheckTime; readonly size?: readonly [number, number] }): Check
export function staysInView(points: Readonly<Record<string, In<"vec3">>>, blockers: readonly { readonly name: string; readonly skeletons: readonly Skeleton[] }[], o:
export type Tolerance = { readonly kind: "units"; readonly units: number } | { readonly kind: "share"; readonly share: number };
export const tolerance =
export type Reaching = { readonly name: string; readonly limb: ChainSpec; readonly outward: In<"vec3"> };
export function limbsReach(limbs: readonly Reaching[], o: { readonly period: number; readonly slack: Tolerance; readonly fold: Tolerance; readonly rate?: number }): Check
export function gripReaches(attack: Attack<Motion, Hold>, fingers: readonly [Measured, Measured], o: { readonly middle: Tolerance; readonly gap: Tolerance; readonly rate?: number }): Check
export function warned(attack: Attack): Check
export function releasesAt(attack: Attack<Motion, Hold<Thrown>>, o: { readonly speed: Tolerance; readonly place: Tolerance }): Check
export function throwLands(attack: Attack<Motion, Hold<Thrown>>, o: { readonly least: number; readonly speed: Tolerance; readonly place: Tolerance; readonly rate?: number }): Check
export function fliesClear(attack: Attack<Motion, Hold<Thrown>>, targets: readonly Target[], o: { readonly size: number; readonly margin?: number; readonly rate?: number }): Check
export function letsGoClean(attack: Attack<Motion, Hold<Thrown>>, fingers: readonly Measured[], thing: Target, o: { readonly seconds: number; readonly depth: Tolerance; readonly rate?: number }): Check
export type Ringing = Attack<Motion, Effect, Ring> | { readonly name: string; readonly wave: RingWave; readonly first: In<"f32"> };
export function shoves(ringing: Ringing, place: In<"vec2">, o: { readonly least: number; readonly speed: Tolerance; readonly place: Tolerance }): Check
export function frontFirst(ringing: Ringing, places: readonly In<"vec2">[], o: { readonly rate?: number } = {}): Check
export function budget(o: { readonly triangles: number }): Check
export function runChecks(scene: Scene, checks: readonly Check[], o: { readonly inputs?: Inputs; readonly compiled?: Compiled } = {}): Verdict[]
export function formatChecks(vs: readonly ReturnType<Check["run"]>[]): string
```

## src/check_state.ts
```ts
export function stateChecks(probe: Probe, loop: Loop): ReturnType<Check["run"]>[]
```

## src/compute.ts
```ts
export type ComputeResource = Shape &
export type ComputePass =
export function resourceError(where: string, given: string, expected: string, fix: string): never
export function keptStates(s: Steps): number { return 1 + Math.ceil(s.count / s.keep); }
export function passLibs(code: EntryText): LibFn[]
export function compute<const W extends Readonly<Record<string, Shape>>>(name: string, spec: Spec<W>): { readonly [K in keyof W]: Handle<W[K]> }
export function orderCompute(given: readonly object[] | undefined): ComputePass[]
```

## src/core.ts
```ts
export type { Artefact } from "./artefact.ts";
export type { Camera } from "./artefact.ts";
```

## src/curve.ts
```ts
export type Curve<T extends "f32" | "vec2" | "vec3" | "vec4" = "vec4"> = (x: In<"f32">) => Expr<T>;
export type Key = readonly [number, number | readonly number[]];
export type Interp = keyof typeof INTERP;
export function keys(points: readonly Key[], options: { interp?: Interp; label?: string } = {}): Curve<"vec4">
export function ramp(colours: readonly (readonly [number, number, number])[], label = "ramp"): Curve<"vec4">
export function through(m: In<"f32">, points: readonly In<"vec3">[]): Expr<"vec3">;
export function through(m: In<"f32">, count: number, point: (j: Expr<"f32">) => In<"vec3">): Expr<"vec3">;
export function through(m: In<"f32">, points: readonly In<"vec3">[] | number, point?: (j: Expr<"f32">) => In<"vec3">): Expr<"vec3">
export function bspline(t: In<"f32">, over: readonly [number, number], values: readonly In<"f32">[]): Expr<"f32">
export type WaveSpec =
export function wave(m: In<"f32">, spec: WaveSpec): Expr<"vec3">
export function retarded(t: In<"f32">, m: In<"f32">, speed: In<"f32">): Expr<"f32">
export type ResponseSpec =
export function response(t: In<"f32">, spec: ResponseSpec): Expr<"f32">
export function pulse(t: In<"f32">, t0: In<"f32">, t1: In<"f32">, ease?: In<"f32">): Expr<"f32">
export const ease = (x: In<"f32">, a: In<"f32">, b: In<"f32">): Expr<"f32"> => smoothstep(a, b, x);
```

## src/design.ts
```ts
export type Overrides<N> = { readonly [K in keyof N]?: N[K] extends readonly unknown[] ? N[K] : N[K] extends object ? Overrides<N[K]> : N[K] };
export type Widen<T> = T extends number ? number : T extends readonly unknown[] ? { readonly [K in keyof T]: Widen<T[K]> } : T extends object ? { readonly [K in keyof T]: Widen<T[K]> } : T;
export type Option<N> = { readonly about: string; readonly numbers?: Overrides<N> };
export type SlotTable<N> = { readonly [slot: string]: { readonly base: Option<N>; readonly [option: string]: Option<N> } };
export type Choice<T> = { readonly [S in keyof T]: keyof T[S] & string };
export type Picked<T> = { readonly [S in keyof T]: T[S][keyof T[S]] };
export type VariantSpec<N, T> = { readonly about: string; readonly pick?: Partial<Choice<T>>; readonly numbers?: Overrides<N> };
export type Built = { readonly scene: Scene; readonly checks: readonly Check[] };
export type Member = { readonly things: readonly Drawable[]; readonly checks: readonly Check[] };
export type Staged<M extends Member = Member> = { readonly deforms: readonly SeaDeform[]; readonly sea: SeaSpec; readonly finish: (water: Sea) => M };
export type DesignSpec<N, T extends SlotTable<N>, B extends Built = Built, H = never, M extends Member = Member> =
export type VariantAsk = { readonly variant: string; readonly picks: readonly (readonly [slot: string, option: string])[] };
export const BASE_ASK: VariantAsk = { variant: "base", picks: [] };
export type Resolved<N = unknown> =
export type DesignErrorKind = "syntax" | "unknown-variant" | "unknown-slot" | "unknown-option" | "unknown-number" | "conflict";
export class DesignError extends Error
export function parseVariantAsk(text: string): VariantAsk
export class Design<N, T extends SlotTable<N>, B extends Built = Built, H = never, M extends Member = Member>
export function design<N, T extends SlotTable<N>, B extends Built = Built, H = never, M extends Member = Member>(spec: DesignSpec<N, T, B, H, M>): Design<N, T, B, H, M>
export type AnyDesign = Design<unknown, SlotTable<unknown>>;
export type Subject =
export function subjectOf(mod: Record<string, unknown>, path: string): Subject
```

## src/errors.ts
```ts
export class SceneError extends Error
export class LoadError extends Error
export class EnvironmentError extends Error
export function shown(x: unknown): string
export function siteOf(stack: string | undefined): string | undefined
export function sitesOf(stack: string | undefined): string[]
export class RawComputeError extends Error
```

## src/expr.ts
```ts
export type Ty = "f32" | "vec2" | "vec3" | "vec4" | "bool";
export type NumTy = Exclude<Ty, "bool">;
export type VecTy = Exclude<NumTy, "f32">;
export class Row
export type ParamSpec = { readonly name: string; readonly def: number; readonly min: number; readonly max: number };
export type InputSpec = { readonly name: string; readonly ty: NumTy; readonly def: readonly number[] };
export type Fn =
export type LibFn =
export type LibRead =
export type TableSource = ProgramTable | DataTable | RecurrenceTable;
export type ProgramTable = TableShape & { readonly program: TableProgram };
export type DataTable = TableShape & { readonly data: Float32Array };
export type RecurrenceTable = TableShape & { readonly recurrence: string };
export type MapRun = { readonly count: number; readonly lanes: (i: Expr<"f32">) => readonly Expr<"vec4">[] };
export type ScanStep = { readonly next: readonly Expr<NumTy>[]; readonly lanes: readonly Expr<"vec4">[] };
export type TableProgram =
export const mapOf = (entries: number, lanes: MapRun["lanes"]): TableProgram => ({ kind: "map", runs: [{ count: entries, lanes }] });
export const listOf = (entries: readonly (readonly Expr<"vec4">[])[]): TableProgram => ({ kind: "map", runs: entries.map((e) => ({ count: 1, lanes: () => e })) });
export type TableFill = (frame: TableFrame, out: Float32Array) => void;
export interface TableFrame
export type Loop =
export type Node =
export type Lit = readonly number[];
export type In<T extends Ty = Ty> = Expr<T> | (T extends "f32" ? number : T extends "bool" ? boolean : readonly number[]);
export type Num = Expr<NumTy> | number | readonly number[];
export class Expr<T extends Ty = Ty>
export function lift<T extends Ty>(x: In<T>): Expr<T>
export const f = (x: number): Expr<"f32"> => lift<"f32">(x);
export function variable<T extends Ty>(name: string, ty: T): Expr<T>
export function namespaced<T>(id: string, build: () => T): T
export const currentNamespace = (): string => namespace;
export function namespaceSeed(): number
export function scopedName(name: string): string
export function input<T extends NumTy>(given: string, ty: T, def: readonly number[]): Expr<T>
export const time: Expr<"f32"> = input("time", "f32", [0]);
export function param(name: string, def: number, range?: { min?: number; max?: number }): Expr<"f32">
export function row(r: Row, x: In<"f32">): Expr<"vec4">
export const rawCall = Symbol("raw call");
export type RawCall = { readonly lib: LibFn; readonly trailing: readonly (number | Expr)[] };
export function fetch(table: TableSource, index: In<"f32">, lane: number): Expr<"vec4">
export function iterate<const S extends readonly Expr<NumTy>[]>(count: In<"f32">, init: S, step: (state: S, i: Expr<"f32">) => S, until?: (state: S) => Expr<"bool">): S
export function rawComputeOf(fn: LibFn): string | undefined { return rawCompute.get(fn)?.resource; }
export function rawComputeExpression(fn: LibFn, resource: string, ...args: In[]): Expr<"vec4">
export function markRaw(fn: LibFn, reason: string): void { rawReasons.set(fn, reason); }
export function rawReasonOf(fn: LibFn): string | undefined { return rawReasons.get(fn); }
export function lib<T extends Ty>(fn: LibFn, ...args: In[]): Expr<T>
export const vec2 = (...p: Num[]): Expr<"vec2"> => vecOf("vec2", p) as Expr<"vec2">;
export const vec3 = (...p: Num[]): Expr<"vec3"> => vecOf("vec3", p) as Expr<"vec3">;
export const vec4 = (...p: Num[]): Expr<"vec4"> => vecOf("vec4", p) as Expr<"vec4">;
export const identitiesOn = (): boolean => identities > 0;
export function withIdentities<T>(f: () => T): T
export const add: Assoc = assoc("add", (a: In<NumTy>, b: In<NumTy>) =>
export const sub = binary("sub");
export const mul: Assoc = assoc("mul", (a: In<NumTy>, b: In<NumTy>) =>
export const div = binary("div");
export const min: Assoc = assoc("min", binary("min"));
export const max: Assoc = assoc("max", binary("max"));
export const pow = binary("pow");
export const mod = binary("mod");
export const atan2 = binary("atan2");
export const step = binary("step");
export const neg = unary("neg");
export const abs = unary("abs");
export const sign = unary("sign");
export const floor = unary("floor");
export const fract = unary("fract");
export const sqrt = unary("sqrt");
export const exp = unary("exp");
export const log = unary("log");
export const sin = unary("sin");
export const cos = unary("cos");
export const tan = unary("tan");
export const saturate = unary("saturate");
export const normalize = unary("normalize");
export const ceil = unary("ceil");
export const trunc = unary("trunc");
export const roundEven = unary("roundEven");
export const rem = binary("rem");
export const inverseSqrt = unary("inverseSqrt");
export const exp2 = unary("exp2");
export const log2 = unary("log2");
export const asin = unary("asin");
export const acos = unary("acos");
export const atan = unary("atan");
export const sinh = unary("sinh");
export const cosh = unary("cosh");
export const tanh = unary("tanh");
export function mix(a: number, b: number, t: In<NumTy>): Expr<"f32">;
export function mix<A extends NumTy, B extends NumTy>(a: Expr<A>, b: Expr<B>, t: In<NumTy>): Expr<Wider<A, B>>;
export function mix<A extends NumTy>(a: Expr<A>, b: number, t: In<NumTy>): Expr<A>;
export function mix<B extends NumTy>(a: number, b: Expr<B>, t: In<NumTy>): Expr<B>;
export function mix<A extends NumTy>(a: Expr<A>, b: Lit, t: In<NumTy>): Expr<A extends "f32" ? VecTy : A>;
export function mix<B extends NumTy>(a: Lit, b: Expr<B>, t: In<NumTy>): Expr<B extends "f32" ? VecTy : B>;
export function mix(a: Lit, b: Lit, t: In<NumTy>): Expr<VecTy>;
export function mix<A extends NumTy = "f32", B extends NumTy = "f32">(a: Expr<A> | number | Lit, b: Expr<B> | number | Lit, t: In<NumTy>): Expr<Wider<A, B>>;
export function mix(a: In<NumTy>, b: In<NumTy>, t: In<NumTy>): Expr
export function clamp(x: number, lo: number, hi: number): Expr<"f32">;
export function clamp<A extends NumTy>(x: Expr<A>, lo: In<NumTy>, hi: In<NumTy>): Expr<A>;
export function clamp(x: Lit, lo: In<NumTy>, hi: In<NumTy>): Expr<VecTy>;
export function clamp<A extends NumTy = "f32">(x: Expr<A> | number | Lit, lo: In<NumTy>, hi: In<NumTy>): Expr<A>;
export function clamp(x: In<NumTy>, lo: In<NumTy>, hi: In<NumTy>): Expr
export function smoothstep(lo: number, hi: number, x: number): Expr<"f32">;
export function smoothstep<A extends NumTy>(lo: In<NumTy>, hi: In<NumTy>, x: Expr<A>): Expr<A>;
export function smoothstep<A extends NumTy = "f32">(lo: In<NumTy>, hi: In<NumTy>, x: Expr<A> | number | Lit): Expr<A>;
export function smoothstep(lo: In<NumTy>, hi: In<NumTy>, x: In<NumTy>): Expr
export const length = (a: In<NumTy>): Expr<"f32"> => call("length", "f32", [a]) as Expr<"f32">;
export const distance = (a: In<NumTy>, b: In<NumTy>): Expr<"f32"> => call("distance", "f32", [a, b]) as Expr<"f32">;
export const dot = (a: In<NumTy>, b: In<NumTy>): Expr<"f32"> => call("dot", "f32", [a, b]) as Expr<"f32">;
export const cross = (a: In<"vec3">, b: In<"vec3">): Expr<"vec3"> => call("cross", "vec3", [a, b]) as Expr<"vec3">;
export const lt = compare("lt");
export const gt = compare("gt");
export const le = compare("le");
export const ge = compare("ge");
export const eq = compare("eq"), ne = compare("ne");
export const and = (a: In<"bool">, b: In<"bool">): Expr<"bool"> => call("and", "bool", [a, b]) as Expr<"bool">;
export const or = (a: In<"bool">, b: In<"bool">): Expr<"bool"> => call("or", "bool", [a, b]) as Expr<"bool">;
export const not = (a: In<"bool">): Expr<"bool"> => call("not", "bool", [a]) as Expr<"bool">;
export function select(cond: In<"bool">, yes: number, no: number): Expr<"f32">;
export function select<A extends NumTy, B extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: Expr<B>): Expr<Wider<A, B>>;
export function select<A extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: number): Expr<A>;
export function select<B extends NumTy>(cond: In<"bool">, yes: number, no: Expr<B>): Expr<B>;
export function select<A extends NumTy>(cond: In<"bool">, yes: Expr<A>, no: Lit): Expr<A extends "f32" ? VecTy : A>;
export function select<B extends NumTy>(cond: In<"bool">, yes: Lit, no: Expr<B>): Expr<B extends "f32" ? VecTy : B>;
export function select<A extends NumTy = "f32", B extends NumTy = "f32">(cond: In<"bool">, yes: Expr<A> | number | Lit, no: Expr<B> | number | Lit): Expr<Wider<A, B>>;
export function select(cond: In<"bool">, yes: In<NumTy>, no: In<NumTy>): Expr
export function pick<T extends NumTy>(i: In<"f32">, values: readonly Expr<T>[]): Expr<T>;
export function pick(i: In<"f32">, values: readonly number[]): Expr<"f32">;
export function pick(i: In<"f32">, values: readonly (Expr<NumTy> | number)[]): Expr
export function round<A extends NumTy = "f32">(x: Expr<A> | number | Lit): Expr<A> { return floor(add(x as Expr<A>, 0.5)) as Expr<A>; }
export function swizzle<T extends NumTy>(a: Expr<VecTy>, sel: string): Expr<T> { return get<T>(a, sel); }
export const TAU = Math.PI * 2;
export function children(e: Expr): readonly Expr[]
export function everything(roots: readonly Expr[]): Expr[]
export function topo(roots: readonly Expr[]): Expr[]
export function splitOn(roots: readonly Expr[], vars: readonly Expr[]): { inner: Expr[]; outer: Expr[] }
export function leaves(roots: readonly Expr[]): { vars: Map<string, Ty>; inputs: InputSpec[]; params: ParamSpec[]; rows: Row[]; tables: TableSource[]; libs: LibFn[] }
export function libsOf(roots: readonly Expr[]): LibFn[]
export type Bind =
export const WGSL_TY: Readonly<Record<Ty, string>> = { f32: "f32", vec2: "vec2<f32>", vec3: "vec3<f32>", vec4: "vec4<f32>", bool: "bool" };
export function noteStatements(note: (e: Expr) => string | undefined): void { statementNote = note; }
export function emitWgsl(roots: readonly Expr[], bind: Bind, prefix = "e", pre: ReadonlyMap<number, string> = new Map(), comment?: (e: Expr) => string | undefined): { lines: string[]; out: string[]; libs: LibFn[]; names:
export function structuralKey(roots: readonly Expr[]): string
export function structuralHash(roots: readonly Expr[]): string
export function recordStacks(): void
export function stackOf(e: Expr): string | undefined { return (stacks?.get(e) ?? stacks?.get(rewrittenFrom(e)))?.stack; }
export function rewrittenFrom(e: Expr): Expr
export function substitute(roots: readonly Expr[], from: Expr, to: Expr): Expr[]
export function replace(roots: readonly Expr[], by: ReadonlyMap<number, Expr>): Expr[]
export type V = number | number[];
export type RowSource =
export type Env =
export function compileJs(roots: readonly Expr[], rows: RowSource = OWN_ROWS): (env: Env) => (V | boolean)[]
export function fold(roots: readonly Expr[]): Expr[]
export function evaluate(e: Expr, env: Partial<Env> = {}): V | boolean
```

## src/grad.ts
```ts
export type DerivativeFailure =
export class DerivativeError extends Error
export type DerivativeRule = (args: readonly Expr[], dargs: readonly Expr[]) => Expr;
export function defineDerivative(fn: LibFn, rule: DerivativeRule): void
export function derivative<T extends NumTy>(e: Expr<T>, x: Expr<"f32">): Expr<T>
export function derivativeAlong<T extends NumTy, P extends NumTy>(e: Expr<T>, p: Expr<P>, dir: In<P>): Expr<T>
export function gradient(e: Expr<"f32">, p: Expr<"vec2">): Expr<"vec2">;
export function gradient(e: Expr<"f32">, p: Expr<"vec3">): Expr<"vec3">;
export function gradient(e: Expr<"f32">, p: Expr<"vec2" | "vec3">): Expr<"vec2" | "vec3">
export function jacobian<T extends VecTy>(fx: Expr<T>, p: Expr<NumTy>): Expr<T>[]
export function normalOf(surface: Expr<"vec3">, u: Expr<"f32">, v: Expr<"f32">): Expr<"vec3">
export function stretchOf(map: Expr<"vec2" | "vec3">, p: Expr<"vec2" | "vec3">): Expr<"f32">
export type ScreenRate = { readonly of: Expr<NumTy>; readonly dx: In<NumTy>; readonly dy: In<NumTy> };
export type Footprint<T extends NumTy> = { readonly dx: Expr<T>; readonly dy: Expr<T>; readonly width: Expr<T> };
export function footprint<T extends NumTy>(coord: Expr<T>, px: readonly ScreenRate[]): Footprint<T>
export function noise2Expr(p: E): E
export function noise3Expr(p: E): E
```

## src/identity.ts
```ts
export type Program =
export type FrameCamera =
export type Identity =
export function identityId(i: Identity): string
export function canonical(v: unknown): string
export function programOf(bundle: Bundle): Program
export function libraryOf(): string
export function buildOf(bundle: Bundle): Identity["build"]
export function deviceOf(device: GPUDevice): Identity["device"]
export function frameOf(m: Pick<Manifest, "params" | "inputs">, view: View, precision: FramePrecision, drawn: StillSpec): Identity["frame"]
```

## src/index.ts
```ts
export type { Posed } from "./motion.ts";
```

## src/interval.ts
```ts
export type Tri = "true" | "false" | "either";
export class Unbounded extends Error
export type Refusal =
export class RangeRefusal extends SceneError
export type I = readonly [number, number];
export function bounds(x: C): I
export type Box = ReadonlyMap<string, I | Track>;
export type Track = { readonly centre: number; readonly slopes: ReadonlyMap<string, number> };
export type Work = { tableFills?: Compiled["tableFills"]; nodes: number; codomain: number; checkTime?: () => void; charge?: () => void };
export class Evaluator
```

## src/light.ts
```ts
export type V3 = readonly [number, number, number];
export type Sun =
export function movable(dir: V3, name = "sun"): { readonly kind: "input"; readonly name: string; readonly dir: V3 }
export function sunDir(s: Sun): V3 | Expr<"vec3">
export function sunParts(s: Sun): readonly [In<"f32">, In<"f32">, In<"f32">]
export type Arrival =
export type Arrived = Arrival & { readonly unshadowed: In<"vec3">; readonly visibility: In<"f32"> };
export type LightSource = { readonly kind: "point"; readonly at: Expr<"vec3"> } | { readonly kind: "direction"; readonly toward: In<"vec3"> };
export type LampShadow = { readonly at: Expr<"vec3">; readonly reach: number; readonly size: number };
export type Receiver = { readonly kind: "surface"; readonly at: Expr<"vec3">; readonly normal: Expr<"vec3"> } | { readonly kind: "point"; readonly at: Expr<"vec3"> };
export type ShadowTest = (r: Receiver, lit: Arrival) => Expr<"f32">;
export type Light = ((p: Expr<"vec3">, xi?: Expr<"vec2">) => Arrival) &
export function lamp(at: In<"vec3">, o: { readonly colour: In<"vec3">; readonly reach: number; readonly size?: number; readonly shadow?: boolean }): Light
export type SunAmount = { readonly kind: "full" } | { readonly kind: "given"; readonly amount: In<"vec3"> };
export type Sampling = { readonly kind: "point" } | { readonly kind: "sampled" };
export type Lighting = { readonly sun: Sun; readonly amount: SunAmount; readonly lights: readonly Light[]; readonly sampling: Sampling; readonly shadows: ReadonlyMap<Light, ShadowTest> };
export type Combine = "sum" | "largest";
export type Which = "sun" | "local";
export type LightFold = <T extends NumTy>(combine: Combine, term: (lit: Arrived, which: Which) => Expr<T>) => Expr<T>;
export function sunArrival(l: Lighting): Arrived
export function foldAt(l: Lighting, r: Receiver): LightFold
export const sunOnly = (l: Lighting): Lighting => ({ ...l, lights: [] });
```

## src/lineup.ts
```ts
export type Standing<N, H, M extends Member> =
export type Stageable<N, H, M extends Member> =
export type Lineup<M extends Member> =
export type LineupLoop = { readonly kind: "open" } | { readonly kind: "every"; readonly seconds: number };
export function lineup<M extends Member>(o: { readonly members: readonly Standing<unknown, unknown, M>[]; readonly water: Omit<SeaSpec, "deform">; readonly loop: LineupLoop }): Lineup<M>
```

## src/mistakes.ts
```ts
export function refuseSurface(name: string, surface: unknown, grid: unknown, look: { readonly kind: string; readonly sides?: unknown; readonly outline?: unknown }, point: () => unknown, paint: (() => unknown) | undefined
export function refuseScene(spec: { readonly ground?: unknown; readonly things?: unknown; readonly sun?: unknown; readonly cameras?: unknown }): void
```

## src/precision.ts
```ts
export const shadingTables = new WeakSet<TableSource>();
```

## src/ranged.ts
```ts
export type Range = readonly [Expr<"f32">, In<"f32">, In<"f32">];
export type Decision =
export type Spent =
export interface Ranged extends Check
export type Leaf = { readonly key: string; readonly label: string; readonly lo: Expr<"f32">; readonly hi: Expr<"f32"> };
export type Quantified = { readonly kind: "always" | "eventually"; readonly leaves: readonly Leaf[]; readonly body: Body; readonly label: string };
export type Settings =
export function measured(what: string, at: Measure["at"]): Expr<"f32">
export function measuredIn(roots: readonly Expr[]): Measure[]
export function always(range: Range, condition: Condition, o: Settings = {}): Ranged
export function eventually(range: Range, condition: Condition, o: Settings = {}): Ranged
export type Pinned =
export function pinned(check: Ranged, at: ReadonlyMap<string, number>, probe: Probe): Pinned
export function heldBy(probe: Probe): ConstructorParameters<typeof Evaluator>[1]
```

## src/scene.ts
```ts
export type SceneSpec =
export const MOST_LIGHTS = 7;
export type OutlineMethod = "hull" | "screen";
export const DEFAULT_OUTLINE: OutlineMethod = "hull";
export type Scene =
export function scene(spec: SceneSpec): Scene
export function restyled(scene: Scene, style: Style): Scene
```

## src/style.ts
```ts
export type Rgb = readonly [number, number, number];
export type ShadeCtx =
export type WaterCtx =
export type Contact =
export type LayerCtx =
export type SkyCtx =
export type GroundCtx =
export type ShadowBlock =
export type BlockCast =
export type BlockOverlap =
export type VolumeCtx =
export type Style =
export function dorsalness(v: Expr<"f32">): Expr<"f32">
export const palette =
export type Said = { readonly channels: ReadonlyMap<string, Expr<NumTy>>; readonly scatter: ((ctx: ShadeCtx, toward: Expr<"vec3">) => Expr<"vec3">) | undefined };
export type Reads = { readonly body: boolean; readonly channels: ReadonlyMap<string, NumTy>; readonly scatter: boolean; readonly geometry: boolean };
export function styleReads(style: Style, kind: ShadeCtx["geometry"]["kind"] = "unoccluded"): Reads
export function readsBody(style: Style): boolean
export type ShadeFields = Omit<ShadeCtx, "lights" | "channel" | "sky" | "scatter" | "geometry">;
export function shadeContext(f: ShadeFields, lighting: Lighting, style: Style, about: About = NOTHING_SAID, record: Recorder = NO_RECORD): ShadeCtx
export function shadeVariables(lighting: Lighting, style: Style, about: About = NOTHING_SAID): ShadeCtx
```

## src/draw/cast.ts
```ts
export const CAST_LIB: LibFn = { name: "loft_cast", ret: "vec4", wgsl: "", js: "gpu-only" };
export function castRay(ray: Ray): Traced & { readonly colour: P3; readonly id: Expr<"f32">; readonly m: Expr<"f32"> }
export type ChainInfo = { readonly first: number; readonly joints: number; readonly thing: number; readonly copy: number };
export type CastPlan = { readonly libs: readonly LibFn[]; wgsl(cpu: (roots: readonly Expr[]) => CpuFn, rest: Parameters<CpuFn>[0]): string };
export function castPlan(ctx: CompileContext, used: readonly LibFn[]): CastPlan | undefined
export function chainSpheres(things: readonly Drawable[]): { readonly infos: readonly ChainInfo[]; readonly at: TableSource; readonly table: TableSource }
export type CastJoint = { readonly p: V3; readonly r: number };
export function castCpu(o: V3, d: V3, t0: number, t1: number, w0: number, w1: number, chains: readonly (readonly CastJoint[])[], ground: boolean): { readonly t: number; readonly gap: number; readonly chain: number; reado
```

## src/draw/copies.ts
```ts
export type CopiesHit = { readonly p: P3; readonly normal: P3; readonly level: F; readonly id: Expr<"vec3"> };
export type CopiesOptions =
export class Copies implements Drawable
export function drawCopies(name: string, o: CopiesOptions): Copies
```

## src/draw/field.ts
```ts
export const fieldPixel: F = variable("loft_field_px", "f32");
export type FieldSchedule = "trace" | "brick" | "mesh" | "auto";
export type FieldHit = { readonly p: P3; readonly normal: P3 };
export type FieldOptions =
export type FieldSafetyFailure =
export type FieldChecker = "draw.field" | "march";
export class FieldScheduleError extends SceneError
export class FieldSafetyError extends SceneError
export const LIPSCHITZ_LIMIT = 1.01;
export const BRICK_CELLS = 256;
export function measureField(field: Field, b: TraceBound): number
export function checkField(name: string, field: Field, b: TraceBound, by: FieldChecker = "draw.field"): number
export function evaluationCost(e: Expr<"f32">, centre: readonly [number, number, number]): number
export const BRICK_WORTH = 1000;
export class FieldDrawable implements Drawable
export function fieldMesh(name: string, field: Field, b: TraceBound): TriangleMesh
export function drawField(name: string, field: Field, o: FieldOptions): FieldDrawable
```

## src/draw/field_shadow.ts
```ts
export type FieldShadow =
export function fieldShadow(field: Field, b: TraceBound, px: Expr<"f32">): FieldShadow
export function fitBlob(pts: readonly { readonly p: V3; readonly w: number }[], minRadius: number): readonly { readonly p: V3; readonly r: number }[]
export function shadowSkeleton(s: FieldShadow): Skeleton
```

## src/draw/grid_detail.ts
```ts
export function gridLevels(nu: number, nv: number, topology: Topology): readonly (readonly [number, number])[]
export type BoundCopies = { readonly kind: "one" } | { readonly kind: "poses"; readonly poses: Poses };
export type GridAt = { readonly u: (s: F) => F; readonly v: (s: F) => F };
export function boundTable(name: string, surface: Surface, at: GridAt, copies: BoundCopies): TableSource
export const GRID_S = variable("loft_grid_s", "f32"), GRID_T = variable("loft_grid_t", "f32");
export function gridDetail(bound: TableSource, surface: Surface, at: GridAt, copies: BoundCopies, nu: number, nv: number): DetailSpec
export const REST_CENTRE = { loft_grid_s: 0.5, loft_grid_t: 0.5 };
export const restPoints: readonly { readonly loft_grid_s: number; readonly loft_grid_t: number }[] =
export function measureRest(samples: readonly (readonly number[])[], anchor: readonly number[]): { readonly lengths: readonly [number, number]; readonly radius: number }
```

## src/draw/ground.ts
```ts
export type Plane = Drawable & { readonly kind: "plane" };
export type Nothing = Drawable & { readonly kind: "nothing" };
export type Ground = Sea | Plane | Nothing;
export const BOTTOMLESS = -1e9;
export class GroundError extends SceneError
export function levelOf(ground: Ground): (xz: Expr<"vec2">, t?: In<"f32">) => F
export function surfaceOf(ground: Ground, who: string): (xz: Expr<"vec2">, t?: In<"f32">) => F
export function seaOf(ground: Ground, who: string): Sea
export const plane: Plane =
export const nothing: Nothing =
```

## src/draw/impostors.ts
```ts
export type Cell = { readonly id: Expr<"vec3">; readonly centre: P3; readonly hash: Expr<"vec3"> };
export type ImpostorShape =
export type ImpostorHit = { readonly p: P3; readonly normal: P3; readonly local: P3; readonly cell: Cell };
export type ImpostorOptions =
export function shapeWgsl(kind: ImpostorShape["kind"]): string
export const CLOSEST_WGSL = /* wgsl */ `
export class Impostors implements Drawable
export function impostors(name: string, o: ImpostorOptions): Impostors
```

## src/draw/index.ts
```ts
export type { DensityOptions };
export type { PointsOptions, Speck } from "../soft/specks.ts";
export type { VolumeOptions };
export type { ShaderOptions };
export interface Poseable
export type Bounds = { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] };
export type { FieldOptions } from "./field.ts";
export type { ReliefOptions } from "./relief.ts";
export type { CopiesHit, CopiesOptions } from "./copies.ts";
export type { MeshData, MeshDrawable, MeshOptions } from "./mesh.ts";
export type { Ray, Traced, TracedDrawable, TracedOptions } from "./traced.ts";
export type CurveOptions =
export const draw =
```

## src/draw/ink.ts
```ts
export const MARK_WGSL = /* wgsl */ `
export const INK_POST = /* wgsl */ `
```

## src/draw/light.ts
```ts
export const light: Drawable =
```

## src/draw/march.ts
```ts
export function march(field: Field, o: { readonly within: TraceBound; readonly steps: number; readonly safety?: number }): (ray: Ray) => Traced
```

## src/draw/mesh.ts
```ts
export type MeshData =
export type MeshOptions =
export type Normals =
export type Mesh =
export type MeshPaint = (row: readonly F[], at: PaintAt) => P3 | Painted;
export function classifyMesh(name: string, data: MeshData, at: MeshOptions["at"]): { readonly mesh: Mesh; readonly motion: Motion }
export function fromNets(m: TriangleMesh): Mesh
export type MeshEmit = { readonly key: string; readonly show: string; readonly gate: string };
export function emitMesh(ctx: CompileContext, name: string, mesh: Mesh, motion: Motion, copies: Copies, o: { readonly paint: MeshPaint; readonly sides: "one" | "two"; readonly outline: "hull" | "none" }, e: MeshEmit): Em
export class MeshDrawable implements Drawable
export function drawMesh(name: string, mesh: MeshData, o: MeshOptions): MeshDrawable
export function meshPoint(mesh: Mesh, at: MeshOptions["at"], copies: Poses | undefined): { readonly world: P3; readonly normal: P3; readonly rowVars: (i: number) => Record<string, number[]> }
export const MESH_PAINT_VARS = { row: (width: number) => paintRowOf(width), at: (): PaintAt => vars().AT };
```

## src/draw/painted.ts
```ts
export type Answer = { readonly kind: "styled" } | { readonly kind: "wrapped"; readonly wrap: (ctx: ShadeCtx, styled: P3) => P3 };
export type Classified = { readonly base: P3; readonly said: Said; readonly answer: Answer };
export function classify(thing: string, x: P3 | Painted): Classified
export type Shading = { readonly kind: "shared" } | { readonly kind: "own"; readonly thing: string; readonly shader: Shader };
export type Notes = { readonly unread: readonly string[]; readonly ignoresStyle: boolean };
export function shadeCall(s: Shading, fn: "shade" | "ink" | "mark", shared: string, paintCall: string): string
export function tracedCall(s: Shading, fn: "shade" | "ink" | "mark", base: string, n: string, p: string, d: string, id: string, paintCall: string): string
export function withShading(paint: Shader, s: Shading): Shader
export function lookFns(ctx: CompileContext, thing: string, c: Classified, paintArgs: FnSpec["args"], geometry?: ShadeCtx["geometry"]): Shading
export function answered(a: Answer, ctx: ShadeCtx, styled: P3): P3
```

## src/draw/record.ts
```ts
export type RecordContext = Pick<CompileContext, "style" | "ground" | "sun" | "sunAt" | "lighting" | "things" | "outline"> &
export type Staged = { readonly phase: Phase; readonly draw: Draw };
export type Recorded = { readonly kind: "draws"; readonly draws: readonly Staged[] } | { readonly kind: "unported"; readonly what: string };
export const NONE: Recorded = { kind: "draws", draws: [] };
export const unported = (what: string): Recorded => ({ kind: "unported", what });
export const seaFootprint = raw.sea_footprint_at! as (p: Expr<"vec3">) => F;
export const screenDir = raw.screen_dir_at! as (uv: Expr<"vec2">) => Expr<"vec3">;
export const pixelOf = raw.pixel_of_point! as (p: Expr<"vec3">) => Expr<"vec2">;
export function cellsRole(name: string, counts: readonly [number, number], grid: Extract<Detail, { kind: "grid" }> | undefined, bound: (i: F) => Expr<"vec4">): Role
export const copyId = (id: number, i: F): F => fract(add(id, mul(i, 0.618034)));
export function litContext(ctx: RecordContext, at: { base: P3; normal: P3; world: P3; uv: Expr<"vec2">; anchor: P3; id: F }, about?: Parameters<typeof shadeContext>[3]): ShadeCtx
export function inkWidth(style: Style, p: P3): F
export function skyOf(ctx: RecordContext, dir: Expr<"vec3">): Expr<"vec3">
export const paintRole = (name: string, f: (p: P3, n: P3, uv: Expr<"vec2">, id: F) => Expr<"vec4">): Role =>
export type RayBound = { readonly kind: "box"; readonly box: { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] } } | { readonly kind: "copies"; readonly count: number; rea
export function rayDraw(ctx: RecordContext, name: string, bound: RayBound, hit: (ray: Ray, copy: F) => Traced, paint: (p: P3, n: P3, d: P3, copy: F) => P3 | Painted): Recorded
export function skyDraw(ctx: RecordContext): Staged
export function cameraOf(name: string, c: SceneCamera): Camera
export function sceneDecls(scene: Scene, artefact: Pick<Artefact, "draws">): { readonly scene: Decls; readonly unported: readonly string[] }
```

## src/draw/relief.ts
```ts
export type ReliefSite = { readonly u: F; readonly v: F; readonly cell: Folded<"vec2"> };
export type ReliefPoint = ReliefSite & { readonly h: F };
export type ReliefOptions =
export class ReliefDrawable implements Drawable
export function drawRelief(name: string, surface: Surface, o: ReliefOptions): ReliefDrawable
```

## src/draw/shader.ts
```ts
export type ShaderOptions = Pick<DrawSpec, "vertices"> & Partial<Pick<DrawSpec, "instances" | "phase" | "cull" | "depthWrite" | "blend">>;
export function drawShader(name: string, module: WgslModule, o: ShaderOptions): Drawable
```

## src/draw/shadows.ts
```ts
export const SHADOWS_TABLE = "shadows";
export const shadows: Drawable =
export type Chain = readonly { readonly p: Expr<"vec3">; readonly r: F }[];
export function jointList(chains: readonly Chain[]): TableSource
export function casters(things: readonly Drawable[]): { readonly chains: readonly Chain[]; readonly at: TableSource }
export function shadowTable(chains: readonly Chain[], seaHeight: (xz: Expr<"vec2">) => F, sun: readonly [In<"f32">, In<"f32">, In<"f32">], at: TableSource = jointList(chains)): TableSource
```

## src/draw/sky.ts
```ts
export const sky: Drawable =
```

## src/draw/surface.ts
```ts
export type Sampling = { readonly count: number; readonly at: (s: F) => F };
export type Grid = { readonly u: Sampling; readonly v: Sampling };
export const spacing =
export type Fragment =
export type Outline = "hull" | "border" | "none";
export type Look =
export type GlazedColour =
export type GlazedDetail = { readonly kind: "fixed" } | Extract<DetailSpec, { kind: "projected" }>;
export class SurfaceDrawable implements Drawable
export function posedJoint(j: { readonly p: P3; readonly r: F }, at: Posed): { readonly p: P3; readonly r: F }
export function withFragment(c: GlazedColour, g: (f: Fragment) => Fragment): GlazedColour
export type SurfaceOptions = { readonly paint: Paint; readonly grid: Grid; readonly sides: "one" | "two"; readonly outline: Outline };
export function drawSurface(name: string, surface: Surface, o: SurfaceOptions): SurfaceDrawable
export function drawGlazed(name: string, surface: Surface, o: { readonly colour: ((f: Fragment) => Expr<"vec4">) | GlazedColour; readonly grid: Grid; readonly phase: Extract<Phase, "water" | "on-water" | "transparent">; 
export function drawFlat(name: string, surface: Surface, o: { readonly paint: Paint; readonly grid: Grid }): SurfaceDrawable
export function nameId(name: string): number
```

## src/draw/trace.ts
```ts
export type TraceBound =
export const bound =
export function aabbOf(b: TraceBound): { readonly min: V3; readonly max: V3 }
export const PROXY_VERTICES = CUBE.length;
export function traceWgsl(b: TraceBound, show: string): string
export function tracePlumbingWgsl(): string
export function traceShadeWgsl(): string
export type TraceOutline = "silhouette" | "none";
export function outlineWgsl(o: TraceOutline): string
export const TRACE_DEPTH: { readonly kind: "entry"; readonly entry: "fs_depth" } = { kind: "entry", entry: "fs_depth" };
export const TRACE_DEPTH_OUT = "struct TraceDepthOut { @builtin(frag_depth) depth: f32 };";
```

## src/draw/traced.ts
```ts
export type Ray = { readonly o: P3; readonly d: P3; readonly t0: F; readonly t1: F; readonly width: (t: F) => F };
export type Traced = { readonly t: F; readonly gap: F; readonly normal: P3 };
export type TracedBound = TraceBound | { readonly kind: "copies"; readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };
export type TracedHit = { readonly p: P3; readonly normal: P3; readonly d: P3; readonly copy: F };
export type TracedPaint = (hit: TracedHit) => P3 | Painted;
export type TracedOptions =
export class TracedDrawable implements Drawable
export function drawTraced(name: string, o: TracedOptions): TracedDrawable
```

## src/draw/volume.ts
```ts
export type VolumeOptions =
export function drawVolume(name: string, density: (p: P3) => In<"f32">, o: VolumeOptions): VolumeDrawable
export class VolumeDrawable implements Drawable
export const TRANSMITTANCE: LibFn =
```

## src/engines/inspect.ts
```ts
export const inspect =
```

## src/engines/print.ts
```ts
export const flat = `fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f
export const toon = `fn shade(x:Point,wo:vec3f,ink:bool,px:vec2f)->vec4f
export const print =
```

## src/engines/realtime.ts
```ts
export const realtime =
```

## src/engines/reference.ts
```ts
export const film = `fn present(c:vec3f)->vec4f
export function resolve(present: string, average: boolean): Source
export type Sampling = { samples?: number; depth?: number; present?: string };
export function sampling(o: Sampling)
export const jittered = `fn primary(px:vec2f,k:u32)->Ray
export const reference =
```

## src/geom/afloat.ts
```ts
export interface Waters
export type AfloatSpec =
export function afloatFrame(w: Waters, spec: AfloatSpec): Frame
export function afloat(w: Waters, spec: AfloatSpec): Chart
```

## src/geom/bend.ts
```ts
export type Bend = readonly [In<"f32">, In<"f32">];
export type BentSpec =
export type Bent =
export class BendError extends SceneError {}
export function bent(spec: BentSpec): Bent
export function bentTip(o:
```

## src/geom/crest.ts
```ts
export type SheetOnSpec =
export function sheetOn(spec: SheetOnSpec): SurfaceDrawable
export function sheetOnEdges(spec: Omit<SheetOnSpec, "paint" | "name" | "rows"> & { readonly name?: string }): { readonly root: Curve; readonly edge: Curve }
```

## src/geom/expression.ts
```ts
export type Expression<K extends string> = Readonly<Record<K, number>>;
export type Posed<K extends string> = Readonly<Record<K, F>>;
export type Cast<K extends string, N extends string> =
export class ExpressionError extends SceneError {}
export function cast<K extends string, N extends string>(rest: Expression<K>, named: Readonly<Record<N, Partial<Expression<K>>>>): Cast<K, N>
export type Layer<K extends string, N extends string> =
export const show = <N extends string>(name: N, weight: In<"f32">): Layer<never, N> => ({ kind: "show", show: name, weight });
export function only<K extends string, N extends string>(name: N, channels: readonly K[], weight: In<"f32">): Layer<K, N>;
export function only<K extends string>(to: Partial<Readonly<Record<K, number>>>, weight: In<"f32">): Layer<K, never>;
export function only<K extends string, N extends string>(a: N | Partial<Readonly<Record<K, number>>>, b: readonly K[] | In<"f32">, c?: In<"f32">): Layer<K, N>
export const added = <K extends string>(by: Partial<Readonly<Record<K, In<"f32">>>>, weight: In<"f32"> = 1): Layer<K, never> => ({ kind: "add", by, weight });
export function during(clock: In<"f32">, o: { readonly from: number; readonly to?: number; readonly rise: number; readonly fall?: number }): F
export type Score<K extends string, N extends string> =
export function score<K extends string, N extends string>(cast_: Cast<K, N>, layersAt: (t: F) => readonly Layer<NoInfer<K>, NoInfer<N>>[]): Score<K, N>
```

## src/geom/fan.ts
```ts
export type FanSpec =
export function catenary(f: F, k: number): F
export function fan(spec: FanSpec): Surface
export const rayOf = (body: Chart, v = 0.25): Curve => (m) => body.frame(vec3(m, v, 0)).o;
```

## src/geom/gait.ts
```ts
export type Course = (s: F) => Frame;
export type GaitSpec =
export type Pace =
export type Gait =
export function gait(spec: GaitSpec): Gait
export const course =
```

## src/geom/grab.ts
```ts
export type AxesSize = Extract<Size, { kind: "axes" }>;
export type GrabBeats = { readonly reach: number; readonly close: number; readonly hold: number; readonly release: number };
export type GrabSpec =
export type Grab =
export function grab(since: In<"f32">, spec: GrabSpec, b: GrabBeats): Grab
export type SetDown = { readonly kind: "setDown" };
export type Thrown =
export type Ending = SetDown | Thrown;
export const SET_DOWN: SetDown = { kind: "setDown" };
export type ThrownSpec = Omit<Thrown, "kind" | "land" | "up"> & { readonly land: In<"vec3">; /** Default +y. */ readonly up?: In<"vec3"> };
export function thrown(o: ThrownSpec): Thrown
export type Throw =
export function throwOf(windback: P3, e: Thrown): Throw
export type Leaving = { readonly palm: P3; readonly open: F; readonly home: F };
export function handTurn(e: Pick<Thrown, "flight" | "stroke">, turns: number): number
export const tumbleRate = (e: Pick<Thrown, "flight">, turns: number): number => (TAU * turns) / e.flight;
export function tumbled(g: Frame, axis: In<"vec3">, e: Pick<Thrown, "flight" | "stroke">, turns: number, at: { readonly whip: In<"f32">; readonly u: In<"f32"> }): Frame
export type Held =
export function heldThing(since: In<"f32">, seconds: number, from: P3, carry: (u: F) => P3, squeeze: number, pulses: number, end: Ending): Held
export type GripperSpec = Pick<GrabSpec, "rest" | "radius" | "reachLift" | "up" | "margin" | "restGap" | "windup">;
export function gripper(since: In<"f32">, spec: GripperSpec, b: GrabBeats, h: Held): Grab
export function fingerAngle(gap: In<"f32">, length: In<"f32">, hinge: In<"f32"> = 0): F
export function pincerAngle(gap: In<"f32">, length: In<"f32">): F
export type WrapSpec =
export function wrap(spec: WrapSpec): { readonly turns: F; spine(m: F): P3 }
```

## src/geom/limb.ts
```ts
export type ChainSpec =
export type Chain =
export const unit = (v: P3): P3 => div(v, max(length(v), 1e-6));
export function chain(spec: ChainSpec): Chain
export function polyline(m: F, points: readonly P3[], knots: readonly F[]): P3
```

## src/geom/lines.ts
```ts
export type Rope = { readonly a: P3; readonly b: P3; readonly sag: In<"f32"> };
export type RopesSpec =
export function ropes(given: string, ropes: readonly Rope[], spec: RopesSpec): Drawable
```

## src/geom/lively.ts
```ts
export type Beats = { readonly anticipate: number; readonly act: number; readonly settle: number };
export type Beat =
export function beats(since: In<"f32">, b: Beats): Beat
export function slowInOut(u: In<"f32">, power = 2): F
export const slowOut = (u: In<"f32">, power = 2): F => sub(1, pow(sub(1, clamp(u, 0, 1)), power));
export const slowIn = (u: In<"f32">, power = 2): F => pow(clamp(u, 0, 1), power);
export function anticipated(u: In<"f32">, back = 1.7): F
export function overshot(u: In<"f32">, back = 1.7): F
export function squash(along: In<"f32">): Size
export const stretchOf = (speed: In<"f32">, per = 0.15, most = 1.6): F => min(add(1, mul(per, speed)), most);
export function impact(after: In<"f32">, depth = 0.35, hz = 3, damping = 0.35): F
export function arc(a: In<"vec3">, b: In<"vec3">, u: In<"f32">, lift_: In<"f32">, up: In<"vec3"> = [0, 1, 0]): P3
export function swing(after: In<"f32">, amount: In<"f32">, hz = 1.4, damping = 0.25): F
export const dragged = (t: In<"f32">, along: In<"f32">, lag: In<"f32">): F => sub(t, mul(along, lag));
export const echo = (f: (t: F) => F, t: In<"f32">, delay: In<"f32">, gain: In<"f32">): F => mul(f(sub(t, delay)), gain);
export function looping<T>(seconds: number, build: () => T): T
export function alive(t: In<"f32">, amount: In<"f32">, seed: In<"f32"> = 0, hz = 0.23, period?: number): F
export const exaggerated = (x: In<"f32">, rest: In<"f32">, by: In<"f32">): F => add(rest, mul(sub(x, rest), by));
export const uneven = (part: In<"f32">, spread = 0.15): F => add(1, mul(sub(mul(hash3(vec3(part, 3.7, 9.2)), 2), 1), spread));
```

## src/geom/motion.ts
```ts
export type LookSpec =
export function lookAt(spec: LookSpec): { readonly yaw: F; readonly pitch: F; readonly dir: P3 }
export type BreathSpec = { readonly rate: In<"f32">; readonly depth: In<"f32">; readonly phase?: In<"f32">; readonly t?: In<"f32"> };
export function breathe(spec: BreathSpec): F
export type BeatSpec =
export function beat(r: In<"f32">, spec: BeatSpec): F
export type StrikeSpec =
export function strike(s: In<"f32">, spec: StrikeSpec): F
```

## src/geom/pendulum.ts
```ts
export type PendulumSpec =
export type Drive =
export const sway = (amplitude: In<"f32">, period: number, phase = 0): Drive => ({ kind: "sway", amplitude, period, phase });
export const jolt = (after: In<"f32">, speed: In<"f32">): Drive => ({ kind: "jolt", after, speed });
export class PendulumError extends SceneError {}
export type Pendulum =
export function pendulum(spec: PendulumSpec): Pendulum
```

## src/geom/placing.ts
```ts
export type Pose = { readonly at: P3; readonly heading: F; readonly pitch: F; readonly heel: F };
export function pose(o: { readonly at: In<"vec3">; readonly heading: In<"f32">; readonly pitch: In<"f32">; readonly heel: In<"f32"> }): Pose
export function poseFrame(p: Pose): Frame
export type StandIn = { readonly kind: "standIn" };
export type Hosted = { readonly kind: "host"; readonly pose: Pose };
export type Placer = StandIn | Hosted;
export const STAND_IN: StandIn = { kind: "standIn" };
export const hosted = (p: Pose): Hosted => ({ kind: "host", pose: p });
export function placedBy<T>(p: Placer, by: { readonly standIn: () => T; readonly host: (pose: Pose) => T }): T
export function blendFrames(a: Frame, b: Frame, w: In<"f32">): Frame
export function turnedAbout(g: Frame, axis: In<"vec3">, angle: In<"f32">): Frame
```

## src/geom/restless.ts
```ts
export type DartSpec =
export type Dart =
export class RestlessError extends SceneError {}
export function dart(t: In<"f32">, o: DartSpec): Dart
export const chosen = (hold: In<"f32">, seed: number): F => hash3(vec3(hold, seed, 5.3));
export const tremor = (t: In<"f32">, hz: number, seed = 0): F =>
```

## src/geom/rig.ts
```ts
export function rigid(g: Frame): Chart
```

## src/geom/ring.ts
```ts
export type RingSpec =
export type Ride =
export class RingWave implements SeaDeform
export function ringWave(spec: RingSpec): RingWave
```

## src/geom/sea.ts
```ts
export type SwellSpec =
export interface SeaDeform
export type SeaSpec =
export class Sea implements Chart, Drawable
export function sea(spec: SeaSpec): Sea
```

## src/geom/skin_distance.ts
```ts
export type V3 = readonly [number, number, number];
export type SkinDistance = { readonly lo: V3; readonly hi: V3; distance(q: V3): number };
export function skinDistance(skin: Surface): SkinDistance
export function skinBoxDistance(h: SkinDistance, q: V3): number
```

## src/geom/solve.ts
```ts
export type Family = { readonly kind: "spline"; readonly n: number; readonly from: F };
export function spline(n: number, from: In<"f32"> = 0): Family
export type HoldLine =
export function equal(a: In<NumTy>, b: In<NumTy>, within: number = TOLERANCE): Expr<"bool">
export function holdsOf(c: Hold, place: string): HoldLine[]
export type Prefer<K extends string> =
export type SolveSpec<K extends string> =
export type HoldReport = { readonly kind: HoldLine["kind"]; readonly said: string; readonly site: string; readonly when: string; readonly worst: number; readonly at: number };
export type SolveReport =
export type Solved<K extends string> =
export type SolveFailure =
export const TOLERANCE = 1e-5;
export class SolveError extends SceneError
export const JET: LibFn = { name: "loft_solve_jet", ret: "f32", wgsl: "fn loft_solve_jet(t: f32, a: f32, b: f32, c: f32, d: f32) -> f32 { return a; }", js: (_t: number, a: number) => a };
export function solve<K extends string>(spec: SolveSpec<K>): Solved<K>
```

## src/geom/spiral.ts
```ts
export type CoilLaw =
export type CoilSection =
export type CoilSpec = CoilLaw &
export function coil(spec: CoilSpec): Surface
export const COIL_TOPOLOGY = { u: ["cap", "free"], v: "periodic" } as const satisfies Topology;
export function coilAt(law: CoilLaw): (u: F, across: F, along: F) => P3
export function coilPlan(spec: CoilLaw & { readonly aspect?: number }):
export const whorl = (u: F, turns: number): F => mul(u, turns);
export const ridgePhase = (u: F, turns: number, perTurn: number): F => fract(mul(u, turns * perTurn));
export type SpiralTubeSpec =
export function spiralTube(spec: SpiralTubeSpec): Strand
export function spiralLine(spec: Pick<SpiralTubeSpec, "length" | "curl" | "taper" | "girth" | "out" | "toward" | "twist">): { readonly spine: (m: F) => P3; readonly girth: (m: F) => F; readonly up: P3 }
```

## src/geom/stance.ts
```ts
export type Carriage = { readonly surge: F; readonly rise: F; readonly sway: F; readonly roll: F; readonly pitch: F; readonly yaw: F };
export type Toward = { readonly x: F; readonly z: F };
export function towardIn(ground: Frame, p: P3): Toward
export function headingIn(ground: Frame, v: P3): Toward
export type Stance =
export function carriage(stances: readonly Stance[]): Carriage
export function carriedFrame(ground: Frame, c: Carriage): Frame
export type Replant = { readonly foot: number; readonly out: number; readonly back: number; readonly to: readonly [number, number] };
export type ReplantSpec =
export type StepTiming = { readonly lead: number; readonly seconds: number; readonly settle: number };
export function replants(spec: ReplantSpec): readonly Replant[]
export type Stamp = { readonly foot: number; readonly since: F; readonly lead: number; readonly height: number };
export type FootingSpec =
export type Footing =
export function footing(spec: FootingSpec): Footing
```

## src/geom/strand.ts
```ts
export type Rgb = readonly [number, number, number];
export type TipSpec = "round" | "point" | "open" | { readonly round: number } | { readonly point: number };
export type PaintSpec = Rgb | Paint;
export type StrandSpec =
export class Strand extends Body implements Drawable
export function strand(spec: StrandSpec): Strand
```

## src/geom/struck.ts
```ts
export class StruckError extends SceneError {}
export type StruckSpec =
export type Struck =
export function struck(clock: In<"f32">, o: StruckSpec): Struck
export function holdsStill<K extends string, N extends string>(score: Score<K, N>, poses: Readonly<Partial<Record<N, Struck>>>, o: { readonly period: number; readonly within: Tolerance; readonly rate?: number }): Check
```

## src/geom/symmetry.ts
```ts
export const reflect = (p: P3): P3 => vec3(p.x, p.y, neg(p.z));
export function mirror(part: Strand, name: string): Strand;
export function mirror(part: SurfaceDrawable, name: string): SurfaceDrawable;
export function mirror(part: Strand | SurfaceDrawable, name: string): Strand | SurfaceDrawable
export function array(part: Strand, count: number, placement: (i: number) => Chart): Strand[];
export function array(part: SurfaceDrawable, count: number, placement: (i: number) => Chart): SurfaceDrawable[];
export function array(part: Strand | SurfaceDrawable, count: number, placement: (i: number) => Chart): (Strand | SurfaceDrawable)[]
```

## src/geom/whirlpool.ts
```ts
export type WhirlpoolSpec =
export class Whirlpool implements SeaDeform
export class PoolChart implements Chart
export function whirlpool(spec: WhirlpoolSpec): Whirlpool
```

## src/kinds/materials.ts
```ts
export const matte = kind(`
export const lamp = (base: readonly number[], emission: readonly number[]) => (
export const glow = (power: readonly number[]) => lamp([0, 0, 0], power);
export const metal = kind(`
```

## src/kinds/shapes.ts
```ts
export const washer = kind(`
export const sphere = (radius: number) => ({ ...sphereKind(radius), radius });
export const bead = kind(`
export const box = kind(`
export const plane = kind(`
export const capsule = kind(`
```

## src/lib/fold.ts
```ts
export type Vec = "vec2" | "vec3";
export type Folded<V extends Vec> =
export type Fold<V extends Vec> = (p: In<V>) => Folded<V>;
export function compose<V extends Vec>(first: Fold<V>, second: Fold<V>): Fold<V>
export const fold2 =
export const fold3 =
```

## src/lib/grow.ts
```ts
export function kifs(spec: { readonly count: In<"f32">; readonly fold: Fold<"vec3">; readonly scale: number; readonly offset: In<"vec3"> }): Fold<"vec3">
export type Grown =
export type GrowSpec =
export function grow(spec: GrowSpec): (p: In<"vec3">) => Grown
export function levels(lo: number, hi: number, x: In<"f32">): F
```

## src/lib/noise.ts
```ts
export const hash2 = (p: In<"vec2">): Expr<"f32"> => lib<"f32">(HASH2, p);
export const hash3 = (p: In<"vec3">): Expr<"f32"> => lib<"f32">(HASH3, p);
export const hash22 = (p: In<"vec2">): Expr<"vec2"> => lib<"vec2">(HASH22, p);
export const hash33 = (p: In<"vec3">): Expr<"vec3"> => lib<"vec3">(HASH33, p);
export const noise2 = (p: In<"vec2">): Expr<"f32"> => lib<"f32">(NOISE2, p);
export const noise3 = (p: In<"vec3">): Expr<"f32"> => lib<"f32">(NOISE3, p);
export type Fbm = { readonly octaves: number; readonly lacunarity: In<"f32">; readonly gain: In<"f32"> };
export function fbm2(p: In<"vec2">, spec: Fbm): Expr<"f32">
export function fbm3(p: In<"vec3">, spec: Fbm): Expr<"f32">
export const cellular2 = (p: In<"vec2">): Expr<"vec4"> => lib<"vec4">(CELLULAR2, p);
export const cellular3 = (p: In<"vec3">): Expr<"vec2"> => lib<"vec2">(CELLULAR3, p);
```

## src/lib/screen.ts
```ts
export const fwidth: (x: In<"f32">) => F = screen.pixel_rate!;
export const viewDir: (p: In<"vec3">) => P3 = screen.toward_point!;
export const eyePosition: () => P3 = screen.eye_point!;
export const pixelWorld: (p: In<"vec3">) => F = screen.pixel_size!;
```

## src/lib/sdf2.ts
```ts
export function sdCircle(p: In<"vec2">, r: In<"f32">): Expr<"f32">
export function sdBox(p: In<"vec2">, halfSize: In<"vec2">): Expr<"f32">
export function sdSegment(p: In<"vec2">, a: In<"vec2">, b: In<"vec2">): Expr<"f32">
export function sdCapsule(p: In<"vec2">, a: In<"vec2">, b: In<"vec2">, r: In<"f32">): Expr<"f32">
export function sdRing(p: In<"vec2">, r: In<"f32">, halfWidth: In<"f32">): Expr<"f32">
export function sdTriangleIsosceles(p: In<"vec2">, halfWidth: In<"f32">, height: In<"f32">): Expr<"f32">
export function repeatAngle(p: In<"vec2">, count: In<"f32">): Expr<"vec2">
export function stroke(d: In<"f32">, halfWidth: In<"f32">): Expr<"f32">
export function coverage(d: In<"f32">, pixelWidth: In<"f32">): Expr<"f32">
```

## src/lib/sdf3.ts
```ts
export type Field<V extends Vec> = (p: Expr<V>) => Expr<"f32">;
export type Sdf3 = Field<"vec3">;
export function sdSphere(p: In<"vec3">, r: In<"f32">): Expr<"f32">
export function sdBox3(p: In<"vec3">, halfSize: In<"vec3">): Expr<"f32">
export function sdRoundBox(p: In<"vec3">, halfSize: In<"vec3">, r: In<"f32">): Expr<"f32">
export function sdCapsule3(p: In<"vec3">, a: In<"vec3">, b: In<"vec3">, r: In<"f32">): Expr<"f32">
export function sdRoundCone(p: In<"vec3">, a: In<"vec3">, b: In<"vec3">, r1: In<"f32">, r2: In<"f32">): Expr<"f32">
export function sdTorus(p: In<"vec3">, major: In<"f32">, minor: In<"f32">): Expr<"f32">
export function sdCylinder(p: In<"vec3">, r: In<"f32">, halfHeight: In<"f32">): Expr<"f32">
export function sdPlane(p: In<"vec3">, normal: In<"vec3">, offset: In<"f32">): Expr<"f32">
export function sdEllipsoid(p: In<"vec3">, radii: In<"vec3">): Expr<"f32">
export const union = (a: In<"f32">, b: In<"f32">): Expr<"f32"> => min(a, b);
export const subtract = (a: In<"f32">, b: In<"f32">): Expr<"f32"> => max(a, neg(lift<"f32">(b)));
export const intersect = (a: In<"f32">, b: In<"f32">): Expr<"f32"> => max(a, b);
export const smoothUnion = (a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32"> => smin(a, b, k);
export const smoothSubtract = (a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32"> => smax(a, neg(lift<"f32">(b)), k);
export const smoothIntersect = (a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32"> => smax(a, b, k);
export const round = (d: In<"f32">, r: In<"f32">): Expr<"f32"> => sub(d, r);
export const shell = (d: In<"f32">, thickness: In<"f32">): Expr<"f32"> => sub(abs(lift<"f32">(d)), mul(thickness, 0.5));
export function elongate(sdf: Sdf3, h: In<"vec3">): Sdf3
export function revolve(sdf2: Field<"vec2">, offset: In<"f32">): Sdf3
export function extrude(sdf2: Field<"vec2">, halfHeight: In<"f32">): Sdf3
export function fieldOf<V extends Vec>(fold: Fold<V>, sdf: Field<V>): (p: In<V>) => Expr<"f32">
export function nearest<V extends Vec>(folds: readonly Fold<V>[], sdf: Field<V>): (p: In<V>) => Expr<"f32">
export type LipschitzSpec =
export function lipschitz<V extends Vec>(field: Field<V>, spec: LipschitzSpec): number
```

## src/lib/similar.ts
```ts
export type V3 = readonly [number, number, number];
export type Axis = "x" | "y" | "z";
export type Plane = "xy" | "xz" | "yz";
export type Gen =
export const gen =
export function ask(chain: readonly Gen[]): (p: In<"vec3">) => Folded<"vec3">
export type Frame = { readonly o: V3; readonly m: readonly [V3, V3, V3]; readonly scale: number };
export const WORLD: Frame = { o: [0, 0, 0], m: [[1, 0, 0], [0, 1, 0], [0, 0, 1]], scale: 1 };
export function toWorld(fr: Frame, q: V3): V3
export type Copy = { readonly frame: Frame; readonly id: V3 };
export function list(chain: readonly Gen[], parent: Frame): Copy[]
export type Part =
export function partDistance(part: Part, q: P3): F
export type Placed = { readonly a: V3; readonly ra: number; readonly b: V3; readonly rb: number };
export function placePart(part: Part, fr: Frame): Placed
export type TreeSpec =
export type Asked3 = { readonly d: F; readonly level: F; readonly id: P3 };
export type Branch = Placed & { readonly level: number; readonly id: V3 };
export type Tree =
export function tree(spec: TreeSpec): Tree
```

## src/lib/util.ts
```ts
export function remap(x: In<"f32">, inLo: In<"f32">, inHi: In<"f32">, outLo: In<"f32">, outHi: In<"f32">): Expr<"f32">
export function remapClamped(x: In<"f32">, inLo: In<"f32">, inHi: In<"f32">, outLo: In<"f32">, outHi: In<"f32">): Expr<"f32">
export function polar(p: In<"vec2">): Expr<"vec2">
export function cartesian(pr: In<"vec2">): Expr<"vec2">
export function rotate2(p: In<"vec2">, angle: In<"f32">): Expr<"vec2">
export function fresnel(normal: In<"vec3">, view: In<"vec3">, power: In<"f32">): Expr<"f32">
export function smin(a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32">
export function smax(a: In<"f32">, b: In<"f32">, k: In<"f32">): Expr<"f32">
export function luminance(rgb: In<"vec3">): Expr<"f32">
export function hsv(h: In<"f32">, s: In<"f32">, v: In<"f32">): Expr<"vec3">
```

## src/light/queries.ts
```ts
export const albedoFill = { text: ggx + fill, opens: [] };
export const defaultSky = `fn sky(d:vec3f)->vec3f
export function lighting(s: Scene, w: ReturnType<typeof world>, fillModule: number)
```

## src/map/brick.ts
```ts
export const BRICK = 8;
export type Variance =
export function varianceOf(e: Expr): Variance
export type BrickSpec =
export type BrickStats = { readonly evaluations: number; readonly bricks: number; readonly coarse: number; readonly bytes: number; readonly ms: number };
export type BrickCell = { readonly at: V3; readonly slot: number };
export type Brick =
export function bakeBrick(name: string, d: Expr<"f32">, pvar: string, spec: BrickSpec): Brick
```

## src/map/chart.ts
```ts
export const rotate = (g: Frame, d: P3): P3 => add(add(mul(g.x, d.x), mul(g.y, d.y)), mul(g.z, d.z));
export function carry(chart: Chart, f: Frame): Frame
export const world: Chart = { frame: (c) => ({ o: c, x: X, y: Y, z: Z }), at: () => world, atNear: () => world };
export function on(host: Chart, c: In<"vec3">): Chart
export function compose(outer: Chart, inner: Chart): Chart
export function sized(chart: Chart, k: In<"vec3">): Chart
export function shifted(chart: Chart, by: In<"vec3">): Chart
export function sizeLengths(z: Size): P3
export function place<T>(shape: Placeable<T>, chart: Chart): T
export type PointSpec = { readonly at: In<"vec3">; readonly scale: In<"f32"> };
export function points(count: number, spec: (i: F) => PointSpec): PoseSet
export type Turn =
export const turn =
export const size =
export type PoseSpec = { readonly at: In<"vec3">; readonly turn: Turn; readonly size: Size };
export function turned(t: Turn): { readonly x: P3; readonly y: P3; readonly z: P3 }
export function posed(count: number, spec: (i: F) => PoseSpec): PoseSet
export function posedList(given: string, specs: readonly PoseSpec[]): Poses
export function sizeBound(s: Size): F
```

## src/map/nets.ts
```ts
export type TriangleMesh =
export type CpuField = (x: number, y: number, z: number) => number;
export function surfaceNets(brick: Brick, field: CpuField): TriangleMesh
```

## src/map/recur.ts
```ts
export type Cells = number | readonly [number, number];
export type Index<C extends Cells> = C extends number ? In<"f32"> : In<"vec2">;
export type Lanes = readonly Expr<"vec4">[];
export type RecurSpec<C extends Cells> =
export type Recurrence<C extends Cells> =
export const KEPT_BYTES = 64 * 1024 * 1024;
export const MOST_REPLAY = 120;
export type Kept = { readonly kind: "every" } | { readonly kind: "checkpoints"; readonly every: number };
export type Shape = { readonly kind: "list"; readonly n: number } | { readonly kind: "grid"; readonly w: number; readonly h: number };
export type RecurState =
export type RecurPart = "kept" | "even" | "odd" | "prev";
export const recurTable = (t: TableSource): { readonly state: RecurState; readonly part: RecurPart } | undefined => recurOf.get(t);
export function recur<const C extends Cells>(spec: RecurSpec<C>): Recurrence<C>
```

## src/map/surface.ts
```ts
export function mapNormal(map: (u: F, v: F) => P3): (u: F, v: F) => P3
export function atRest(point: (u: F, v: F) => P3): (u: F, v: F) => P3
export function surface(map: (u: F, v: F) => P3, topology: Topology): Surface
export function framed(s: Surface, at: Placed): Surface
export function framedPose(s: Surface, at: Posed): Surface
export function sheet(root: Curve, edge: Curve): Surface
export function lathe(profile: (u: F) => In<"vec2">, ends: readonly [End, End]): Surface
export function loft(curves: readonly Curve[]): Surface
export function extrude(curve: Curve, by: In<"vec3">): Surface
export function thicken(s: Surface, half: In<"f32">): Surface
```

## src/map/sweep.ts
```ts
export type Tip =
export type Section =
export type Transport =
export type FrameSample = { readonly p: P3; readonly der: P3; readonly t: P3; readonly d: P3; readonly s: P3; readonly stretch: F };
export function stationSample(t: TableSource, m: F): FrameSample
export type Slices = { readonly whole: Map<Expr<"f32">, Body>; readonly prefix: Map<string, Body>; readonly skeleton: Map<string, Skeleton>; count: number };
export const newSlices = (): Slices => ({ whole: new Map(), prefix: new Map(), skeleton: new Map(), count: 0 });
export type Posted =
export class Body implements Surface, Chart, Placeable<Body>
export class StationTable implements ProgramTable
export class SkeletonTable implements ProgramTable
export function sweepArgs(given: string, spine: (m: F) => P3, section: Section, transport: Transport, kind: "sweep" | "strand" = "sweep"): ConstructorParameters<typeof Body>
export function sweep(name: string, spine: (m: F) => P3, section: Section, transport: Transport): Body
```

## src/map/table.ts
```ts
export type Schedule = "static" | "frame" | "static-shading";
export function tabulate<T extends NumTy>(given: string, fn: (x: F) => Expr<T>, n: number, when: Schedule): (x: In<"f32">) => Expr<T>
```

## src/map/transport.ts
```ts
export const STATION_VEC4S = 3;
export type V3 = [number, number, number];
export type Q4 = [number, number, number, number];
export const v3 =
export function quatFromColumns(a: V3, b: V3, c: V3): Q4
export function qrot(q: Q4 | readonly number[], v: V3): V3
export const SAMPLE_VEC4S = 3;
export const SAMPLE = { world: 0, local: 1, rest: 2 } as const;
export type StationScan =
export function quatOf(a: P3, b: P3, c: P3): Expr<"vec4">
export function stationRadii(S: TableSource, k: F, leastStretch: number): { readonly a: F; readonly b: F; readonly stretch: F }
export function stationScan(s: StationScan): TableProgram
export function rotateAbout(v: In<"vec3">, axis: In<"vec3">, angle: In<"f32">): P3
```

## src/map/types.ts
```ts
export type F = Expr<"f32">;
export type P3 = Expr<"vec3">;
export type Frame = { readonly o: P3; readonly x: P3; readonly y: P3; readonly z: P3 };
export type Placed = { readonly frame: Frame; readonly scale: F };
export type Size = { readonly kind: "even"; readonly k: F } | { readonly kind: "axes"; readonly k: P3 };
export type Posed = { readonly frame: Frame; readonly size: Size };
export interface Poses
export type Curve = CurveOf<"vec3">;
export type End = "free" | "pole" | "cap";
export type Topology =
export interface Skeleton
export const NO_SKELETON: Skeleton = { joints: 0, joint: (k) => { throw new Error(`NO_SKELETON has no joint ${k}`); } };
export interface Surface
export interface Chart
export type Field = (p: P3) => F;
export type Density = (p: P3) => F;
export type PaintAt = { readonly world: P3; readonly normal: P3; readonly copy: F };
export type Paint = (u: F, v: F, at: PaintAt) => P3 | Painted;
export type Painted =
export type Warp<V extends "vec2" | "vec3"> = Fold<V>;
export interface Placeable<T>
```

## src/map/warp.ts
```ts
export const warp =
export function tile(w: Warp<"vec2"> | readonly Warp<"vec2">[], motif: (cell: Folded<"vec2">) => P3): Paint
```

## src/motion/body.ts
```ts
export type V3 = readonly [number, number, number];
export const foot = (at: V3) => ({ kind: "foot" as const, at });
export function neighbours(chains: readonly Chain[])
export function body(spec: Body, edges = neighbours(spec.chains))
```

## src/motion/move.ts
```ts
export function move(shape: Body | readonly Body[], source: string, inputs:
```

## src/motion/roam.ts
```ts
export function roam(laws: Partial<{ arch: string; step: string; present: string; deck: string }> = {})
```

## src/relief/displace.ts
```ts
export type ShellFrame =
export function shellFrame(s: Surface, u: F, v: F): ShellFrame
export function displace(s: Surface, height: Height): Surface
```

## src/relief/height.ts
```ts
export type Height = (u: F, v: F) => F;
export type Wrap = "none" | "u" | "v" | "uv";
export type Patch = { readonly size: readonly [number, number]; readonly wrap: Wrap };
export type UV = readonly [number, number];
export function offset(patch: Patch, u: F, v: F, a: readonly [F | number, F | number]): Expr<"vec2">
export function segmentDistance(patch: Patch, u: F, v: F, a: UV, b: UV): F
export function pathDistance(patch: Patch, u: F, v: F, path: readonly UV[]): F
export type Profile = "smooth" | "dome" | "cone" | "spike" | "crater" | "plateau";
export function profile(p: Profile, r: F): F
export type BumpSpec = { readonly at: UV; readonly radius: number; readonly height: number; readonly profile: Profile };
export function bump(patch: Patch, s: BumpSpec): Height
export type RidgeSpec = { readonly width: number; readonly height: number; readonly profile: Profile };
export function ridge(patch: Patch, path: readonly UV[], s: RidgeSpec): Height
export type CreaseSpec = { readonly width: number; readonly depth: number; readonly sharpness: number };
export function crease(patch: Patch, path: readonly UV[], s: CreaseSpec): Height
export type Region =
export function weight(patch: Patch, r: Region, u: F, v: F): F
export function flatten(patch: Patch, h: Height, region: Region, level: number): Height
export function inflate(patch: Patch, h: Height, region: Region, amount: number): Height
export const level = (h: number): Height => () => lift<"f32">(h);
export function sum(...hs: readonly Height[]): Height
export function smoothMax(k: number, ...hs: readonly Height[]): Height
export function clampHeight(h: Height, lo: number, hi: number): Height
export function scaled(h: Height, by: (u: F, v: F) => F): Height
```

## src/relief/sculpt.ts
```ts
export function sculptLayer(name: string, samples: ArrayLike<number>, size: readonly [number, number], o: { readonly wrap: Wrap }): Height
export function sampleGrid(fn: (u: number, v: number) => number, size: readonly [number, number], o: { readonly wrap: Wrap }): Float32Array
```

## src/relief/trace.ts
```ts
export type ShellRay = { readonly entry: P3; readonly dir: P3; readonly top: F };
export type Traced =
export const KIND = { miss: 0, ink: 0.5, hit: 1 } as const;
export function firstRoot(a: F, b: F, c: F): F
export type ShellModel =
export function shellModel(fr: ShellFrame, uv0: Expr<"vec2">, ray: ShellRay): ShellModel
export type MarchBudget =
export function traceMarch(fr: ShellFrame, uv0: Expr<"vec2">, ray: ShellRay, height: Height, b: MarchBudget): Traced
export function tracePlain(fr: ShellFrame, uv0: Expr<"vec2">, ray: ShellRay, height: Height, ink: F): Traced
```

## src/render/find.ts
```ts
export function find(w: ReturnType<typeof world>, samples: 1 | 4)
export function shadePixel(samples: 1 | 4, shade: string, background: string): string
export const identityFinish = "fn finish(c:vec4f,t:f32)->vec4f{return c;}";
```

## src/render/record.ts
```ts
export type V3 = readonly [number, number, number];
export type Thing =
export const thing = (shape: Thing["shape"], material: Thing["material"], at: V3 = [0, 0, 0]): Thing => (
export type World =
export type Scene = World &
export const scene = (s: Scene) => (
export type OrderedScene = ReturnType<typeof scene>;
export function world(s: World)
export function write(target: Write["target"], clear = 0, blend: Write["blend"] = "Opaque"): Write
export function raster(
export const screenDraw = (fragment: string): Draw => (
```

## src/render/twin.ts
```ts
export type Pick = { kind: "miss" } | { kind: "hit"; id: number; t: number; normal: V3; uv: readonly [number, number] };
export async function twin(scene: World, snapshots: Readonly<Record<string, Float32Array>> = {})
```

## src/render/covers/grid.ts
```ts
export function grid(u: number, v: number)
export function compact(law: string, indexCounts: readonly number[]): Source
```

## src/soft/cloud.ts
```ts
export type Rgb = readonly [number, number, number];
export type CloudSpec =
export type Cloud = { readonly puffs: readonly Puff[]; readonly bounds: Box };
export function hashUnit(seed: number, i: number, k: number): number
export function cloud(spec: CloudSpec): Cloud
export type PuffPlace = { readonly at: V3; readonly radii: V3; readonly density: number };
export type BillowSpec = Pick<CloudSpec, "turbulence" | "billow" | "drift" | "colour" | "seed" | "seconds">;
export function billows(places: readonly PuffPlace[], motion: BillowSpec): Cloud
```

## src/soft/density.ts
```ts
export type Rgb = readonly [number, number, number];
export type DensityOptions =
export type DensitySettings = Required<DensityOptions>;
export class DensityDrawable implements Drawable
export function densityIntegral(ctx: CompileContext, density: DensityDrawable): Shader & { readonly mixedCode: string }
export function boxVertexWgsl(b: Box): string
export function softRayWgsl(b: Box): string
```

## src/soft/gauss.ts
```ts
export const SIMPSON = 0.05;
export const FAR = 1e4;
export function erfcPos(z: number): number
export function gseg(a: number, b: number): number
export function gtail(a: number): number
export type Seg = { tau: number; tc: number; spread: number };
export function gaussRay(O: readonly number[], D: readonly number[], t0: number, t1: number): Seg
export function gaussDepth(O: readonly number[], D: readonly number[], t0: number, t1: number): number
export function gaussTail(O: readonly number[], D: readonly number[]): number
export const GAUSS_WGSL = /* wgsl */ `
```

## src/soft/glow.ts
```ts
export type Rgb = readonly [number, number, number];
export interface Strokes
export function strokes(name: string, count: number, segments: number, point: (u: F, i: F) => P3, taper: (u: F, i: F) => In<"f32"> = () => 1): Strokes
export type Veil = { readonly kind: "clear" } | { readonly kind: "medium"; readonly medium: Medium };
export type GlowOptions =
export type Shape = { readonly kind: "line" } | { readonly kind: "star"; readonly rays: number; readonly length: number; readonly thin: number };
export class GlowDrawable implements Drawable
export function glows(name: string, s: Strokes, o: GlowOptions): Drawable
export function glow(name: string, curve: (u: F) => P3, o: Omit<GlowOptions, "brightness"> & { readonly intensity: In<"f32">; readonly segments: number }): Drawable
export type FlareOptions = Omit<GlowOptions, "brightness" | "width"> &
export function flare(name: string, at: In<"vec3">, o: FlareOptions): Drawable
```

## src/soft/lightning.ts
```ts
export type StrikeSchedule = { readonly first: number; readonly every: number; readonly jitter: number; readonly seed: number };
export function strikeAt(s: StrikeSchedule, t: In<"f32"> = time): { readonly index: F; readonly since: F }
export function flicker(since: F): F
export function afterglow(since: F, seconds: number): F
export type BoltSpec =
export type Bolt =
export function displaced(u: F, a: P3, b: P3, levels: number, rough: number, key: F): P3
export function bolt(spec: BoltSpec): Bolt
export function boltFlash(b: Bolt, o: { readonly power: number; readonly colour: readonly [number, number, number]; readonly reach: number; readonly lift: number }): Light
export function sheetFlash(s: StrikeSchedule, box: { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] }, o: { readonly power: number; readonly colour: readonly [number, num
export function skyFlash(name: string, brightness: In<"f32">, colour: readonly [number, number, number]): Drawable
```

## src/soft/medium.ts
```ts
export type Blob = { readonly kind: "blob"; readonly centre: P3; readonly inv: P3; readonly density: F; readonly colour: P3 };
export type Layer = { readonly kind: "layer"; readonly height: F; readonly inv: number; readonly density: F; readonly colour: P3 };
export type Puff = Blob | Layer;
export type Bank =
export type Box = { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] };
export type BlobSpec =
export function blob(b: BlobSpec): Blob
export type LayerSpec =
export function layer(l: LayerSpec): Layer
export type BankSpec = LayerSpec &
export function bank(b: BankSpec): Bank
export const BLOB_LANES = 5;
export const CLUSTER = 8;
export const EXTENT = 3.6;
export type MediumTables =
export type Optional = { readonly kind: "none" } | { readonly kind: "some"; readonly table: TableSource; readonly count: number };
export class Medium
export function medium(name: string, parts: { readonly puffs?: readonly Puff[]; readonly banks?: readonly Bank[] }): Medium
export const GAUSS_LIB: LibFn = { name: "loft_gauss_ray", ret: "vec3", wgsl: GAUSS_WGSL, js: "gpu-only" };
export const BANK_LIB: LibFn = { name: "loft_bank_split", ret: "vec4", wgsl: BANK_SPLIT_WGSL, js: "gpu-only" };
export type MediumAccess =
export function accessMedium(m: Medium, sun: V3, lights: readonly Light[], table: (t: TableSource) => { readonly wgsl: string }): MediumAccess
export function clusterLoop(m: Medium, acc: MediumAccess, body: string): string
export function tauFunction(m: Medium, fn: string, acc: MediumAccess): { readonly code: string; readonly libs: readonly LibFn[] }
export function unionBox(boxes: readonly Box[]): Box
```

## src/soft/rain.ts
```ts
export type Rgb = readonly [number, number, number];
export type RainSpec =
export type SplashSpec =
export function rain(name: string, r: RainSpec): Drawable
export function splashes(name: string, r: RainSpec, s: SplashSpec): Drawable
```

## src/soft/specks.ts
```ts
export type Speck =
export type PointsOptions =
export function specks(name: string, count: number, speck: (index: F) => Speck, o: PointsOptions = {}): Drawable
```

## src/soft/specks_veil.ts
```ts
export function specksVeil(ctx: CompileContext, cloud: DensityDrawable, count: number): Shader & { cached: boolean }
export function specksVolumeVeil(ctx: CompileContext, v: VolumeDrawable, count: number): Shader & { cached: boolean }
```

## src/spacetime/attack.ts
```ts
export type Spine = (m: F) => P3;
export type Target =
export type Part = { readonly name: string; readonly body: Strand; readonly from: number; readonly to: number; readonly tip: number };
export function part(name: string, body: Strand, range: { readonly from?: number; readonly to?: number; readonly tip: number }): Part
export type Timing =
export type LungeSpec =
export type SweepSpec =
export type RemoteSpec = { readonly kind: "remote"; readonly hit: Hit };
export type GripSpec = Omit<GripperSpec, "rest" | "radius"> &
export type Motion = LungeSpec | SweepSpec | RearSpec | RemoteSpec | GripSpec;
export type BodyMotion = LungeSpec | SweepSpec | RearSpec;
export type Carry =
export type CarrySpec =
export type Instant = { readonly kind: "instant" };
export type Hold<N extends Ending = Ending> =
export type Effect = Instant | Hold;
export const INSTANT: Instant = { kind: "instant" };
export function hold<N extends Ending>(o: HoldSpec & { readonly end: N }): Hold<N>;
export function hold(o: HoldSpec): Hold<SetDown>;
export function hold(o: HoldSpec & { readonly end?: Ending }): Hold
export type Still = { readonly kind: "still" };
export type Ring = { readonly kind: "ring"; readonly speed: number; readonly height: number; readonly length: number; readonly reach: number; readonly hold: number; readonly drag: number };
export type Aftermath = Still | Ring;
export const STILL: Still = { kind: "still" };
export const ring =(o: Omit<Ring, "kind">): Ring => ({ kind: "ring", ...o });
export type AttackBeat = Beat & { readonly hold: F; readonly release: F; readonly flight: F };
export type Arming =
export const once = (at: In<"f32">): Arming => ({ kind: "once", at: lift<"f32">(at) });
export function every(period: number, first: In<"f32">): Arming
export type BodyAttackSpec<W extends Aftermath = Aftermath> = Common<W> &
export type RemoteAttackSpec<E extends Effect = Effect, W extends Aftermath = Aftermath> = Common<W> & { readonly motion: RemoteSpec; readonly effect?: E };
export type GripAttackSpec<H extends Hold = Hold, W extends Aftermath = Aftermath> = Common<W> & { readonly motion: GripSpec; readonly effect: H };
export type AttackSpec = BodyAttackSpec | RemoteAttackSpec | GripAttackSpec;
export type Envelope = { readonly p: F; readonly q: F; readonly tau: F };
export class Attack<M extends Motion = Motion, E extends Effect = Effect, W extends Aftermath = Aftermath>
export class BodyAttack<W extends Aftermath = Aftermath> extends Attack<BodyMotion, Instant, W>
export function attack<W extends Aftermath = Still>(spec: BodyAttackSpec<W>): BodyAttack<W>;
export function attack<H extends Hold = Hold, W extends Aftermath = Still>(spec: GripAttackSpec<H, W>): Attack<GripSpec, H, W>;
export function attack<E extends Effect = Instant, W extends Aftermath = Still>(spec: RemoteAttackSpec<E, W>): Attack<RemoteSpec, E, W>;
export function attack(spec: AttackSpec): Attack
export type Creature = { readonly things: readonly Drawable[]; readonly parts: Readonly<Record<string, Part>> };
export type Strike = { readonly attack: BodyAttack; readonly part: Part; readonly before: Spine };
export type Animated = Creature &
export function animate(rest: Spine, attacks: readonly BodyAttack[], build: (spine: Spine) => Creature): Animated
```

## src/spacetime/emote.ts
```ts
export type EmoteSpec =
export type EmoteBeat =
export class Emote
export function emote(spec: EmoteSpec): Emote
```

## src/spacetime/hit.ts
```ts
export type Capsule = { readonly kind: "capsule"; readonly a: V2; readonly b: V2; readonly radius: F };
export type Disc = { readonly kind: "disc"; readonly centre: V2; readonly radius: F };
export type Arc = { readonly kind: "arc"; readonly centre: V2; readonly inner: F; readonly outer: F; readonly from: F; readonly to: F };
export type Hit = Capsule | Disc | Arc;
export const hit =
export type Contour = { readonly d: F; readonly phase: F; readonly pitch: F };
export const polar = (a: In<"f32">): V2 => vec2(cos(a), sin(a));
export function hitSdf(h: Hit, p: V2): F
export function hitPatch(h: Hit, u: F, v: F, pad: F): V2
export function hitContour(h: Hit, p: V2, spacing: In<"f32">): Contour
export function hitInradius(h: Hit): F
export const laneOf = (h: Capsule): V2 => normalize(sub(h.b, h.a));
```

## src/spacetime/rear.ts
```ts
export type RearSpec =
export function rearSpine(a: Attack, m: RearSpec, rest: Spine): Spine
export function rearLanding(m: RearSpec): P3
```

## src/spacetime/routine.ts
```ts
export type Aim = V3 | "rest";
export type Who = "mirror" | { readonly canon: number } | { readonly only: readonly number[] };
export type Key =
export type Solo = { readonly solo: number; readonly on: readonly number[] };
export type RoutineSpec =
export type Dancer =
export type Readouts =
export class RoutineError extends SceneError {}
export class Routine
export type SyncRow = { readonly key: number; readonly count: number; readonly channel: string; readonly who: string; readonly arrival: number; readonly size: number; readonly pass: boolean; readonly arrivals: readonly n
export function routine(spec: RoutineSpec): Routine
```

## src/spacetime/smear.ts
```ts
export type SmearSpec =
export function smear(strike: Strike, spec: SmearSpec): Drawable
```

## src/spacetime/telegraph.ts
```ts
export type TelegraphStyle =
export const DANGER: TelegraphStyle =
export function telegraph(attack: Attack, o: { readonly style?: Partial<TelegraphStyle>; readonly name?: string } = {}): Drawable
```

## src/spacetime/wake.ts
```ts
export type WakeSpec =
export function wake(body: Body, spec: WakeSpec): Drawable
```

## src/style/biolume.ts
```ts
export function biolume(): Style
```

## src/style/foam.ts
```ts
export type FoamInk = "fill" | "line" | "edge";
export type InkLayers = readonly { readonly ink: FoamInk; readonly cover: F }[];
export function printLayers(under: Expr<"vec3">, layers: InkLayers, inks: Readonly<Record<FoamInk, Rgb>>): Expr<"vec3">
export type ContactFoam =
export type HeadSize = { readonly kind: "ofWidth"; readonly share: number } | { readonly kind: "world"; readonly radius: number };
export type MarksPrint = { readonly kind: "flat" } | { readonly kind: "heads"; readonly head: number };
export const foam =
export const marksAs =
export function contactFoam(p: ContactFoam, ctx: WaterCtx, lines: Pick<SwellLines, "off" | "live">): InkLayers
export function marksFoam(p: MarksPrint, ctx: WaterCtx): InkLayers
```

## src/style/gloss.ts
```ts
export function glossed(colour: V, ctx: ShadeCtx, o: { readonly gloss: In<"f32">; readonly metal?: In<"f32">; readonly seen?: (dir: V) => V }): V
```

## src/style/index.ts
```ts
export const STYLES =
export type StyleName = keyof typeof STYLES;
export const STYLE_NAMES = Object.keys(STYLES) as readonly StyleName[];
export function styleNamed(name: string): Style
export function withStyle(scene: Scene, style: Style): Scene
export function withLight(scene: Scene, name: string): Scene
```

## src/style/light.ts
```ts
export type Key = { readonly kind: "sun" } | { readonly kind: "offLens"; readonly least: number } | { readonly kind: "camera"; readonly side: number; readonly up: number };
export type Terminator =
export type Core = { readonly kind: "off" } | { readonly kind: "flat"; readonly edge: Terminator };
export type Highlight = { readonly kind: "off" } | { readonly kind: "shoulder"; readonly size: number; readonly ink: Rgb; readonly amount: number; readonly key: Key };
export type Cast = { readonly kind: "off" } | { readonly kind: "capsules"; readonly reach: number; readonly thin: number };
export type Waterline = { readonly kind: "off" } | { readonly kind: "band"; readonly height: number; readonly amount: number };
export type Overlap = { readonly kind: "off" } | { readonly kind: "band"; readonly width: number; readonly gap: number; readonly amount: number };
export type Bokashi = { readonly kind: "off" } | { readonly kind: "wash"; readonly foot: number; readonly under: readonly [number, number]; readonly amount: number };
export type Hatch = { readonly kind: "off" } | { readonly kind: "cross"; readonly edge: Terminator; readonly over: number; readonly deep: number; readonly cap: number; readonly spacing: number; readonly least: number; re
export type Rim = { readonly kind: "off" } | { readonly kind: "edge"; readonly width: number; readonly ink: Rgb; readonly amount: number; readonly key: Key };
export type Block = { readonly kind: "rule" } | { readonly kind: "ink"; readonly ink: Rgb; readonly amount: number; readonly value: number };
export type BlockInk = { readonly ink: Rgb; readonly amount: number; readonly value: number };
export type LightRecipe =
export const light =
export const LIGHT =
export type LightName = keyof typeof LIGHT;
export function keyOf(k: Key, ctx: ShadeCtx): V
export function lit(r: LightRecipe, ink: V, ctx: ShadeCtx): { readonly colour: V; readonly tone: F }
export type Bodies = Pick<Style, "shade" | "mark" | "block">;
export function litBodies(recipe: LightRecipe, own: BlockInk, ink: (ctx: ShadeCtx) => V, after: (c: V, ctx: ShadeCtx) => V = (c) => c): Bodies
export function blockInk(b: Block, own: BlockInk): BlockInk
export type Pull = { readonly floor: number; readonly add: Rgb };
export function pullOf(b: BlockInk): Pull
export const markOf = (p: Pull, tone: F): F => mix(p.floor, 1, tone);
export function printedBlock(p: Pull, colour: V, tone: F): V
export function castOf(c: Cast): BlockCast
export function overlapOf(o: Overlap): BlockOverlap
```

## src/style/neon.ts
```ts
export function neon(): Style
```

## src/style/paint.ts
```ts
export type Hue = "red" | "yellow";
export function standsOut(hue: Hue, c: Expr<"vec3">): Expr<"f32">
export type Warm = { readonly kind: "asPainted" } | { readonly kind: "hotter"; readonly margin: number; readonly gain: Rgb };
export type Mottle = { readonly kind: "off" } | { readonly kind: "blotches"; readonly size: number; readonly cut: number; readonly value: number; readonly margin: number };
export type LineWeight =
export const paint =
export function warmed(w: Warm, c: Expr<"vec3">): Expr<"vec3">
export function mottled(m: Mottle, c: Expr<"vec3">, ctx: ShadeCtx): Expr<"vec3">
export function weighted(l: LineWeight, c: Expr<"vec3">, ink: Rgb, ctx: ShadeCtx): Expr<"vec3">
```

## src/style/papercraft.ts
```ts
export const PAPERCRAFT_POST = { studio: { label: "papercraft studio", wgsl: STUDIO } } as const satisfies Record<string, PostSpec>;
export function papercraft(): Style
```

## src/style/pbr.ts
```ts
export function pbr(parts: Partial<PbrParts> = {}): Style
```

## src/style/pbr_parts.ts
```ts
export function ggx(n_h: F, rough: F): F
export function smithGgx(n_l: F, n_v: F, rough: F): F
export function schlick(v_h: F, f0: P3): P3
export function lambert(albedo: P3, n_l: F, _n_v: F, _v_h: F, _rough: F): P3
export function aces(c: P3): P3
export function daylight(dir: P3, sun: P3): P3
export type PbrParts =
export type PbrFailure =
export class PbrError extends SceneError
export function horizonClear(sky: SkyLaw, tone: (c: P3) => P3): readonly [number, number, number]
export function albedoOf(distribution: PbrParts["distribution"], visibility: PbrParts["visibility"]): PbrParts["lobeAlbedo"]
export function skyLight(at: { readonly normal: P3; readonly view: P3; readonly sun: P3 }, rough: F, sky: (dir: P3, sun: P3) => P3): { readonly diffuse: P3; readonly specular: P3 }
```

## src/style/post.ts
```ts
export function depthOfField({ focus, focalLength, fStop }: { readonly focus: number; readonly focalLength: number; readonly fStop: number }): PostSpec
export function filmGrain({ amount = 0.045 }: { readonly amount?: number } = {}): PostSpec
export function paperGrain(o: { readonly fibre?: number; readonly tooth?: number } = {}): PostSpec
export function bloom(o: { readonly radius?: number; readonly gain?: number; readonly from?: number; readonly to?: number } = {}): PostSpec
export function reflection(o: { readonly gain?: number } = {}): PostSpec
```

## src/style/sea.ts
```ts
export type SwellLines = { readonly cover: Expr<"f32">; readonly off: Expr<"f32">; readonly live: Expr<"f32">; readonly swell: Expr<"f32">; readonly calm: boolean };
export const calmSea = (ctx: WaterCtx): boolean => ctx.phase.node.op === "const";
export function swellLines(ctx: WaterCtx, o: { readonly perWave: number; readonly width: readonly [number, number] }): SwellLines
```

## src/style/shared.ts
```ts
export const rgb = (c: Rgb): Expr<"vec3"> => vec3(c[0], c[1], c[2]);
export const wgslRgb = (c: Rgb): string => `vec3<f32>(${c.map((x) => x.toFixed(4)).join(", ")})`;
export function edgeOf(x: Expr<"f32">, edge: number | Expr<"f32">, soft: number, widest = 0.05): Expr<"f32">
export function pixelsInside(ctx: ShadeCtx): Expr<"f32">
export const POST_COMMON = /* wgsl */ `
```

## src/style/sheet.ts
```ts
export const SHEET_CAMERA: Camera = { kind: "perspective", eye: [12.5, 12.5, 14.5], target: [0.3, 1.2, 0], fovY: 0.6 };
export function grid(frames: readonly Frame[], columns: number): Frame
```

## src/style/sketch.ts
```ts
export function sketch(): Style
```

## src/style/sky.ts
```ts
export type Wash = { readonly ink: Rgb; readonly from: number; readonly to: number };
export type SkySun = { readonly kind: "none" } | { readonly kind: "disc"; readonly toward: readonly [number, number, number]; readonly radius: number; readonly ink: Rgb };
export type SkyClouds =
export type Sky = { readonly horizon: Rgb; readonly washes: readonly Wash[]; readonly below: Rgb; readonly sun: SkySun; readonly clouds: SkyClouds };
export function skyOf(sky: Sky, ctx: SkyCtx): Expr<"vec3">
```

## src/style/toon.ts
```ts
export type ToonSpec =
export function toon(spec: ToonSpec = {}): Style
```

## src/style/whitewater.ts
```ts
export type Lean = { readonly kind: "none" } | { readonly kind: "toward"; readonly dir: V2 };
export type WhiteOn = { readonly xz: V2; readonly mass: F; readonly slope: V2; readonly lean: Lean };
export const slopeOf = (m: In<"f32">, xz: In<"vec2">): Expr<"vec2"> => lib<"vec2">(SLOPE_OF, m, xz);
export type White = { readonly fill: F; readonly line: F };
export type Roll = { readonly head: number; readonly line: number; readonly held: number };
export function rollOf(head: number, held = 0.75): Roll
export const along = (xz: V2, degrees: number, period: number): F =>
export const pixelOn = (xz: V2): F => max(max(fwidth(xz.x), fwidth(xz.y)), 1e-5);
export function rollingFoam(on: WhiteOn, r: Roll): White
export function curledHeads(on: WhiteOn, r: Roll): White
export function crown(d: V2, rim: F, reach: F, count: number, r: Roll): White
export function behind(back: White, front: White): White
export function joined(a: White, b: White): White
```

## src/style/woodblock.ts
```ts
export const WOODBLOCK_PALETTE =
export const WOODBLOCK_SEA = { deep: [0.11, 0.18, 0.35], mid: [0.16, 0.26, 0.45], line: [0.58, 0.68, 0.78], haze: [0.7, 0.76, 0.78] } as const satisfies Record<string, Rgb>;
export type WoodblockInks = { readonly black: Rgb; readonly indigo: Rgb; readonly pale: Rgb; readonly cream: Rgb };
export type AccentInk = { readonly ink: Rgb; readonly from: number };
export type Accent = { readonly hue: Hue; readonly margin: number; readonly inks: readonly AccentInk[] };
export type Print =
export type InkKey = { readonly key: Rgb; readonly shadow: Rgb; readonly lit: Rgb };
export const print =
export type WoodblockEye = { readonly kind: "none" } | { readonly kind: "ink"; readonly below: number; readonly amount: number };
export type SeaPalette =
export const seaPalette =
export type WoodblockSpec =
export function woodblock(spec: WoodblockSpec = {}): Style
export const REFERENCE_PAINT: NonNullable<WoodblockSpec["paint"]> = { warm: paint.hotter(), mottle: paint.mottle(), line: paint.swelling() };
export type WoodblockLook = { readonly about: string; readonly on: (s: WoodblockSpec) => WoodblockSpec };
export const LOOKS =
export type LookName = keyof typeof LOOKS;
```


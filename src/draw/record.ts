// What a scene records for the Rust path (lean Rust plan §1.5, §3.2): its cameras, and one view pass of the draws its
// things record (`Drawable.record`), in the TS engine's phase order. Each thing records its own draws: a cover (grid,
// mesh, quads, ray, screen) and its roles, graph functions over binders that loft-core's templates call (place,
// normal, hit, paint). The style's shading is an expression (src/style.ts), so a lit paint is the style's `shade` at
// the template's point, normal and (u, v), as the TS fragment stages evaluate it. What a thing cannot record yet is a
// named `unported` answer the build prints, never a silent gap.

import { lib, type LibFn } from "../expr.ts";
import { abs, add, and, distance, div, fract, gt, le, length, lift, lt, max, mul, or, select, sub, vec2, vec3, vec4, type Expr } from "../expr.ts";
import type { CompileContext, Phase, PostSpec } from "../compile/context.ts";
import { PHASES } from "../compile/context.ts";
import { CELL_PIXELS, type Artefact, type Camera as SceneCamera, type Detail } from "../artefact.ts";
import type { Scene } from "../scene.ts";
import { foldAt, sunDir, sunOnly } from "../light.ts";
import { shadeContext, type ShadeCtx, type Style } from "../style.ts";
import { eyePosition, pixelWorld, viewDir } from "../lib/screen.ts";
import { wgsl } from "../text/index.ts";
import { levelOf } from "./ground.ts";
import { role, type Camera, type Draw, type Pass, type Role, type Scene as Decls } from "../record/decls.ts";
import type { F, P3, Painted } from "../map/types.ts";
import type { Ray, Traced } from "./traced.ts";
import type { Cover } from "../record/decls.ts";
import { classify, answered } from "./painted.ts";
import { nameId } from "./surface.ts";
import { mapOf, type TableSource } from "../expr.ts";

/**
 * What a thing is told when it records: the scene's style, ground, sun, lights, things and outline method, and `rest`,
 * a lit grid's levels of detail with the rest measurements the TS compiler took (`Detail` "grid": the cell sizes and
 * the reach from the anchor, evaluated on the CPU at time 0), by draw label. Until the Rust tape measures them, the
 * Rust path reads them from the TS compile it still runs.
 */
export type RecordContext = Pick<CompileContext, "style" | "ground" | "sun" | "sunAt" | "lighting" | "things" | "outline"> & {
  readonly rest: (label: string) => Extract<Detail, { kind: "grid" }> | undefined;
};
/** A draw and the phase it runs in (the pass orders draws by phase, then by the order things recorded them). */
export type Staged = { readonly phase: Phase; readonly draw: Draw };
/** What a thing recorded: its draws, or what it cannot record yet, by name. */
export type Recorded = { readonly kind: "draws"; readonly draws: readonly Staged[] } | { readonly kind: "unported"; readonly what: string };

export const NONE: Recorded = { kind: "draws", draws: [] };
export const unported = (what: string): Recorded => ({ kind: "unported", what });

// Raw functions of the screen the roles read (the prelude's `frame`): Rust's emit carries them as raw Fns.
const raw = wgsl`
fn screen_dir_at(uv: vec2<f32>) -> vec3<f32> {
  let ndc = vec2<f32>(uv.x * 2.0 - 1.0, 1.0 - uv.y * 2.0);
  let a = frame.inv_view_proj * vec4<f32>(ndc, 0.0, 1.0);
  let b = frame.inv_view_proj * vec4<f32>(ndc, 1.0, 1.0);
  return normalize(b.xyz / b.w - a.xyz / a.w);
}
fn pixel_of_point(p: vec3<f32>) -> vec2<f32> {
  let c = frame.view_proj * vec4<f32>(p, 1.0);
  return vec2<f32>((c.x / c.w * 0.5 + 0.5) * frame.viewport.x, (0.5 - c.y / c.w * 0.5) * frame.viewport.y);
}
fn view_aspect() -> f32 { return frame.viewport.x / frame.viewport.y; }
fn lod_view(p: vec3<f32>) -> vec4<f32> {
  let v = frame.view_proj;
  let persp = length(vec3<f32>(v[0][3], v[1][3], v[2][3]));
  let r2 = vec4<f32>(v[0][2], v[1][2], v[2][2], v[3][2]);
  let near = select(0.0, -dot(r2, vec4<f32>(frame.eye.xyz, 1.0)) / dot(r2.xyz, frame.forward.xyz), persp > 0.0);
  let fy = length(vec3<f32>(v[0][1], v[1][1], v[2][1]));
  return vec4<f32>((v * vec4<f32>(p, 1.0)).w, persp, near, frame.viewport.y * fy * 0.5);
}
fn sea_footprint_at(p: vec3<f32>) -> f32 { return pixel_world(p) / max(abs(view_dir(p).y), 0.02); }`;
/** Longest extent of one pixel's footprint on the sea at p (the prelude's `sea_footprint`). */
export const seaFootprint = raw.sea_footprint_at! as (p: Expr<"vec3">) => F;
/** The unit view direction through screen uv ([0, 1]², y down), as the TS sky's full-screen triangle finds it. */
export const screenDir = raw.screen_dir_at! as (uv: Expr<"vec2">) => Expr<"vec3">;
/** The pixel position of a world point (the fragment's `@builtin(position).xy`). */
export const pixelOf = raw.pixel_of_point! as (p: Expr<"vec3">) => Expr<"vec2">;
const aspect = raw.view_aspect! as () => F;
const lodView = raw.lod_view! as (p: Expr<"vec3">) => Expr<"vec4">;

/**
 * A grid's cells role: TS src/host/detail.ts `chosenRuns` for a frame with no memory (a still, as R0 drew), on the GPU:
 * the level whose cells, at the copy's bounding sphere's nearest point, are at most CELL_PIXELS (the first coarser
 * level that is not stops the search); no cells for a copy behind the eye or under a pixel across. `bound(i)` is
 * the copy's (anchor, scale) as TS's bound table posts it. `grid` absent: the authored counts, always.
 */
export function cellsRole(name: string, counts: readonly [number, number], grid: Extract<Detail, { kind: "grid" }> | undefined, bound: (i: F) => Expr<"vec4">): Role {
  return role(`${name}_cells`, ["f32"], (i) => {
    if (grid === undefined) return vec2(counts[0], counts[1]);
    const b = bound(i), v = lodView(b.xyz), scale = abs(b.w), r = mul(scale, grid.radius);
    const [w, persp, near, k] = [v.x, v.y, v.z, v.w];
    const nearPx = div(k, max(sub(w, mul(r, persp)), near)), across = div(mul(r, mul(k, 2)), max(w, near));
    const levels = grid.levels;
    let chosen: Expr<"vec2"> = vec2(...levels[0]!.cells), going: Expr<"bool"> = lift<"bool">(true);
    for (let j = 1; j < levels.length; j++) {
      going = and(going, le(mul(mul(scale, levels[j]!.cell), nearPx), CELL_PIXELS));
      chosen = select(going, vec2(...levels[j]!.cells), chosen);
    }
    const hidden = or(le(add(w, mul(r, persp)), near), lt(across, 1));
    return select(hidden, vec2(0, 0), chosen);
  });
}

/** TS surface.ts `copy_id`: a stable identity in [0, 1) for copy i of a thing whose own is `id`. */
export const copyId = (id: number, i: F): F => fract(add(id, mul(i, 0.618034)));

/** The fields of a lit fragment's shade context at a point, as TS surface.ts's LIT_PRE gives them. */
export function litContext(ctx: RecordContext, at: { base: P3; normal: P3; world: P3; uv: Expr<"vec2">; anchor: P3; id: F }, about?: Parameters<typeof shadeContext>[3]): ShadeCtx {
  const sun = lift<"vec3">(sunDir(ctx.lighting.sun));
  const depth = sub(levelOf(ctx.ground)(at.world.xz), at.world.y);
  return shadeContext({ base: at.base, normal: at.normal, sun, view: mul(viewDir(at.world), -1), world: at.world, depth, uv: at.uv, anchor: at.anchor, id: at.id }, ctx.lighting, ctx.style, about);
}

/** The ink hull's push: the style's world width, never under 1.5 pixels, none for a width of 0 (TS `ink_width`). */
export function inkWidth(style: Style, p: P3): F {
  const w = lift<"f32">(style.outline as Expr<"f32">);
  return select(gt(w, 0), max(w, mul(pixelWorld(p), 1.5)), 0);
}

/** The style's sky along a direction, with the scene's sun. */
export function skyOf(ctx: RecordContext, dir: Expr<"vec3">): Expr<"vec3"> {
  return ctx.style.sky({ dir, sun: lift<"vec3">(sunDir(ctx.lighting.sun)), lights: foldAt(sunOnly(ctx.lighting), { kind: "point", at: dir }) });
}

/** A paint role over the template's (p, n, uv, id). */
export const paintRole = (name: string, f: (p: P3, n: P3, uv: Expr<"vec2">, id: F) => Expr<"vec4">): Role =>
  role(`${name}_paint`, ["vec3", "vec3", "vec2", "f32"], f);

/** A ray-found solid's per-copy box, as an instance table row: (centre, half of the largest side), proxy local ±1. */
export type RayBound = { readonly kind: "box"; readonly box: { readonly min: readonly [number, number, number]; readonly max: readonly [number, number, number] } } | { readonly kind: "copies"; readonly count: number; readonly box: (copy: F) => { readonly centre: P3; readonly half: P3 } };

/**
 * A solid found by a ray (draw.field's march, draw.traced's own hit): a Ray cover over its bound, the hit role building
 * TS's `Ray` from the template's span and width (a hit where the gap is under 0.3 of the ray's width there, TS
 * traced.ts's HIT), and the style's shade at the hit as paint (TS `trace_shade`: no surface coordinates, the anchor the
 * point). TS's inner-silhouette ink (a near miss within the ink width) is not drawn.
 */
export function rayDraw(ctx: RecordContext, name: string, bound: RayBound, hit: (ray: Ray, copy: F) => Traced, paint: (p: P3, n: P3, d: P3, copy: F) => P3 | Painted): Recorded {
  const hitRole = role(`${name}_hit`, ["vec3", "vec3", "vec2", "vec2", "f32"], (o, d, span, width, copy) => {
    const ray: Ray = { o, d, t0: span.x, t1: span.y, width: (t) => add(width.x, mul(width.y, t)) };
    const h = hit(ray, copy);
    return vec4(h.normal, select(le(h.gap, mul(ray.width(h.t), 0.3)), h.t, -1));
  });
  const id = nameId(name);
  const paintR = paintRole(name, (p, n, _uv, copy) => {
    const said = classify(name, paint(p, n, viewDir(p), copy));
    const sc = litContext(ctx, { base: said.base, normal: n, world: p, uv: vec2(0, 0), anchor: p, id: copyId(id, copy) }, { thing: name, ...said.said });
    return vec4(answered(said.answer, sc, ctx.style.block === undefined ? ctx.style.shade(sc) : ctx.style.block.body(sc)), ctx.style.mark(sc));
  });
  const proxy = ((): Extract<Cover, { kind: "ray" }>["proxy"] => {
    switch (bound.kind) {
      case "box": return { kind: "box", min: bound.box.min, max: bound.box.max };
      case "copies": {
        const table: TableSource = { name: `${name}_boxes`, lanes: 1, entries: bound.count, role: "data", program: mapOf(bound.count, (i) => { const b = bound.box(i); return [vec4(b.centre, max(abs(b.half.x), max(abs(b.half.y), abs(b.half.z))))]; }) };
        return { kind: "instances", table, local: [[-1, -1, -1], [1, 1, 1]] };
      }
    }
  })();
  const draw: Draw = { name, layer: "subject", raster: { cull: "back", depthWrite: true, blend: "opaque" }, paint: paintR, cover: { kind: "ray", proxy, find: { kind: "hit", hit: hitRole } } };
  return { kind: "draws", draws: [{ phase: "opaque", draw }] };
}

/** The sky: a full-screen draw behind everything (TS sky.ts). Below the horizon off the sea the ray is levelled. */
export function skyDraw(ctx: RecordContext): Staged {
  const paint = paintRole("sky", (_p, _n, uv) => {
    const d = screenDir(uv);
    const dir = ((): Expr<"vec3"> => {
      switch (ctx.ground.kind) {
        case "sea": return d;
        case "plane": case "nothing": return normalizeOf(vec3(d.x, max(d.y, 0), add(d.z, 1e-7)));
      }
    })();
    return vec4(skyOf(ctx, dir), 1);
  });
  return { phase: "background", draw: { name: "sky", layer: "surroundings", raster: { cull: "none", depthWrite: false, blend: "opaque" }, paint, cover: { kind: "screen" } } };
}
const normalizeOf = (v: Expr<"vec3">): Expr<"vec3"> => mul(v, lift<"f32">(1).div(length(v))) as Expr<"vec3">;

/** A plane ground (TS ground.ts): the ray meets y = 0 in closed form, coloured by the style's `ground`. */
function planeGround(ctx: RecordContext): Pass["ground"] {
  const hit = role("ground_hit", ["vec3", "vec3", "vec2", "vec2", "f32"], (o, d) => {
    const t = mul(o.y, -1).div(d.y);
    return vec4(0, 1, 0, select(gt(t, 0), t, -1));
  });
  const paint = paintRole("ground", (p) => {
    const sun = lift<"vec3">(sunDir(ctx.lighting.sun));
    const at = { world: p, view: mul(viewDir(p), -1), sun, distance: distance(p, eyePosition()), shadow: lift<"f32">(0), lights: foldAt(ctx.lighting, { kind: "surface", at: p, normal: lift<"vec3">([0, 1, 0]) }) };
    return vec4(ctx.style.ground(at), 1);
  });
  return { kind: "traced", hit, paint };
}

/** A scene camera as a camera decl (TS src/host/camera.ts's conventions; loft-gpu camera.wgsl builds a look). */
function cameraOf(name: string, c: SceneCamera): Camera {
  switch (c.kind) {
    case "perspective": return { name, kind: "look", eye: lift<"vec3">([...c.eye]), target: lift<"vec3">([...c.target]), up: lift<"vec3">([0, 1, 0]), fovy: lift<"f32">(c.fovY) };
    case "orthographic": {
      // clip from world = orthographic(height, aspect, near, far) * lookAt(eye, target); only x reads the aspect.
      const d = [0, 1, 2].map((k) => c.target[k]! - c.eye[k]!), dist = Math.hypot(...d), f = d.map((x) => x / dist);
      const ref = Math.abs(f[1]!) > 0.999999 ? [0, 0, -Math.sign(f[1]!)] : [0, 1, 0];
      const cross = (a: number[], b: number[]) => [a[1]! * b[2]! - a[2]! * b[1]!, a[2]! * b[0]! - a[0]! * b[2]!, a[0]! * b[1]! - a[1]! * b[0]!];
      const r0 = cross(f, ref), rl = Math.hypot(...r0), r = r0.map((x) => x / rl), u = cross(r, f);
      const dot = (a: number[], b: readonly number[]) => a[0]! * b[0]! + a[1]! * b[1]! + a[2]! * b[2]!;
      const near = dist / 100, far = dist * 100, sx = 2 / c.height, z = 1 / (near - far), zw = near / (near - far);
      const view = [0, 1, 2, 3].map((k) => (k < 3 ? [r[k]!, u[k]!, -f[k]!, 0] : [-dot(r, c.eye), -dot(u, c.eye), dot(f, c.eye), 1]));
      const col = (v: number[]): Expr<"vec4"> => vec4(mul(sx * v[0]!, lift<"f32">(1).div(aspect())), sx * v[1]!, z * v[2]! + zw * v[3]!, v[3]!);
      return { name, kind: "matrix", cols: [col(view[0]!), col(view[1]!), col(view[2]!), col(view[3]!)] };
    }
  }
}

/**
 * A style's post pass (TS POST_WGSL): its fragment text as a raw function of the screen uv, drawn as a Screen cover in a
 * pass of its own that loads the view (the host resolves the picture so far before it; the prelude's `post_color`
 * samples it). A text that reads the TS scene's style functions (`sea_height`) cannot be lowered so; helpers that read
 * them and that the pass never reaches (POST_COMMON's `sty_water`, carried by every post pass) are left out first.
 */
function postPass(p: PostSpec, k: number, camera: Camera): { readonly kind: "pass"; readonly pass: Pass } | { readonly kind: "unported"; readonly what: string } {
  const head = /@fragment\s+fn\s+fs\s*\(\s*in\s*:\s*PostIn\s*\)\s*->\s*@location\(0\)\s*vec4<f32>/;
  if (!head.test(p.wgsl)) return { kind: "unported", what: "its text has no `@fragment fn fs(in: PostIn) -> @location(0) vec4<f32>`" };
  const text = withoutStyleReaders(p.wgsl);
  const reads = [...new Set([...text.matchAll(STYLE_CALL)].map((m) => m[1]!))];
  if (reads.length > 0) return { kind: "unported", what: `it reads the TS scene's style functions (${reads.join(", ")})` };
  const name = `post${k}`;
  const fn: LibFn = {
    name: `${name}_at`, ret: "vec4", js: "gpu-only",
    wgsl: `struct PostIn { px: vec4<f32>, uv: vec2<f32> };\n${text.replace(head, `fn ${name}(in: PostIn) -> vec4<f32>`)}\nfn ${name}_at(uv: vec2<f32>) -> vec4<f32> { return ${name}(PostIn(vec4<f32>(uv * frame.viewport.xy, 0.0, 1.0), uv)); }`,
  };
  const paint = paintRole(name, (_p, _n, uv) => lib<"vec4">(fn, uv));
  const draw: Draw = { name, layer: "surroundings", raster: { cull: "none", depthWrite: false, blend: "opaque" }, paint, cover: { kind: "screen" } };
  return { kind: "pass", pass: { camera, draws: [draw], clear: { kind: "load" }, ground: { kind: "none" }, outline: { kind: "none" } } };
}

/** A call of a function only the TS scene's style defines. */
const STYLE_CALL = /\b(sea_height|sea_gradient|shade|ink|sky)\s*\(/g;

/**
 * `wgsl` without its top-level helper functions that call a style function, directly or through another such helper.
 * A helper the rest still calls stays in the rest's calls, so the caller's check above names what it reads.
 */
function withoutStyleReaders(wgsl: string): string {
  const fns: { name: string; start: number; end: number }[] = [];
  const head = /(^|\n)fn\s+([A-Za-z_]\w*)\s*\(/g;
  for (let m = head.exec(wgsl); m !== null; m = head.exec(wgsl)) {
    let i = wgsl.indexOf("{", head.lastIndex), depth = 0;
    for (; i < wgsl.length; i++) if (wgsl[i] === "{") depth++; else if (wgsl[i] === "}" && --depth === 0) break;
    fns.push({ name: m[2]!, start: m.index + m[1]!.length, end: i + 1 });
    head.lastIndex = i + 1;
  }
  const calls = (body: string, names: ReadonlySet<string>) => [...names].some((n) => new RegExp(`\\b${n}\\s*\\(`).test(body));
  const bad = new Set<string>();
  for (let grew = true; grew;) {
    grew = false;
    for (const f of fns) {
      const body = wgsl.slice(f.start, f.end);
      if (!bad.has(f.name) && (new RegExp(STYLE_CALL.source).test(body) || calls(body, bad))) { bad.add(f.name); grew = true; }
    }
  }
  const dropped = fns.filter((f) => bad.has(f.name));
  const rest = dropped.reduceRight((t, f) => t.slice(0, f.start) + t.slice(f.end), wgsl);
  // A dropped helper the pass still calls: keep the text whole, so the check names the style function it reads.
  return calls(rest, bad) ? wgsl : rest;
}

/** The style's clear colour, as the TS host clears the view. */
const clearOf = (style: Style): readonly [number, number, number, number] => [...style.clear, 1];

/** Everything scene-level the Rust path draws: the cameras, a sky pass, then one view pass of every thing's draws. */
export function sceneDecls(scene: Scene, artefact: Pick<Artefact, "draws">): { readonly scene: Decls; readonly unported: readonly string[] } {
  const grids = new Map(artefact.draws.flatMap((d) => (d.detail.kind === "grid" ? [[d.label, d.detail] as const] : [])));
  const ctx: RecordContext = { style: scene.style, ground: scene.ground, sun: scene.sun, sunAt: scene.sunAt, lighting: scene.lighting, things: scene.things, outline: scene.outline, rest: (label) => grids.get(label) };
  const cameras = Object.entries(scene.cameras).map(([name, c]) => cameraOf(name, c));
  // A pass names one camera; the view's camera is the host's choice among the bundle's (loft-gpu `use_camera`).
  const view = cameras[0] ?? cameraOf("front", { kind: "perspective", eye: [0, 3, 18], target: [0, 1, 0], fovY: 0.6 });
  const staged: Staged[] = [], unportedList: string[] = [];
  for (const t of scene.things) {
    const r = t.record === undefined ? unported("a drawable of its own (it emits WGSL for the TS engine and records no draw)") : t.record(ctx);
    switch (r.kind) {
      case "draws": staged.push(...r.draws); break;
      case "unported": unportedList.push(`${t.name}: ${r.what}`); break;
    }
  }
  const posts = scene.style.post.flatMap((p, k): Pass[] => {
    const r = postPass(p, k, view);
    switch (r.kind) {
      case "pass": return [r.pass];
      case "unported": unportedList.push(`style ${scene.style.name} post pass "${p.label}": ${r.what}`); return [];
    }
  });
  const ordered = PHASES.flatMap((ph) => staged.filter((s) => s.phase === ph).map((s) => s.draw));
  const background = ordered.filter((d) => staged.find((s) => s.draw === d)!.phase === "background");
  const rest = ordered.filter((d) => !background.includes(d));
  const ground = ((): Pass["ground"] => {
    switch (scene.ground.kind) {
      case "plane": return planeGround(ctx);
      case "sea": case "nothing": return { kind: "none" };
    }
  })();
  // Two passes: the background (the sky), then the ground (a plane's, traced, which the host draws before a pass's
  // draws) and everything else over it.
  const passes: Pass[] = [
    { camera: view, draws: background, clear: { kind: "colour", rgba: clearOf(scene.style) }, ground: { kind: "none" }, outline: { kind: "none" } },
    { camera: view, draws: rest, clear: { kind: "load" }, ground, outline: { kind: "none" } },
    ...posts,
  ];
  return { scene: { cameras, passes, checks: [], solves: [] }, unported: unportedList };
}

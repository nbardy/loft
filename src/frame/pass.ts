// `pass`: a picture drawn off screen this frame (docs/proposals/headroom.md section 5).
//
// A pass is found, not registered. `pass(spec)` makes a value whose `at(uv)` is a GPU-only library function naming the
// pass, and whose `project(p)` reads the pass's camera from a one-entry table. The compiler collects the passes named
// by the library functions of every emitted shader (src/compile/passes.ts), so a pass is in a scene's files exactly
// when something drawn reads it, and a scene that reads none compiles to the same bytes as before passes existed.
//
// The camera is a closed form of the frame like any other table: twelve vec4 lanes laid out as the prelude's
// `struct Frame` (view-projection, its inverse, eye, forward, viewport, time), filled once a frame by a pose stage and
// copied into the uniform the pass binds as its group 0 (src/host/lower.ts). So every drawable draws into a pass
// with the pipelines it draws the view with, reading its camera from `frame` as it always does.

import { SceneError, shown } from "../errors.ts";
import {
  abs, add, cross, div, dot, f, fetch, gt, length, lib, lift, listOf, mul, neg, normalize, round, select, sign, sub, tan, time, variable, vec3, vec4,
  type Expr, type In, type LibFn, type ProgramTable,
} from "../expr.ts";
import type { TextureSize } from "../artefact.ts";

type Rgb = readonly [number, number, number];

/**
 * How a pass looks at the world. A mechanism, so a closed list, with "your own" as the last variant.
 * - look: from `eye` toward `target`; `fovY` (perspective, radians) or `halfHeight` (orthographic, world units).
 *   `up` defaults to world y (world -z looking straight down). `near` and `far` default to the distance d from eye to
 *   target as d / 100 and 100 d (perspective) or 0 and 2 d (orthographic, so a shadow map's depth spans 4 d of world).
 * - mirror: the view's own camera reflected in the plane dot(normal, p) = offset, clipped at it by an oblique near plane,
 *   so nothing behind the mirror shows in it. The picture is flipped left to right (so triangles keep their winding);
 *   read it through `project`, which flips the same way.
 * - matrix: your own view-projection, four columns (clip = columns * (p, 1), WebGPU clip space, depth 0 near to 1 far),
 *   and the eye it is from (for view directions and pixel sizes).
 * Every field is an expression: a camera may follow the sun, a creature's eye or the view. It is computed once a frame
 * on the GPU, and on the CPU for `project` (except a mirror's, which reads the view's camera, and a camera whose
 * aspect follows a view-sized pass).
 */
export type PassCamera =
  | {
    readonly kind: "look"; readonly eye: In<"vec3">; readonly target: In<"vec3">; readonly up?: In<"vec3">;
    readonly lens: { readonly fovY: In<"f32"> } | { readonly halfHeight: In<"f32"> }; readonly near?: number; readonly far?: number;
  }
  | { readonly kind: "mirror"; readonly normal: Rgb; readonly offset: number }
  | { readonly kind: "matrix"; readonly columns: readonly [In<"vec4">, In<"vec4">, In<"vec4">, In<"vec4">]; readonly eye: In<"vec3"> };

export type PassSpec = {
  readonly name: string;
  readonly size: "view" | { readonly scale: number } | readonly [number, number];
} & (
  | { readonly what?: "scene"; readonly camera: PassCamera; readonly keep?: "colour" | "depth" | "light" }
  | { readonly what: "screen"; readonly colour: (uv: Expr<"vec2">) => Expr<"vec4"> }
);

export type Pass = {
  readonly name: string;
  /** The picture at uv (0..1, v down): colour, or the depth in x. Bilinear and clamped at the edges for colour; nearest for depth, and 1 (nothing there) outside the picture. GPU-only. */
  at(uv: In<"vec2">): Expr<"vec4">;
  /** Where a world point lands: xy its uv (0..1, v down), z its depth (0 near to 1 far, the depth `at` holds). An expression: runs on the CPU. */
  project(p: In<"vec3">): Expr<"vec3">;
};

/** What a pass keeps, and the format its picture is held in. */
export type PassKeep = "colour" | "depth" | "light";

/** A pass as the compiler reads it: classified once, here. */
export type PassDef = {
  readonly name: string;
  readonly size: TextureSize;
  /** The WGSL name of its texture (group 3, src/artefact.ts `passBindingsWgsl`). */
  readonly texture: string;
  readonly what:
    | { readonly kind: "scene"; readonly keep: PassKeep; readonly camera: ProgramTable }
    | { readonly kind: "screen"; readonly colour: Expr<"vec4">; readonly uv: string };
};

const byLib = new WeakMap<LibFn, PassDef>();
const byName = new Map<string, PassDef>();

/** The pass a library function reads, if it is a pass's `at`. */
export function passOf(fn: LibFn): PassDef | undefined {
  return byLib.get(fn);
}

/** Lanes of the camera table: the prelude's `struct Frame`, as 12 vec4s. */
export const FRAME_LANES = 12;

/** Column-major 4x4 of scalar expressions: element (row r, column c) at c * 4 + r. */
type M4 = readonly Expr<"f32">[];
const F0 = f(0), F1 = f(1);

function mulM(a: M4, b: M4): M4 {
  const out: Expr<"f32">[] = [];
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) {
    let s: Expr<"f32"> = mul(a[r]!, b[c * 4]!);
    for (let k = 1; k < 4; k++) s = add(s, mul(a[k * 4 + r]!, b[c * 4 + k]!));
    out.push(s);
  }
  return out;
}

/** The general inverse by cofactors (the same formula as src/host/camera.ts `invert`), for a camera given as a matrix. */
function invM(m: M4): M4 {
  const [a00, a01, a02, a03, a10, a11, a12, a13, a20, a21, a22, a23, a30, a31, a32, a33] = m as [Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">, Expr<"f32">];
  const d2 = (a: Expr<"f32">, b: Expr<"f32">, c: Expr<"f32">, e: Expr<"f32">) => sub(mul(a, b), mul(c, e));
  const b00 = d2(a00, a11, a01, a10), b01 = d2(a00, a12, a02, a10), b02 = d2(a00, a13, a03, a10);
  const b03 = d2(a01, a12, a02, a11), b04 = d2(a01, a13, a03, a11), b05 = d2(a02, a13, a03, a12);
  const b06 = d2(a20, a31, a21, a30), b07 = d2(a20, a32, a22, a30), b08 = d2(a20, a33, a23, a30);
  const b09 = d2(a21, a32, a22, a31), b10 = d2(a21, a33, a23, a31), b11 = d2(a22, a33, a23, a32);
  const det = add(sub(add(sub(add(mul(b00, b11), neg(mul(b01, b10))), neg(mul(b02, b09))), neg(mul(b03, b08))), mul(b04, b07)), mul(b05, b06));
  const id = div(1, det);
  const t3 = (x: Expr<"f32">, p: Expr<"f32">, y: Expr<"f32">, q: Expr<"f32">, z: Expr<"f32">, r: Expr<"f32">) => mul(add(sub(mul(x, p), mul(y, q)), mul(z, r)), id);
  return [
    t3(a11, b11, a12, b10, a13, b09), t3(a02, b10, a01, b11, neg(a03), b09),
    t3(a31, b05, a32, b04, a33, b03), t3(a22, b04, a21, b05, neg(a23), b03),
    t3(a12, b08, a10, b11, neg(a13), b07), t3(a00, b11, a02, b08, a03, b07),
    t3(a32, b02, a30, b05, neg(a33), b01), t3(a20, b05, a22, b02, a23, b01),
    t3(a10, b10, a11, b08, a13, b06), t3(a01, b08, a00, b10, neg(a03), b06),
    t3(a30, b04, a31, b02, a33, b00), t3(a21, b02, a20, b04, neg(a23), b00),
    t3(a11, b07, a10, b09, neg(a12), b06), t3(a00, b09, a01, b07, a02, b06),
    t3(a31, b01, a30, b03, neg(a32), b00), t3(a20, b03, a21, b01, a22, b00),
  ].map((e) => e as Expr<"f32">);
}

const cols = (m: M4): Expr<"vec4">[] => [0, 1, 2, 3].map((c) => vec4(m[c * 4]!, m[c * 4 + 1]!, m[c * 4 + 2]!, m[c * 4 + 3]!));
const fromCols = (c: readonly Expr<"vec4">[]): M4 => c.flatMap((v) => [v.x, v.y, v.z, v.w]);
const diag = (d: readonly Expr<"f32">[]): Expr<"f32">[] => Array.from({ length: 16 }, (_, k) => (k % 5 === 0 ? d[k / 5]! : F0));

/**
 * Lane k of the view's own `struct Frame` (0-3 view_proj columns, 4-7 its inverse, 8 eye, 9 forward, 10 viewport,
 * 11 time): what a mirror reflects and a view-sized pass measures itself by. A pose stage reads the view's frame.
 */
const VIEW_FRAME: LibFn = {
  name: "loft_view_frame", ret: "vec4", js: "gpu-only",
  wgsl: /* wgsl */ `fn loft_view_frame(k: f32) -> vec4<f32> {
  switch u32(k) {
    case 0u, 1u, 2u, 3u: { return frame.view_proj[u32(k)]; }
    case 4u, 5u, 6u, 7u: { return frame.inv_view_proj[u32(k) - 4u]; }
    case 8u: { return frame.eye; }
    case 9u: { return frame.forward; }
    case 10u: { return frame.viewport; }
    default: { return frame.time; }
  }
}`,
};
const viewLane = (k: number): Expr<"vec4"> => lib<"vec4">(VIEW_FRAME, k);

/** The pass's size in pixels as an expression: a number, or the view's size times the scale, rounded (as a loader sizes it). */
function pixels(size: TextureSize): readonly [Expr<"f32">, Expr<"f32">] {
  switch (size.kind) {
    case "fixed": return [f(size.width), f(size.height)];
    case "view": {
      const v = viewLane(10);
      return [round(mul(v.x, size.scale)), round(mul(v.y, size.scale))];
    }
  }
}

/** The 12 lanes of a `struct Frame`. */
function frameLanes(vp: M4, inv: M4, eye: Expr<"vec3">, perspective: Expr<"f32">, forward: Expr<"vec3">, size: TextureSize): readonly Expr<"vec4">[] {
  const [w, h] = pixels(size);
  return [...cols(vp), ...cols(inv), vec4(eye, perspective), vec4(forward, 0), vec4(w, h, div(1, w), div(1, h)), vec4(time, 0, 0, 0)];
}

function lookLanes(c: Extract<PassCamera, { kind: "look" }>, size: TextureSize): readonly Expr<"vec4">[] {
  const eye = lift<"vec3">(c.eye), target = lift<"vec3">(c.target);
  const forward = normalize(sub(target, eye));
  // As src/host/camera.ts: world y is up; looking straight up or down, -z (times the sign) takes its place.
  const ref = c.up === undefined ? select(gt(abs(forward.y), 0.999999), vec3(0, 0, neg(sign(forward.y))), vec3(0, 1, 0)) : lift<"vec3">(c.up);
  const right = normalize(cross(forward, ref)), up = cross(right, forward);
  const rows = [right, up, neg(forward)];
  const view: Expr<"f32">[] = Array.from({ length: 16 }, () => F0);
  rows.forEach((r, i) => { view[i] = r.x; view[4 + i] = r.y; view[8 + i] = r.z; view[12 + i] = neg(dot(r, eye)); });
  view[15] = F1;
  // The view's inverse: the rotation's transpose, and the eye.
  const viewInv: Expr<"f32">[] = Array.from({ length: 16 }, () => F0);
  rows.forEach((r, c2) => { viewInv[c2 * 4] = r.x; viewInv[c2 * 4 + 1] = r.y; viewInv[c2 * 4 + 2] = r.z; });
  viewInv[12] = eye.x; viewInv[13] = eye.y; viewInv[14] = eye.z; viewInv[15] = F1;
  const d = length(sub(target, eye));
  const [w, h] = pixels(size);
  const aspect = div(w, h);
  const proj: Expr<"f32">[] = Array.from({ length: 16 }, () => F0), projInv: Expr<"f32">[] = Array.from({ length: 16 }, () => F0);
  let perspective: Expr<"f32">;
  if ("fovY" in c.lens) {
    const near = c.near === undefined ? div(d, 100) : f(c.near), far = c.far === undefined ? mul(d, 100) : f(c.far);
    const fy = div(1, tan(mul(lift<"f32">(c.lens.fovY), 0.5)));
    const a = div(far, sub(near, far)), b = div(mul(near, far), sub(near, far));
    proj[0] = div(fy, aspect); proj[5] = fy; proj[10] = a; proj[11] = f(-1); proj[14] = b;
    // Inverse of the perspective: x = X / m0, y = Y / m5, z = -W, w = (Z + a W) / b.
    projInv[0] = div(aspect, fy); projInv[5] = div(1, fy); projInv[14] = f(-1); projInv[11] = div(1, b); projInv[15] = div(a, b);
    perspective = F1;
  } else {
    const near = c.near === undefined ? F0 : f(c.near), far = c.far === undefined ? mul(d, 2) : f(c.far);
    const hh = lift<"f32">(c.lens.halfHeight);
    const a = div(1, sub(near, far)), b = div(near, sub(near, far));
    proj[0] = div(1, mul(hh, aspect)); proj[5] = div(1, hh); proj[10] = a; proj[14] = b; proj[15] = F1;
    projInv[0] = mul(hh, aspect); projInv[5] = hh; projInv[10] = div(1, a); projInv[14] = neg(div(b, a)); projInv[15] = F1;
    perspective = F0;
  }
  return frameLanes(mulM(proj, view), mulM(viewInv, projInv), eye, perspective, forward, size);
}

/**
 * The view's camera reflected in dot(n, p) = offset (Lengyel 2005's oblique near plane, in WebGPU's 0..1 depth):
 * clip = T F VP R p, with R the reflection, F a flip of x (a reflection reverses winding; the flip restores it, so
 * culling and the drawables' own winding stay right), and T replacing the depth row so the near plane is the mirror.
 */
function mirrorLanes(c: Extract<PassCamera, { kind: "mirror" }>, size: TextureSize): readonly Expr<"vec4">[] {
  const n = c.normal, o = c.offset;
  // R: p - 2 (n . p - o) n, column-major; its own inverse.
  const R: Expr<"f32">[] = Array.from({ length: 16 }, () => F0);
  for (let col = 0; col < 3; col++) for (let r = 0; r < 3; r++) R[col * 4 + r] = f((r === col ? 1 : 0) - 2 * n[r]! * n[col]!);
  for (let r = 0; r < 3; r++) R[12 + r] = f(2 * o * n[r]!);
  R[15] = F1;
  const Fx = diag([f(-1), F1, F1, F1]);
  const vp = fromCols([viewLane(0), viewLane(1), viewLane(2), viewLane(3)]);
  const vpInv = fromCols([viewLane(4), viewLane(5), viewLane(6), viewLane(7)]);
  const m0 = mulM(Fx, mulM(vp, R)), m0Inv = mulM(R, mulM(vpInv, Fx));
  // The mirror plane in clip space: C_c = m0^-T C, C = (n, -o) (what is in front of the mirror has C . p > 0).
  const C = [f(n[0]), f(n[1]), f(n[2]), f(-o)];
  const cc = [0, 1, 2, 3].map((j) => C.reduce<Expr<"f32">>((s, ci, i) => add(s, mul(m0Inv[j * 4 + i]!, ci)), F0));
  // The far corner opposite the near plane, and the new depth row v = C_c / (C_c . Q).
  const q = [sign(cc[0]!), sign(cc[1]!), F1, F1];
  const cq = cc.reduce<Expr<"f32">>((s, x, i) => add(s, mul(x, q[i]!)), F0);
  const v = cc.map((x) => div(x, cq));
  const m = [...m0];
  for (let col = 0; col < 4; col++) m[col * 4 + 2] = v.reduce<Expr<"f32">>((s, vk, k) => add(s, mul(vk, m0[col * 4 + k]!)), F0);
  // T is the identity with row 2 = v; its inverse has row 2 = (-v0, -v1, 1, -v3) / v2.
  const tInv: Expr<"f32">[] = diag([F1, F1, div(1, v[2]!), F1]);
  tInv[2] = neg(div(v[0]!, v[2]!)); tInv[6] = neg(div(v[1]!, v[2]!)); tInv[14] = neg(div(v[3]!, v[2]!));
  const eye4 = viewLane(8), fwd = viewLane(9).xyz;
  const reflect = (p: Expr<"vec3">): Expr<"vec3"> => sub(p, mul(lift<"vec3">([...n]), mul(sub(dot(lift<"vec3">([...n]), p), o), 2)));
  const reflectDir = (d: Expr<"vec3">): Expr<"vec3"> => sub(d, mul(lift<"vec3">([...n]), mul(dot(lift<"vec3">([...n]), d), 2)));
  return frameLanes(m, mulM(m0Inv, tInv), reflect(eye4.xyz), eye4.w, reflectDir(fwd), size);
}

function matrixLanes(c: Extract<PassCamera, { kind: "matrix" }>, size: TextureSize): readonly Expr<"vec4">[] {
  const m = fromCols(c.columns.map((x) => lift<"vec4">(x)));
  // Clip w's row is the eye's forward direction for a perspective camera and zero for an orthographic one, whose
  // forward is then along the depth row (src/host/camera.ts builds both that way).
  const row3 = vec3(m[3]!, m[7]!, m[11]!), row2 = vec3(m[2]!, m[6]!, m[10]!);
  const perspective = select(gt(length(row3), 0), 1, 0);
  return frameLanes(m, invM(m), lift<"vec3">(c.eye), perspective, normalize(select(gt(length(row3), 0), row3, row2)), size);
}

/** The camera's 12 lanes: one handler per kind. */
function cameraLanes(c: PassCamera, size: TextureSize): readonly Expr<"vec4">[] {
  switch (c.kind) {
    case "look": return lookLanes(c, size);
    case "mirror": return mirrorLanes(c, size);
    case "matrix": return matrixLanes(c, size);
  }
}

const NAME = /^[A-Za-z][A-Za-z0-9_]*$/;
/** WebGPU's default `maxTextureDimension2D`. */
const MAX_SIDE = 8192;

function sizeOf(where: string, s: PassSpec["size"]): TextureSize {
  if (s === "view") return { kind: "view", scale: 1 };
  if (Array.isArray(s)) {
    const [w, h] = s as readonly [number, number];
    if (s.length !== 2 || ![w, h].every((x) => Number.isInteger(x) && x >= 1 && x <= MAX_SIDE)) {
      throw new SceneError(where, `size ${shown(s)}`, `two whole numbers of pixels from 1 to ${MAX_SIDE}`, "size: [2048, 2048]");
    }
    return { kind: "fixed", width: w, height: h };
  }
  const scale = (s as { readonly scale: number }).scale;
  if (!(Number.isFinite(scale) && scale > 0 && scale <= 4)) throw new SceneError(where, `size { scale: ${shown(scale)} }`, "a share of the view's size above 0 and at most 4", "size: { scale: 0.5 }");
  return { kind: "view", scale };
}

function cameraOf(where: string, c: PassCamera): PassCamera {
  switch (c.kind) {
    case "look": {
      const fov = "fovY" in c.lens ? c.lens.fovY : undefined;
      if (typeof fov === "number" && !(fov > 0 && fov < Math.PI)) throw new SceneError(where, `lens { fovY: ${fov} }`, "an angle in radians between 0 and pi", "lens: { fovY: 0.8 }");
      const hh = "halfHeight" in c.lens ? c.lens.halfHeight : undefined;
      if (typeof hh === "number" && !(hh > 0)) throw new SceneError(where, `lens { halfHeight: ${hh} }`, "a positive half height in world units", "lens: { halfHeight: 6 }");
      if (c.near !== undefined && c.far !== undefined && !(c.far > c.near && c.near >= 0)) throw new SceneError(where, `near ${c.near}, far ${c.far}`, "0 <= near < far", "leave both out for the defaults");
      return c;
    }
    case "mirror": {
      const len = Math.hypot(...c.normal);
      if (!(c.normal.length === 3 && Number.isFinite(len) && len > 0)) throw new SceneError(where, `normal ${shown(c.normal)}`, "a non-zero direction [x, y, z]", "normal: [0, 1, 0]");
      if (!Number.isFinite(c.offset)) throw new SceneError(where, `offset ${shown(c.offset)}`, "a finite number", "offset: 0");
      // Classified once: a unit normal from here on.
      return { kind: "mirror", normal: [c.normal[0] / len, c.normal[1] / len, c.normal[2] / len], offset: c.offset / len };
    }
    case "matrix": return c;
  }
}

/**
 * A picture drawn off screen this frame. Nothing registers it: it is in the scene's files exactly when an expression
 * that is drawn reads it (`at`), and it is drawn before every draw that reads it; a draw that reads a pass is never
 * drawn into that pass (`loft` notes which were left out). `project` is the same camera as arithmetic, so a check can
 * ask where anything lands in the picture before anything is drawn.
 *
 * - name: letters, digits and _; unique in a process.
 * - size: the view's size ("view"), a share of it ({ scale }), or fixed pixels ([w, h]).
 * - what: "scene" (the default when `camera` is given) draws the scene's things from `camera` (`PassCamera`); "screen"
 *   is a full-screen expression of the pixel's uv (0..1, v down), held in 16-bit float, which may read other passes
 *   (a blur, a smaller copy, one level of a bloom chain).
 * - keep (scene only): "colour" (default) is the picture as the style draws it, through the view's own pipelines (no
 *   new pipeline); "depth" runs each thing's depth-only entry (coverage and depth, no shading) and keeps the depth, 0
 *   near to 1 far; "light" keeps colour in 16-bit float, for a style that writes values above 1.
 *
 * Cannot: read the previous frame (nothing carries over); scatter; draw a list of things other than the scene's; be
 * read on the CPU (`at` is GPU-only; `project` is not); be read by a table, a camera or a baked step. A pass draws each
 * thing at the level of detail the view chose and leaves out what the view leaves out (instances behind the eye or
 * under a pixel); a depth pass leaves out what has no depth entry (transparent draws, the ground, ink hulls,
 * and drawables that state none), and every pass leaves out the sea's grid, which is laid over the view's own image.
 * Going past it: raw WGSL may call `loft_pass_<name>_at(uv)` once an expression has read the pass. Relatives:
 * cascades (three `look` passes over nested squares); a normals picture for a custom ink (a scene pass under a style
 * whose shade returns the normal); six `look` passes as a probe's cube; still mode (`--samples`) for soft shadows
 * and blur without a pass.
 */
export function pass(spec: PassSpec): Pass {
  const where = `pass "${spec.name}"`;
  if (typeof spec.name !== "string" || !NAME.test(spec.name)) throw new SceneError("pass", `name ${shown(spec.name)}`, "letters, digits and _, starting with a letter", `pass({ name: "fromSun", ... })`);
  if (byName.has(spec.name)) throw new SceneError(where, "a second pass with this name", "one pass per name in a process", "give it another name");
  const size = sizeOf(where, spec.size);
  const texture = `loft_passtex_${spec.name}`;
  let def: PassDef;
  if (spec.what === "screen") {
    if (typeof spec.colour !== "function") throw new SceneError(where, `colour ${shown(spec.colour)}`, "a function of uv returning a vec4", "colour: (uv) => vec4(uv, 0, 1)");
    const uv = `loft_screen_uv`;
    def = { name: spec.name, size, texture, what: { kind: "screen", colour: spec.colour(variable(uv, "vec2")), uv } };
  } else {
    if (spec.camera === undefined) throw new SceneError(where, "no camera", "a camera for a scene pass, or what: \"screen\" with a colour", `camera: { kind: "look", eye: [0, 20, 0], target: [0, 0, 0], lens: { halfHeight: 6 } }`);
    const keep = spec.keep ?? "colour";
    if (!["colour", "depth", "light"].includes(keep)) throw new SceneError(where, `keep ${shown(keep)}`, "\"colour\", \"depth\" or \"light\"", `keep: "depth"`);
    const camera: ProgramTable = { name: `loft_pass_${spec.name}_camera`, lanes: FRAME_LANES, entries: 1, role: "data", program: listOf([cameraLanes(cameraOf(where, spec.camera), size)]) };
    def = { name: spec.name, size, texture, what: { kind: "scene", keep, camera } };
  }
  const reads = def.what.kind === "scene" && def.what.keep === "depth"
    ? /* wgsl */ `fn loft_pass_${spec.name}_at(uv: vec2<f32>) -> vec4<f32> {
  let s = vec2<i32>(textureDimensions(${texture}));
  let inside = all(uv >= vec2<f32>(0.0)) && all(uv < vec2<f32>(1.0));
  let c = clamp(vec2<i32>(floor(uv * vec2<f32>(s))), vec2<i32>(0), s - 1);
  return vec4<f32>(select(1.0, textureLoad(${texture}, c, 0).x, inside), 0.0, 0.0, 1.0);
}`
    : /* wgsl */ `fn loft_pass_${spec.name}_at(uv: vec2<f32>) -> vec4<f32> { return textureSampleLevel(${texture}, loft_pass_sampler, uv, 0.0); }`;
  const at: LibFn = { name: `loft_pass_${spec.name}_at`, ret: "vec4", wgsl: reads, js: "gpu-only" };
  byLib.set(at, def);
  byName.set(spec.name, def);
  const camera = def.what.kind === "scene" ? def.what.camera : undefined;
  return {
    name: spec.name,
    at: (uv) => lib<"vec4">(at, uv),
    project: (p) => {
      if (camera === undefined) throw new SceneError(where, "project on a screen pass", "a scene pass (a screen pass has no camera)", "read a screen pass with at(uv) only");
      const q = lift<"vec3">(p);
      const c = [0, 1, 2, 3].map((k) => fetch(camera, 0, k));
      const clip = add(add(add(mul(c[0]!, q.x), mul(c[1]!, q.y)), mul(c[2]!, q.z)), c[3]!);
      const ndc = div(clip.xyz, clip.w);
      return vec3(add(mul(ndc.x, 0.5), 0.5), sub(0.5, mul(ndc.y, 0.5)), ndc.z);
    },
  };
}

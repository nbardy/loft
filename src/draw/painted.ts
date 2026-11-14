// What a paint says, classified once at the boundary, and the functions a lit fragment calls for it.
//
// A paint returns a colour (`P3`) or a `Painted`: a colour (`base`) with named channels, an optional `scatter` (the
// surface's answer to one light) and an optional `shade` (a change to the finished picture). The style reads channels
// by name with a default (`ShadeCtx.channel`); the names it reads are recorded on its one evaluation. A thing that gives
// a channel the style reads, a `scatter` the style asks for, or a `shade`, gets its own copies of the style's functions
// in its own module (`look_<thing>`, `ink_<thing>`, `mark_<thing>`), evaluated with its channels in place of the
// defaults. Every other thing calls the shared `shade`, `ink` and `mark`, so a scene that says nothing more than a colour
// emits the same shader text as before channels existed.

import { Expr, leaves, lift, variable, withIdentities, type NumTy } from "../expr.ts";
import type { CompileContext, Shader } from "../compile/context.ts";
import { emitShaderFn, type FnSpec } from "../compile/bind.ts";
import { SceneError } from "../errors.ts";
import type { P3, Painted } from "../map/types.ts";
import { shadeVariables, styleReads, type ShadeCtx, type Said } from "../style.ts";

/** How a thing answers once the style has: as the style says, or through its own `shade`. */
export type Answer = { readonly kind: "styled" } | { readonly kind: "wrapped"; readonly wrap: (ctx: ShadeCtx, styled: P3) => P3 };

/** A paint's result, classified: its colour, what it says (channels, scatter) and how it answers. */
export type Classified = { readonly base: P3; readonly said: Said; readonly answer: Answer };

const RESERVED = new Set(["base", "shade", "scatter"]);

/**
 * A paint's result, classified once: an expression is a bare colour; an object is `base`, channels, `scatter` and
 * `shade`. Refuses a channel that is not a number or a vector, and a `base` that is not a colour, naming the thing.
 */
export function classify(thing: string, x: P3 | Painted): Classified {
  if (x instanceof Expr) return { base: x, said: { channels: new Map(), scatter: undefined }, answer: { kind: "styled" } };
  if (typeof x !== "object" || x === null || !("base" in x)) {
    throw new SceneError(`paint of "${thing}"`, `${typeof x}`, "a colour, or { base, ...channels }", "return vec3(r, g, b), or { base: vec3(r, g, b), gloss: 0.8 }");
  }
  const base = lift<"vec3">(x.base as Expr<"vec3">);
  if (base.ty !== "vec3") throw new SceneError(`paint of "${thing}"`, `base as ${base.ty}`, "base as a colour (vec3)", "base: vec3(r, g, b)");
  const channels = new Map<string, Expr<NumTy>>();
  for (const [name, v] of Object.entries(x)) {
    if (RESERVED.has(name) || v === undefined) continue;
    if (typeof v === "function" || !(v instanceof Expr || typeof v === "number" || Array.isArray(v))) {
      throw new SceneError(`paint of "${thing}"`, `channel "${name}" as ${typeof v}`, "a number or a vector (f32 to vec4)", `${name}: 0.8, or ${name}: vec3(...)`);
    }
    channels.set(name, lift(v as Expr<NumTy>));
  }
  const scatter = typeof x.scatter === "function" ? x.scatter : undefined;
  const answer: Answer = typeof x.shade === "function" ? { kind: "wrapped", wrap: x.shade } : { kind: "styled" };
  return { base, said: { channels, scatter }, answer };
}

/** The shared shade functions' arguments, in the order every call site passes them. */
const SHADE_ARGS: FnSpec["args"] = [["c_base", "vec3"], ["c_normal", "vec3"], ["c_view", "vec3"], ["c_world", "vec3"], ["c_depth", "f32"], ["c_uv", "vec2"], ["c_anchor", "vec3"], ["c_id", "f32"]];

/**
 * The functions a lit fragment calls for a thing: the shared `shade`, `ink` and `mark` (and `trace_*` over them), or
 * its own copies, `look_<thing>`, `ink_<thing>` and `mark_<thing>`, in its module (`shader`), which take the paint's
 * arguments after the shared ones.
 */
export type Shading = { readonly kind: "shared" } | { readonly kind: "own"; readonly thing: string; readonly shader: Shader };

/** What the scene reports about what a thing said: channels no part of the style reads, a wrap that ignores the style. */
export type Notes = { readonly unread: readonly string[]; readonly ignoresStyle: boolean };

const OWN = { shade: "look", ink: "ink", mark: "mark" } as const;

/** The call of `fn` for a surface's fragment: `shared` the shared functions' arguments, `paintCall` the paint's. */
export function shadeCall(s: Shading, fn: "shade" | "ink" | "mark", shared: string, paintCall: string): string {
  switch (s.kind) {
    case "shared": return `${fn}(${shared})`;
    case "own": return `${OWN[fn]}_${s.thing}(${shared}, ${paintCall})`;
  }
}

/**
 * The call of `fn` for a traced point (src/draw/trace.ts, `trace_shade`): no surface coordinates, the anchor the world
 * point, the depth below the sea from it.
 */
export function tracedCall(s: Shading, fn: "shade" | "ink" | "mark", base: string, n: string, p: string, d: string, id: string, paintCall: string): string {
  switch (s.kind) {
    case "shared": return `trace_${fn}(${base}, ${n}, ${p}, ${d}, ${id})`;
    case "own": return shadeCall(s, fn, `${base}, ${n}, -${d}, ${p}, sea_height(${p}.xz) - ${p}.y, vec2<f32>(0.0), ${p}, ${id}`, paintCall);
  }
}

/** A paint's shader with the thing's own shade functions after it, when it has them. */
export function withShading(paint: Shader, s: Shading): Shader {
  switch (s.kind) {
    case "shared": return paint;
    case "own": return { code: `${paint.code}\n${s.shader.code}`, libs: [...new Set([...paint.libs, ...s.shader.libs])] };
  }
}

/**
 * The functions a lit fragment calls for this thing: shared when it gives no channel the style reads, no scatter the
 * style asks for and no wrap (byte-identical to before channels); otherwise its own copies, emitted into its own module.
 * `paintArgs` are the paint function's arguments (the channels are expressions of them).
 */
export function lookFns(ctx: CompileContext, thing: string, c: Classified, paintArgs: FnSpec["args"]): Shading {
  // A bare colour asks nothing of the style: the style is not evaluated here (its expressions would be made earlier
  // than before, and every later name in the shader text would move).
  if (c.said.channels.size === 0 && c.said.scatter === undefined && c.answer.kind === "styled") return { kind: "shared" };
  const notes = notesOf(ctx, thing, c);
  for (const n of notes.unread) ctx.note(`${thing} gives "${n}", which no part of ${ctx.style.name} reads`);
  if (notes.ignoresStyle) ctx.note(`${thing}'s own shade never reads the style's answer: it no longer follows --style`);
  const reads = styleReads(ctx.style);
  const readChannel = [...c.said.channels.keys()].some((n) => reads.channels.has(n));
  const readScatter = c.said.scatter !== undefined && reads.scatter;
  if (!readChannel && !readScatter && c.answer.kind === "styled") return { kind: "shared" };
  const look = shadeVariables(ctx.lighting, ctx.style, { thing, ...c.said });
  const answer = withIdentities(() => answered(c.answer, look, ctx.style.block === undefined ? ctx.style.shade(look) : ctx.style.block.body(look)));
  const args = [...SHADE_ARGS, ...paintArgs];
  const fn = (name: string, out: string, root: Expr): Shader => emitShaderFn(ctx.slots, { name, args, out, roots: [root], ret: (o) => o[0]! });
  const shaders = [fn(`look_${thing}`, "vec3<f32>", answer), fn(`ink_${thing}`, "vec3<f32>", withIdentities(() => ctx.style.ink(look))), fn(`mark_${thing}`, "f32", withIdentities(() => ctx.style.mark(look)))];
  return { kind: "own", thing, shader: { code: shaders.map((s) => s.code).join("\n"), libs: [...new Set(shaders.flatMap((s) => s.libs))] } };
}

/** The colour a thing shows: the style's, or the style's through the thing's own `shade`. */
export function answered(a: Answer, ctx: ShadeCtx, styled: P3): P3 {
  switch (a.kind) {
    case "styled": return styled;
    case "wrapped": return a.wrap(ctx, styled);
  }
}

/** What no part of the style reads, and whether a wrap reads the style's answer at all. */
function notesOf(ctx: CompileContext, thing: string, c: Classified): Notes {
  const reads = styleReads(ctx.style);
  const unread = [...c.said.channels.keys()].filter((n) => !reads.channels.has(n));
  if (c.said.scatter !== undefined && !reads.scatter) unread.push("scatter");
  const ignoresStyle = ((): boolean => {
    switch (c.answer.kind) {
      case "styled": return false;
      case "wrapped": {
        const styled = variable(`wrap_styled_${thing}`, "vec3");
        const look = shadeVariables(ctx.lighting, ctx.style, { thing, ...c.said });
        return !leaves([c.answer.wrap(look, styled)]).vars.has(`wrap_styled_${thing}`);
      }
    }
  })();
  return { unread, ignoresStyle };
}

// What a paint says, classified once at the boundary, and the functions a lit fragment calls for it.
//
// A paint returns a colour (`P3`) or a `Painted`: a colour (`base`) with named channels, an optional `scatter` (the
// surface's answer to one light) and an optional `shade` (a change to the finished picture). The style reads channels
// by name with a default (`ShadeCtx.channel`); the names it reads are recorded on its one evaluation. A thing that gives
// a channel the style reads, a `scatter` the style asks for, or a `shade`, gets its own copies of the style's functions
// in its own module (`look_<thing>`, `ink_<thing>`, `mark_<thing>`), evaluated with its channels in place of the
// defaults. Every other thing calls the shared `shade`, `ink` and `mark`, so a scene that says nothing more than a colour
// emits the same shader text as before channels existed.

import { Expr, leaves, lift, variable, type NumTy } from "../expr.ts";
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

/** The functions a lit fragment calls: their names, the module code they need (none for the shared ones), and how to call one. */
export type Shading = {
  readonly shader: Shader | undefined;
  /** `call("shade", "base, n, eye, ...")`: the call for this thing, its paint's arguments appended when it has its own copies. */
  call(fn: "shade" | "ink" | "mark", shared: string): string;
};

/** What the scene reports about what a thing said: channels no part of the style reads, a wrap that ignores the style. */
export type Notes = { readonly unread: readonly string[]; readonly ignoresStyle: boolean };

const SHARED: Shading = { shader: undefined, call: (fn, shared) => `${fn}(${shared})` };

/**
 * The functions a lit fragment calls for this thing: the shared `shade`, `ink` and `mark` when it gives nothing the
 * style reads and no wrap (byte-identical to before channels); otherwise its own copies, emitted into its own module.
 * `paintArgs` are the paint function's arguments (the channels are expressions of them) and `paintCall` the call
 * site's values for them.
 */
export function lookFns(ctx: CompileContext, thing: string, c: Classified, paintArgs: FnSpec["args"], paintCall: string): Shading {
  const notes = notesOf(ctx, thing, c);
  for (const n of notes.unread) ctx.note(`${thing} gives "${n}", which no part of ${ctx.style.name} reads`);
  if (notes.ignoresStyle) ctx.note(`${thing}'s own shade never reads the style's answer: it no longer follows --style`);
  const reads = styleReads(ctx.style);
  const readChannel = [...c.said.channels.keys()].some((n) => reads.channels.has(n));
  const readScatter = c.said.scatter !== undefined && reads.scatter;
  if (!readChannel && !readScatter && c.answer.kind === "styled") return SHARED;
  const look = shadeVariables(ctx.lighting, ctx.style, { thing, ...c.said });
  const body = ctx.style.block === undefined ? ctx.style.shade(look) : ctx.style.block.body(look);
  const answer = answered(c.answer, look, body);
  const args = [...SHADE_ARGS, ...paintArgs];
  const fn = (name: string, out: string, root: Expr): Shader => emitShaderFn(ctx.slots, { name, args, out, roots: [root], ret: (o) => o[0]! });
  const shaders = [fn(`look_${thing}`, "vec3<f32>", answer), fn(`ink_${thing}`, "vec3<f32>", ctx.style.ink(look)), fn(`mark_${thing}`, "f32", ctx.style.mark(look))];
  const shader: Shader = { code: shaders.map((s) => s.code).join("\n"), libs: [...new Set(shaders.flatMap((s) => s.libs))] };
  const own = { shade: `look_${thing}`, ink: `ink_${thing}`, mark: `mark_${thing}` } as const;
  return { shader, call: (f, shared) => `${own[f]}(${shared}, ${paintCall})` };
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

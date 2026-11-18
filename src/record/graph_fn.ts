// Library graph functions (docs/proposals/2026-11-19_lean_rust_loft.md section 3.2): a TypeScript function of
// expressions recorded once, over binders, and each call as one `Call` node, so a surface evaluated at 30 sites costs
// the recorder one body and 30 calls, not 30 bodies. Rust inlines the calls during interning. The TS engine still reads
// every call inlined (the log keeps those nodes, uncounted), so its output does not change.

import { identitiesOn, variable, type Expr, type NumTy } from "../expr.ts";
import { rec } from "./recorder.ts";

type Args<P extends readonly NumTy[]> = { [K in keyof P]: Expr<P[K]> };
let bodies = 0;
const made = new WeakSet<object>();

/**
 * `body` as a graph function named `name` over parameters of types `params`. A call is recorded as a `Call` when it is
 * a function of its arguments' values alone, as src/text/lower.ts decides for text functions: every argument a
 * non-constant expression (a constant folds differently inside the body) and no identities open. Any other call is
 * the body inlined and recorded as such.
 */
export function graphFn<P extends readonly NumTy[], R extends NumTy>(name: string, params: P, body: (...a: Args<P>) => Expr<R>): (...a: Args<P>) => Expr<R> {
  // A graph function of a graph function is that function: `surface` records its map, and `mapNormal` reads the same.
  if (made.has(body)) return body;
  let fn: number | undefined;
  const call = (...args: Args<P>): Expr<R> => {
    if (identitiesOn() || args.some((a) => a.node.op === "const")) return body(...args);
    // Lowered over binders only when a job is written: nodes made now would shift the ids the TS engine names WGSL by.
    fn ??= rec.fn(name, () => {
      const id = bodies++, binders = params.map((t, j) => variable(`loft_g${id}_${j}`, t)) as unknown as Args<P>;
      return { binders: binders as readonly Expr[], body: body(...binders) };
    });
    const since = rec.size, out = rec.muted(() => body(...args));
    rec.call(fn, args, out, since);
    return out;
  };
  made.add(call);
  return call;
}

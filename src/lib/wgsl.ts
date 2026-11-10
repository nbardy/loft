// The raw-shader escape hatch: a WGSL function with no CPU twin.
import { lib, type Expr, type In, type LibFn, type Ty } from "../expr.ts";

export type WgslFnSpec<T extends Ty> = {
  /** The WGSL function's name; the definition in `wgsl` must declare exactly this function. */
  readonly name: string;
  readonly ret: T;
  /** The full function definition (and nothing else that other definitions could collide with). */
  readonly wgsl: string;
  /** Other library functions the body calls; their definitions are placed before this one. */
  readonly deps?: readonly LibFn[];
};

/**
 * A callable that builds a call to a GPU-only WGSL function. Evaluating the resulting expression on the CPU throws
 * (`compileJs` raises its "GPU-only" error), so use it only in shader-stage expressions.
 */
export function wgslFn<T extends Ty>(spec: WgslFnSpec<T>): (...args: In[]) => Expr<T> {
  if (!new RegExp(`\\bfn\\s+${spec.name}\\s*\\(`).test(spec.wgsl)) {
    throw new Error(`wgslFn "${spec.name}": the wgsl text does not define a function with that name`);
  }
  const fn: LibFn = { name: spec.name, ret: spec.ret, wgsl: spec.wgsl, js: "gpu-only", deps: spec.deps ?? [] };
  return (...args) => lib<T>(fn, ...args);
}

/**
 * How much `x` changes from one pixel to the next (WGSL `fwidth`): the width to antialias an edge of `x` with.
 * Fragment stage only, and only in uniform control flow (WGSL's derivative rule); GPU-only.
 */
export const fwidth = wgslFn<"f32">({ name: "loft_fwidth", ret: "f32", wgsl: "fn loft_fwidth(x: f32) -> f32 { return fwidth(x); }" });

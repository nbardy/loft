// A station sample as free variables: the frame a strand's surface expressions are written over. A leaf module, so
// geometry and the draw modules can both import it without an import cycle.

import { variable, type Expr } from "../expr.ts";

/** Interpolated pose at some m, as expressions bound by whoever emits the surface (WGSL locals or JS values). */
export type FrameSample = {
  readonly p: Expr<"vec3">;
  /** d(position)/dm on the interpolated spine. */
  readonly der: Expr<"vec3">;
  readonly t: Expr<"vec3">;
  readonly d: Expr<"vec3">;
  readonly s: Expr<"vec3">;
  readonly stretch: Expr<"f32">;
};

/** The frame sample as free variables, named as WGSL `StationSample` fields (`st.p`, ...). */
export const FRAME_VARS: FrameSample = {
  p: variable("st_p", "vec3"), der: variable("st_der", "vec3"), t: variable("st_t", "vec3"),
  d: variable("st_d", "vec3"), s: variable("st_s", "vec3"), stretch: variable("st_stretch", "f32"),
};
export const FRAME_WGSL: Readonly<Record<string, string>> = {
  st_p: "st.p", st_der: "st.der", st_t: "st.t", st_d: "st.d", st_s: "st.s", st_stretch: "st.stretch",
};

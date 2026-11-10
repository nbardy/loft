// A closed-form frame as a rigid chart: a creature's root (hovering, walking a course) is a Frame of expressions, and
// its parts are authored in that frame's coordinates and placed with `rigid(frame)`, so mirroring them across the
// creature's own z = 0 plane (`mirror`, before placing) is a mirror across its sagittal plane wherever it is.

import { add, substitute, time, type Expr } from "../expr.ts";
import { rotate } from "../map/chart.ts";
import type { Chart, F, Frame } from "../map/types.ts";

export function rigid(g: Frame): Chart {
  return {
    frame: (l) => ({ o: add(g.o, rotate(g, l)), x: g.x, y: g.y, z: g.z }),
    at: (t: F) => {
      const [o, x, y, z] = substitute([g.o, g.x, g.y, g.z], time, t) as Expr<"vec3">[];
      return rigid({ o, x, y, z });
    },
    atNear(t: F) { return this.at(t); },
  };
}

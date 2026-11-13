// The `whole` view, guaranteed (`--view whole`, `--views whole`): the scene's own camera of that name when it declares
// one; otherwise a camera derived here, on the CPU, that frames every thing the scene draws with room round it. The
// scene's other cameras are never used in its place: a caller asking for `whole` gets the whole or a refusal.
//
// What is measured: each thing's bakeable surfaces (sampled on a coarse grid, as `loft bake` samples them finely) and
// mesh vertices, and its shadow chains (each joint grown by its radius, which is how a traced field or a list of copies
// is known on the CPU). Sky, ground and the shadow pass offer neither and are left out, as is anything else with no CPU
// shape (soft media, glows, rain), which stderr names. The bounds are taken over fixed times (one period of a looping
// scene, else 0..4 s), never the times asked for, so the camera is the same for every render of the scene at one aspect.
import type { Camera } from "../artefact.ts";
import type { Compiled } from "../compile/index.ts";
import { SceneError } from "../errors.ts";
import { variable, vec4, type Expr } from "../expr.ts";
import type { Scene } from "../scene.ts";
import { light } from "../draw/light.ts";
import { shadows } from "../draw/shadows.ts";
import { sky } from "../draw/sky.ts";

export const WHOLE = "whole";

/** Coarse grid per surface: enough to bound it (a bound, not a picture). */
const ALONG = 16, AROUND = 8;
/** Times the bounds are taken over, as fractions of one period or of OPEN_SECONDS. */
const SAMPLES = 9, OPEN_SECONDS = 4;
/** Looking from in front (+z), to the right and above, as the docs' own `whole` cameras do (docs/craft.md). */
const DIRECTION: readonly [number, number, number] = [0.6, 0.35, 0.8];
const FOV_Y = 0.6;
/** Room round it: the bounding sphere fills 1 / ROOM of the narrower half-angle. */
const ROOM = 1.15;

export type Derived = {
  readonly camera: Camera;
  readonly min: readonly number[];
  readonly max: readonly number[];
  readonly times: readonly number[];
  readonly framed: readonly string[];
  /** Things with no shape on the CPU: drawn, but not framed. */
  readonly unframed: readonly string[];
};

const U = variable("u", "f32"), W = variable("v", "f32");

/** Every finite point the scene's things reach at time t, by thing name (empty for a thing with no CPU shape). */
function pointsAt(scene: Scene, c: Compiled, t: number, out: Map<string, number[][]>): void {
  const a = c.artefact;
  const inputs = new Float32Array(Math.max(1, a.inputFloats));
  for (const s of a.inputs) inputs.set(s.name === "time" ? [t] : s.def, s.slot);
  const frame = c.frame({ time: t, inputs, params: Float32Array.from(a.params.map((p) => p.def)), curves: a.curves });
  for (const thing of scene.things) {
    // The scene's own surroundings: the sky and ground have no extent to frame, the shadow and light passes no shape.
    if (thing === sky || thing === shadows || thing === light || thing === scene.ground) continue;
    const pts = out.get(thing.name) ?? [];
    for (const s of thing.surfaces) {
      if ("mesh" in s) {
        const P = s.mesh.positions;
        for (let i = 0; i < P.length; i += 3) pts.push([P[i]!, P[i + 1]!, P[i + 2]!]);
        continue;
      }
      const run = c.cpu([s.surface.point(U, W)]), uAt = c.cpu([s.grid.u.at(U)]), vAt = c.cpu([s.grid.v.at(W)]);
      for (let i = 0; i <= ALONG; i++) {
        const u = uAt(frame, { u: i / ALONG })[0] as number;
        for (let j = 0; j <= AROUND; j++) pts.push(run(frame, { u, v: vAt(frame, { v: j / AROUND })[0] as number })[0] as number[]);
      }
    }
    for (const sk of thing.skeletons) {
      if (sk.joints === 0) continue;
      const roots: Expr[] = Array.from({ length: sk.joints }, (_, k) => { const j = sk.joint(k); return vec4(j.p, j.r); });
      for (const v of c.cpu(roots)(frame, {}) as number[][]) {
        const r = Math.abs(v[3]!);
        pts.push([v[0]! - r, v[1]! - r, v[2]! - r], [v[0]! + r, v[1]! + r, v[2]! + r]);
      }
    }
    out.set(thing.name, pts);
  }
}

/**
 * A camera framing the scene's things over its sample times, for pictures `width / height` wide. Refuses (SceneError)
 * when nothing in the scene has a shape on the CPU, naming the fix: declare `cameras.whole`.
 */
export function deriveWhole(scene: Scene, c: Compiled, aspect: number): Derived {
  const span = scene.loop.kind === "every" ? scene.loop.seconds : OPEN_SECONDS;
  const times = Array.from({ length: SAMPLES }, (_, k) => +((span * k) / (scene.loop.kind === "every" ? SAMPLES : SAMPLES - 1)).toFixed(4));
  const byThing = new Map<string, number[][]>();
  for (const t of times) pointsAt(scene, c, t, byThing);
  const min = [Infinity, Infinity, Infinity], max = [-Infinity, -Infinity, -Infinity];
  const framed: string[] = [], unframed: string[] = [];
  for (const [name, pts] of byThing) {
    const finite = pts.filter((p) => p.length >= 3 && p.every(Number.isFinite));
    if (finite.length === 0) { unframed.push(name); continue; }
    framed.push(name);
    for (const p of finite) for (let k = 0; k < 3; k++) { min[k] = Math.min(min[k]!, p[k]!); max[k] = Math.max(max[k]!, p[k]!); }
  }
  if (framed.length === 0) {
    throw new SceneError(`camera "${WHOLE}"`, `a scene with no camera "${WHOLE}" and no thing whose shape Loft can measure on the CPU (${unframed.join(", ")})`,
      `a camera named "${WHOLE}", or a thing with a surface or shadow chain to frame`,
      `add cameras: { ${WHOLE}: { kind: "perspective", eye: [x, y, z], target: [x, y, z], fovY: 0.6 } } showing the subject with room round it`);
  }
  const centre = [0, 1, 2].map((k) => (min[k]! + max[k]!) / 2) as [number, number, number];
  // A thing smaller than this still gets a picture it fills, not one at a pinhole's distance.
  const radius = Math.max(0.25, Math.hypot(max[0]! - min[0]!, max[1]! - min[1]!, max[2]! - min[2]!) / 2);
  const half = Math.min(FOV_Y / 2, Math.atan(Math.tan(FOV_Y / 2) * aspect));
  const distance = (radius * ROOM) / Math.sin(half);
  const l = Math.hypot(...DIRECTION);
  const round = (x: number) => +x.toFixed(4);
  const eye = centre.map((x, k) => round(x + (DIRECTION[k]! / l) * distance)) as [number, number, number];
  const camera: Camera = { kind: "perspective", eye, target: centre.map(round) as [number, number, number], fovY: FOV_Y };
  return { camera, min: min.map(round), max: max.map(round), times, framed, unframed };
}

/** The stderr note that says the camera was derived, and from what. */
export function derivedNote(d: Derived): string {
  const v = (x: readonly number[]) => `[${x.join(", ")}]`;
  const cam = d.camera.kind === "perspective" ? `eye ${v(d.camera.eye)}, target ${v(d.camera.target)}, fovY ${d.camera.fovY}` : "";
  return `camera "${WHOLE}": the scene declares none, so it was derived: framing ${d.framed.length <= 6 ? d.framed.join(", ") : `${d.framed.length} things (${d.framed.slice(0, 4).join(", ")}, ...)`} within ${v(d.min)}..${v(d.max)} over t=${d.times.join(",")}: ${cam}` +
    (d.unframed.length > 0 ? `; not measured (no shape on the CPU): ${d.unframed.join(", ")}` : "");
}

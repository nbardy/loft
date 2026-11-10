// compile(scene) -> Artefact. One pass: every drawable emits its shaders and draws (slots for inputs, params and
// curve rows are assigned as code is emitted, and strands register with the pose pass when a drawable asks for their
// stations); then the leaf tables are final and the pose function is assembled over the registered strands.

import type { Artefact, Draw, DynamicBuffer, FrameState } from "../artefact.ts";
import { compileJs, variable, vec2, type Env } from "../expr.ts";
import type { Strand } from "../geom/strand.ts";
import { STATION_VEC4S, type V3 } from "../geom/stations.ts";
import { poseProgram } from "./pose.ts";
import type { Scene } from "../scene.ts";
import { Slots } from "./bind.ts";
import type { CompileContext, StationRange } from "./context.ts";
import { collars, decalVec4s, readStations, shadows } from "./decals.ts";
import { sceneWgsl, seaDraws, skyDraws, TABLES } from "./shaders.ts";

export function compile(scene: Scene): Artefact {
  const slots = new Slots();
  const posed: { strand: Strand; range: StationRange }[] = [];
  let total = 0;
  const ctx: CompileContext = {
    slots, style: scene.style, sea: scene.sea,
    stations(strand) {
      const seen = posed.find((p) => p.strand === strand);
      if (seen) return seen.range;
      const range = { base: total, count: strand.stations };
      total += strand.stations;
      posed.push({ strand, range });
      return range;
    },
  };

  const shaders: Record<string, string> = {};
  const draws: Draw[] = [];
  const add = (e: { shaders: Readonly<Record<string, string>>; draws: readonly Draw[] }) => {
    for (const [k, code] of Object.entries(e.shaders)) {
      if (k in shaders) throw new Error(`compile: two drawables emit a shader named "${k}"`);
      shaders[k] = code;
    }
    draws.push(...e.draws);
  };
  // The sky first: it writes no depth, so everything after it draws over it.
  add(skyDraws());
  for (const t of scene.things) add(t.draws(ctx));
  add(seaDraws(scene.sea));
  const wgsl = sceneWgsl(slots, scene.style, scene.sea, scene.sun);

  // The pose pass. Its CPU expressions register their leaves, so a param only the spine reads still gets a slot.
  const program = poseProgram(posed);
  slots.register(program.roots);
  const seaXZ = vec2(variable("x", "f32"), variable("z", "f32"));
  const seaHeightExpr = scene.sea.height(seaXZ);
  slots.register([seaHeightExpr]);
  const seaHeightJs = compileJs([seaHeightExpr]);
  const tables = slots.tables();
  const readEnv = slots.envReader();
  const sizes = decalVec4s(total);
  const dynamic: DynamicBuffer[] = [
    { name: "stations", binding: TABLES.stations, vec4s: Math.max(1, total * STATION_VEC4S) },
    { name: "collars", binding: TABLES.collars, vec4s: sizes.collars },
    { name: "shadows", binding: TABLES.shadows, vec4s: sizes.shadows },
  ];
  const ranges = posed.map((p) => p.range);
  const sun = scene.sun as V3;

  const pose = (state: FrameState): Record<string, Float32Array> => {
    const { inputs, params } = readEnv(state.inputs, state.params);
    const env: Env = { vars: {}, inputs, params };
    // The rest pose, against which stretch is measured, is the same scene at time 0.
    const rest: Env = { vars: {}, inputs: { ...inputs, time: 0 }, params };
    const stations = new Float32Array(dynamic[0].vec4s * 4);
    program.run(env, rest, stations);
    const vars = { x: 0, z: 0 };
    const at: Env = { vars, inputs, params };
    const height = (x: number, z: number): number => { vars.x = x; vars.z = z; return seaHeightJs(at)[0] as number; };
    const foam = new Float32Array(sizes.collars * 4), shade = new Float32Array(sizes.shadows * 4);
    const read = readStations(stations, total, height);
    collars(read, ranges, foam);
    shadows(read, ranges, sun, shade);
    return { stations, collars: foam, shadows: shade };
  };

  const triangles = draws.filter((d) => d.layer === "scene").reduce((n, d) => n + (d.vertices / 3) * d.instances, 0);
  return {
    version: 1,
    clear: scene.style.clear,
    inputs: tables.inputs,
    inputFloats: tables.inputFloats,
    params: tables.params,
    curves: tables.curves,
    dynamic,
    sceneWgsl: wgsl,
    shaders,
    draws,
    pose,
    cameras: scene.cameras,
    stats: { triangles, draws: draws.length, curveRows: tables.rows, stations: total },
  };
}

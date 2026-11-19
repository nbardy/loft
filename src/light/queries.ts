import { bits, reading, layout, type Kind } from "../text/kind.ts";
import { SceneError } from "../errors.ts";
import type { Declarations } from "../record/passes.ts";
import { world, type Scene } from "../render/record.ts";
import bindings from "./interface.wgsl" with { type: "text" };
import key from "./key.wgsl" with { type: "text" };
import transport from "./transport.wgsl" with { type: "text" };
import ggx from "../kinds/ggx.wgsl" with { type: "text" };
import fill from "./albedo_fill.wgsl" with { type: "text" };
export const albedoFill = { text: ggx + fill, opens: [] };
const lookup = `
@group(1) @binding(3) var<storage,read> ggxTable:array<vec4f>;
fn directional_albedo(mu:f32,rough:f32)->vec2f {
  let p=clamp(vec2f(mu,rough),vec2f(0),vec2f(1))*31;
  let a=vec2u(p);let b=min(a+1u,vec2u(31));let f=fract(p);
  return mix(mix(ggxTable[32u*a.y+a.x].xy,ggxTable[32u*a.y+b.x].xy,f.x),
             mix(ggxTable[32u*b.y+a.x].xy,ggxTable[32u*b.y+b.x].xy,f.x),f.y);
}`;
export const defaultSky = `fn sky(d:vec3f)->vec3f {
  return mix(vec3f(0.62,0.72,0.85),vec3f(0.13,0.32,0.62),pow(clamp(d.y,0,1),0.45));
}`;
export function lighting(s: Scene, w: ReturnType<typeof world>, fillModule: number) {
  const ks = w.materials;
  const receiver = (k: Kind) =>
    k.functions.has("surface") ? `${k.name}_surface(load_${k.name}(at),x)` : `load_${k.name}(at)`;
  const opens = [
    reading(ks, "eval", "x:Point,wo:vec3f,wi:vec3f", "wo,wi", "vec3f", "vec3f(0)", undefined, receiver),
    reading(ks, "emit", "x:Point,wo:vec3f", "wo", "vec3f", "vec3f(0)", () => "vec3f(0)", receiver),
    reading(
      ks,
      "albedo",
      "x:Point",
      "",
      "vec3f",
      "vec3f(0)",
      (k, m) => `PI*${k.name}_eval(${m},vec3f(0,0,1),vec3f(0,0,1))`,
      receiver,
    ),
    reading(
      ks,
      "sample",
      "x:Point,wo:vec3f,u:vec2f",
      "wo,u",
      "Scatter",
      "Scatter(vec3f(0),vec3f(0),0,false)",
      (k) => `sample_eval(${k.tag}u,at,x,wo,u)`,
      receiver,
    ),
  ];
  const towards = s.sun?.towards ?? [-0.4, 0.8, 0.45],
    len = Math.hypot(...towards);
  const energy = s.sun?.power ?? [2, 1.9, 1.7];
  const luminance = (v: readonly number[]) => 0.2126 * v[0] + 0.7152 * v[1] + 0.0722 * v[2];
  if (!Number.isFinite(len) || len === 0 || energy.some((x) => !Number.isFinite(x) || x < 0))
    throw new SceneError("sun: supply a nonzero direction and finite, nonnegative RGB power");
  const sun = { pos: towards.map((x) => x / len), radius: 0, L: energy, weight: luminance(energy), row: 0 };
  const area = w.things
    .flatMap((t, row) => {
      if (!t.shape.radius || !t.material.emission || !s.things.includes(t)) return [];
      const L = t.material.emission,
        radius = t.shape.radius;
      return [{ pos: t.at, radius, L, weight: luminance(L) * 4 * Math.PI * radius * radius, row }];
    })
    .filter((e) => e.weight > 0)
    .sort((a, b) => b.weight - a.weight);
  const emitters = [sun, ...area],
    total = emitters.reduce((sum, e) => sum + e.weight, 0);
  let cdf = 0;
  const words = emitters.flatMap((e, i) => {
    const prob = total === 0 ? 1 : e.weight / total;
    cdf += prob;
    return [...e.pos, e.radius, ...e.L, i === emitters.length - 1 ? 1 : cdf].map(bits).concat(e.row, bits(prob), 0, 0);
  });
  const tables: Declarations["tables"] = [...w.tables, { lanes: 3, words }];
  const hasMetal = ks.some((k) => k.source.includes("directional_albedo("));
  if (hasMetal) tables.push({ lanes: 1, rows: 1024, module: fillModule, entry: "fill", extent: [32, 1, 1] });
  const laws =
    bindings +
    key +
    ggx +
    (hasMetal ? lookup : "") +
    ks.map((k) => k.source).join("\n") +
    (s.sky ?? defaultSky) +
    transport;
  return {
    tables,
    hasMetal,
    module: (body: string) =>
      w.query(
        laws + body,
        opens,
        ks.map((k) => layout(k, "words")),
      ),
  };
}

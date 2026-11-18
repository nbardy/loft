// Five probe functions and six example functions, each in call form and in WGSL text (docs/proposals/
// 2026-11-18_authoring_api.md section 9, lane L1, check 1). The call forms are the code in the files named, with two
// normalizations stated where they apply: a constant vector is written as an array (`[0.1, 0.2, 0.3]`, which text's
// `vec3(0.1, 0.2, 0.3)` builds: one constant node, where the call form's `vec3(0.1, 0.2, 0.3)` is a vector node of three
// scalar constants), and a JavaScript loop that unrolls becomes `iterate` (text's `for` is a loop in the graph).
//
// `bun tests/text.probes.ts <probe> <text|call>` prints the probe's structural hash and emitted WGSL as JSON, from a
// fresh process: tests/text.test.ts compares the two forms. The hash is the same whatever order the nodes were made in;
// the WGSL names each value by its node's number, so it is byte-identical when the text makes its nodes in the call
// form's order. Where a call form's `mul(a, b, c)` evaluates all three operands before the first product, the text
// names the third with a `let` first.

import {
  abs, add, and, cos, div, dot, exp, f, fract, ge, gt, hash22, iterate, length, lt, max, min, mix, mul, ne, noise3, not, select,
  sin, smoothstep, sqrt, sub, time, variable, vec2, vec3, wgsl, type Expr, type F, type P3,
} from "../src/index.ts";
import { emitWgsl, structuralHash, type Bind } from "../src/expr.ts";
import { sdCapsule3, sdRoundBox } from "../src/lib/sdf3.ts";
import { smin } from "../src/lib/util.ts";

type Probe = { readonly text: () => readonly Expr[]; readonly call: () => readonly Expr[] };
const v1 = (name: string): F => variable(name, "f32");
const v3 = (name: string): P3 => variable(name, "vec3");
const record = (r: Record<string, Expr>): Expr[] => Object.values(r);

// ---- spray_burst.ts -----------------------------------------------------------------------------------------------------

function flight(i: Expr<"f32">, t: F) {
  const h = hash22(vec2(i, 31)), k = hash22(vec2(i, 73));
  const age = max(0, sub(t, mul(k.x, 0.19)));
  const angle = mix(-1.25, 0.93, h.x);
  const reach = mul(mix(0.7, 4.6, h.y), sub(1, exp(mul(age, -2.2))));
  const rise = mul(mix(0.7, 4.3, k.y), add(0.60, mul(0.40, sin(mul(h.x, Math.PI)))));
  const height = sub(mul(rise, sub(1, exp(mul(age, -2.8)))), mul(mul(age, age), 1.65));
  const edge = (u: F) => smoothstep(0, 0.19, mul(u, sub(1, u)));
  const taper = mul(edge(h.x), mul(edge(h.y), edge(k.y)));
  return { at: vec3(add(-0.5, mul(cos(angle), reach)), add(0.06, height), mul(sin(angle), reach)),
    fade: mul(taper, mul(smoothstep(0, 0.075, age), sub(1, smoothstep(0.72, 1.4, age)))), h: h.y, k: k.y };
}
const sprayText = () => wgsl`
struct Flight { at: vec3f, fade: f32, h: f32, k: f32 }
fn edge(u: f32) -> f32 { return smoothstep(0.0, 0.19, u * (1.0 - u)); }
fn flight(i: f32, t: f32) -> Flight {
  let h = hash22(vec2(i, 31.0));
  let k = hash22(vec2(i, 73.0));
  let age = max(0.0, t - k.x * 0.19);
  let angle = mix(-1.25, 0.93, h.x);
  let reach = mix(0.7, 4.6, h.y) * (1.0 - exp(age * -2.2));
  let rise = mix(0.7, 4.3, k.y) * (0.60 + 0.40 * sin(h.x * 3.141592653589793));
  let height = rise * (1.0 - exp(age * -2.8)) - age * age * 1.65;
  let taper = edge(h.x) * (edge(h.y) * edge(k.y));
  return Flight(vec3(-0.5 + cos(angle) * reach, 0.06 + height, sin(angle) * reach),
                taper * (smoothstep(0.0, 0.075, age) * (1.0 - smoothstep(0.72, 1.4, age))), h.y, k.y);
}
struct Grain { at: vec3f, radius: f32, colour: vec3f, opacity: f32 }
fn grain(i: f32, t: f32) -> Grain {
  let q = flight(i, t);
  let size = hash22(vec2(i, 111.0));
  let coarse = size.x * (size.x * size.x);
  return Grain(q.at, mix(0.0019, 0.012, coarse), mix(vec3(0.55, 0.18, 0.063), vec3(1.0, 0.56, 0.25), size.y), mix(0.18, 0.65, coarse) * q.fade);
}`;

// The boot: `heel` and `local` are the example's; `laces` and `cleats` build their lists in order, then join them.
const heelOf = () => vec3(-0.88, add(0.46, mul(smoothstep(0, 1.15, time), 1.55)), 0);
function bootCall() {
  const heel = heelOf();
  const local = (p: P3): P3 => {
    const q = sub(p, heel);
    return vec3(add(mul(q.x, 0.896), mul(q.y, 0.444)), sub(mul(q.y, 0.896), mul(q.x, 0.444)), q.z);
  };
  const laces = (q: P3) => [0.16, 0.29, 0.42, 0.55].map((x) =>
    sdCapsule3(q, [x - 0.025, 0.157, -0.21], [x + 0.025, 0.157, 0.21], 0.022)).reduce((a, b) => min(a, b));
  const field = (p: P3) => {
    const q = local(p);
    const sole = sdRoundBox(sub(q, [0.32, -0.29, 0]), [0.99, 0.09, 0.39], 0.085);
    const upper = sdRoundBox(sub(q, [0.40, -0.08, 0]), [0.88, 0.23, 0.35], 0.20);
    const ankle = sdRoundBox(sub(q, [-0.28, 0.20, 0]), [0.38, 0.50, 0.35], 0.23);
    const cleats = [-0.43, 0.30, 0.95].map((x) =>
      sdRoundBox(vec3(sub(q.x, x), add(q.y, 0.40), sub(max(q.z, mul(q.z, -1)), 0.245)), [0.10, 0.12, 0.10], 0.035));
    return min(min(sole, smin(upper, ankle, 0.16)), min(laces(q), cleats.reduce((a, b) => min(a, b))));
  };
  // The paint, with its colours as arrays.
  const paint = (p: P3) => {
    const q = local(p);
    const leather = mix([0.24, 0.16, 0.09], [0.061, 0.075, 0.084], smoothstep(-0.27, -0.16, q.y));
    return mix([0.34, 0.30, 0.24], leather, smoothstep(0.003, 0.012, laces(q)));
  };
  return { field, paint };
}
const bootText = () => wgsl`
fn local(p: vec3f) -> vec3f {
  let q = p - ${heelOf()};
  return vec3(q.x * 0.896 + q.y * 0.444, q.y * 0.896 - q.x * 0.444, q.z);
}
fn lace(q: vec3f, x: f32) -> f32 { return sdCapsule3(q, vec3(x - 0.025, 0.157, -0.21), vec3(x + 0.025, 0.157, 0.21), 0.022); }
fn laces(q: vec3f) -> f32 {
  let a = lace(q, 0.16);
  let b = lace(q, 0.29);
  let c = lace(q, 0.42);
  let d = lace(q, 0.55);
  return min(min(min(a, b), c), d);
}
fn cleat(q: vec3f, x: f32) -> f32 { return sdRoundBox(vec3(q.x - x, q.y + 0.40, max(q.z, q.z * -1.0) - 0.245), vec3(0.10, 0.12, 0.10), 0.035); }
fn field(p: vec3f) -> f32 {
  let q = local(p);
  let sole = sdRoundBox(q - vec3(0.32, -0.29, 0.0), vec3(0.99, 0.09, 0.39), 0.085);
  let upper = sdRoundBox(q - vec3(0.40, -0.08, 0.0), vec3(0.88, 0.23, 0.35), 0.20);
  let ankle = sdRoundBox(q - vec3(-0.28, 0.20, 0.0), vec3(0.38, 0.50, 0.35), 0.23);
  let c0 = cleat(q, -0.43);
  let c1 = cleat(q, 0.30);
  let c2 = cleat(q, 0.95);
  return min(min(sole, smin(upper, ankle, 0.16)), min(laces(q), min(min(c0, c1), c2)));
}
fn paint(p: vec3f) -> vec3f {
  let q = local(p);
  let leather = mix(vec3(0.24, 0.16, 0.09), vec3(0.061, 0.075, 0.084), smoothstep(-0.27, -0.16, q.y));
  return mix(vec3(0.34, 0.30, 0.24), leather, smoothstep(0.003, 0.012, laces(q)));
}`;

// The ground of the spray's style, `world` and `shadow` from the style's record; colours as arrays.
const groundCall = (world: P3, shadow: F) => {
  const near = exp(mul(add(mul(world.x, world.x), mul(world.z, world.z)), -0.017));
  const grit = noise3(mul(world, 23));
  return mul(mix([0.11, 0.052, 0.039], [0.31, 0.109, 0.058], near), mul(add(0.88, mul(grit, 0.12)), sub(1, mul(shadow, 0.55))));
};
const groundText = () => wgsl`
fn ground(world: vec3f, shadow: f32) -> vec3f {
  let near = exp((world.x * world.x + world.z * world.z) * -0.017);
  let grit = noise3(world * 23.0);
  return mix(vec3(0.11, 0.052, 0.039), vec3(0.31, 0.109, 0.058), near) * ((0.88 + grit * 0.12) * (1.0 - shadow * 0.55));
}`;

// ---- volume_dust.ts ---------------------------------------------------------------------------------------------------

const local = (p: P3) => sub(p, [0, 2, 0]);
const dustCall = (p: P3, t: F) => {
  const q = local(p);
  const drift = sub(q, vec3(mul(t, 0.35), mul(t, 0.08), 0));
  const shape = length(div(q, [3.65, 1.85, 2.18]));
  const billow = noise3(mul(drift, [0.75, 1.2, 1.0]));
  const wisps = noise3(mul(drift, [2.7, 4.9, 3.8]));
  const body = sub(1, smoothstep(0.58, 0.99, shape));
  return mul(2.1, body, smoothstep(-0.14, 0.48, add(billow, mul(wisps, 0.24))));
};
// The example unrolls four steps in JavaScript; here they are `iterate`, as text's for loop is.
const foldedCall = (p: P3, t: F) => {
  const q = div(local(p), [3.6, 1.85, 2.14]);
  const [r] = iterate(4, [q] as const, ([r]) => {
    const x = abs(r);
    return [sub(mul(vec3(add(x.y, mul(x.z, 0.38)), add(x.z, mul(x.x, 0.38)), add(x.x, mul(x.y, 0.38))), 1.62), [0.7, 0.62, 0.54])] as const;
  });
  const seam = abs(add(dot(r, [0.7, -0.46, 0.55]), mul(sin(mul(t, 0.7)), 0.09)));
  return mul(1.2, sub(1, smoothstep(0.035, 0.20, seam)), sub(1, smoothstep(0.70, 0.99, length(q))));
};
const emissionCall = (p: P3) => mul([0.045, 0.15, 0.24], sub(1, smoothstep(0.0, 1.25, abs(sub(p.y, 1.65)))));
const volumeText = () => wgsl`
fn local(p: vec3f) -> vec3f { return p - vec3(0.0, 2.0, 0.0); }
fn dust(p: vec3f, t: f32) -> f32 {
  let q = local(p);
  let drift = q - vec3(t * 0.35, t * 0.08, 0.0);
  let shape = length(q / vec3(3.65, 1.85, 2.18));
  let billow = noise3(drift * vec3(0.75, 1.2, 1.0));
  let wisps = noise3(drift * vec3(2.7, 4.9, 3.8));
  let body = 1.0 - smoothstep(0.58, 0.99, shape);
  let lumps = smoothstep(-0.14, 0.48, billow + wisps * 0.24);
  return 2.1 * body * lumps;
}
fn folded(p: vec3f, t: f32) -> f32 {
  let q = local(p) / vec3(3.6, 1.85, 2.14);
  var r = q;
  for (var i = 0; i < 4; i++) {
    let x = abs(r);
    r = vec3(x.y + x.z * 0.38, x.z + x.x * 0.38, x.x + x.y * 0.38) * 1.62 - vec3(0.7, 0.62, 0.54);
  }
  let seam = abs(dot(r, vec3(0.7, -0.46, 0.55)) + sin(t * 0.7) * 0.09);
  let sheet = 1.0 - smoothstep(0.035, 0.20, seam);
  let inside = 1.0 - smoothstep(0.70, 0.99, length(q));
  return 1.2 * sheet * inside;
}
fn emission(p: vec3f) -> vec3f { return vec3(0.045, 0.15, 0.24) * (1.0 - smoothstep(0.0, 1.25, abs(p.y - 1.65))); }`;

// ---- src/draw/march.ts: the sphere-trace loop ----------------------------------------------------------------------------
// march's 9-state iterate, over a unit sphere's field and a ray width that grows with t (march's `at` and `ray.width`).

const HIT = 0.3;
function marchCall(o: P3, d: P3, t0: F, t1: F) {
  const field = (p: P3): F => sub(length(p), 1);
  const width = (t: F): F => add(0.001, mul(t, 0.002));
  const point = (t: F): P3 => add(o, mul(d, t));
  const init = [t0, t0, f(0), f(0), t0, f(1e30), f(1e30), select(gt(t0, t1), f(2), f(0)), f(0)] as const;
  const [t, , , , nearT, , nearGap, done, hitDist] = iterate(48, init, (s) => {
    const [t, prevT, prevDist, prevExact, nearT, nearPx, nearGap, done, hitDist] = s;
    const running = lt(done, 0.5);
    const fp = width(t);
    const dist = field(point(t));
    const hit = lt(dist, mul(fp, HIT));
    const step = sub(t, prevT);
    const x = div(add(sub(mul(dist, dist), mul(prevDist, prevDist)), mul(step, step)), max(mul(step, 2), 1e-20));
    const between = and(gt(prevExact, 0.5), and(gt(x, 0), lt(x, step)));
    const h = select(between, sqrt(max(sub(mul(dist, dist), mul(x, x)), 0)), dist);
    const th = select(between, sub(t, x), t);
    const hPx = div(h, width(th));
    const nearer = and(running, and(not(hit), lt(hPx, nearPx)));
    const advance = and(running, not(hit));
    const tn = add(t, mul(dist, 1));
    const ended = select(hit, f(1), select(gt(tn, t1), f(2), f(0)));
    return [
      select(advance, tn, t), select(advance, t, prevT), select(advance, dist, prevDist), select(advance, f(1), prevExact),
      select(nearer, th, nearT), select(nearer, hPx, nearPx), select(nearer, h, nearGap),
      select(running, ended, done), select(and(running, hit), dist, hitDist),
    ] as const;
  }, (s) => ne(s[7], 0));
  return [t, nearT, nearGap, done, hitDist];
}
const marchText = () => wgsl`
fn field(p: vec3f) -> f32 { return length(p) - 1.0; }
fn width(t: f32) -> f32 { return 0.001 + t * 0.002; }
struct Trace { t: f32, nearT: f32, nearGap: f32, done: f32, hitDist: f32 }
fn march(o: vec3f, d: vec3f, t0: f32, t1: f32) -> Trace {
  var t = t0;
  var prevT = t0;
  var prevDist = 0.0;
  var prevExact = 0.0;
  var nearT = t0;
  var nearPx = 1e30;
  var nearGap = 1e30;
  var done = select(0.0, 2.0, t0 > t1);
  var hitDist = 0.0;
  for (var k = 0; k < 48; k++) {
    let running = done < 0.5;
    let fp = width(t);
    let dist = field(o + d * t);
    let hit = dist < fp * 0.3;
    let step = t - prevT;
    let x = (dist * dist - prevDist * prevDist + step * step) / max(step * 2.0, 1e-20);
    let between = prevExact > 0.5 && (x > 0.0 && x < step);
    let h = select(dist, sqrt(max(dist * dist - x * x, 0.0)), between);
    let th = select(t, t - x, between);
    let hPx = h / width(th);
    let nearer = running && (!hit && hPx < nearPx);
    let advance = running && !hit;
    let tn = t + dist * 1.0;
    let ended = select(select(0.0, 2.0, tn > t1), 1.0, hit);
    if (advance) { prevT = t; prevDist = dist; prevExact = 1.0; t = tn; }
    if (nearer) { nearT = th; nearPx = hPx; nearGap = h; }
    if (running) { done = ended; }
    if (running && hit) { hitDist = dist; }
    if (done != 0.0) { break; }
  }
  return Trace(t, nearT, nearGap, done, hitDist);
}`;

// ---- stormwing.ts and reeds_shadow.ts --------------------------------------------------------------------------------------

const S = { membrane: [0.11, 0.09, 0.24], vein: [0.3, 0.6, 0.95], edge: [0.5, 0.46, 0.9] };
const wingCall = (u: F, v: F) => {
  const vein = mul(smoothstep(0.06, 0.0, abs(sub(fract(mul(u, 6)), 0.5))), smoothstep(0.25, 0.55, v));
  return mix(mix(S.membrane, S.vein, mul(vein, 0.6)), S.edge, smoothstep(0.82, 0.97, v));
};
const wingText = () => wgsl`
fn wing(u: f32, v: f32) -> vec3f {
  let vein = smoothstep(0.06, 0.0, abs(fract(u * 6.0) - 0.5)) * smoothstep(0.25, 0.55, v);
  return mix(mix(${S.membrane}, ${S.vein}, vein * 0.6), ${S.edge}, smoothstep(0.82, 0.97, v));
}`;
const K = 3;
const spineCall = (k: number) => (m: F) => vec3(add(k * 0.7 - 2, mul(mul(m, m), mul(sin(add(time, k)), 0.5))), mul(m, 3.5), k % 2 ? 0.3 : -0.3);
const spineText = (k: number) => wgsl`
fn spine(m: f32) -> vec3f { return vec3(${k * 0.7 - 2} + m * m * (sin(time + ${k}) * 0.5), m * 3.5, ${k % 2 ? 0.3 : -0.3}); }`;

// ---- the probes -------------------------------------------------------------------------------------------------------

export const PROBES: Readonly<Record<string, Probe>> = {
  // 1. The brief's own line.
  brief: {
    call: () => { const p = v3("p"), m = v1("m"); return [sub(length(sub(vec3(abs(p.x), p.y, p.z), vec3(0.35, m, 0))), sqrt(f(0.09)))]; },
    text: () => { const p = v3("p"), m = v1("m"); return [wgsl`fn d(p: vec3f, m: f32) -> f32 { return length(vec3(abs(p.x), p.y, p.z) - vec3(0.35, m, 0.0)) - sqrt(0.09); }`.d(p, m)]; },
  },
  // 2. spray_burst.ts: flight and the grain slot.
  grain: {
    call: () => {
      const i = v1("i"), t = v1("t");
      const q = flight(i, t);
      const size = hash22(vec2(i, 111));
      const coarse = mul(size.x, mul(size.x, size.x));
      return record({ at: q.at, radius: mix(0.0019, 0.012, coarse), colour: mix([0.55, 0.18, 0.063], [1.0, 0.56, 0.25], size.y), opacity: mul(mix(0.18, 0.65, coarse), q.fade) });
    },
    text: () => { const i = v1("i"), t = v1("t"); return record(sprayText().grain(i, t)); },
  },
  // 3. spray_burst.ts: the boot's field (laces, cleats, local).
  boot: {
    call: () => { const p = v3("p"); return [bootCall().field(p)]; },
    text: () => { const p = v3("p"); return [bootText().field(p)]; },
  },
  // 4. volume_dust.ts: folded.
  folded: {
    call: () => { const p = v3("p"), t = v1("t"); return [foldedCall(p, t)]; },
    text: () => { const p = v3("p"), t = v1("t"); return [volumeText().folded(p, t)]; },
  },
  // 5. src/draw/march.ts: the sphere-trace loop.
  march: {
    call: () => { const o = v3("o"), d = v3("d"), t0 = v1("t0"), t1 = v1("t1"); return marchCall(o, d, t0, t1); },
    text: () => { const o = v3("o"), d = v3("d"), t0 = v1("t0"), t1 = v1("t1"); return record(marchText().march(o, d, t0, t1)); },
  },
  // Six example functions.
  dust: {
    call: () => { const p = v3("p"), t = v1("t"); return [dustCall(p, t)]; },
    text: () => { const p = v3("p"), t = v1("t"); return [volumeText().dust(p, t)]; },
  },
  emission: {
    call: () => { const p = v3("p"); return [emissionCall(p)]; },
    text: () => { const p = v3("p"); return [volumeText().emission(p)]; },
  },
  ground: {
    call: () => { const w = v3("world"), s = v1("shadow"); return [groundCall(w, s)]; },
    text: () => { const w = v3("world"), s = v1("shadow"); return [groundText().ground(w, s)]; },
  },
  bootPaint: {
    call: () => { const p = v3("p"); return [bootCall().paint(p)]; },
    text: () => { const p = v3("p"); return [bootText().paint(p)]; },
  },
  wing: {
    call: () => { const u = v1("u"), v = v1("v"); return [wingCall(u, v)]; },
    text: () => { const u = v1("u"), v = v1("v"); return [wingText().wing(u, v)]; },
  },
  reed: {
    call: () => { const m = v1("m"); return [spineCall(K)(m)]; },
    text: () => { const m = v1("m"); return [spineText(K).spine(m)]; },
  },
};

const BIND: Bind = {
  variable: (name) => name,
  input: (spec) => `in_${spec.name}`,
  param: (spec) => `param_${spec.name}`,
  row: (r, x) => `row${r.id}(${x})`,
  table: (t, i, lane) => `${t.name}[${i}][${lane}]`,
};

if (import.meta.main) {
  const [name, form] = process.argv.slice(2) as [string, "text" | "call"];
  const roots = PROBES[name]![form]();
  const { lines, out, libs } = emitWgsl(roots, BIND);
  console.log(JSON.stringify({ hash: structuralHash(roots), wgsl: [...libs.map((l) => l.wgsl), ...lines, `out ${out.join(" ")}`].join("\n") }));
}

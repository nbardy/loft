// WGSL text (src/text): the lane checks of docs/proposals/2026-11-18_authoring_api.md section 9, L1.
import { expect, test } from "bun:test";
import { variable, wgsl } from "../src/index.ts";
import * as E from "../src/expr.ts";
import { compileJs, rawReasonOf } from "../src/expr.ts";
import { Evaluator, Unbounded } from "../src/interval.ts";
import { WGSL_STATE } from "../src/text/index.ts";
import { PROBES } from "./text.probes.ts";

// Check 1. Each probe in call form and in text, each in a fresh process (node numbers are counted per process): the same
// structural hash, and the same emitted WGSL byte for byte. The bug this catches: lowering that builds a different graph
// from the call form (an operand order, a constant lifted where the call form keeps a number, an `if` merged in another
// order), which would change every artefact a scene moved to text makes.
test("text and call form intern to the same graph and emit the same WGSL", () => {
  const run = (name: string, form: string): { hash: string; wgsl: string } => {
    const p = Bun.spawnSync(["bun", "./tests/text.probes.ts", name, form], { stdout: "pipe", stderr: "pipe" });
    if (p.exitCode !== 0) throw new Error(`${name} ${form}: ${p.stderr.toString()}`);
    return JSON.parse(p.stdout.toString());
  };
  const rows = Object.keys(PROBES).map((name) => {
    const call = run(name, "call"), text = run(name, "text");
    return { name, hash: call.hash === text.hash, bytes: call.wgsl === text.wgsl, lines: call.wgsl.split("\n").length };
  });
  console.log(rows.map((r) => `${r.name}: hash ${r.hash ? "same" : "DIFFERENT"}, wgsl ${r.bytes ? "byte-identical" : "DIFFERENT"} (${r.lines} lines)`).join("\n"));
  expect(rows.length).toBe(11);
  expect(rows.filter((r) => !r.hash || !r.bytes).map((r) => r.name)).toEqual([]);
}, 120_000);

// Check 3. A function outside the subset is raw: it renders, and the twin refuses it by name, line and reason. The bug
// this catches: a construct the graph cannot represent slipping into a graph function (a silent wrong value on the CPU)
// or a raw function refused without saying why.
test("each construct outside the subset makes its function raw, with its reason", () => {
  const cases: [string, string][] = [
    ["fn f(x: f32) -> f32 { return f32(u32(x) * 3u); }", "u32 integers"],
    ["fn f(x: f32) -> f32 { let k = vec2i(1, 2); return x; }", "i32 integers"],
    ["fn f(x: f32) -> f32 { return dpdx(x); }", "derivatives (dpdx)"],
    ["fn f(x: f32) -> f32 { return fwidth(x); }", "derivatives (fwidth)"],
    ["fn f(x: f32) -> f32 { var y = x; while (y > 1.0) { y = y * 0.5; } return y; }", "a while loop"],
    ["fn f(x: f32) -> f32 { var y = x; loop { y = y * 0.5; if (y < 1.0) { break; } } return y; }", "a loop without a counter"],
    ["fn f(x: f32) -> f32 { return select(0.0, 1.0, (x > 0.0) & (x < 1.0)); }", "bit operations or pointers (&)"],
    ["fn f(x: f32) -> f32 { return x + f32(0x10); }", "integer literals (0x10)"],
    ["fn f(x: f32) -> f32 { return f32(f16(x)); }", "f16"],
    ["fn f(x: f32) -> f32 { return x * frame.time.x; }", "the frame uniform"],
    ["fn f(x: f32) -> f32 { switch (u32(x)) { default: { return x; } } }", "a switch"],
    ["var<private> acc: f32;\nfn f(x: f32) -> f32 { return acc + x; }", "the module variable acc"],
    ["@group(0) @binding(0) var tex: texture_2d<f32>;\nfn f(x: f32) -> f32 { return textureLoad(tex, vec2i(0), 0).x * x; }", "texture reads"],
    // Builtins whose names continue with a digit, and matrices outside the subset: raw, not "unknown name".
    ["fn f(x: f32) -> f32 { return unpack2x16float(pack2x16float(vec2(x, x))).x; }", "packing"],
    ["fn f(x: f32) -> f32 { let m = mat4x4f(); return m[0].x + x; }", "mat4x4 matrices"],
  ];
  const x = variable("x", "f32");
  for (const [text, why] of cases) {
    const m = wgsl(text);
    const raw = m[WGSL_STATE].raw("f");
    expect(rawReasonOf(raw.lib)).toMatch(new RegExp(`^raw function f at line \\d+ of the wgsl text loaded at tests/text\\.test\\.ts:\\d+: ${why.replace(/[()]/g, "\\$&")}$`));
    // It still renders: graph code calls it, and its text is in the module.
    const call = m.f(x);
    expect(call.node.op).toBe("lib");
    expect(() => compileJs([call])({ vars: { x: 1 }, inputs: {}, params: {} })).toThrow(/raw function f at .*: the CPU twin cannot evaluate it/);
  }
});

// Errors name the author's line. The bug this catches: an off-by-one between a template's lines and the file's, which
// sends the author to the wrong line.
test("a lowering error quotes the file line of the text and suggests the name meant", () => {
  const line = new Error().stack!.split("\n")[1]!.match(/:(\d+):\d+\)?$/)![1];
  const m = wgsl`
fn f(p: vec3f) -> f32 {
  return nois3(p);
}`;
  let message = "";
  try { m.f(variable("p", "vec3")); } catch (e) { message = (e as Error).message; }
  expect(message).toContain(`tests/text.test.ts:${Number(line) + 3}:10: given unknown name "nois3"`);
  expect(message).toContain("did you mean noise3?");
  expect(message).toContain("  return nois3(p);\n         ^");
});

// Two declarations of one name are refused. The bug this catches: Bun's transpiler silently dropped the checking call
// when the helper was named `declare` (an ambient declaration to it), so the second `fn f` replaced the first unseen.
test("a second declaration of a name is refused, quoting its line", () => {
  expect(() => wgsl("fn f(x: f32) -> f32 { return x; }\nfn f(y: f32) -> f32 { return y; }")).toThrow(/line 2 of the wgsl text .*: given a second declaration of f/);
  expect(() => wgsl("struct vec3f { a: f32 }")).toThrow(/given a declaration named vec3f/);
});

// Check 4. A 300-line module parses and lowers in under 20 ms (the target that keeps `loft check` and `build` quick).
test("a 300-line module parses and lowers in under 20 ms", () => {
  const fns = Array.from({ length: 34 }, (_, k) => `
fn g${k}(p: vec3f, t: f32) -> f32 {
  let q = p * ${k + 1}.0 - vec3(0.5, ${k}.0, 0.25);
  var acc = 0.0;
  for (var i = 0; i < 4; i++) {
    let w = sin(q.x * f32(i) + t) * cos(q.y - f32(i));
    if (w > 0.2) { acc += w; } else { acc -= w * 0.5; }
  }
  return acc + smoothstep(0.0, 1.0, length(q)) * ${(k % 7) + 1}.0;
}`).join("");
  const text = fns.split("\n");
  expect(text.length).toBeGreaterThanOrEqual(300);
  const p = variable("p", "vec3"), t = variable("t", "f32");
  const once = (salt: number): number => {
    const start = performance.now();
    const m = wgsl(fns.replaceAll("0.25", String(0.25 + salt)));
    for (let k = 0; k < 34; k++) m[`g${k}`]!(p, t);
    return performance.now() - start;
  };
  const times = Array.from({ length: 7 }, (_, s) => once(s));
  const median = [...times].sort((a, b) => a - b)[3]!;
  console.log(`300-line module (${text.length} lines): parse and lower ${times.map((x) => x.toFixed(1)).join(", ")} ms; median ${median.toFixed(1)} ms`);
  expect(median).toBeLessThan(20);
});

// The thirteen operators text added carry interval enclosures (src/interval.ts) that ranged checks build on. Random
// trees of them over random boxes must hold the CPU twin's value at every sampled point. The bug this catches: an
// unsound rule (a cosh that forgets its minimum at 0, a % whose sign follows the divisor, an acos with its ends swapped).
test("the enclosures of text's operators hold the CPU twin", () => {
  let seed = 0x7e11;
  const random = () => ((seed = (Math.imul(seed, 1664525) + 1013904223) >>> 0) / 2 ** 32);
  const x = E.variable("encX", "f32"), y = E.variable("encY", "f32");
  type S = E.Expr<"f32">;
  const rules: ((a: S, b: S) => S)[] = [
    E.ceil, E.trunc, E.roundEven, (a, b) => E.rem(a, b), E.inverseSqrt, E.exp2, E.log2, E.asin, E.acos, E.atan, E.sinh, E.cosh, E.tanh,
    (a, b) => E.add(a, b), (a, b) => E.mul(a, b), (a) => E.mul(a, 0.4),
  ];
  const leaf = () => [x, y, E.f(random() * 4 - 2)][Math.floor(random() * 3)]!;
  const tree = (depth: number): S => (!depth || random() < 0.3 ? leaf() : rules[Math.floor(random() * rules.length)]!(tree(depth - 1), tree(depth - 1)));
  const covered = rules.map(() => 0);
  let comparisons = 0;
  for (let start = 0; start < 4096; start += 32) {
    const lo = random() * 4 - 2, hi = lo + random() * 0.8, bot = random() * 4 - 2, top = bot + random() * 0.8;
    const box = new Map<string, readonly [number, number]>([["var:encX", [lo, hi]], ["var:encY", [bot, top]]]);
    const roots = Array.from({ length: 32 }, (_, i) => rules[(start + i) % 13]!(tree(2), tree(2)));
    const read = compileJs(roots);
    const bounds = roots.map((root, i) => {
      try { const v = new Evaluator(box, () => undefined, "text", { nodes: 0, codomain: 0 }).scalar(root).at; covered[(start + i) % 13]++; return v; }
      catch (e) { if (!(e instanceof Unbounded)) throw e; return undefined; }
    });
    for (let j = 0; j < 24; j++) {
      const px = j < 4 ? (j & 1 ? hi : lo) : lo + random() * (hi - lo), py = j < 4 ? (j & 2 ? top : bot) : bot + random() * (top - bot);
      const values = read({ vars: { encX: px, encY: py }, inputs: {}, params: {} });
      bounds.forEach((b, i) => {
        const v = values[i] as number;
        if (!b || !Number.isFinite(v)) return;
        if (!(b[0] <= v && v <= b[1])) throw new Error(`rule ${(start + i) % 13} at (${px}, ${py}): ${v} outside [${b}]`);
        comparisons++;
      });
    }
  }
  console.log(`text enclosures: ${covered.slice(0, 13).join(" ")} enclosed per operator, ${comparisons} twin comparisons`);
  expect(Math.min(...covered.slice(0, 13))).toBeGreaterThan(0);
});

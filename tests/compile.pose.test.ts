// The pose function (src/compile/pose.ts, src/compile/fold.ts): pure, and the same numbers as the plain description.
import { expect, test } from "bun:test";
import serpent from "../examples/serpent.ts";
import { compileJs, lift, type Env } from "../src/expr.ts";
import { foldConstants } from "../src/compile/fold.ts";
import { compile, type Artefact } from "../src/index.ts";
import { Strand } from "../src/geom/strand.ts";

const state = (a: Artefact, t: number, set: Record<string, number> = {}) => {
  const inputs = new Float32Array(Math.max(1, a.inputFloats));
  for (const s of a.inputs) if (s.name === "time") inputs[s.slot] = t;
  return { time: t, inputs, params: Float32Array.from(a.params.map((p) => set[p.name] ?? p.def)) };
};

// Catches: the rest-pose memo returning a stale rest after params change (the pose would then depend on what was
// asked before, not only on its argument), or any other hidden state in the pose pass.
test("pose is a function of its state alone", () => {
  const a = compile(serpent);
  const first = a.pose(state(a, 1.3));
  a.pose(state(a, 0.2, { "serpent.neckRise": 6, "serpent.girth": 1.2 }));
  const again = a.pose(state(a, 1.3));
  for (const k of Object.keys(first)) expect(Array.from(again[k])).toEqual(Array.from(first[k]));
  const fresh = compile(serpent).pose(state(a, 0.2, { "serpent.neckRise": 6, "serpent.girth": 1.2 }));
  const changed = a.pose(state(a, 0.2, { "serpent.neckRise": 6, "serpent.girth": 1.2 }));
  for (const k of Object.keys(fresh)) expect(Array.from(changed[k])).toEqual(Array.from(fresh[k]));
});

// Catches: constant folding that changes a value (a wrong operator mapping, a selector read from the wrong lane, a
// vector folded to a scalar). Compares the folded and unfolded pose roots of every strand in the serpent.
test("constant folding does not change the serpent's pose values", () => {
  const strands: Strand[] = [];
  const probe = { slots: { wgsl: () => ({ variable: () => "", input: () => "", param: () => "", row: () => "" }) }, style: serpent.style, sea: serpent.sea,
    stations: (s: Strand) => { if (!strands.includes(s)) strands.push(s); return { base: 0, count: s.stations }; } };
  for (const t of serpent.things) t.draws(probe as never);
  expect(strands.length).toBeGreaterThan(5);
  const roots = strands.flatMap((s) => [0, 0.004, 0.31, 0.5, 1].flatMap((m) => [s.spine(lift(m)), s.halfWidth(lift(m)), s.aspect(lift(m)), s.up]));
  const env: Env = { vars: {}, inputs: { time: 2.1 }, params: {} };
  const plain = compileJs(roots)(env), folded = compileJs(foldConstants(roots))(env);
  plain.forEach((v, i) => {
    const want = typeof v === "number" ? [v] : (v as number[]), got = typeof folded[i] === "number" ? [folded[i] as number] : (folded[i] as number[]);
    expect(got.length).toBe(want.length);
    want.forEach((w, j) => expect(Math.abs(got[j] - w)).toBeLessThan(1e-12 * Math.max(1, Math.abs(w))));
  });
});

// A recurrence's frame replay on the GPU that baked it is bitwise its bake (docs/proposals/state.md 4.1): the design
// rests on this, since the checkpoints are the truth and every frame between them is rebuilt from one. One step, two
// recurrences: `every` keeps all 241 states of 8 s of a 128 x 128 Gray-Scott grid (63 MB), `sparse` runs the same step
// for 30 s, which keeps checkpoints (K = 4). The bug it catches: a replay that does not repeat the bake's arithmetic (a
// step emitted twice and compiled differently, a step number off by one, the wrong parity of the two replay tables, a
// checkpoint read at the wrong offset). Bake against bake (the checkpoints against the states kept) is checked too: a
// bake that is not deterministic would make the first claim meaningless.
import { expect, test } from "bun:test";
import { add, draw, mul, plane, recur, scene, spacing, sub, surface, toon, vec2, vec3, vec4, lt, dot, select, type F } from "../src/index.ts";
import { compile } from "../src/compile/index.ts";
import { createHost } from "../src/host/host.ts";
import { headlessDevice } from "../src/host/headless.ts";

const N = 128;
const spec = (name: string, until: number) => recur({
  name, size: [N, N], rate: 30, until,
  start: (c) => { const d = sub(c, [N / 2, N / 2]); return [vec4(select(lt(dot(d, d), 100), 0.5, 1), select(lt(dot(d, d), 100), 0.25, 0), 0, 0)]; },
  step: (prev, c) => {
    const [s] = prev(c);
    const n = (dx: number, dy: number) => prev(add(c, [dx, dy]))[0]!;
    const lap = sub(add(add(n(1, 0), n(-1, 0)), add(n(0, 1), n(0, -1))), mul(s!, 4));
    const uvv = mul(mul(s!.x, s!.y), s!.y);
    return [vec4(add(add(s!.x, mul(lap.x, 0.2)), add(mul(uvv, -1), mul(sub(1, s!.x), 0.037))), add(add(s!.y, mul(lap.y, 0.1)), sub(uvv, mul(s!.y, 0.097))), 0, 0)];
  },
});

test("a checkpointed recurrence's replay is bitwise its bake, on the GPU that baked it", async () => {
  const every = spec("every", 8), sparse = spec("sparse", 30);
  const mat = surface((u: F, v: F) => vec3(u, add(every.on(vec2(u, v))[0]!.y, sparse.on(vec2(u, v))[0]!.y), v), { u: ["free", "free"], v: "free" });
  const a = compile(scene({ ground: plane, style: toon(), sun: [0, 1, 0], things: [draw.surface("mat", mat, { paint: () => vec3(1, 1, 1), grid: { u: spacing.even(8), v: spacing.even(8) }, sides: "two", outline: "none" })], cameras: { c: { kind: "perspective", eye: [0, 2, 2], target: [0.5, 0, 0.5], fovY: 0.8 } } }));
  const [e, s] = [a.recurrences!.find((r) => r.name === "every")!, a.recurrences!.find((r) => r.name === "sparse")!];
  expect(e.kept).toEqual({ kind: "every" });
  expect(s.kept).toEqual({ kind: "checkpoints", every: 4 });
  const loaded = await createHost(await headlessDevice()).load(a);
  const S = (N * N + 1) * 4; // floats per state, the clock entry included
  const state = (r: typeof e, k: number) => r.data.subarray(k * S, (k + 1) * S);
  // Bake against bake: each checkpoint is the state the every-step bake kept at that step.
  for (let c = 0; c * 4 <= 240; c++) expect(Buffer.compare(Buffer.from(state(s, c).buffer.slice(state(s, c).byteOffset, state(s, c).byteOffset + S * 4)), Buffer.from(state(e, c * 4).buffer.slice(state(e, c * 4).byteOffset, state(e, c * 4).byteOffset + S * 4)))).toBe(0);
  const table = (name: string) => a.tables.find((t) => t.name === name)!;
  // Replay against bake: at n = 4c + k for every k (each parity, each count of ping and pong), states n and n + 1.
  for (const n of [0, 1, 2, 3, 4, 5, 6, 7, 101, 102, 103, 238]) {
    const t = (n + 0.5) / 30;
    const read = await loaded.readTables({ camera: a.cameras.c!, width: 8, height: 8, time: t });
    const [even, odd] = ["sparse_even", "sparse_odd"].map((name) => { const x = table(name); return read[x.buffer]!.subarray(x.offset * 4, (x.offset + x.vec4s) * 4); });
    const k = n % 4, [now, next] = k % 2 === 0 ? [even!, odd!] : [odd!, even!];
    const same = (x: Float32Array, y: Float32Array) => x.every((v, i) => Object.is(v, y[i]));
    expect({ n, now: same(now, state(e, n)), next: same(next, state(e, n + 1)) }).toEqual({ n, now: true, next: true });
  }
  loaded.destroy();
}, 120_000);

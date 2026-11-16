// Resume (src/host/host.ts `advance`): a frame reached by stepping on from the last frame's pair draws the same bytes as
// the same time drawn alone. The bug it catches: a resume that steps the wrong table (the parity of the pair), reads a
// stale pair after a seek, or lands on a different arithmetic than the bake's, any of which makes a frame depend on the
// frames drawn before it. One sequence walks forward, repeats a step, crosses a checkpoint, seeks back and seeks past R;
// every frame's pixels are compared with a fresh Loaded drawing that time first.
import { expect, test } from "bun:test";
import { createHash } from "node:crypto";
import { add, draw, mul, plane, recur, scene, spacing, sub, surface, toon, vec2, vec3, vec4, lt, dot, select, type F } from "../src/index.ts";
import { compile } from "../src/compile/index.ts";
import { advance, createHost } from "../src/host/host.ts";
import { replayClocks } from "../src/artefact.ts";
import { headlessDevice } from "../src/host/headless.ts";

const N = 128;
const sparse = recur({
  name: "sparse", size: [N, N], rate: 30, until: 30,
  start: (c) => { const d = sub(c, [N / 2, N / 2]); return [vec4(select(lt(dot(d, d), 100), 0.5, 1), select(lt(dot(d, d), 100), 0.25, 0), 0, 0)]; },
  step: (prev, c) => {
    const [s] = prev(c);
    const n = (dx: number, dy: number) => prev(add(c, [dx, dy]))[0]!;
    const lap = sub(add(add(n(1, 0), n(-1, 0)), add(n(0, 1), n(0, -1))), mul(s!, 4));
    const uvv = mul(mul(s!.x, s!.y), s!.y);
    return [vec4(add(add(s!.x, mul(lap.x, 0.2)), add(mul(uvv, -1), mul(sub(1, s!.x), 0.037))), add(add(s!.y, mul(lap.y, 0.1)), sub(uvv, mul(s!.y, 0.097))), 0, 0)];
  },
});

test("a frame reached by resume draws the bytes it draws alone", async () => {
  const mat = surface((u: F, v: F) => vec3(u, mul(sparse.on(vec2(u, v))[0]!.y, 4), v), { u: ["free", "free"], v: "free" });
  const a = compile(scene({ ground: plane, style: toon(), sun: [0, 1, 0], things: [draw.surface("mat", mat, { paint: () => vec3(1, 1, 1), grid: { u: spacing.even(8), v: spacing.even(8) }, sides: "two", outline: "none" })], cameras: { c: { kind: "perspective", eye: [0, 2, 2], target: [0.5, 0, 0.5], fovY: 0.8 } } }));
  const clock = replayClocks(a.recurrences)[0]!;
  expect(clock.every).toBe(4);
  const at = (n: number, f = 0.5) => (n + f) / 30;
  // [step, fraction of the step]: forward by one and two, the same step again, over a checkpoint (104, 108), a jump back,
  // a jump past R (51 -> 57), a short hop after it, and the clamps (before the start, past the end).
  const walk: [number, number][] = [
    [100, 0.5], [101, 0.5], [102, 0.5], [102, 0.9], [103, 0.5], [104, 0.5], [105, 0.5], [107, 0.5], [108, 0.5], [109, 0.5], [112, 0.5],
    [50, 0.5], [51, 0.5], [57, 0.5], [58, 0.5], [60, 0.5], [61, 0.2], [899, 0.5], [900, 0.5], [0, 0.5],
  ];
  const view = (t: number) => ({ camera: a.cameras.c!, width: 48, height: 48, time: t });
  const hash = (p: Uint8Array) => createHash("sha256").update(p).digest("hex");
  const device = await headlessDevice();
  const host = createHost(device);
  const warm = await host.load(a);
  const kinds: string[] = [];
  let last: number | undefined;
  const got: string[] = [];
  for (const [n, f] of walk) {
    const adv = advance(clock, last, at(n, f));
    kinds.push(adv.kind);
    last = Math.min(Math.max(n, 0), clock.steps - 1);
    got.push(hash(await warm.frame(view(at(n, f)))));
  }
  warm.destroy();
  // The sequence exercises every branch, or the comparison below proves nothing about it.
  expect(new Set(kinds)).toEqual(new Set(["replay", "resume", "kept"]));
  for (const [i, [n, f]] of walk.entries()) {
    const alone = await host.load(a);
    expect({ n, f, hash: hash(await alone.frame(view(at(n, f)))) }).toEqual({ n, f, hash: got[i]! });
    alone.destroy();
  }
  // Different steps are different pictures (the hash is not blind).
  expect(new Set(got).size).toBeGreaterThan(walk.length / 2);
}, 240_000);

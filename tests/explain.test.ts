// `loft check --explain` and `--at` (src/cli/explain.ts, src/ranged.ts `pinned`), on the heron's leap checked against
// a rail raised 0.2 m (tests/fixtures/explain/heron_rail_raised.ts). CPU only.
import { expect, test } from "bun:test";
import { parseArgs } from "../src/cli/args.ts";
import type { Ranged } from "../src/ranged.ts";

const FIXTURE = "tests/fixtures/explain/heron_rail_raised.ts";

/** The least margin of a check, sampled every millisecond of its range, and where. */
async function sampledMinimum() {
  const m = await import(`../${FIXTURE}`);
  const { compileScene } = await import("../src/compile/index.ts");
  const { Probe } = await import("../src/check.ts");
  const probe = new Probe(compileScene(m.default), {});
  return { m, probe, least: (margin: unknown) => {
    let best = { t: 0, v: Infinity };
    for (let i = 0; i <= 1800; i++) { const v = probe.eval([margin as never], i / 1000)[0] as number; if (v < best.v) best = { t: i / 1000, v }; }
    return best;
  } };
}

// Catches: stacks recorded after the scene is imported (no line named), the wrong side of an `and` or the wrong check
// explained, and a printed render command that `loft render` would refuse.
test("--explain names each failing hold's line, a witness at the sampled minimum, and a render command that parses", async () => {
  const p = Bun.spawn(["bun", "src/cli/main.ts", "check", FIXTURE, "--explain"], { stdout: "pipe", stderr: "pipe" });
  const [out, code] = [await new Response(p.stdout).text(), await p.exited];
  expect(code).toBe(5);
  const explained = out.split("\nexplain: ").slice(1);
  expect(explained.length).toBe(2);
  // The two clearance holds the raised rail breaks: the lower leg (line 87) and the foot (line 88) of heron_leap.ts.
  expect(explained.map((e) => e.match(/decided by ge\(a, b\)\s+at (examples\/second\/heron_leap\.ts:\d+)/)?.[1])).toEqual(["examples/second/heron_leap.ts:87", "examples/second/heron_leap.ts:88"]);

  const { m, probe, least } = await sampledMinimum();
  // Every range here is [0, 1.8] or narrower, and the failing two are the whole span.
  const failing = m.checks.filter((c: Ranged) => c.decide(probe).holds === false).map((c: Ranged) => least((c.formula.body as { margin: unknown }).margin));
  expect(failing.length).toBe(2);
  explained.forEach((e, i) => {
    const command = e.match(/see it: loft (render .*)$/m)![1]!.split(" ");
    const parsed = parseArgs(command);
    if (parsed.kind !== "render") throw new Error(`not a render command: ${command.join(" ")}`);
    expect(Math.abs(parsed.time - failing[i]!.t)).toBeLessThan(0.02);
  });
}, 60_000);

// Catches: `--at` holding the wrong leaf, or a pinned check searching its old range instead of the point.
test("--at time=t gives the margin at t", async () => {
  const { pinned } = await import("../src/ranged.ts");
  const { m, probe } = await sampledMinimum();
  const check = m.checks.at(-2);
  const p = pinned(check, new Map([["time", 0.95]]), probe);
  if (p.kind !== "pinned") throw new Error(`0.95 is inside [0, 1.8], got ${p.kind}`);
  const d = p.check.decide(probe);
  expect(d.witness).toEqual({ time: 0.95 });
  expect(d.margin).toBeCloseTo(probe.eval([check.formula.body.margin], 0.95)[0] as number, 4);
  expect(pinned(check, new Map([["time", 2]]), probe).kind).toBe("outside");
}, 60_000);

// `loft serve` drawing on `loft-draw serve` (LOFT_ENGINE=rust; plan 2026-11-19 lane 3.3): Bundle V1 files on
// loft-gpu. The same request twice gives the same Identity and RGBA, a draw that never ends is `budget` and the next
// request draws on a fresh child, and a child that dies answers every request in flight before the server exits 6.
// The bundle is tests/fixtures/ball_bundle.ts's (no corpus scene records draws yet). The --check hook runs in the
// supervisor here, so it stops or kills the child by pid, as a hung driver or a crash would.
// A fresh checkout needs the lab builds of loft-node and loft-draw.
import { expect, test } from "bun:test";
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const MAIN = join(ROOT, "src/cli/main.ts");
type Answer = { id: string; ok: boolean; exit?: number; line?: string; identity?: object; frames?: { sha256: string; id: string; file: string }[] };

async function served(flags: readonly string[], requests: readonly object[]) {
  const p = Bun.spawn([process.execPath, MAIN, "serve", ...flags], { stdin: "pipe", stdout: "pipe", stderr: "pipe", env: { ...process.env, LOFT_ENGINE: "rust" } });
  p.stdin.write(requests.map((l) => JSON.stringify(l)).join("\n") + "\n");
  p.stdin.end();
  const [text, code] = await Promise.all([new Response(p.stdout).text(), p.exited]);
  const answers = text.trim().split("\n").map((l) => JSON.parse(l));
  expect(answers[0].ready).toBe(true);
  return { code, by: new Map<string, Answer>(answers.slice(1).map((a) => [a.id, a])), count: answers.length - 1 };
}

/** A --check hook that, on its `nth` call, sends `signal` to this server's loft-draw child. */
function hook(dir: string, nth: number, signal: string): string {
  const file = join(dir, `${signal}.ts`);
  writeFileSync(file, `let calls = 0;
export default function () {
  if (++calls !== ${nth}) return;
  const pid = Bun.spawnSync(["pgrep", "-P", String(process.pid), "loft-draw"]).stdout.toString().trim();
  process.kill(Number(pid), "${signal}");
}
`);
  return file;
}

const sha = (a: Answer | undefined) => a!.frames!.map((f) => f.sha256);

/** `out/ball/scene.loft`, built in its own process (the recorder is global state). */
function ball(out: string): string {
  const dir = join(out, "ball");
  mkdirSync(dir);
  const fixture = Bun.spawnSync([process.execPath, join(ROOT, "tests/fixtures/ball_bundle.ts"), dir]);
  expect(fixture.exitCode, fixture.stderr.toString()).toBe(0);
  return dir;
}

test("serve on loft-draw: A, B, A gives A's identity and RGBA both times", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-rust-"));
  try {
    const dir = ball(out);
    const ask = (id: string, params: object) => ({ id, artefact: dir, camera: "front", times: [0.4, 1.3], size: "96x64", out: join(out, id), params });
    const given = { id: "given", artefact: dir, camera: { kind: "perspective", eye: [0, 0, 5], target: [0, 0, 0], fovY: 0.8 }, times: [0.4], size: "96x64", out: join(out, "given") };
    const { code, by } = await served([], [ask("A1", {}), ask("B", { "ball.tint": 0.5 }), { id: "bad", scene: "x.ts" }, { ...ask("nope", {}), camera: "side2" }, given, ask("A2", {})]);
    expect(code).toBe(0);
    expect(by.get("bad")).toMatchObject({ ok: false, exit: 2, line: expect.stringMatching(/^loft: usage: /) });
    expect(by.get("nope")).toMatchObject({ ok: false, exit: 2, line: expect.stringContaining("the bundle names [front]") });
    expect((by.get("A1")!.identity as { frame: { precision: string } }).frame.precision).toBe("f32");
    const ids = (a: Answer | undefined) => a!.frames!.map((f) => f.id);
    expect(new Set(ids(by.get("A1"))).size).toBe(2);
    expect(ids(by.get("B")).filter((id) => ids(by.get("A1")).includes(id))).toEqual([]);
    expect(sha(by.get("A2"))).toEqual(sha(by.get("A1")));
    expect(by.get("A2")!.identity).toEqual(by.get("A1")!.identity);
    expect(by.get("A2")!.frames!.map((f) => f.id)).toEqual(by.get("A1")!.frames!.map((f) => f.id));
    expect(sha(by.get("B"))).not.toEqual(sha(by.get("A1")));
    // The host look camera the request gives is the bundle's own "front": the same pixels, a different identity.
    expect(sha(by.get("given"))[0]).toEqual(sha(by.get("A1"))[0]);
    // The ball covers the middle of the frame and not the corner.
    const rgba = readFileSync(by.get("A1")!.frames![0]!.file);
    const px = (x: number, y: number) => [...rgba.subarray((y * 96 + x) * 4, (y * 96 + x) * 4 + 4)];
    expect(px(48, 32)[2]).toBeGreaterThan(200);
    expect(px(1, 1)).toEqual([0, 0, 0, 255]);
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve on loft-draw: a draw that never ends answers budget and the next draws on a fresh child", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-rust-"));
  try {
    const dir = ball(out);
    const ask = (id: string) => ({ id, artefact: dir, camera: "front", times: [0.4], size: "96x64", out: join(out, id) });
    const t0 = performance.now();
    const { code, by, count } = await served(["--check", hook(out, 2, "SIGSTOP"), "--draw-seconds", "3"], [ask("warm"), ask("late"), ask("next")]);
    expect(performance.now() - t0).toBeLessThan(15_000);
    expect([code, count]).toEqual([0, 3]);
    expect(by.get("late")).toMatchObject({ ok: false, exit: 7, line: expect.stringContaining("3 s (--draw-seconds)") });
    expect(existsSync(join(out, "late/0.rgba"))).toBe(false);
    expect(sha(by.get("next"))).toEqual(sha(by.get("warm")));
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve on loft-draw: a child that dies answers every request in flight, then the server exits 6", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-rust-"));
  try {
    const dir = ball(out);
    const ask = (id: string) => ({ id, artefact: dir, camera: "front", times: [0.4], size: "96x64", out: join(out, id) });
    const { code, by, count } = await served(["--check", hook(out, 2, "SIGKILL")], [ask("l1"), ask("l2"), ask("l3")]);
    expect([code, count]).toEqual([6, 3]);
    expect(by.get("l1")!.ok).toBe(true);
    for (const id of ["l2", "l3"]) expect(by.get(id)).toMatchObject({ ok: false, exit: 6 });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

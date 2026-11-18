// `loft serve` drawing on `loft-draw serve` (LOFT_ENGINE=rust; plan 2026-11-19 lane 3.3). The engine-neutral cases of
// tests/serve.test.ts run as they are under LOFT_ENGINE=rust; these replace the three that reach into Dawn: the same
// request twice gives the same Identity and RGBA, a draw that never ends is `budget` and the next request draws on a
// fresh child, and a child that dies answers every request in flight before the server exits 6. The --check hook runs
// in the supervisor here, so it stops or kills the child by pid, as a hung driver or a crash would.
import { expect, test } from "bun:test";
import { existsSync, mkdtempSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const MAIN = join(ROOT, "src/cli/main.ts");
type Answer = { id: string; ok: boolean; exit?: number; line?: string; identity?: object; frames?: { sha256: string; id: string }[] };

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

test("serve on loft-draw: A, B, A gives A's identity and RGBA both times", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-rust-"));
  try {
    const ask = (id: string, scene: string) => ({ id, scene: join(ROOT, scene), camera: "three", times: [0.4, 1.3], size: "96x64", out: join(out, id) });
    const dir = join(out, "shell");
    expect(Bun.spawnSync([process.execPath, MAIN, "build", join(ROOT, "examples/shell.ts"), "--out", dir]).exitCode).toBe(0);
    const mixed = { id: "mixed", artefact: dir, camera: "three", times: [0.4], size: "96x64", out: join(out, "mixed"), precision: "mixed" };
    const { code, by } = await served(["--compiles", "1"], [ask("A1", "examples/shell.ts"), ask("B", "examples/pearls.ts"), { id: "bad", scene: "x.ts" }, mixed, ask("A2", "examples/shell.ts")]);
    expect(code).toBe(0);
    expect(by.get("bad")).toMatchObject({ ok: false, exit: 2, line: expect.stringMatching(/^loft: usage: /) });
    // As on the TS host: "mixed" asked of an f32-only artefact is refused, not drawn in f32.
    expect(by.get("mixed")).toMatchObject({ ok: false, exit: 3 });
    expect((by.get("A1")!.identity as { frame: { precision: string } }).frame.precision).toBe("f32");
    const ids = (a: Answer | undefined) => a!.frames!.map((f) => f.id);
    expect(new Set(ids(by.get("A1"))).size).toBe(2);
    expect(ids(by.get("B")).filter((id) => ids(by.get("A1")).includes(id))).toEqual([]);
    expect(sha(by.get("A2"))).toEqual(sha(by.get("A1")));
    expect(by.get("A2")!.identity).toEqual(by.get("A1")!.identity);
    expect(by.get("A2")!.frames!.map((f) => f.id)).toEqual(by.get("A1")!.frames!.map((f) => f.id));
    expect(sha(by.get("B"))).not.toEqual(sha(by.get("A1")));
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve on loft-draw: a draw that never ends answers budget and the next draws on a fresh child", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-rust-"));
  try {
    const dir = join(out, "shell");
    expect(Bun.spawnSync([process.execPath, MAIN, "build", join(ROOT, "examples/shell.ts"), "--out", dir]).exitCode).toBe(0);
    const ask = (id: string) => ({ id, artefact: dir, camera: "three", times: [0.4], size: "96x64", out: join(out, id) });
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
    const dir = join(out, "shell");
    expect(Bun.spawnSync([process.execPath, MAIN, "build", join(ROOT, "examples/shell.ts"), "--out", dir]).exitCode).toBe(0);
    const ask = (id: string) => ({ id, artefact: dir, camera: "three", times: [0.4], size: "96x64", out: join(out, id) });
    const { code, by, count } = await served(["--check", hook(out, 2, "SIGKILL")], [ask("l1"), ask("l2"), ask("l3")]);
    expect([code, count]).toEqual([6, 3]);
    expect(by.get("l1")!.ok).toBe(true);
    for (const id of ["l2", "l3"]) expect(by.get(id)).toMatchObject({ ok: false, exit: 6 });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

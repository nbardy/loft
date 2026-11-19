// `loft serve` (src/cli/serve.ts). The bug this catches: a warm server whose frame depends on what it drew before. That
// can come from the GPU object cache (src/host/cache.ts) keying a pipeline on too little, so one scene draws with
// another's, or from per-load state that outlives a request. Scene A, then B, then A again must give A's pixels both
// times, equal to a fresh `loft frames` of A. A refused request must be answered with Loft's exit code, and the server
// must go on.
//
// The rest are the admission boundary (D10 of agent_notes/2026-11-18_plan_review/CONSOLIDATED.md): requests carry
// params and inputs; two requests in flight with overlapping `out` cannot corrupt each other; a manifest is checked
// before the files it names are read; pending requests are bounded; a compile past its deadline or a CPU limit is
// `budget` (7), not `internal`; and a lost device answers every request in flight before the server exits 6.
import { expect, test } from "bun:test";
import { chmodSync, existsSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const MAIN = join(ROOT, "src/cli/main.ts");

type Answer = { id: string; ok: boolean; exit?: number; line?: string; identity?: { frame: { precision: string } }; frames?: { file: string; sha256: string; id: string }[] };

/** Runs one server over `requests` (sent at once, then stdin closed): its exit code and its answers by id. */
async function served(flags: readonly string[], requests: readonly object[], engine: "view" | "dawn" = "view"): Promise<{ code: number; by: Map<string, Answer>; count: number }> {
  const p = Bun.spawn([process.execPath, MAIN, "serve", ...flags], { stdin: "pipe", stdout: "pipe", stderr: "pipe", env: { ...process.env, LOFT_ENGINE: engine } });
  p.stdin.write(requests.map((l) => JSON.stringify(l)).join("\n") + "\n");
  p.stdin.end();
  const [text, code] = await Promise.all([new Response(p.stdout).text(), p.exited]);
  const answers = text.trim().split("\n").map((l) => JSON.parse(l));
  expect(answers[0].ready).toBe(true);
  return { code, by: new Map(answers.slice(1).map((a) => [a.id, a])), count: answers.length - 1 };
}

const hashes = (a: Answer | undefined) => a!.frames!.map((f) => f.sha256.slice(0, 16));
const ids = (a: Answer | undefined) => a!.frames!.map((f) => f.id.slice(0, 16));

function build(scene: string, dir: string): void {
  const b = Bun.spawnSync([process.execPath, MAIN, "build", join(ROOT, scene), "--out", dir]);
  expect(b.exitCode).toBe(0);
}

test("serve: A, B, A draws A's pixels both times, as a fresh process does; a bad request does not end it", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const ask = (id: string, scene: string) => ({ id, scene: join(ROOT, scene), camera: "three", times: [0.4, 1.3], size: "96x64", out: join(out, id) });
    // --compiles 1 and requests in this order: A1's draw comes before B's, and B's before A2's.
    const { code, by } = await served(["--compiles", "1"], [ask("A1", "examples/shell.ts"), ask("B", "examples/pearls.ts"), { id: "bad", scene: "x.ts" }, ask("A2", "examples/shell.ts")]);
    expect(code).toBe(0);
    expect(by.get("bad")).toMatchObject({ ok: false, exit: 2 });
    expect(by.get("bad")!.line).toStartWith("loft: usage: ");
    expect(by.get("B")!.ok).toBe(true);
    expect(hashes(by.get("A2"))).toEqual(hashes(by.get("A1")));
    // The same request twice: the same ids (src/identity.ts); each time its own id, another scene other ids.
    expect(ids(by.get("A2"))).toEqual(ids(by.get("A1")));
    expect(new Set(ids(by.get("A1"))).size).toBe(2);
    expect(ids(by.get("B")).filter((id) => ids(by.get("A1")).includes(id))).toEqual([]);
    expect(by.get("A1")!.identity!.frame.precision).toBe("f32");

    const fresh = Bun.spawnSync([process.execPath, MAIN, "frames", join(ROOT, "examples/shell.ts"), "--view", "three", "--times", "0.4,1.3", "--size", "96x64", "--out", join(out, "fresh.webp")]);
    expect(fresh.exitCode).toBe(0);
    expect([...fresh.stdout.toString().matchAll(/rgba ([0-9a-f]{16})/g)].map((m) => m[1])).toEqual(hashes(by.get("A1")));
    // A scene compiled in the CLI and the directory serve built of it are one program: the same frames' ids agree.
    expect([...fresh.stdout.toString().matchAll(/ id ([0-9a-f]{16})/g)].map((m) => m[1])).toEqual(ids(by.get("A1")));
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve: params and inputs reach the frame, and a name the scene lacks is refused by name", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const dir = join(out, "stormwing");
    build("examples/stormwing.ts", dir);
    const ask = (id: string, extra: object) => ({ id, artefact: dir, camera: "three", times: [0.7], size: "96x64", out: join(out, id), ...extra });
    const { code, by } = await served([], [
      ask("plain", {}),
      ask("wide", { params: { "stormwing.span": 1.4 } }),
      ask("struck", { inputs: { strike: 0.5 } }),
      ask("typo", { params: { "stormwing.spam": 1.4 } }),
      ask("vector", { inputs: { strike: [1, 2] } }),
      ask("clock", { inputs: { time: 1 } }),
    ]);
    expect(code).toBe(0);
    for (const id of ["plain", "wide", "struck"]) expect(by.get(id)!.ok).toBe(true);
    expect(hashes(by.get("wide"))).not.toEqual(hashes(by.get("plain")));
    expect(hashes(by.get("struck"))).not.toEqual(hashes(by.get("plain")));
    expect(new Set(["plain", "wide", "struck"].map((id) => ids(by.get(id))[0])).size).toBe(3);
    expect(by.get("typo")).toMatchObject({ ok: false, exit: 2 });
    expect(by.get("typo")!.line).toContain("stormwing.span");
    expect(by.get("vector")).toMatchObject({ ok: false, exit: 2 });
    expect(by.get("clock")).toMatchObject({ ok: false, exit: 2, line: expect.stringContaining('give the times in "times"') });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve: requests in flight with overlapping out cannot overwrite each other's build or frames", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const shared = join(out, "same");
    const ask = (id: string, scene: string, dest: string) => ({ id, scene: join(ROOT, scene), camera: "three", times: [0.4, 1.3], size: "96x64", out: dest });
    // All three arrive while the first compiles: the second names the same out, the third a folder inside it.
    const { by } = await served(["--compiles", "3"], [ask("A", "examples/shell.ts", shared), ask("B", "examples/pearls.ts", shared), ask("C", "examples/pearls.ts", join(shared, "inner"))]);
    expect(by.get("A")!.ok).toBe(true);
    for (const id of ["B", "C"]) {
      expect(by.get(id)).toMatchObject({ ok: false, exit: 2 });
      expect(by.get(id)!.line).toContain("\"A\"");
    }
    // What A's answer says is what A's files hold, and the build stayed private (nothing but frames in out).
    for (const f of by.get("A")!.frames!) expect(new Bun.CryptoHasher("sha256").update(readFileSync(f.file)).digest("hex")).toBe(f.sha256);
    expect(Bun.spawnSync(["ls", shared]).stdout.toString().trim().split("\n").sort()).toEqual(["0.rgba", "1.rgba"]);
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve: an artefact is checked before its files are read, and pending requests are bounded", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const good = join(out, "shell"), evil = join(out, "evil");
    build("examples/shell.ts", good);
    build("examples/shell.ts", evil);
    // A chunk named outside wgsl/: the old loader read `wgsl/../manifest.json.wgsl`-style paths as given.
    const m = JSON.parse(readFileSync(join(evil, "manifest.json"), "utf8"));
    writeFileSync(join(evil, "outside.wgsl"), "// not in wgsl/\n");
    m.chunks[0] = "../outside";
    writeFileSync(join(evil, "manifest.json"), JSON.stringify(m));
    const ask = (id: string, dir: string) => ({ id, artefact: dir, camera: "three", times: [0.4], size: "96x64", out: join(out, id) });
    const small = await served(["--limits", "bytes=20000"], [ask("big", good), ask("escape", evil)]);
    expect(small.by.get("big")).toMatchObject({ ok: false, exit: 3 });
    expect(small.by.get("big")!.line).toContain("bytes=");
    expect(small.by.get("escape")).toMatchObject({ ok: false, exit: 3 });
    expect(small.by.get("escape")!.line).toContain("../outside");

    // One pending at most: the first is admitted, the two that arrive while it draws are refused as `budget`.
    const busy = await served(["--pending", "1"], [ask("p1", good), ask("p2", good), ask("p3", good)]);
    expect(busy.code).toBe(0);
    expect(busy.by.get("p1")!.ok).toBe(true);
    for (const id of ["p2", "p3"]) {
      expect(busy.by.get(id)).toMatchObject({ ok: false, exit: 7 });
      expect(busy.by.get(id)!.line).toStartWith("loft: budget: ");
    }
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve: a compile past its deadline, or killed by a CPU limit, is budget (7), not internal", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const spin = join(out, "spin.ts");
    writeFileSync(spin, "while (true) {}\nexport {};\n");
    // `ulimit -t` sends SIGXCPU when the CPU seconds run out; exec keeps the signal on the compile itself.
    const limit = join(out, "cpu1.sh");
    writeFileSync(limit, "#!/bin/sh\nulimit -t 1\nexec \"$@\"\n");
    chmodSync(limit, 0o755);
    const ask = (id: string) => ({ id, scene: spin, camera: "three", times: [0], size: "16x16", out: join(out, id) });
    const t0 = performance.now();
    const deadline = await served(["--compile-seconds", "1.5"], [ask("late")]);
    expect(deadline.by.get("late")).toMatchObject({ ok: false, exit: 7 });
    expect(deadline.by.get("late")!.line).toStartWith("loft: budget: ");
    expect(performance.now() - t0).toBeLessThan(20_000);
    const cpu = await served(["--confine", limit], [ask("cpu")]);
    expect(cpu.by.get("cpu")).toMatchObject({ ok: false, exit: 7 });
    expect(cpu.by.get("cpu")!.line).toContain("SIGXCPU");
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve: a blocked draw answers budget and the next request draws on a fresh GPU process", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const dir = join(out, "shell");
    build("examples/shell.ts", dir);
    const block = join(out, "block.ts");
    // Block the second draw inside readback. Even a timer racing the draw in this same process cannot run here.
    // The finite stall makes a broken implementation fail the test without leaving an infinite loop behind.
    writeFileSync(block, `let calls = 0;
export default function () {
  if (++calls !== 2) return;
  const proto = GPUDevice.prototype, make = proto.createBuffer;
  proto.createBuffer = function (descriptor: GPUBufferDescriptor) {
    const buffer = make.call(this, descriptor);
    if (descriptor.label !== "loft readback") return buffer;
    proto.createBuffer = make;
    const map = buffer.mapAsync.bind(buffer);
    buffer.mapAsync = (...args) => {
      const until = performance.now() + 15_000;
      while (performance.now() < until) {}
      return map(...args);
    };
    return buffer;
  };
}
`);
    const ask = (id: string) => ({ id, artefact: dir, camera: "three", times: [0.4], size: "96x64", out: join(out, id) });
    const t0 = performance.now();
    // The hook reaches into Dawn's GPUDevice: this deadline is the Dawn worker's (the supervisor's kill is the same for loft-view).
    const { code, by, count } = await served(["--check", block, "--draw-seconds", "5"], [ask("warm"), ask("late"), ask("next")], "dawn");
    expect(performance.now() - t0).toBeLessThan(15_000);
    expect(code).toBe(0);
    expect(count).toBe(3);
    expect(by.get("late")).toMatchObject({ ok: false, exit: 7, line: expect.stringContaining("5 s (--draw-seconds)") });
    expect(existsSync(join(out, "late/0.rgba"))).toBe(false);
    expect(hashes(by.get("next"))).toEqual(hashes(by.get("warm")));
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

test("serve: a lost device answers every request in flight, then the server exits 6", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const dir = join(out, "shell");
    build("examples/shell.ts", dir);
    // The --check hook runs in the server before each load: the second time (l2's, with l3 queued behind it), it makes
    // the next buffer (shader modules come from the cache) destroy the device: a loss arrives as a promise, between awaits.
    const lose = join(out, "lose.ts");
    writeFileSync(lose, `let calls = 0;
export default function () {
  if (++calls !== 2) return;
  const proto = (globalThis as any).GPUDevice.prototype, make = proto.createBuffer;
  proto.createBuffer = function (d: unknown) { proto.createBuffer = make; this.destroy(); return make.call(this, d); };
}
`);
    const ask = (id: string) => ({ id, artefact: dir, camera: "three", times: [0.4], size: "96x64", out: join(out, id) });
    // The hook destroys a Dawn GPUDevice: this loss is the Dawn worker's (loft-view's loss is its exit 6, main.rs `gpu`).
    const { code, by, count } = await served(["--check", lose], [ask("l1"), ask("l2"), ask("l3")], "dawn");
    expect(code).toBe(6);
    expect(count).toBe(3);
    expect(by.get("l1")!.ok).toBe(true);
    for (const id of ["l2", "l3"]) expect(by.get(id)).toMatchObject({ ok: false, exit: 6 });
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

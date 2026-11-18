// `loft serve` (src/cli/serve.ts). The bug this catches: a warm server whose frame depends on what it drew before. That
// can come from the GPU object cache (src/host/cache.ts) keying a pipeline on too little, so one scene draws with
// another's, or from per-load state that outlives a request. Scene A, then B, then A again must give A's pixels both
// times, equal to a fresh `loft frames` of A. A refused request must be answered with Loft's exit code, and the server
// must go on.
import { expect, test } from "bun:test";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const MAIN = join(ROOT, "src/cli/main.ts");

test("serve: A, B, A draws A's pixels both times, as a fresh process does; a bad request does not end it", async () => {
  const out = mkdtempSync(join(tmpdir(), "loft-serve-"));
  try {
    const ask = (id: string, scene: string) => ({ id, scene: join(ROOT, scene), camera: "three", times: [0.4, 1.3], size: "96x64", out: join(out, id) });
    const lines = [ask("A1", "examples/shell.ts"), ask("B", "examples/pearls.ts"), { id: "bad", scene: "x.ts" }, ask("A2", "examples/shell.ts")];
    // --compiles 1 and requests in this order: A1's draw comes before B's, and B's before A2's.
    const p = Bun.spawn([process.execPath, MAIN, "serve", "--compiles", "1"], { stdin: "pipe", stdout: "pipe", stderr: "pipe" });
    p.stdin.write(lines.map((l) => JSON.stringify(l)).join("\n") + "\n");
    p.stdin.end();
    const [text, code] = await Promise.all([new Response(p.stdout).text(), p.exited]);
    expect(code).toBe(0);
    const answers = text.trim().split("\n").map((l) => JSON.parse(l));
    expect(answers[0].ready).toBe(true);
    const by = new Map(answers.slice(1).map((a) => [a.id, a]));
    expect(by.get("bad")).toMatchObject({ ok: false, exit: 2 });
    expect(by.get("bad").line).toStartWith("loft: usage: ");
    const hashes = (id: string) => by.get(id).frames.map((f: { sha256: string }) => f.sha256.slice(0, 16));
    expect(by.get("B").ok).toBe(true);
    expect(hashes("A2")).toEqual(hashes("A1"));
    // B shares shader modules with A (the sea, the sky): the second A was drawn partly from the cache.
    expect(by.get("A2").cache.hits).toBeGreaterThan(by.get("A1").cache.hits);

    const fresh = Bun.spawnSync([process.execPath, MAIN, "frames", join(ROOT, "examples/shell.ts"), "--view", "three", "--times", "0.4,1.3", "--size", "96x64", "--out", join(out, "fresh.webp")]);
    expect(fresh.exitCode).toBe(0);
    expect([...fresh.stdout.toString().matchAll(/rgba ([0-9a-f]{16})/g)].map((m) => m[1])).toEqual(hashes("A1"));
  } finally {
    rmSync(out, { recursive: true, force: true });
  }
}, 120_000);

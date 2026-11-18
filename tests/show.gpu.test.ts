// Lane L4's check (docs/proposals/2026-11-18_authoring_api.md section 9): generated code made visible, and the GPU's
// refusals of author text named as the scene's.
//   1. `loft show` on the dust burst gives each module as text that compiles as one module, and every line of the
//      scene's WGSL that computes something is cited by a site in it. Catches: a site note that breaks the WGSL (a `//`
//      inside a statement joined onto one line), a chunk banner splitting a line, an author line the sites miss.
//   2. A typo in a raw function exits with the scene code (4), its last line quoting the file and line, compiled here
//      and from a built directory. Catches: the old blanket "every shader refusal is the environment's", and line
//      arithmetic across chunks that names the wrong line.
//   3. A refusal of a library chunk still exits as the environment (6). Catches: mapping that blames the scene for
//      text the library wrote.
import { expect, test } from "bun:test";
import { readdirSync, readFileSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { headlessDevice } from "../src/host/headless.ts";
import { shownModules } from "../src/cli/show.ts";

const DUST = "tests/fixtures/show/dust_burst.ts";
const TYPO = "tests/fixtures/mistakes/36_raw_wgsl_typo.ts";
const OK = "tests/fixtures/mistakes/_ok.ts";

async function loft(...args: string[]): Promise<{ code: number; last: string }> {
  const p = Bun.spawn(["bun", "src/cli/main.ts", ...args], { stdout: "pipe", stderr: "pipe" });
  const [err] = await Promise.all([new Response(p.stderr).text(), new Response(p.stdout).text()]);
  return { code: await p.exited, last: err.trim().split("\n").at(-1) ?? "" };
}

test("loft show: every module compiles as one module, and every computing line of the scene's text is sited", async () => {
  const modules = await shownModules(DUST, { kind: "default" });
  const device = await headlessDevice();
  for (const m of modules) {
    device.pushErrorScope("validation");
    device.createShaderModule({ label: m.label, code: m.text });
    const error = await device.popErrorScope();
    expect(error?.message ?? "", m.label).toBe("");
  }
  // The lines of the fixture's text that compute: a let, an assignment or a return (struct and fn headers do not).
  const file = readFileSync(DUST, "utf8").split("\n");
  const open = file.findIndex((l) => l.includes("wgsl`")), close = file.findIndex((l, k) => k > open && l.includes("}`;"));
  // A `let` that only binds a call of the text's own function to plain names (`let q = flight(i, time);`) computes
  // nothing itself: the call is inlined, its work is sited at the callee's lines, and since lane L5 `time` in a speck
  // is the shader's parameter `t` (no statement). Under L4 alone that line was cited only through `time`'s input read.
  const fns = new Set(file.slice(open, close + 1).flatMap((l) => [...l.matchAll(/\bfn (\w+)\(/g)].map((x) => x[1]!)));
  const forwards = (l: string): boolean => { const m = /^\s*let \w+ = (\w+)\(([\w\s,]*)\);\s*$/.exec(l); return m !== null && fns.has(m[1]!); };
  const computing = file.flatMap((l, k) => k > open && k <= close && /\b(let|return)\b|^\s*\w+ = /.test(l) && !forwards(l) ? [k + 1] : []);
  const cited = new Set(modules.flatMap((m) => [...m.text.matchAll(/dust_burst\.ts:(\d+)/g)].map((x) => Number(x[1]))));
  const missed = computing.filter((n) => !cited.has(n));
  const sited = modules.reduce((n, m) => n + m.sited, 0);
  console.log(`show: ${modules.length} modules compile as shown; ${computing.length - missed.length} of ${computing.length} computing lines of the text cited; ${sited} sited lines`);
  expect(missed).toEqual([]);
}, 120_000);

test("a raw-function typo exits with the scene code at file:line, compiled and from files", async () => {
  const compiled = await loft("render", TYPO, "--views", "three", "--size", "64x40", "--out", "out/l4/typo.webp");
  console.log(`typo, compiled: exit ${compiled.code}: ${compiled.last.slice(0, 160)}`);
  expect(compiled.code).toBe(4);
  expect(compiled.last).toStartWith(`loft: scene: ${TYPO}:9: `);
  expect(compiled.last).toContain("unresolved value 'hsh'");
  expect((await loft("build", TYPO, "--out", "out/l4/typo_files")).code).toBe(0);
  const files = await loft("render", "--artefact", "out/l4/typo_files", "--views", "three", "--size", "64x40", "--out", "out/l4/typo.webp");
  console.log(`typo, from files: exit ${files.code}`);
  expect(files.code).toBe(4);
  expect(files.last).toStartWith(`loft: scene: ${TYPO}:9: `);
}, 120_000);

test("a refusal of a library chunk still exits as the environment", async () => {
  const dir = "out/l4/library_broken";
  expect((await loft("build", OK, "--out", dir)).code).toBe(0);
  const prelude = readdirSync(join(dir, "wgsl")).find((f) => readFileSync(join(dir, "wgsl", f), "utf8").includes("struct Frame {"))!;
  writeFileSync(join(dir, "wgsl", prelude), readFileSync(join(dir, "wgsl", prelude), "utf8").replace("fn curve(", "fn curve(r: u32, x: f32) -> vec4<f32> { return nois3(x); }\nfn curve_("));
  const r = await loft("render", "--artefact", dir, "--views", "three", "--size", "64x40", "--out", "out/l4/broken.webp");
  console.log(`library chunk refused: exit ${r.code}: ${r.last.slice(0, 120)}`);
  expect(r.code).toBe(6);
  expect(r.last).toStartWith("loft: environment");
}, 120_000);

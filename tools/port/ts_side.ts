// The TypeScript side of one port-diff entry (tools/port/diff.ts runs it as a child, one process per entry, so Expr ids
// start fresh for every entry exactly as `loft build` does).
//
//   bun tools/port/ts_side.ts <scene.ts> <out> [--shift N]
//
// Writes, under <out>:
//   outcome.json      how the TS build ended: { kind: "built" }, { kind: "refused", stage, verdict, code, head } (a typed
//                     refusal at stage compile or lower is an outcome the Rust side must equal; at stage author no
//                     lowering ran), or { kind: "crashed" } (an untyped throw: the TS compiler's own bug)
//   compile.json      the compiler's Artefact (src/artefact.ts) before any bake, as JSON (tools/port/sections.ts
//                     `plain`): the boundary between the compile lanes and the lower lanes; compared section by section
//   artefact/         the artefact files, as `loft build` writes them (src/cli/main.ts buildTs: compile, bake on this GPU,
//                     lower, write)
//   input/            what the Rust side receives (agent_notes/2026-11-19_port/PLAN.md, "The cut"): job.bin (the
//                     recorder's job, src/record/port.ts) and baked/<name>.bin (GPU bake outputs: data, not lowering)
//
// --shift N makes N distinct expression nodes before the scene loads, so every Expr id the scene makes is N higher. The
// artefact must not change (lane P0, canonical names): `diff.ts --mode shift` compares it with the unshifted build.
import { mkdirSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";
import { plain } from "./sections.ts";
import { LoadError } from "../../src/errors.ts";
import type { Artefact } from "../../src/artefact.ts";
import type { Scene } from "../../src/scene.ts";

const [scene, out, ...rest] = Bun.argv.slice(2);
if (scene === undefined || out === undefined) { console.error("usage: bun tools/port/ts_side.ts <scene.ts> <out> [--shift N]"); process.exit(2); }
const shift = rest[0] === "--shift" ? Number(rest[1]) : 0;
if (!Number.isSafeInteger(shift) || shift < 0) { console.error(`ts_side: --shift takes a whole number, got ${rest[1]}`); process.exit(2); }

rmSync(out, { recursive: true, force: true });
mkdirSync(join(out, "input", "baked"), { recursive: true });
const write = (name: string, x: unknown) => writeFileSync(join(out, name), JSON.stringify(x) + "\n");

/** Where a refusal came from: the author's module (`author`: no lowering ran, so the port has nothing to match), the
 * compiler, or bake, lower and write. */
let stage: "author" | "compile" | "lower" = "author";
try {
  const { lift } = await import("../../src/expr.ts");
  for (let i = 0; i < shift; i++) lift(1e9 + i + 0.5);
  const [{ rec }, { styled }, { chosenAt }, { compile }, { lower }, { writeBundle }] = await Promise.all([
    import("../../src/record/recorder.ts"), import("../../src/cli/load.ts"), import("../../src/cli/variant.ts"), import("../../src/compile/index.ts"),
    import("../../src/host/lower.ts"), import("../../src/host/files.ts")]);
  // As src/cli/load.ts `compiledScene` (a frame, the scene's own style and light, the default variant), in two stages:
  // the author's module (loading, constructing, styling: TS forever) and the compiler (what the port translates).
  const stop = rec.watch();
  let roots: ReturnType<typeof stop>, compiled: { scene: Scene; artefact: Artefact };
  try {
    const chosen = await chosenAt(scene, { kind: "default" });
    const subject: unknown = chosen.scene;
    // The corpus also includes modules for other tools, such as Film exports for tools/film.ts.
    if (typeof subject !== "object" || subject === null || !("style" in subject && "lighting" in subject && "things" in subject)) {
      throw new LoadError(`${scene} does not export a scene. Fix: export default scene({ ... })`);
    }
    const drawn = await styled(chosen.scene, { kind: "scene" }, { kind: "scene" }) as Scene;
    stage = "compile";
    compiled = { scene: drawn, artefact: compile(drawn) };
  } finally { roots = stop(); }
  stage = "lower";
  const { artefact } = compiled;
  write("compile.json", plain(artefact));

  // As buildTs: bake recurrences on this GPU, lower, run a compute build's raw dispatches and lower again with them.
  const baked = new Map<string, Uint8Array>();
  let gpu: GPUDevice | undefined;
  const device = async () => (gpu ??= await (await import("../../src/host/headless.ts")).headlessDevice());
  if (artefact.recurrences !== undefined) {
    await (await import("../../src/host/bake.ts")).bakeRecurrences(await device(), artefact);
    for (const r of artefact.recurrences) baked.set(`recurrence.${r.name}`, new Uint8Array(r.data.buffer, r.data.byteOffset, r.data.byteLength).slice());
  }
  let bundle = lower(artefact, "f32");
  if (bundle.manifest.build !== undefined && process.env.LOFT_NO_GPU !== "1") {
    const data = await (await import("../../src/host/host.ts")).bakeRawBuild(await device(), bundle);
    for (const [name, bytes] of data) baked.set(`build.${name}`, bytes);
    bundle = lower(artefact, "f32", data);
  }
  gpu?.destroy();
  writeBundle(join(out, "artefact"), bundle);
  for (const [name, bytes] of baked) writeFileSync(join(out, "input", "baked", `${name}.bin`), bytes);

  // Probes: the graph lanes' own sections, from the roots the recorder watched (the TS compiler's emitted graphs).
  mkdirSync(join(out, "probe"), { recursive: true });
  writeFileSync(join(out, "probe", "graph.txt"), graphProbe(roots));
  writeFileSync(join(out, "probe", "eval.txt"), evalProbe(roots));

  // The Rust side's input, written after the artefact so nothing it makes can move an id the artefact used.
  const { portInput } = await import("../../src/record/port.ts");
  const input = portInput(compiled, roots);
  switch (input.kind) {
    case "job": writeFileSync(join(out, "input", "job.bin"), input.bytes); writeFileSync(join(out, "probe", "wire.txt"), input.print); break;
    case "refused": write("input/refused.json", { why: input.why }); break;
  }
  write("outcome.json", { kind: "built" });
  process.exit(0);
} catch (e) {
  const { verdictOf, ending } = await import("../../src/cli/verdict.ts");
  const v = await verdictOf(e);
  const end = ending(v);
  // An untyped throw inside Loft is the TS compiler's bug, not an outcome the port must copy: the entry fails by name.
  write("outcome.json", v.kind === "internal" ? { kind: "crashed", message: v.message, stack: v.stack } : { kind: "refused", stage, verdict: v.kind, code: end.code, head: end.head });
  process.exit(0);
}

/**
 * `probe.graph` (lane G1): every watched root emitted in one `emitWgsl` call, in watch order, with a probe binding
 * that spells each leaf by name (`input_time`, `param_toon.outline`, `row_<label>(x)`, `fetch_<table>_<lane>(i)`),
 * then the roots' names and the library functions called. Rust emits the decoded job's roots with the same binding.
 */
function graphProbe(roots: readonly import("../../src/expr.ts").Expr[]): string {
  const { emitWgsl } = require("../../src/expr.ts") as typeof import("../../src/expr.ts");
  const e = emitWgsl(roots, {
    variable: (name) => name,
    input: (s) => `input_${s.name}`,
    param: (s) => `param_${s.name}`,
    row: (r, x) => `row_${r.label}(${x})`,
    table: (t, i, lane) => `fetch_${t.name}_${lane}(${i})`,
  });
  return [...e.lines, `out ${e.out.join(" ")}`, `libs ${e.libs.map((l) => l.name).join(" ")}`, ""].join("\n");
}

/**
 * `probe.eval` (lane G2): `compileJs` at rest (time 0, every input and param at its default) of each watched root that
 * reads no free variable, table, curve row, loop or GPU-only function, as `k: f64 bits in hex` per component.
 */
function evalProbe(roots: readonly import("../../src/expr.ts").Expr[]): string {
  const { children, compileJs, topo, leaves } = require("../../src/expr.ts") as typeof import("../../src/expr.ts");
  const open = new Set<number>();
  for (const x of topo(roots)) {
    const n = x.node;
    if (n.op === "var" || n.op === "fetch" || n.op === "row" || n.op === "iterate" || (n.op === "lib" && n.fn.js === "gpu-only") || children(x).some((c) => open.has(c.id))) open.add(x.id);
  }
  const closed = roots.map((r, k) => [r, k] as const).filter(([r]) => !open.has(r.id));
  if (closed.length === 0) return "closed 0\n";
  const { inputs, params } = leaves(closed.map(([r]) => r));
  const run = compileJs(closed.map(([r]) => r));
  const values = run({ vars: {}, inputs: Object.fromEntries(inputs.map((s) => [s.name, s.def.length === 1 ? s.def[0]! : [...s.def]])), params: Object.fromEntries(params.map((p) => [p.name, p.def])) });
  const bits = (x: number) => { const d = new DataView(new ArrayBuffer(8)); d.setFloat64(0, x, true); return d.getBigUint64(0, true).toString(16).padStart(16, "0"); };
  const spell = (v: number | number[] | boolean) => typeof v === "boolean" ? String(v) : (typeof v === "number" ? [v] : v).map(bits).join(",");
  return [`closed ${closed.length} of ${roots.length}`, ...closed.map(([, k], j) => `${k}: ${spell(values[j]!)}`), ""].join("\n");
}

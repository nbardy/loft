// A stopgap for the Rust loader's proof (docs/gpu_native.md, "Rust loader"), not part of any loader: copies an artefact
// folder, rewriting the one WGSL construct Naga 30 rejects and Tint accepts. Naga's constant evaluator does not
// implement `mix` or `smoothstep`, so a call whose arguments are all abstract float literals stays abstract inside a
// function, and validation fails ("Abstract types may only appear in constant expressions"). The rewrite gives each
// literal of such a call an `f` suffix (concrete f32): the call is then an ordinary f32 call. The fix belongs in the
// compiler (fold constant calls, or type the literals); until then this tool makes a folder both hosts can load.
//   bun tools/naga_compat.ts <in dir> <out dir>      prints the calls rewritten
import { cpSync, readdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { join } from "node:path";

const [from, to] = process.argv.slice(2) as [string, string];
if (!from || !to) throw new Error("usage: bun tools/naga_compat.ts <in dir> <out dir>");
rmSync(to, { recursive: true, force: true });
cpSync(from, to, { recursive: true });
const LIT = String.raw`-?(?:\d+\.\d*|\.\d+|\d+)(?:[eE][+-]?\d+)?`;
const CALL = new RegExp(String.raw`\b(mix|smoothstep)\(\s*(${LIT})\s*,\s*(${LIT})\s*,\s*(${LIT})\s*\)`, "g");
let calls = 0;
for (const f of readdirSync(join(to, "wgsl"))) {
  const path = join(to, "wgsl", f);
  const text = readFileSync(path, "utf8");
  // The chunk keeps its name: a loader opens the file the manifest names and never recomputes the hash.
  const out = text.replace(CALL, (_m, fn: string, a: string, b: string, c: string) => { calls++; return `${fn}(${a}f, ${b}f, ${c}f)`; });
  if (out !== text) writeFileSync(path, out);
}
console.log(`${to}: ${calls} all-literal mix/smoothstep calls typed f32`);

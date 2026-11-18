// Run from the Loft root. Patches a copy under out/, so the benchmark leaves the compiler and CLI alone.
import { cpSync, mkdirSync, readFileSync, symlinkSync, writeFileSync } from "node:fs";
import { join, relative, resolve } from "node:path";

const [inputArg, outputArg, extra] = process.argv.slice(2);
const work = resolve("out/compiler-speed");
if (!inputArg || !outputArg || (extra !== undefined && extra !== "--intern-rss")) {
  throw new Error("Usage: bun instrument.ts <source directory> <new name under out/compiler-speed> [--intern-rss]");
}
const input = resolve(inputArg), output = resolve(work, outputArg);
if (!output.startsWith(`${work}/`)) throw new Error("Use a new directory under out/compiler-speed");
mkdirSync(output);
for (const folder of ["src", "examples"]) cpSync(join(input, folder), join(output, folder), { recursive: true });
symlinkSync(resolve("node_modules"), join(output, "node_modules"), "dir");

function edit(file: string, change: (source: string) => string) {
  const path = join(output, "src", file);
  const importPath = relative(join(path, ".."), join(output, "phase.ts"));
  const phaseImport = importPath.startsWith(".") ? importPath : `./${importPath}`;
  writeFileSync(path, `import { phase, asyncPhase, phaseInterner, phaseNodes, phaseStart, phaseSwitch } from ${JSON.stringify(phaseImport)};\n${change(readFileSync(path, "utf8"))}`);
}

function replace(source: string, old: string, replacement: string) {
  const at = source.indexOf(old);
  if (at === -1 || source.indexOf(old, at + old.length) !== -1) throw new Error(`Expected one source anchor: ${old}`);
  return source.replace(old, () => replacement);
}

function wrap(source: string, phase: string, signature: string, end: string) {
  const start = source.indexOf(signature);
  if (start === -1) throw new Error(`Cannot find ${signature}`);
  const body = start + signature.length;
  const stop = source.indexOf(end, body);
  if (stop === -1) throw new Error(`Cannot find end of ${signature}`);
  return `${source.slice(0, body)}\nreturn phase(${JSON.stringify(phase)}, () => {${source.slice(body, stop)}\n});${source.slice(stop)}`;
}

edit("cli/main.ts", (s) => {
  s = replace(s, '  let t = performance.now();\n  const artefact = await loadArtefact(cmd.source);', '  phaseStart();\n  let t = performance.now();\n  const artefact = await asyncPhase("import", () => loadArtefact(cmd.source));');
  s = replace(s, 'if (artefact.recurrences !== undefined) {\n    // The files carry', 'if (artefact.recurrences !== undefined) {\n    phaseSwitch("GPU bake");\n    // The files carry');
  s = replace(s, '  const [{ lower }, { writeBundle }] = await Promise.all([import("../host/lower.ts"), import("../host/files.ts")]);', '  const [{ lower }, { writeBundle }] = await asyncPhase("import", () => Promise.all([import("../host/lower.ts"), import("../host/files.ts")]));');
  s = replace(s, 'const bundle = lower(artefact);', 'const bundle = phase("lower", () => lower(artefact));');
  s = replace(s, 'const sizes = writeBundle(cmd.out, bundle);', 'const sizes = phase("write", () => writeBundle(cmd.out, bundle));');
  return replace(s, 'async function main(argv:', 'phaseSwitch("CLI setup");\n\nasync function main(argv:');
});
edit("cli/load.ts", (s) => replace(s,
  'const artefact = need<(s: unknown) => Artefact>(index, "compile", "src/index.ts")(sampledFor(await styled(scene, source.style, source.light), still));',
  'const styledScene = sampledFor(await styled(scene, source.style, source.light), still);\n      const artefact = phase("lower", () => need<(s: unknown) => Artefact>(index, "compile", "src/index.ts")(styledScene));'));
edit("cli/variant.ts", (s) => replace(s, 'return choose(await subjectAt(path), path, v);', 'const subject = await subjectAt(path);\n  return phase("graph", () => choose(subject, path, v));'));
// Dependency loading/transpilation and top-level scene execution share the import() boundary. Decouple the two on
// these five scenes by marking the first evaluated statement after their static imports. Nested dependency evaluation
// (including the beetle for thin_film and top-level math in libraries) stays charged to import.
for (const [file, marker] of [
  ["second/owl.ts", "const "], ["bosses/reefback_titan.ts", "const "],
  ["materials/thin_film.ts", "const "], ["storm.ts", "const "], ["second/heron_leap.ts", "const "],
] as const) {
  const path = join(output, "examples", file);
  const source = readFileSync(path, "utf8");
  const at = source.indexOf(`\n${marker}`, source.lastIndexOf("\nimport "));
  if (at === -1) throw new Error(`Cannot find start of ${file}`);
  writeFileSync(path, `import { phaseSwitch } from "../../phase.ts";\n${source.slice(0, at)}\nphaseSwitch("graph");${source.slice(at)}`.replace(file === "storm.ts" ? '"../../phase.ts"' : "\0", '"../phase.ts"') + '\nphaseSwitch("import");\n');
}
edit("compile/index.ts", (s) => {
  s = replace(s, 'const e = t.draws(ctx);', 'const e = phase("graph", () => t.draws(ctx));');
  s = replace(s, 'const style = styleWgsl(slots, scene.style, scene.ground, scene.sun, lighting);', 'const style = phase("graph", () => styleWgsl(slots, scene.style, scene.ground, scene.sun, lighting));');
  s = replace(s, 'const layout = tables.layout();\n  if', 'const layout = phase("tables", () => tables.layout());\n  if');
  s = replace(s, 'const restFrame = poseFrame(layout, envOf, restState, undefined, "start").frame;', 'const restFrame = phase("tables", () => poseFrame(layout, envOf, restState, undefined, "start")).frame;');
  s = replace(s, 'map(({ phase, detail, depth, ...d })', 'map(({ phase: drawPhase, detail, depth, ...d })');
  s = replace(s, 'pass: PASS_OF[phase]', 'pass: PASS_OF[drawPhase]');
  return replace(s, 'const resolved = detailOf(detail, d);', 'const resolved = phase("tables", () => detailOf(detail, d));');
});
edit("compile/tables.ts", (s) => {
  s = replace(s, 'd.canon = classify(t, prep, this.c, (p) => this.lowerItem(p), (g) => { this.programs.push(g); });', 'd.canon = phase("tables", () => classify(t, prep, this.c, (p) => this.lowerItem(p), (g) => { this.programs.push(g); }));');
  s = wrap(s, "lower", '  lower(p: Program): readonly Expr[] {', '\n  }');
  s = wrap(s, "lower", '  private lowerItem(p: Program): readonly Expr[] {', '\n  }');
  return wrap(s, "lower", '  private lowerOnce(): void {', '\n  }');
});
edit("expr.ts", (s) => {
  s = wrap(s, "WGSL", 'libs: LibFn[]; names: ReadonlyMap<number, string> } {', '\n}');
  if (extra === undefined) return s;
  s = replace(s, 'let nextId = 0;', 'let nextId = 0;\nphaseNodes(() => nextId);');
  for (const entry of [
    'function trieOf(tries: Map<string, IdTrie>, name: string): IdTrie {',
    'function leafOf(root: IdTrie, args: readonly Expr[]): IdTrie {',
    'function internGet(a: Expr, sel: string, node: () => Node): Expr {',
    'static make<T extends Ty>(node: Node, key: string): Expr<T> {',
    'function constant(ty: Ty, v: readonly number[]): Expr {',
  ]) s = replace(s, entry, `${entry}\n  phaseInterner();`);
  return s;
});
edit("geom/solve.ts", (s) => wrap(s, "solve", 'export function solve<K extends string>(spec: SolveSpec<K>): Solved<K> {', '\n}'));

writeFileSync(join(output, "phase.ts"), `
const data: Record<string, { wall: number; cpu: number; high: number; growth: number }> = {};
let active = "Bun start", wall = 0, cpu = 0, high = 0;
let internCalls = 0, internHigh = 0, nodes = () => 0;
function snapshot() {
  const r = process.resourceUsage();
  return { wall: process.uptime() * 1000, cpu: (r.userCPUTime + r.systemCPUTime) / 1000, high: r.maxRSS * (process.platform === "linux" ? 1024 : 1) };
}
export function phaseSwitch(name: string) {
  if (active === name) return;
  const p = snapshot();
  const row = data[active] ??= { wall: 0, cpu: 0, high: 0, growth: 0 };
  row.wall += p.wall - wall;
  row.cpu += p.cpu - cpu;
  row.high = Math.max(row.high, p.high);
  row.growth += p.high - high;
  ({ wall, cpu, high } = p);
  active = name;
}
phaseSwitch("import");
export function phaseInterner() {
  if ((++internCalls & 4095) === 0) internHigh = Math.max(internHigh, snapshot().high);
}
export function phaseNodes(read: () => number) { nodes = read; }
export function phaseStart() { phaseSwitch("CLI setup"); }
export function phase<T>(name: string, fn: () => T): T {
  const before = active;
  phaseSwitch(name);
  try { return fn(); } finally { phaseSwitch(before); }
}
export async function asyncPhase<T>(name: string, fn: () => Promise<T>): Promise<T> {
  const before = active;
  phaseSwitch(name);
  try { return await fn(); } finally { phaseSwitch(before); }
}
function phaseReport() {
  phaseSwitch("exit");
  if (internCalls > 0) Object.assign(data, { intern: { calls: internCalls, high: internHigh, nodes: nodes() } });
  console.error("COMPILE_PHASES " + JSON.stringify(data));
}
process.on("exit", phaseReport);
`);

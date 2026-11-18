// Baseline and candidate come from different copies of src/ and examples/ under out/.
import { appendFileSync, mkdirSync, readFileSync, readdirSync } from "node:fs";
import { createHash } from "node:crypto";
import { basename, join, resolve } from "node:path";
import { loadavg } from "node:os";

const scenes = [
  ["owl", "examples/second/owl.ts", "game"],
  ["titan", "examples/bosses/reefback_titan.ts", "game"],
  ["film", "examples/materials/thin_film.ts", "whole"],
  ["storm", "examples/storm.ts", "three"],
  ["leap", "examples/second/heron_leap.ts", "side"],
] as const;

function digest(dir: string) {
  const hash = createHash("sha256");
  let bytes = 0;
  const folders = ["bin", "wgsl"].flatMap((name) => readdirSync(join(dir, name)).sort().map((file) => `${name}/${file}`));
  for (const file of ["manifest.json", ...folders]) {
    const content = readFileSync(join(dir, file));
    hash.update(JSON.stringify([file, content.length])).update(content);
    bytes += content.length;
  }
  return { digest: hash.digest("hex"), bytes, files: folders.length + 1 };
}

async function child(cmd: string[], cwd: string, dir: string) {
  const load = loadavg()[0];
  const profileDir = process.env.LOFT_BENCH_PROFILES;
  const profile = profileDir === undefined ? undefined : `${basename(dir)}.cpuprofile`;
  if (profile !== undefined) cmd = [cmd[0]!, "--cpu-prof", "--cpu-prof-interval=500", "--cpu-prof-dir", profileDir!, "--cpu-prof-name", profile, ...cmd.slice(1)];
  const t = performance.now();
  const p = Bun.spawn(cmd, { cwd, stdin: "ignore", stdout: "ignore", stderr: "pipe" });
  const [exit, stderr] = await Promise.all([p.exited, new Response(p.stderr).text()]);
  const wall = performance.now() - t;
  const usage = p.resourceUsage()!;
  const rss = usage.maxRSS;
  const probes = stderr.split("\n").filter((line) => line.startsWith("COMPILE_PHASES "));
  const phases = probes.length === 0 ? undefined : JSON.parse(probes[0]!.slice("COMPILE_PHASES ".length));
  const stats = { wall, cpu: Number(usage.cpuTime.total) / 1000, rss, load, phases, exit, profile };
  const lines = stderr.split("\n").filter((line) => !line.startsWith("COMPILE_PHASES "));
  return { ...stats, stderr: lines.join("\n"), ...(exit === 0 ? digest(dir) : { verdict: lines.filter(Boolean).at(-1) }) };
}

const [mode, label, roundsText, ...roots] = process.argv.slice(2);
if (mode === "timed") {
  const [log, ...cmd] = [label!, roundsText!, ...roots];
  const { stderr, ...stats } = await child(cmd, process.cwd(), cmd.at(-1)!);
  if (stderr !== "") process.stderr.write(stderr);
  appendFileSync(log!, `${JSON.stringify(stats)}\n`);
  process.exit(stats.exit);
}

if ((mode !== "build" && mode !== "serve") || !label || !roundsText || roots.length === 0) {
  throw new Error("Usage: bun bench.ts <build|serve> <label> <rounds> <source directory> [<source directory> ...]");
}
const rounds = Number(roundsText);
const output = resolve("out/compiler-speed", label);
mkdirSync(output, { recursive: true });
const rows = join(output, "rows.jsonl");
const row = (data: object) => {
  const json = JSON.stringify(data);
  appendFileSync(rows, `${json}\n`);
  console.log(json);
};

async function server(root: string, slot: number) {
  const log = join(output, `child-${slot}.jsonl`);
  const tmp = join(output, `tmp-${slot}`);
  mkdirSync(tmp, { recursive: true });
  const p = Bun.spawn([process.execPath, "src/cli/main.ts", "serve", "--compiles", "1", "--compile-seconds", "120", "--confine", `${process.execPath} ${import.meta.path} timed ${log}`], {
    cwd: root, env: { ...process.env, TMPDIR: tmp }, stdin: "pipe", stdout: "pipe", stderr: "inherit",
  });
  const reader = p.stdout.pipeThrough(new TextDecoderStream()).getReader();
  let buffer = "";
  async function next() {
    for (;;) {
      const nl = buffer.indexOf("\n");
      if (nl >= 0) {
        const value = JSON.parse(buffer.slice(0, nl));
        buffer = buffer.slice(nl + 1);
        return value;
      }
      const item = await reader.read();
      if (item.done) throw new Error(`serve exited ${await p.exited}`);
      buffer += item.value;
    }
  }
  const ready = await next();
  if (!ready.ready) throw new Error(JSON.stringify(ready));
  return {
    async send(id: string, scene: string, camera: string, out: string) {
      p.stdin.write(`${JSON.stringify({ id, scene, camera, times: [0], size: "64x64", out })}\n`);
      p.stdin.flush();
      const a = await next();
      const stats = JSON.parse(readFileSync(log, "utf8").trimEnd().split("\n").at(-1)!);
      if (a.ok !== (stats.exit === 0) || (!a.ok && (a.exit !== stats.exit || a.line !== stats.verdict))) throw new Error(JSON.stringify(a));
      return { ...stats, serverCompile: a.ms?.compile };
    },
    async close() {
      p.stdin.end();
      const exit = await p.exited;
      if (exit !== 0) throw new Error(`serve ended with exit ${exit}`);
    },
  };
}

const sources = roots.map((root) => resolve(root));
const servers = mode === "serve" ? await Promise.all(sources.map(server)) : [];
try {
  for (let round = 0; round < rounds; round++) {
    for (const [scene, path, camera] of scenes) {
      const order = sources.map((_, i) => (round + i) % sources.length);
      for (const i of order) {
        const source = sources[i]!;
        const id = `${round}-${scene}-${i}`;
        const dir = join(output, id);
        const { stderr, ...stats } = mode === "serve"
          ? await servers[i]!.send(id, path, camera, dir)
          : await child([process.execPath, "src/cli/main.ts", "build", path, "--out", dir], source, dir);
        if (stats.exit !== (scene === "storm" ? 4 : 0) || (scene === "storm" && !stats.verdict?.includes('"storm_blobs"'))) {
          throw new Error(`${scene} exited ${stats.exit}\n${stderr ?? stats.verdict}`);
        }
        row({ mode, round, source: roots[i], scene, ...stats });
      }
    }
  }
} finally {
  await Promise.all(servers.map((s) => s.close()));
}

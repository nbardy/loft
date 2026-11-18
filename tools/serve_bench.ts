// Cold `loft frames` against warm `loft serve`, per scene, interleaved, with pixel identity. Writes rows (JSON lines) to
// out/serve/bench/rows.jsonl and prints them.
//   bun tools/serve_bench.ts [--rounds 2] scene.ts:camera ...
// Each round starts a fresh server. In a round, each scene is drawn cold (its own process: compile, device, load, four
// frames) and warm (a request to the round's server: a compile child, then load and four frames there), the order
// alternating by round; then every scene is requested once more ("repeat": its shaders are cached). Measured per run:
// wall seconds; Bun CPU seconds (cold: the process's rusage; warm: the server's own CPU from `ps` around the request,
// plus the compile child's rusage, recorded by this file acting as the server's --confine wrapper); MTLCompilerService
// CPU seconds, sampled every 100 ms (Metal compiles shaders there, outside Bun; approximate: other GPU clients would
// count, and the last <100 ms of a dying service is missed); the 1-minute load average.
import { appendFileSync, mkdirSync, readFileSync } from "node:fs";
import { loadavg } from "node:os";
import { join, resolve } from "node:path";

const ROOT = resolve(import.meta.dir, "..");
const MAIN = join(ROOT, "src/cli/main.ts");
const OUT = join(ROOT, "out/serve/bench");
const TIMES = [0, 0.37, 0.91, 1.53];
const SIZE = "512x512";

// --- as the server's --confine wrapper: run the compile, log its CPU, end as it ended -------------------------------
if (process.argv[2] === "--timed") {
  const [, , , log, ...cmd] = process.argv;
  const child = Bun.spawn(cmd, { stdin: "ignore", stdout: "inherit", stderr: "inherit" });
  const code = await child.exited;
  const cpu = child.resourceUsage()!.cpuTime;
  appendFileSync(log!, JSON.stringify({ scene: cmd[cmd.indexOf("build") + 1], cpu: Number(cpu.total) / 1e6 }) + "\n");
  process.exit(code);
}

const args = process.argv.slice(2);
const rounds = args[0] === "--rounds" ? Number(args[1]) : 2;
const scenes = (args[0] === "--rounds" ? args.slice(2) : args).map((s) => {
  const [path, camera] = s.split(":");
  return { name: path!.replace(/^.*\//, "").replace(/\.ts$/, ""), path: resolve(path!), camera: camera! };
});
mkdirSync(OUT, { recursive: true });
const ROWS = join(OUT, "rows.jsonl"), COMPILES = join(OUT, "compiles.jsonl");

// --- MTLCompilerService CPU, sampled -------------------------------------------------------------------------------
const seconds = (t: string) => t.split(":").reduce((s, p) => s * 60 + Number(p), 0);
/** Per pid, the CPU seconds at each sample: [ms since start, seconds]. */
const mtl = new Map<number, [number, number][]>();
let sampling = true;
(async () => {
  while (sampling) {
    const ps = Bun.spawnSync(["ps", "-A", "-o", "pid=,time=,comm="]).stdout.toString();
    const now = performance.now();
    for (const line of ps.split("\n")) {
      const m = /^\s*(\d+)\s+(\S+)\s+(.*MTLCompilerService.*)$/.exec(line);
      if (!m) continue;
      const pid = Number(m[1]);
      if (!mtl.has(pid)) mtl.set(pid, []);
      mtl.get(pid)!.push([now, seconds(m[2]!)]);
    }
    await Bun.sleep(100);
  }
})();
/** MTLCompilerService CPU seconds spent between two instants (by the samples nearest inside them). */
function mtlBetween(t0: number, t1: number): number {
  let total = 0;
  for (const samples of mtl.values()) {
    const before = samples.filter(([t]) => t <= t0).at(-1)?.[1] ?? 0;
    const last = samples.filter(([t]) => t <= t1 + 150).at(-1)?.[1];
    if (last !== undefined && last > before) total += last - before;
  }
  return total;
}
const settle = () => Bun.sleep(250);

// --- cold ----------------------------------------------------------------------------------------------------------
async function cold(s: (typeof scenes)[number]) {
  const load1 = loadavg()[0]!;
  const t0 = performance.now();
  const p = Bun.spawn([process.execPath, MAIN, "frames", s.path, "--view", s.camera, "--times", TIMES.join(","), "--size", SIZE, "--out", join(OUT, `cold-${s.name}.webp`)], { stdout: "pipe", stderr: "pipe" });
  const [text, err, code] = await Promise.all([new Response(p.stdout).text(), new Response(p.stderr).text(), p.exited]);
  const t1 = performance.now();
  if (code !== 0) throw new Error(`cold ${s.name}: exit ${code}\n${err}`);
  await settle();
  const hashes = [...text.matchAll(/rgba ([0-9a-f]{16})/g)].map((m) => m[1]!);
  return { wall: (t1 - t0) / 1000, cpu: Number(p.resourceUsage()!.cpuTime.total) / 1e6, mtl: mtlBetween(t0, t1), load1, hashes };
}

// --- warm ----------------------------------------------------------------------------------------------------------
type Server = { send(req: object): Promise<Record<string, unknown>>; pid: number; close(): Promise<number>; readyWall: number };
async function server(): Promise<Server> {
  const t0 = performance.now();
  const p = Bun.spawn([process.execPath, MAIN, "serve", "--compiles", "1", "--confine", `${process.execPath} ${import.meta.path} --timed ${COMPILES}`], { stdin: "pipe", stdout: "pipe", stderr: "inherit" });
  const reader = p.stdout.pipeThrough(new TextDecoderStream()).getReader();
  let buffered = "";
  const nextLine = async (): Promise<string> => {
    for (;;) {
      const i = buffered.indexOf("\n");
      if (i >= 0) { const line = buffered.slice(0, i); buffered = buffered.slice(i + 1); return line; }
      const { value, done } = await reader.read();
      if (done) throw new Error("server ended");
      buffered += value;
    }
  };
  await nextLine();
  return {
    pid: p.pid, readyWall: (performance.now() - t0) / 1000,
    send: async (req) => { p.stdin.write(JSON.stringify(req) + "\n"); p.stdin.flush(); return JSON.parse(await nextLine()); },
    close: async () => { p.stdin.end(); return p.exited; },
  };
}
const serverCpu = (pid: number) => seconds(Bun.spawnSync(["ps", "-o", "time=", "-p", String(pid)]).stdout.toString().trim());
const compileCpus = () => { try { return readFileSync(COMPILES, "utf8").trim().split("\n").filter((l) => l).length; } catch { return 0; } };

async function warm(srv: Server, s: (typeof scenes)[number], tag: string) {
  const load1 = loadavg()[0]!;
  const before = compileCpus();
  const c0 = serverCpu(srv.pid), t0 = performance.now();
  const a = await srv.send({ id: `${s.name}/${tag}`, scene: s.path, camera: s.camera, times: TIMES, size: SIZE, out: join(OUT, `warm-${s.name}-${tag}`) });
  const t1 = performance.now();
  await settle();
  if (a.ok !== true) throw new Error(`warm ${s.name}: ${JSON.stringify(a)}`);
  const logged = readFileSync(COMPILES, "utf8").trim().split("\n").slice(before).map((l) => JSON.parse(l) as { cpu: number });
  const compileCpu = logged.reduce((n, l) => n + l.cpu, 0);
  const server = serverCpu(srv.pid) - c0;
  const ms = a.ms as { compile: number; load: number; frames: number };
  return {
    wall: (t1 - t0) / 1000, cpu: compileCpu + server, compileCpu, serverCpu: server, mtl: mtlBetween(t0, t1), load1,
    ms: { compile: Math.round(ms.compile), load: Math.round(ms.load), frames: Math.round(ms.frames) }, cache: a.cache,
    hashes: (a.frames as { sha256: string }[]).map((f) => f.sha256.slice(0, 16)),
  };
}

const r3 = (x: number) => Math.round(x * 1000) / 1000;
const rounded = (o: Record<string, unknown>) => Object.fromEntries(Object.entries(o).map(([k, v]) => [k, typeof v === "number" ? r3(v) : v]));
const row = (o: Record<string, unknown>) => { const line = JSON.stringify(o); console.log(line); appendFileSync(ROWS, line + "\n"); };

for (let r = 1; r <= rounds; r++) {
  const srv = await server();
  row({ round: r, kind: "server start", readyWall: r3(srv.readyWall), load1: loadavg()[0] });
  for (const s of scenes) {
    const order = r % 2 === 1 ? ["cold", "warm"] : ["warm", "cold"];
    const got: Record<string, Awaited<ReturnType<typeof cold>> | Awaited<ReturnType<typeof warm>>> = {};
    for (const k of order) got[k] = k === "cold" ? await cold(s) : await warm(srv, s, `r${r}`);
    const c = got.cold!, w = got.warm!;
    row({ round: r, scene: s.name, order: order.join(","), cold: rounded(c), warm: rounded(w), identical: c.hashes.join() === w.hashes.join() });
  }
  for (const s of scenes) {
    const w = await warm(srv, s, `r${r}-repeat`);
    row({ round: r, scene: s.name, kind: "repeat", warm: rounded(w) });
  }
  row({ round: r, kind: "server end", exit: await srv.close() });
}
sampling = false;

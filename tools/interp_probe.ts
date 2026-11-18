// bun tools/interp_probe.ts > out/interp/measurement.json
import { headlessDevice } from "../src/host/headless.ts";
import { lowerPaint, interpreter } from "./interp/bytecode.ts";
import { sceneSetup } from "./interp/scene_setup.ts";
import titan from "../examples/bosses/reefback_titan.ts";
import wyrm from "../examples/bosses/maelstrom_wyrm.ts";
import { common, paints } from "./interp/paints.ts";
import { group, instrument, layout, pipeline, size, upload, type Draw } from "./interp/gpu.ts";

const uptime = () => Bun.spawnSync(["uptime"]).stdout.toString().trim();

// A compiler-visible, unused-at-this-time input branch makes a new fragment binary key.
// Comments, labels and renaming a WGSL function can all disappear in the Metal translation.
function newKey(source: string, token: number) {
  const head = "@fragment fn fs(@builtin(position) pixel: vec4f) -> @location(0) vec4f {";
  if (!source.includes(head)) throw new Error("missing paint entry");
  return source.replace(head, head + `\n  if (frame.time.y == ${token}.0) { return vec4f(${token}.0); }`);
}

const calibration = common + `
@fragment fn fs(@builtin(position) p: vec4f) -> @location(0) vec4f {
  let m = vec4f(.99991, .99973, .99969, .99971);
  let b = p.xyxy * .000001 + vec4f(.0003, .0005, .0007, .0009);
  var a0 = b; var a1 = b.yzwx; var a2 = b.zwxy; var a3 = b.wxyz;
  for (var k = 0u; k < u32(frame.time.z); k++) {
    a0 = fma(a0, m, b); a1 = fma(a1, m.yzwx, b);
    a2 = fma(a2, m.zwxy, b); a3 = fma(a3, m.wxyz, b);
  }
  return a0 + a1 + a2 + a3;
}`;

function compare(ref: Float32Array, got: Float32Array) {
  let max = 0, at = 0, squared = 0, changed = 0, overByte = 0;
  let varies = false;
  const range = [Infinity, -Infinity];
  for (let p = 0; p < ref.length; p += 4) {
    let pixelMax = 0;
    for (let c = 0; c < 4; c++) {
      const i = p + c, diff = Math.abs(ref[i] - got[i]);
      if (!Number.isFinite(diff)) throw new Error(`non-finite pixel at channel ${i}`);
      if (ref[i] !== ref[c]) varies = true;
      if (diff > max) { max = diff; at = i; }
      if (c < 3) { range[0] = Math.min(range[0], ref[i]); range[1] = Math.max(range[1], ref[i]); }
      squared += diff * diff;
      pixelMax = Math.max(pixelMax, diff);
    }
    if (pixelMax > 0) changed++;
    if (pixelMax > 1 / 255) overByte++;
  }
  if (!varies) throw new Error("constant reference render");
  return { max, xy: [Math.floor(at / 4) % size, Math.floor(at / (size * 4))], channel: at % 4,
    compiled: ref[at], interpreted: got[at], rms: Math.sqrt(squared / ref.length), changed, overByte, range };
}

async function run() {
  const initialLoad = uptime();
  const graphs = paints();
  const vm = "diagnostic(off, derivative_uniformity);\n" + common + interpreter;
  const setup: unknown[] = [];
  // The child sees no live Dawn pipeline or module objects from an earlier measurement.
  for (let round = 0; round < 3; round++) {
    const token = crypto.getRandomValues(new Uint32Array(1))[0] % 8000000 + 1;
    for (const reverse of [false, true]) {
      const child = Bun.spawnSync([process.execPath, import.meta.path, "compile", String(token), String(Number(reverse)), String(round)]);
      if (child.exitCode !== 0) throw new Error(child.stderr.toString() + child.stdout.toString());
      setup.push({ round, token, cache: reverse ? "repeat" : "first", ...JSON.parse(child.stdout.toString()) });
    }
  }
  const d = await headlessDevice(), l = layout(d), timer = instrument(d);
  d.pushErrorScope("validation");
  const fixed = await pipeline(d, l.interpreted, vm);
  const draws: { name: string; draw: Draw; batch: number; ms: number[] }[] = [];
  const reports = [];
  const frames = (frame: Float32Array<ArrayBuffer>, inputs: Float32Array<ArrayBuffer>) => group(d, l.frame, [
    upload(d, frame, GPUBufferUsage.UNIFORM), upload(d, inputs, GPUBufferUsage.STORAGE)]);
  for (const g of graphs) {
    const drawFrame = frames(g.frame, g.inputs as Float32Array<ArrayBuffer>);
    const direct = { pipeline: (await pipeline(d, l.compiled, g.shader)).result, groups: [drawFrame] };
    const uploadTimes = [];
    let drawVM: Draw | undefined;
    let buffers: GPUBuffer[] = [];
    for (let i = 0; i < 9; i++) {
      buffers.forEach(b => b.destroy());
      const start = performance.now();
      const bc = lowerPaint(g.root, g.slots);
      const lowered = performance.now();
      buffers = [upload(d, bc.code, GPUBufferUsage.STORAGE), upload(d, bc.constants, GPUBufferUsage.STORAGE)];
      const resources = group(d, l.program, buffers);
      d.queue.submit([]); // Flush WebGPU's deferred queue writes before stopping the upload clock.
      await d.queue.onSubmittedWorkDone();
      uploadTimes.push({ lower: lowered - start, upload: performance.now() - lowered });
      drawVM = { pipeline: fixed.result, groups: [drawFrame, resources] };
    }
    const bc = lowerPaint(g.root, g.slots);
    const diff = compare(await timer.read(direct), await timer.read(drawVM!));
    reports.push({ name: g.name, ops: bc.count, peak: bc.peak, bytes: bc.code.byteLength + bc.constants.byteLength,
      uploadTimes, diff });
    draws.push({ name: g.name + ".compiled", draw: direct, batch: 16, ms: [] },
      { name: g.name + ".interpreted", draw: drawVM!, batch: 1, ms: [] });
  }

  const cal = (await pipeline(d, l.compiled, calibration)).result;
  for (const count of [128, 256]) {
    const f = graphs[0].frame.slice();
    f[46] = count;
    draws.push({ name: "FMA" + count, draw: { pipeline: cal, groups: [frames(f, new Float32Array(4))] }, batch: 4, ms: [] });
  }
  for (const { draw, batch } of draws) for (let k = 0; k < 3; k++) await timer.time(draw, batch);
  for (let k = 0; k < 15; k++) {
    // Rotate the first draw and alternate direction so no graph always has the same neighbour.
    for (let j = 0; j < draws.length; j++) {
      const t = draws[(k + (k % 2 ? -j : j) + draws.length) % draws.length];
      t.ms.push(await timer.time(t.draw, t.batch));
    }
  }
  const error = await d.popErrorScope();
  if (error) throw new Error(error.message);
  console.log(JSON.stringify({ initialLoad, finalLoad: uptime(), clock: timer.clock, fixedCompile: fixed.ms, setup, reports,
    timing: draws.map(({ name, batch, ms }) => ({ name, batch, min: Math.min(...ms), ms })) }, null, 2));
  d.destroy();
}

if (process.argv[2] === "compile") {
  const token = Number(process.argv[3]), reverse = Number(process.argv[4]) ^ (Number(process.argv[5]) % 2);
  const d = await headlessDevice(), l = layout(d);
  // Pay driver initialization outside each scene clock, as in a running loft serve.
  await pipeline(d, l.compiled, calibration);
  const shaders = paints();
  if (reverse) shaders.reverse();
  const ms: Record<string, number> = {};
  for (const g of shaders) ms[g.name] = (await pipeline(d, l.compiled, newKey(g.shader, token))).ms;
  const whole: Record<string, unknown> = {};
  if (process.argv[5] === "0") {
    for (const g of shaders) whole[g.name] = await sceneSetup(d, g.name === "paint_titan" ? titan : wyrm, token);
  }
  console.log(JSON.stringify({ ms, whole, uptime: uptime() }));
  d.destroy();
} else {
  await run();
}

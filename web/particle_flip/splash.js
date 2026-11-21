const params = new URLSearchParams(location.search);
const status = document.getElementById('status');
const result = document.getElementById('result');
const canvas = document.querySelector('canvas');
const failures = [];

async function fetchAsset(name, kind) {
  const response = await fetch(name);
  if (!response.ok) throw new Error(`HTTP ${response.status} for ${name}`);
  return response[kind]();
}

async function load(device, name, format) {
  const plan = await fetchAsset(`${name}.json`, 'json');
  const modules = new Map(plan.modules.map(module => [module.id, device.createShaderModule({
    label: `${name}/${module.hash}`, code: module.wgsl,
  })]));
  const buffers = new Map(plan.buffers.map(({ id, size, initial }) => {
    const buffer = device.createBuffer({ size, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC |
      GPUBufferUsage.COPY_DST | (initial.length ? GPUBufferUsage.INDIRECT : 0) });
    if (initial.length) device.queue.writeBuffer(buffer, 0, Uint32Array.from(initial));
    return [id, buffer];
  }));

  plan.stages = await Promise.all(plan.passes.map(async pass => {
    const layouts = [], groups = [];
    const max = Math.max(0, ...pass.bindings.map(([group]) => group));
    const visibility = pass.kind === 'compute' ? GPUShaderStage.COMPUTE : GPUShaderStage.VERTEX | GPUShaderStage.FRAGMENT;
    for (let group = 0; group <= max; group++) {
      const rows = pass.bindings.filter(([g]) => g === group);
      const layout = device.createBindGroupLayout({
        entries: rows.map(([, binding, , type]) => ({ binding, visibility, buffer: { type } })),
      });
      layouts.push(layout);
      groups.push(device.createBindGroup({
        layout, entries: rows.map(([, binding, id]) => ({ binding, resource: { buffer: buffers.get(id) } })),
      }));
    }
    const layout = device.createPipelineLayout({ bindGroupLayouts: layouts });
    const module = modules.get(pass.module);
    let pipeline;
    switch (pass.kind) {
      case 'compute':
        pipeline = await device.createComputePipelineAsync({ layout, compute: { module, entryPoint: pass.entry } });
        break;
      case 'render':
        pipeline = await device.createRenderPipelineAsync({
          layout, vertex: { module, entryPoint: pass.vertex },
          fragment: { module, entryPoint: pass.fragment, targets: [{ format }] },
          primitive: { topology: 'triangle-list' },
        });
        break;
      default:
        throw new Error(`Unsupported pass ${pass.kind}`);
    }
    return {
      name: pass.name,
      record(encoder, timestampWrites, view) {
        const command = pass.kind === 'compute' ? encoder.beginComputePass({ timestampWrites }) :
          encoder.beginRenderPass({ timestampWrites, colorAttachments: [
            { view, loadOp: 'clear', storeOp: 'store', clearValue: [0, 0, 0, 1] },
          ] });
        command.setPipeline(pipeline);
        groups.forEach((group, i) => command.setBindGroup(i, group));
        if (pass.kind === 'compute') command.dispatchWorkgroups(...pass.groups);
        else command.drawIndirect(buffers.get(pass.args), 0);
        command.end();
      },
    };
  }));
  return Object.assign(plan, {
    buffer(name) { return buffers.get(plan.tables[name]); },
    upload(name, data) { device.queue.writeBuffer(this.buffer(name), 0, data); },
  });
}

async function read(device, buf, size) {
  const output = device.createBuffer({ size, usage: GPUBufferUsage.COPY_DST | GPUBufferUsage.MAP_READ });
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(buf, 0, output, 0, size);
  device.queue.submit([encoder.finish()]);
  await output.mapAsync(GPUMapMode.READ);
  const data = output.getMappedRange().slice(0);
  output.unmap();
  output.destroy();
  return data;
}

async function save(name, data) {
  const response = await fetch(`/capture/${name}`, { method: 'POST', body: data });
  if (!response.ok) throw new Error(`Could not save ${name}: HTTP ${response.status}`);
}

function depositStages(plan) {
  return plan.stages.filter(({ name }) => !/^(pack_|update_|g2p)/.test(name));
}

async function checkGrids(device, sorted) {
  const cases = [];
  for (const name of ['spread', 'clumped', 'boundaries', 'cancellation']) {
    const expected = new Int32Array(await fetchAsset(`fixed8/${name}.i32`, 'arrayBuffer'));
    for (const [order, count] of [['forward', sorted.particles], ['reversed', sorted.particles], ['capacity', sorted.particles + 1]]) {
      const input = order === 'capacity' ? null :
        await fetchAsset(`fixed8/${name}-${order}.particles`, 'arrayBuffer');
      if (input) sorted.upload('Packed', input);
      sorted.upload('Control', Uint32Array.of(count));
      const encoder = device.createCommandEncoder();
      for (const stage of depositStages(sorted)) stage.record(encoder);
      device.queue.submit([encoder.finish()]);
      const grid = await read(device, sorted.buffer('Grid'), expected.byteLength);
      const actual = new Int32Array(grid);
      const first = actual.findIndex((word, i) => word !== expected[i]);
      const code = new Uint32Array(await read(device, sorted.buffer('Status'), 4))[0];
      await save(`${name}-${order}.grid`, grid);
      let orderError = null;
      if (input) {
        const particles = new Uint32Array(input);
        const pairs = new Uint32Array(await read(device, sorted.buffer('KeysA'), sorted.particles * 8));
        const seen = new Uint8Array(sorted.particles);
        for (let i = 0; i < pairs.length; i += 2) {
          const key = pairs[i], id = pairs[i + 1], row = id * 4;
          const actualKey = (particles[row] >>> 4 & 31) + 32 * ((particles[row + 1] >>> 4 & 31) +
            32 * (particles[row + 2] >>> 4 & 31));
          const before = i === 0 || key > pairs[i - 2] || (key === pairs[i - 2] && id > pairs[i - 1]);
          if (id >= seen.length || seen[id] || key !== actualKey || !before) {
            orderError = { index: i / 2, key, id };
            break;
          }
          seen[id] = 1;
        }
      }
      const entry = { fixture: name, order, compared_words: actual.length, status: code, order_error: orderError,
        first_mismatch: first < 0 ? null : { word: first, expected: expected[first], actual: actual[first] } };
      cases.push(entry);
      if (first >= 0 || orderError || code !== Number(order === 'capacity')) failures.push(entry);
    }
  }
  return cases;
}

function clock(device, stages, timestamps) {
  if (!timestamps) return { stages, destroy() {}, record(encoder) { stages.forEach(s => s.record(encoder)); } };
  const count = stages.length * 2;
  const querySet = device.createQuerySet({ type: 'timestamp', count });
  const size = count * 8;
  const resolve = device.createBuffer({ size, usage: GPUBufferUsage.QUERY_RESOLVE | GPUBufferUsage.COPY_SRC });
  return {
    stages,
    destroy() { querySet.destroy(); resolve.destroy(); },
    record(encoder) {
      stages.forEach((s, i) => s.record(encoder, { querySet, beginningOfPassWriteIndex: 2 * i, endOfPassWriteIndex: 2 * i + 1 }));
    },
    async timestamps() {
      const encoder = device.createCommandEncoder();
      encoder.resolveQuerySet(querySet, 0, count, resolve, 0);
      device.queue.submit([encoder.finish()]);
      const ticks = Array.from(new BigUint64Array(await read(device, resolve, size)));
      if (ticks.at(-1) < ticks[0] || ticks.some((t, i) => i % 2 === 1 && t < ticks[i - 1])) {
        throw new Error(`Unusable WebGPU timestamps: ${ticks}`);
      }
      return {
        ticks: ticks.map(String), gpu_ms: Number(ticks.at(-1) - ticks[0]) / 1e6,
        pass_ms: stages.map((_, i) => Number(ticks[2 * i + 1] - ticks[2 * i]) / 1e6),
      };
    },
  };
}

async function sample(device, timer) {
  const start = performance.now();
  const encoder = device.createCommandEncoder();
  timer.record(encoder);
  const commands = encoder.finish();
  const encoded = performance.now();
  device.queue.submit([commands]);
  const submitted = performance.now();
  await device.queue.onSubmittedWorkDone();
  const completed = performance.now();
  return {
    cpu_encode_ms: encoded - start, cpu_submit_ms: submitted - encoded,
    queue_wait_ms: completed - submitted,
    ...(timer.timestamps ? await timer.timestamps() : {}),
  };
}

function percentile(values, f) {
  return values.slice().sort((a, b) => a - b)[Math.max(0, Math.ceil(f * values.length) - 1)];
}

function summary(plan, timer, rows) {
  const statistics = values => ({ p50: percentile(values, 0.5), p95: percentile(values, 0.95) });
  const report = {
    route: plan.route, scratch_bytes: plan.scratch_bytes, measured_dispatches: timer.stages.length,
    pipeline_count: plan.stages.length, gpu: null,
    cpu_encode_ms: statistics(rows.map(s => s.cpu_encode_ms)),
    cpu_submit_ms: statistics(rows.map(s => s.cpu_submit_ms)),
    queue_wait_ms: statistics(rows.map(s => s.queue_wait_ms)), samples: rows,
  };
  if ('gpu_ms' in rows[0]) {
    const groups = {
      clear: s => /^(clear_|check_capacity)/.test(s),
      sort: s => /^(make_keys|count_|scan_|scatter_low|scatter_high|mark_ranges)/.test(s),
      solver: s => /^(update_|g2p)/.test(s),
    };
    const sum = (row, test) => timer.stages.reduce((n, s, i) => n + (test(s.name) ? row.pass_ms[i] : 0), 0);
    const transfer = timer.stages.findIndex(s => /^(gather_sorted|scatter_linear)$/.test(s.name));
    const transferMs = percentile(rows.map(s => s.pass_ms[transfer]), 0.50);
    const stepMs = percentile(rows.map(s => s.gpu_ms), 0.50);
    report.gpu = {
      full_ms: statistics(rows.map(s => s.gpu_ms)),
      passes: timer.stages.map((s, i) => ({ name: s.name, ms: statistics(rows.map(r => r.pass_ms[i])) })),
      ...Object.fromEntries(Object.entries(groups).map(([key, filter]) => [`${key}_ms`, statistics(rows.map(s => sum(s, filter)))])),
      particles_per_s_p2g: transferMs > 0 ? plan.particles * 1000 / transferMs : null,
      contributions_per_s_p2g: transferMs > 0 ? plan.contributions * 1000 / transferMs : null,
      particles_per_s_full: plan.particles * 1000 / stepMs,
      contributions_per_s_full: plan.contributions * 1000 / stepMs,
    };
  }
  return report;
}

function pairedInterval(a, b) {
  let state = 7249;
  function index() {
    state = (Math.imul(state, 1664525) + 1013904223) >>> 0;
    return state >>> 16 & (a.length - 1);
  }
  const delta = [];
  for (let r = 0; r < 2048; r++) {
    const indices = a.map(index);
    delta.push(percentile(indices.map(i => b[i].gpu_ms), 0.95) - percentile(indices.map(i => a[i].gpu_ms), 0.95));
  }
  return [percentile(delta, 0.025), percentile(delta, 0.975)];
}

async function paired(device, a, b, selector, hasTimestamps) {
  const left = clock(device, selector(a), hasTimestamps);
  const right = clock(device, selector(b), hasTimestamps);
  const x = [], y = [];
  for (let i = 0; i < 72; i++) {
    let aa, bb;
    if (i % 2 === 0) { aa = await sample(device, left); bb = await sample(device, right); }
    else { bb = await sample(device, right); aa = await sample(device, left); }
    if (i >= 8) { x.push(aa); y.push(bb); }
  }
  left.destroy();
  right.destroy();
  return { a: summary(a, left, x), b: summary(b, right, y),
    paired_b_minus_a_gpu_p95_ci95_ms: hasTimestamps ? pairedInterval(x, y) : null };
}

async function triangle(device) {
  const source = `
    @vertex fn vs(@builtin(vertex_index) i: u32) -> @builtin(position) vec4f {
      let p = array(vec2f(-1, -1), vec2f(3, -1), vec2f(-1, 3));
      return vec4f(p[i], 0, 1);
    }
    @fragment fn fs() -> @location(0) vec4f { return vec4f(0.25, 0.5, 0.75, 1); }
  `;
  const format = 'rgba8unorm';
  const texture = device.createTexture({ size: [1280, 800], format, usage: GPUTextureUsage.RENDER_ATTACHMENT });
  const view = texture.createView();
  const module = device.createShaderModule({ code: source });
  const pipeline = await device.createRenderPipelineAsync({
    layout: 'auto', vertex: { module, entryPoint: 'vs' },
    fragment: { module, entryPoint: 'fs', targets: [{ format }] },
  });
  const timer = clock(device, [{
    name: 'triangle',
    record(encoder, timestampWrites) {
      const pass = encoder.beginRenderPass({ timestampWrites, colorAttachments: [
        { view, loadOp: 'clear', clearValue: [0, 0, 0, 1], storeOp: 'store' },
      ] });
      pass.setPipeline(pipeline);
      pass.draw(3);
      pass.end();
    },
  }], true);
  const timings = [];
  for (let i = 0; i < 36; i++) {
    const measured = await sample(device, timer);
    if (i >= 4) timings.push(measured.gpu_ms);
  }
  timer.destroy();
  texture.destroy();
  return { p50: percentile(timings, 0.5), p95: percentile(timings, 0.95), samples_ms: timings };
}

async function benchmark(device, adapter, timestamps) {
  canvas.hidden = true;
  const fixed = await load(device, 'fixed-browser');
  const sorted = await load(device, 'sorted-browser');
  const grids = await checkGrids(device, sorted);
  const before = timestamps ? await triangle(device) : null;
  const runs = {};
  for (const fixture of ['spread', 'clumped']) {
    status.textContent = `Measuring ${fixture} transfer`;
    const rows = await fetchAsset(`fixed8/${fixture}-forward.particles`, 'arrayBuffer');
    for (const plan of [fixed, sorted]) {
      plan.upload('Control', Uint32Array.of(plan.particles));
      plan.upload('Packed', rows);
    }
    runs[fixture] = await paired(device, fixed, sorted, depositStages, timestamps);
  }
  status.textContent = 'Measuring full FLIP step';
  const splash = await fetchAsset('splash.state', 'arrayBuffer');
  for (const plan of [fixed, sorted]) plan.upload('State', splash);
  runs.full = await paired(device, fixed, sorted, p => p.stages, timestamps);
  const after = timestamps ? await triangle(device) : null;
  const { vendor, architecture, device: model, description, isFallbackAdapter } = adapter.info;
  const report = {
    route: sorted.route, chosen_by: 'forced browser sort/gather', arithmetic: 'i32 words at 2^12, ties away from zero',
    adapter: { vendor, architecture, model, description, isFallbackAdapter }, user_agent: navigator.userAgent,
    timestamp_granted: timestamps, triangle_before: before, triangle_after: after,
    triangle_admitted: !!(before && after && before.p95 < 0.15 && after.p95 < 0.15),
    grids, runs, failures,
  };
  await save('measurements.json', JSON.stringify(report, null, 2));
  result.textContent = JSON.stringify({ route: report.route, adapter: report.adapter, timestamps, grids: grids.length, failures });
  result.hidden = false;
  status.textContent = failures.length ? 'Grid check failed' : 'Browser measurement complete';
}

async function show(device) {
  const context = canvas.getContext('webgpu');
  const preferred = navigator.gpu.getPreferredCanvasFormat();
  const format = `${preferred}-srgb`;
  context.configure({ device, format: preferred, viewFormats: [format], alphaMode: 'opaque' });
  const plan = await load(device, 'splash-browser', format);
  const splash = await fetchAsset('splash.state', 'arrayBuffer');
  const play = document.getElementById('play');
  const reset = document.getElementById('reset');
  let playing = true;
  let pending = false;
  function initialize() {
    plan.upload('State', splash);
    plan.upload('Control', Uint32Array.of(plan.particles));
  }
  async function frame() {
    if (pending) return;
    pending = true;
    const encoder = device.createCommandEncoder();
    const view = context.getCurrentTexture().createView({ format });
    for (const stage of plan.stages) stage.record(encoder, undefined, view);
    device.queue.submit([encoder.finish()]);
    await device.queue.onSubmittedWorkDone();
    pending = false;
    if (playing) requestAnimationFrame(frame);
  }
  play.onclick = () => {
    playing = !playing;
    play.textContent = playing ? 'Pause' : 'Play';
    if (playing) requestAnimationFrame(frame);
  };
  reset.onclick = () => { initialize(); if (!playing) requestAnimationFrame(frame); };
  play.disabled = reset.disabled = false;
  initialize();
  status.textContent = 'Quadratic transfer · 65,536 particles';
  requestAnimationFrame(frame);
}

try {
  if (!navigator.gpu) throw new Error('WebGPU unavailable');
  const adapter = await navigator.gpu.requestAdapter({ powerPreference: 'high-performance' });
  if (!adapter) throw new Error('WebGPU adapter unavailable');
  const timestamps = adapter.features.has('timestamp-query');
  const device = await adapter.requestDevice({ requiredFeatures: timestamps ? ['timestamp-query'] : [] });
  device.addEventListener('uncapturederror', e => { failures.push(e.error.message); console.error(e.error); });
  if (params.has('bench')) await benchmark(device, adapter, timestamps);
  else await show(device);
} catch (error) {
  status.textContent = error.message;
  if (params.has('bench')) await save('error.json', JSON.stringify({ error: error.stack, failures }, null, 2));
  throw error;
}

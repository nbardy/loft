import init, { loadBundle } from './generated/loft_std.js';

// Keep the native Present entry and use a depth-writing entry for this host.
export function hostShader(source, sunSlot) {
  if (source.includes('fn shade(')) {
    const direction = /let sun = normalize\(\w+\.xyz\);/;
    if (!direction.test(source)) throw new Error('Hermit shade sun input changed');
    return source.replace(direction, `let sun = normalize(loft_params[${sunSlot}u].xyz);`);
  }
  if (!source.includes('fn fs_present(')) return source;
  return source + /* wgsl */ `
    @group(3) @binding(2) var loft_depth: texture_depth_2d;
    struct HostPixel {
      @location(0) color: vec4f,
      @builtin(frag_depth) depth: f32,
    }
    @fragment fn fs_host(@builtin(position) pos: vec4f) -> HostPixel {
      let px = vec2i(pos.xy);
      if (textureLoad(vis_tex, px, 0).x == LOFT_EMPTY) { discard; }
      let linear = loft_aces(textureLoad(hdr_tex, px, 0).rgb);
      let srgb = select(12.92 * linear, 1.055 * pow(linear, vec3f(1.0 / 2.4)) - 0.055,
                        linear > vec3f(0.0031308));
      return HostPixel(vec4f(srgb, 1), textureLoad(loft_depth, px, 0));
    }
  `;
}

export async function loadLoft(device) {
  await init();
  const response = await fetch('./generated/hermit.loftb');
  if (!response.ok) throw new Error(`Hermit bundle HTTP ${response.status}`);
  return loadBundle(device, new Uint8Array(await response.arrayBuffer()), hostShader);
}

export function prepareLoft(plan, colorFormat, depthFormat) {
  const { device, modules, buffers } = plan;
  const queue = device.queue;
  const uniform = GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST;
  const storage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST;
  const gpuBuffer = (size, usage) => device.createBuffer({ size, usage });
  const params = gpuBuffer(plan.params.length + 16, storage);
  queue.writeBuffer(params, 0, plan.params);
  const stages = GPUShaderStage;
  const fixed = device.createBindGroupLayout({
    entries: [
      { binding: 0, visibility: stages.VERTEX | stages.FRAGMENT | stages.COMPUTE, buffer: { type: 'uniform' } },
      { binding: 2, visibility: stages.VERTEX | stages.FRAGMENT | stages.COMPUTE, buffer: { type: 'read-only-storage' } },
    ],
  });
  const frames = new Map(Array.from(plan.frames, ([id, initial]) => {
    if (initial.length !== 256) throw new Error('Expected Loft Frame ABI (256 bytes)');
    const buffer = gpuBuffer(initial.length, uniform);
    return [id, {
      words: new Float32Array(initial.buffer),
      buffer,
      bind: device.createBindGroup({ layout: fixed, entries: [
        { binding: 0, resource: { buffer } },
        { binding: 2, resource: { buffer: params } },
      ] }),
    }];
  }));

  const textures = new Map(plan.textures.map(([id, format, size, sampleCount, usage, clear]) => {
    const texture = device.createTexture({ format, size, sampleCount, usage });
    return [id, { format, view: texture.createView(), clear, size }];
  }));
  const samplers = new Map(plan.samplers.map(id => [id, device.createSampler({
    compare: 'less-equal', minFilter: 'linear', magFilter: 'linear',
  })]));
  const [hostColor] = plan.hostAttachments;
  if (plan.hostAttachments.length !== 1) throw new Error('Expected one external color attachment');
  // Find the main camera depth by its writer, independently of texture allocation order.
  const mainPass = plan.passes.find(([frame, , [kind, , , , , , , depth]]) =>
    kind === 'render' && frame === plan.mainFrame && depth !== null);
  if (!mainPass) throw new Error('Bundle has no main-camera depth');
  const [, , [, , , , , , , [sceneDepth]]] = mainPass;
  const shadowTexture = plan.textures.find(([id, format]) => id !== sceneDepth && format === 'depth32float');
  if (!shadowTexture) throw new Error('Bundle has no sun shadow map');

  function binding([group, slot, id, access], visibility) {
    const layout = { binding: slot, visibility };
    const entry = { binding: slot };
    switch (access) {
      case 'ReadStorage':
      case 'ReadWriteStorage':
        layout.buffer = { type: access === 'ReadStorage' ? 'read-only-storage' : 'storage' };
        entry.resource = { buffer: buffers.get(id) };
        break;
      case 'SampledTex': {
        const { format, view } = textures.get(id);
        const sampleTypes = { rg32uint: 'uint', depth32float: 'depth', rgba16float: 'unfilterable-float' };
        layout.texture = { sampleType: sampleTypes[format] };
        entry.resource = view;
        break;
      }
      case 'StorageTex': {
        const { format, view } = textures.get(id);
        layout.storageTexture = { format, access: 'write-only' };
        entry.resource = view;
        break;
      }
      case 'Sampler':
        layout.sampler = { type: 'comparison' };
        entry.resource = samplers.get(id);
        break;
      default:
        throw new Error(`Unsupported WebGPU binding ${access}`);
    }
    return { group, layout, entry };
  }

  function groups(frame, slots, visibility) {
    const declarations = slots.map(slot => binding(slot, visibility));
    const bindGroupLayouts = [fixed];
    const bindGroups = [frames.get(frame).bind];
    const last = Math.max(0, ...declarations.map(({ group }) => group));
    for (let group = 1; group <= last; group++) {
      const items = declarations.filter(decl => decl.group === group);
      const layout = device.createBindGroupLayout({ entries: items.map(item => item.layout) });
      bindGroupLayouts.push(layout);
      bindGroups.push(device.createBindGroup({ layout, entries: items.map(item => item.entry) }));
    }
    return {
      layout: device.createPipelineLayout({ bindGroupLayouts }),
      bind(pass) { bindGroups.forEach((group, index) => pass.setBindGroup(index, group)); },
    };
  }

  const written = new Set();
  function attachment(id, depth) {
    const { view, clear } = textures.get(id);
    const first = !written.has(id);
    written.add(id);
    const load = first && clear !== null ? 'clear' : 'load';
    return depth
      ? { view, depthLoadOp: load, depthStoreOp: 'store', depthClearValue: clear?.[0] }
      : { view, loadOp: load, storeOp: 'store', clearValue: clear ?? undefined };
  }

  function compute(frame, slots, [moduleId, entryPoint, [kind, ...args]]) {
    const { layout, bind } = groups(frame, slots, stages.COMPUTE);
    const pipeline = device.createComputePipeline({ layout, compute: { module: modules.get(moduleId), entryPoint } });
    const dispatch = {
      direct: (pass, x, y, z) => pass.dispatchWorkgroups(x, y, z),
      indirect: (pass, id, offset) => pass.dispatchWorkgroupsIndirect(buffers.get(id), offset),
    }[kind];
    if (!dispatch) throw new Error(`Unsupported dispatch ${kind}`);
    return encoder => {
      const pass = encoder.beginComputePass();
      pass.setPipeline(pipeline);
      bind(pass);
      dispatch(pass, ...args);
      pass.end();
    };
  }

  function draw(frame, slots, { moduleId, vs, fs, args, offset, cullFront, colors, depthFormat, depthWrite }) {
    const { layout, bind } = groups(frame, slots, stages.VERTEX | stages.FRAGMENT);
    const module = modules.get(moduleId);
    const pipeline = device.createRenderPipeline({
      layout,
      vertex: { module, entryPoint: vs },
      fragment: { module, entryPoint: fs, targets: colors.map(format => ({ format })) },
      primitive: { cullMode: cullFront ? 'front' : 'none' },
      depthStencil: { format: depthFormat, depthCompare: 'less', depthWriteEnabled: depthWrite },
    });
    const indirect = buffers.get(args);
    return (encoder, descriptor) => {
      const pass = encoder.beginRenderPass(descriptor);
      pass.setPipeline(pipeline);
      bind(pass);
      pass.drawIndirect(indirect, offset);
      pass.end();
    };
  }

  function render(frame, slots, [moduleId, vs, fs, args, offset, colors, depth, cullFront]) {
    const spec = { moduleId, vs, fs, args, offset, cullFront };
    if (colors.length === 1 && colors[0] === hostColor && depth === null && fs === 'fs_present') {
      if (slots.some(([group, slot]) => group === 3 && slot === 2)) {
        throw new Error('Hermit Present depth binding is occupied');
      }
      const present = draw(frame, [...slots, [3, 2, sceneDepth, 'SampledTex']], {
        ...spec, fs: 'fs_host', colors: [colorFormat], depthFormat, depthWrite: true,
      });
      return (encoder, colorView, depthView) => present(encoder, {
        colorAttachments: [{ view: colorView, loadOp: 'load', storeOp: 'store' }],
        depthStencilAttachment: { view: depthView, depthLoadOp: 'load', depthStoreOp: 'store' },
      });
    }
    if (!depth || colors.some(id => !textures.has(id))) {
      throw new Error(`Unexpected Hermit render attachments for ${fs}`);
    }
    const [id, depthWrite] = depth;
    const record = draw(frame, slots, {
      ...spec, colors: colors.map(color => textures.get(color).format),
      depthFormat: textures.get(id).format, depthWrite,
    });
    const descriptor = {
      colorAttachments: colors.map(color => attachment(color, false)),
      depthStencilAttachment: attachment(id, true),
    };
    return encoder => record(encoder, descriptor);
  }

  const passes = plan.passes.map(([frame, bindings, [kind, ...work]]) => {
    switch (kind) {
      case 'compute': return compute(frame, bindings, work);
      case 'render': return render(frame, bindings, work);
      default: throw new Error(`Unsupported Loft pass ${kind}`);
    }
  });
  const [width, height] = textures.get(sceneDepth).size;
  return {
    width, height, passCount: passes.length, shadowSize: shadowTexture[2][0],
    encode(encoder, colorView, depthView, camera, sunCamera, sun, time) {
      queue.writeBuffer(params, plan.params.length, new Float32Array([sun.x, sun.y, sun.z, 0]));
      for (const [id, { words, buffer }] of frames) {
        const { vp, inverse, eye } = id === plan.mainFrame ? camera : sunCamera;
        // Frame is column-major: view/projection pair, eye/time, sun/texel, size/flags, light matrix.
        words.set(vp, 0);
        words.set(inverse, 16);
        words.set([eye.x, eye.y, eye.z, time], 32);
        words.set([sun.x, sun.y, sun.z, sunCamera.texel], 36);
        words.set(sunCamera.vp, 48);
        queue.writeBuffer(buffer, 0, words);
      }
      for (const [id, initial] of plan.resets) queue.writeBuffer(buffers.get(id), 0, initial);
      for (const pass of passes) pass(encoder, colorView, depthView);
    },
  };
}

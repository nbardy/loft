import * as THREE from 'three/webgpu';
import { loadLoft, prepareLoft } from './runner.js';

const canvas = document.querySelector('canvas');
const status = document.querySelector('#time');
const position = document.querySelector('#position');
const sunAngle = document.querySelector('#sun');
const alert = document.querySelector('[role=alert]');

export let demo;
try {
  demo = await start();
} catch (error) {
  report(error);
}

function report(error) {
  alert.hidden = false;
  alert.textContent = String(error);
  status.textContent = 'WebGPU demo stopped';
  console.error(error);
}

async function start() {
  if (!navigator.gpu) throw new Error('WebGPU is required');
  const renderer = new THREE.WebGPURenderer({ canvas, antialias: false, alpha: false });
  renderer._getFallback = null;
  // Linear output bypasses Three's intermediate framebuffer and presentation quad.
  renderer.outputColorSpace = THREE.LinearSRGBColorSpace;
  await renderer.init();
  if (!renderer.backend.isWebGPUBackend) throw new Error('Three could not start WebGPU');
  const { device, context, textureUtils } = renderer.backend;
  device.addEventListener('uncapturederror', event => report(event.error.message));

  device.pushErrorScope('validation');
  let loft;
  try {
    const plan = await loadLoft(device);
    if (plan.device !== device) throw new Error('Loft did not adopt the Three GPUDevice');
    renderer.setSize(1280, 800, false);
    const color = context.getCurrentTexture();
    const depth = textureUtils.getDepthBuffer(true, false);
    loft = prepareLoft(plan, color.format, depth.format);
    if (loft.width !== color.width || loft.height !== color.height) {
      throw new Error('Three framebuffer must match the Hermit bundle extent');
    }
  } finally {
    const error = await device.popErrorScope();
    if (error) throw new Error(error.message);
  }
  const scene = new THREE.Scene();
  scene.background = new THREE.Color().setRGB(0.145, 0.194, 0.234);
  const camera = new THREE.PerspectiveCamera(40, loft.width / loft.height, 0.05, 400);
  camera.coordinateSystem = THREE.WebGPUCoordinateSystem;
  camera.position.set(2.2, 1.55, 4.4);
  const focus = new THREE.Vector3(0, 0.55, 0);
  camera.lookAt(focus);
  camera.updateProjectionMatrix();

  const cube = new THREE.Mesh(new THREE.BoxGeometry(1, 1, 1), new THREE.MeshStandardMaterial({
    color: 0x298fff, roughness: 0.68,
  }));
  cube.rotation.y = 0.15;
  scene.add(cube, new THREE.HemisphereLight(0xf5fbff, 0x5d708c, 1.35));
  const sun = new THREE.DirectionalLight(0xfff3df, 2.15);
  scene.add(sun, sun.target);

  const shadowRadius = 2.5;
  const shadowCamera = new THREE.OrthographicCamera(
    -shadowRadius, shadowRadius, shadowRadius, -shadowRadius, 0, shadowRadius + 100,
  );
  shadowCamera.coordinateSystem = THREE.WebGPUCoordinateSystem;
  shadowCamera.updateProjectionMatrix();
  const shadowCenter = new THREE.Vector3(0, 0.65, 0);
  const towardsSun = new THREE.Vector3();
  const viewProj = new THREE.Matrix4();
  const inverse = new THREE.Matrix4();
  const shadowProj = new THREE.Matrix4();
  const inverseShadow = new THREE.Matrix4();
  const cubeCenter = new THREE.Vector3(0.12, 0.75, 0.08);
  const sunRay = new THREE.Vector3();
  const cubeDistance = { front: 0.70, crossing: 1, behind: 1.48 };
  const samples = [];
  let request = 0;
  let previous;
  let showLoft = true;

  function draw(now, timed) {
    const start = performance.now();
    const scale = cubeDistance[position.value];
    cube.position.copy(camera.position).lerp(cubeCenter, scale);
    cube.scale.setScalar(0.86 * scale);
    const azimuth = THREE.MathUtils.degToRad(Number(sunAngle.value));
    sun.position.set(3 * Math.sin(azimuth), 4, 3 * Math.cos(azimuth));
    towardsSun.subVectors(sun.position, sun.target.position).normalize();
    renderer.render(scene, camera);
    if (showLoft) {
      const encoder = device.createCommandEncoder({ label: 'Loft Hermit' });
      viewProj.multiplyMatrices(camera.projectionMatrix, camera.matrixWorldInverse);
      shadowCamera.position.copy(shadowCenter).addScaledVector(towardsSun, shadowRadius);
      shadowCamera.lookAt(shadowCenter);
      shadowCamera.updateMatrixWorld();
      shadowProj.multiplyMatrices(shadowCamera.projectionMatrix, shadowCamera.matrixWorldInverse);
      loft.encode(encoder, context.getCurrentTexture().createView(), textureUtils.getDepthBuffer(true, false).createView(), {
        vp: viewProj.elements, inverse: inverse.copy(viewProj).invert().elements, eye: camera.position,
      }, {
        vp: shadowProj.elements, inverse: inverseShadow.copy(shadowProj).invert().elements,
        eye: sunRay.copy(towardsSun).negate(), texel: 2 * shadowRadius / loft.shadowSize,
      }, towardsSun, now / 1000);
      device.queue.submit([encoder.finish()]);
    }
    if (!timed) return;
    const cpu = performance.now() - start;
    const interval = now - previous;
    previous = now;
    if (Number.isFinite(interval)) {
      samples.push({ cpu, interval });
      if (samples.length > 150) samples.shift();
    }
    const cadence = Number.isFinite(interval) ? ` · ${interval.toFixed(1)} ms frame interval` : '';
    status.textContent = `${cpu.toFixed(2)} ms CPU submit${cadence}`;
  }

  function animate(now) {
    try {
      draw(now, true);
      if (alert.hidden) request = requestAnimationFrame(animate);
    } catch (error) {
      report(error);
    }
  }
  request = requestAnimationFrame(animate);

  // Browser checks can freeze the scene and isolate each producer using the same frame path.
  return {
    device, renderer, samples, passCount: loft.passCount,
    capture({ three = true, loft = true } = {}) {
      cancelAnimationFrame(request);
      cube.visible = three;
      showLoft = loft;
      draw(performance.now(), false);
      return device.queue.onSubmittedWorkDone();
    },
  };
}

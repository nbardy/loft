// The live viewer. Draws the compiled artefact into a full-window WebGPU canvas through the host's browser path.
// State that should survive a reload (camera, time, sliders) lives in sessionStorage under one key.
import type { Artefact, Camera, View } from "../src/artefact.ts";
import { orbit } from "../src/host/camera.ts";
import { COLOR_FORMAT, createHost, type GpuLoaded } from "../src/host/host.ts";
import { FALLBACK_CAMERAS } from "../src/cli/cameras.ts";
// @ts-expect-error virtual module supplied by `loft dev` (src/cli/dev.ts)
import { build, sceneName } from "scene:current";

const $ = <T extends HTMLElement>(id: string) => document.getElementById(id) as T;
const canvas = $<HTMLCanvasElement>("view");
const errorBox = $<HTMLDivElement>("error");
const LOOP_SECONDS = 20;
const KEY = `loft.viewer.${sceneName as string}`;

type Saved = { camera: Camera | null; time: number; params: Record<string, number>; construction: boolean; playing: boolean; speed: number };
const state: Saved = { camera: null, time: 0, params: {}, construction: false, playing: true, speed: 1 };

function restore() {
  try {
    const raw = sessionStorage.getItem(KEY);
    if (raw) Object.assign(state, JSON.parse(raw) as Partial<Saved>);
  } catch { /* storage can be blocked; the viewer works without it */ }
}
function persist() {
  try { sessionStorage.setItem(KEY, JSON.stringify(state)); } catch { /* see restore */ }
}

function showError(text: string) { errorBox.textContent = text; errorBox.hidden = false; }
function clearError() { errorBox.hidden = true; errorBox.textContent = ""; }

// ---- live reload: the server tells us when a rebuild finished or failed ----------------------------------------
function connect() {
  const ws = new WebSocket(`ws://${location.host}/ws`);
  ws.onmessage = (e) => {
    const msg = JSON.parse(e.data as string) as { type: "reload" } | { type: "error"; text: string };
    switch (msg.type) {
      case "reload": persist(); location.reload(); break;
      case "error": showError(`build failed:\n${msg.text}`); break;
    }
  };
  ws.onclose = () => setTimeout(connect, 500);
}

async function start() {
  restore();
  connect();
  if (!("gpu" in navigator)) return showError("WebGPU is not available in this browser (navigator.gpu is missing). Use a recent Chrome or Edge.");
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) return showError("WebGPU is present but no adapter was returned (blocked GPU or unsupported hardware).");
  const device = await adapter.requestDevice();
  device.addEventListener("uncapturederror", (e) => showError(`WebGPU error:\n${(e as GPUUncapturedErrorEvent).error.message}`));
  const context = canvas.getContext("webgpu")!;
  context.configure({ device, format: COLOR_FORMAT, alphaMode: "opaque" });

  let artefact: Artefact;
  let loaded: GpuLoaded;
  try {
    artefact = (build as () => Artefact)();
    loaded = await createHost(device).load(artefact);
  } catch (e) {
    return showError(`scene failed to compile or load:\n${e instanceof Error ? (e.stack ?? e.message) : String(e)}`);
  }

  const cameras: Record<string, Camera> = { ...FALLBACK_CAMERAS, ...artefact.cameras };
  if (!state.camera) state.camera = cameras["three"] ?? Object.values(cameras)[0]!;
  const defaults = Object.fromEntries(artefact.params.map((p) => [p.name, p.def]));
  state.params = { ...defaults, ...Object.fromEntries(Object.entries(state.params).filter(([k]) => k in defaults)) };

  // ---- controls ----
  const camRow = $("cameras");
  for (const name of Object.keys(cameras)) {
    const b = document.createElement("button");
    b.textContent = name;
    b.onclick = () => { state.camera = cameras[name]!; persist(); };
    camRow.append(b);
  }
  const construction = $<HTMLInputElement>("construction");
  construction.checked = state.construction;
  construction.onchange = () => { state.construction = construction.checked; persist(); };

  const sliders = new Map<string, { input: HTMLInputElement; value: HTMLSpanElement }>();
  for (const p of artefact.params) {
    const wrap = document.createElement("div");
    wrap.className = "param";
    const label = document.createElement("span");
    label.textContent = p.name;
    const value = document.createElement("span");
    const input = document.createElement("input");
    input.type = "range";
    input.min = String(p.min); input.max = String(p.max); input.step = String((p.max - p.min) / 500);
    input.value = String(state.params[p.name]);
    value.textContent = Number(input.value).toFixed(3);
    input.oninput = () => { state.params[p.name] = Number(input.value); value.textContent = Number(input.value).toFixed(3); persist(); };
    wrap.append(label, value, input);
    $("params").append(wrap);
    sliders.set(p.name, { input, value });
  }
  $("reset").onclick = () => {
    for (const p of artefact.params) {
      state.params[p.name] = p.def;
      const s = sliders.get(p.name)!;
      s.input.value = String(p.def); s.value.textContent = p.def.toFixed(3);
    }
    persist();
  };

  const play = $<HTMLButtonElement>("play");
  const time = $<HTMLInputElement>("time");
  const speed = $<HTMLSelectElement>("speed");
  const syncPlay = () => { play.textContent = state.playing ? "pause" : "play"; };
  play.onclick = () => { state.playing = !state.playing; syncPlay(); persist(); };
  time.oninput = () => { state.time = Number(time.value); persist(); };
  speed.value = String(state.speed);
  speed.onchange = () => { state.speed = Number(speed.value); persist(); };
  syncPlay();

  // ---- camera: drag orbits, wheel zooms ----
  let drag: { x: number; y: number } | null = null;
  canvas.onpointerdown = (e) => { drag = { x: e.clientX, y: e.clientY }; canvas.setPointerCapture(e.pointerId); };
  canvas.onpointerup = () => { drag = null; persist(); };
  canvas.onpointermove = (e) => {
    if (!drag) return;
    // Dragging right swings the camera left around the target, so the scene appears to follow the pointer.
    state.camera = orbit(state.camera!, -(e.clientX - drag.x) * 0.006, (e.clientY - drag.y) * 0.006, 1);
    drag = { x: e.clientX, y: e.clientY };
  };
  canvas.onwheel = (e) => { e.preventDefault(); state.camera = orbit(state.camera!, 0, 0, Math.exp(e.deltaY * 0.001)); persist(); };

  // ---- frame loop ----
  const readout = $("readout");
  let last = performance.now();
  let smoothed = 0;
  const tick = async (now: number) => {
    const dt = (now - last) / 1000;
    last = now;
    if (state.playing) state.time = (state.time + dt * state.speed) % LOOP_SECONDS;
    time.value = String(state.time);
    $("timeText").textContent = `${state.time.toFixed(2)} s`;

    const dpr = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.floor(canvas.clientWidth * dpr));
    const height = Math.max(1, Math.floor(canvas.clientHeight * dpr));
    if (canvas.width !== width || canvas.height !== height) { canvas.width = width; canvas.height = height; }
    const view: View = { camera: state.camera!, width, height, time: state.time, params: state.params, construction: state.construction };
    const t = performance.now();
    try {
      await loaded.draw(view, context.getCurrentTexture());
      clearError();
    } catch (e) {
      showError(`draw failed:\n${e instanceof Error ? e.message : String(e)}`);
      return; // stop the loop: the next successful rebuild reloads the page
    }
    smoothed = smoothed === 0 ? performance.now() - t : smoothed * 0.9 + (performance.now() - t) * 0.1;
    readout.textContent = `${smoothed.toFixed(1)} ms/frame\n${artefact.stats.triangles} triangles\n${artefact.stats.draws} draws`;
    requestAnimationFrame(tick);
  };
  requestAnimationFrame(tick);
}

start().catch((e) => showError(`viewer failed:\n${e instanceof Error ? (e.stack ?? e.message) : String(e)}`));

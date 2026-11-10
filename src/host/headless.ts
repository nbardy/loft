// A GPU device for bun, through bun-webgpu (Dawn). The only file in src/host that imports bun-webgpu, so a browser
// bundle of the rest never pulls in FFI code.
import { setupGlobals } from "bun-webgpu";

export async function headlessDevice(): Promise<GPUDevice> {
  // setupGlobals also installs GPUBufferUsage, GPUTextureUsage and GPUMapMode, which host.ts reads.
  await setupGlobals();
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new Error("headlessDevice: no WebGPU adapter found (bun-webgpu/Dawn could not open a GPU on this machine)");
  const device = await adapter.requestDevice();
  device.addEventListener("uncapturederror", (e) => {
    // Errors outside a validation scope would otherwise vanish; the host scopes its own work, so this is a backstop.
    console.error(`loft host: uncaptured WebGPU error: ${(e as GPUUncapturedErrorEvent).error.message}`);
  });
  return device;
}

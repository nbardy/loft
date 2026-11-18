// A GPU device for bun, through bun-webgpu (Dawn). The only file in src/host that imports bun-webgpu, so a browser
// bundle of the rest never pulls in FFI code.
import { EnvironmentError } from "../errors.ts";
import { setupGlobals } from "bun-webgpu";

export async function headlessDevice(): Promise<GPUDevice> {
  // `loft serve` compiles in a child process that must not reach the GPU (src/cli/serve.ts); a confinement hook can
  // enforce that against hostile code, this says it plainly to honest code (a scene with `recur` bakes on the GPU).
  if (process.env.LOFT_NO_GPU === "1") {
    throw new EnvironmentError("headlessDevice: this process was started without a GPU (LOFT_NO_GPU=1, as `loft serve` compiles); a scene with recur bakes on the GPU: `loft build` it outside serve and send the directory", false);
  }
  // setupGlobals also installs GPUBufferUsage, GPUTextureUsage and GPUMapMode, which host.ts reads.
  await setupGlobals();
  const adapter = await navigator.gpu.requestAdapter();
  if (!adapter) throw new EnvironmentError("headlessDevice: no WebGPU adapter found (bun-webgpu/Dawn could not open a GPU on this machine)", false);
  // Timestamp queries let `GpuLoaded.profile` time each draw on the GPU; ordinary frames never use them.
  const requiredFeatures: GPUFeatureName[] = adapter.features.has("timestamp-query") ? ["timestamp-query"] : [];
  if (adapter.features.has("shader-f16")) requiredFeatures.push("shader-f16");
  const device = await adapter.requestDevice({ requiredFeatures }).catch((e: unknown) => {
    throw new EnvironmentError(`headlessDevice: the adapter refused a device: ${e instanceof Error ? e.message : String(e)}`, true);
  });
  device.addEventListener("uncapturederror", (e) => {
    // Errors outside a validation scope would otherwise vanish; the host scopes its own work, so this is a backstop.
    console.error(`loft host: uncaptured WebGPU error: ${(e as GPUUncapturedErrorEvent).error.message}`);
  });
  return device;
}

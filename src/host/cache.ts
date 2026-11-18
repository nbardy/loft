// A device whose costly creations are shared across loads, by content: equal WGSL is one shader module, equal
// descriptors one layout or pipeline. For a process that loads many artefacts on one device (`loft serve`), where shader
// and pipeline compilation is most of a load (creative measured about 80% of a training program's CPU seconds).
//
// It cannot change pixels: a module is a pure function of its WGSL, a pipeline of its descriptor. Keys never include
// labels, so a cached object keeps the label of the load that created it (a GPU error may name another scene's draw).
// Inherited from creative's training host (training/loft/host/server.ts `caching`, 16 Nov); here the key is the whole
// descriptor with each GPU object replaced by its own key, so a field added to a descriptor later is keyed too.

/** Past this many cached objects, `bound` empties the cache (between loads, never during one). */
const ENTRIES = 8192;

export type CachingDevice = {
  /** The device to load with: the five creations below are memoised, everything else is the device's own. */
  readonly device: GPUDevice;
  /** Runs one load; if it fails, forgets what it created, so a cached object is always one the GPU accepted. */
  settle<T>(load: () => Promise<T>): Promise<T>;
  counts(): { readonly hits: number; readonly misses: number; readonly entries: number };
  /** Empties the cache when it holds more than ENTRIES objects. Returns whether it did. */
  bound(): boolean;
};

const MEMOISED = ["createBindGroupLayout", "createPipelineLayout", "createShaderModule", "createComputePipelineAsync", "createRenderPipelineAsync"] as const;
type Memoised = (typeof MEMOISED)[number];

export function cachingDevice(device: GPUDevice): CachingDevice {
  const cache = new Map<string, unknown>();
  const keyOf = new WeakMap<object, string>();
  let fresh: string[] = [], hits = 0, misses = 0;
  const sha256 = (text: string): string => new Bun.CryptoHasher("sha256").update(text).digest("hex");

  /** The descriptor as text: labels dropped, WGSL hashed, GPU objects by their own keys. */
  const keyText = (kind: Memoised, descriptor: object): string => kind + " " + JSON.stringify(descriptor, (field, value: unknown) => {
    if (field === "label") return undefined;
    if (field === "code" && typeof value === "string") return sha256(value);
    if (typeof value === "object" && value !== null && !Array.isArray(value) && Object.getPrototypeOf(value) !== Object.prototype) {
      const k = keyOf.get(value);
      // A GPU object this cache did not create (a buffer, a sampler) has no content key: refuse rather than key it by
      // `{}`, which every such object would share. None of the five descriptors names one today.
      if (k === undefined) throw new Error(`cachingDevice: ${kind} names a GPU object the cache did not create (field "${field}")`);
      return `#${k}`;
    }
    return value;
  });
  const memo = <T>(kind: Memoised, descriptor: object, make: () => T): T => {
    const k = keyText(kind, descriptor);
    if (cache.has(k)) { hits++; return cache.get(k) as T; }
    misses++;
    const made = make();
    cache.set(k, made); fresh.push(k);
    // The async pipelines are cached as their promises; nothing names a pipeline in a descriptor, so they need no key.
    if (typeof made === "object" && made !== null && !(made instanceof Promise)) keyOf.set(made, k);
    return made;
  };
  const own: Pick<GPUDevice, Memoised> = {
    createBindGroupLayout: (d) => memo("createBindGroupLayout", d, () => device.createBindGroupLayout(d)),
    createPipelineLayout: (d) => memo("createPipelineLayout", d, () => device.createPipelineLayout(d)),
    createShaderModule: (d) => memo("createShaderModule", d, () => device.createShaderModule(d)),
    createComputePipelineAsync: (d) => memo("createComputePipelineAsync", d, () => device.createComputePipelineAsync(d)),
    createRenderPipelineAsync: (d) => memo("createRenderPipelineAsync", d, () => device.createRenderPipelineAsync(d)),
  };
  const proxy = new Proxy(device, {
    get(target, prop) {
      if (Object.hasOwn(own, prop)) return own[prop as Memoised];
      const value = Reflect.get(target, prop, target);
      return typeof value === "function" ? value.bind(target) : value;
    },
  });
  return {
    device: proxy,
    async settle(load) {
      fresh = [];
      try {
        return await load();
      } catch (error) {
        for (const k of fresh) cache.delete(k);
        throw error;
      }
    },
    counts: () => ({ hits, misses, entries: cache.size }),
    bound() {
      if (cache.size <= ENTRIES) return false;
      cache.clear();
      return true;
    },
  };
}

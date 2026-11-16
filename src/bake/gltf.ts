// A glTF 2.0 binary (.glb), written by hand. Layout: 12-byte header, a JSON chunk (padded with spaces), a BIN chunk
// (padded with zeros); every chunk and every buffer view starts on a 4-byte boundary. All data is float32 or uint32,
// so tightly packed attribute views are aligned by construction. glTF is y-up, right-handed, counter-clockwise front
// faces: the same as Loft's world, so positions go through unchanged.

import { triangleCount, vertexCount, type Baked, type BodyMesh, type Colours } from "./mesh.ts";
import type { Animated, Motion } from "./motion.ts";

const MAGIC = 0x46546c67, VERSION = 2, JSON_CHUNK = 0x4e4f534a, BIN_CHUNK = 0x004e4942;
const ARRAY_BUFFER = 34962, ELEMENT_ARRAY_BUFFER = 34963, FLOAT = 5126, UNSIGNED_INT = 5125, UNSIGNED_SHORT = 5123;

type Accessor = Record<string, unknown>;

function bounds(a: Float32Array, width: number): { min: number[]; max: number[] } {
  const min = new Array<number>(width).fill(Infinity), max = new Array<number>(width).fill(-Infinity);
  for (let i = 0; i < a.length; i++) {
    const c = i % width;
    if (a[i] < min[c]) min[c] = a[i];
    if (a[i] > max[c]) max[c] = a[i];
  }
  return { min, max };
}

const pad4 = (n: number): number => (n + 3) & ~3;

/**
 * Writes the bodies, one node and one mesh each, named after the body (nodes 0 .. bodies - 1). With `motion` (from
 * `bakeAnimated`, whose bodies these are), one animation "loft" keys them: a skinned body's node takes a skin whose
 * joints are a root node `<name>_rig` and its children `<name>_j<k>` (after the bodies' nodes), keyed in translation,
 * rotation and scale; a morphed body's mesh takes one target per key and its node's weights are keyed one-hot.
 */
export function writeGlb(baked: Pick<Baked, "bodies">, motion?: Pick<Animated, "motions" | "times">): Uint8Array {
  if (baked.bodies.length === 0) throw new Error("writeGlb: nothing to write (the scene has no bakeable body)");
  const chunks: Uint8Array[] = [];
  const bufferViews: Record<string, unknown>[] = [];
  const accessors: Accessor[] = [];
  let offset = 0;

  const view = (bytes: Uint8Array, target: number | undefined): number => {
    bufferViews.push({ buffer: 0, byteOffset: offset, byteLength: bytes.byteLength, ...(target === undefined ? {} : { target }) });
    const padded = new Uint8Array(pad4(bytes.byteLength));
    padded.set(bytes);
    chunks.push(padded);
    offset += padded.byteLength;
    return bufferViews.length - 1;
  };
  const attribute = (data: Float32Array, type: "VEC2" | "VEC3" | "VEC4", width: number, withBounds: boolean): number => {
    const bufferView = view(new Uint8Array(data.buffer, data.byteOffset, data.byteLength), ARRAY_BUFFER);
    accessors.push({ bufferView, componentType: FLOAT, count: data.length / width, type, ...(withBounds ? bounds(data, width) : {}) });
    return accessors.length - 1;
  };
  /** Data that is not a vertex attribute (inverse binds, key times and values): no buffer view target. */
  const data = (a: Float32Array, type: string, width: number, withBounds: boolean): number => {
    const bufferView = view(new Uint8Array(a.buffer, a.byteOffset, a.byteLength), undefined);
    accessors.push({ bufferView, componentType: FLOAT, count: a.length / width, type, ...(withBounds ? bounds(a, width) : {}) });
    return accessors.length - 1;
  };
  const indices = (b: BodyMesh): number => {
    const bufferView = view(new Uint8Array(b.indices.buffer, b.indices.byteOffset, b.indices.byteLength), ELEMENT_ARRAY_BUFFER);
    let lo = Infinity, hi = -Infinity;
    for (const i of b.indices) { if (i < lo) lo = i; if (i > hi) hi = i; }
    accessors.push({ bufferView, componentType: UNSIGNED_INT, count: b.indices.length, type: "SCALAR", min: [lo], max: [hi] });
    return accessors.length - 1;
  };

  const colourAttribute = (c: Colours): Record<string, number> => {
    switch (c.kind) {
      case "rgb": return { COLOR_0: attribute(c.rgb, "VEC3", 3, false) };
      case "none": return {}; // the material's white; `summary` says why
    }
  };

  const meshes: Record<string, unknown>[] = [], materials: unknown[] = [], nodes: Record<string, unknown>[] = [];
  const primitives: Record<string, unknown>[] = [];
  baked.bodies.forEach((b, k) => {
    materials.push({
      name: b.name,
      pbrMetallicRoughness: { baseColorFactor: [1, 1, 1, 1], metallicFactor: 0, roughnessFactor: 1 },
      doubleSided: b.doubleSided,
    });
    const primitive = {
      attributes: {
        POSITION: attribute(b.positions, "VEC3", 3, true),
        NORMAL: attribute(b.normals, "VEC3", 3, false),
        TEXCOORD_0: attribute(b.uvs, "VEC2", 2, false),
        ...colourAttribute(b.paint),
      } as Record<string, number>,
      indices: indices(b),
      material: k,
      mode: 4,
    };
    primitives.push(primitive);
    meshes.push({ name: b.name, primitives: [primitive] });
    nodes.push({ name: b.name, mesh: k });
  });

  const skins: unknown[] = [], channels: unknown[] = [], samplers: unknown[] = [];
  if (motion !== undefined) {
    if (motion.motions.length !== baked.bodies.length) throw new Error(`writeGlb: ${motion.motions.length} motions for ${baked.bodies.length} bodies`);
    const input = data(Float32Array.from(motion.times), "SCALAR", 1, true);
    const keyed = (node: number, path: string, values: Float32Array, type: string, width: number): void => {
      samplers.push({ input, output: data(values, type, width, false), interpolation: "LINEAR" });
      channels.push({ sampler: samplers.length - 1, target: { node, path } });
    };
    const write = (m: Motion, k: number): void => {
      const b = baked.bodies[k]!;
      switch (m.kind) {
        case "still": case "refused": return;
        case "skinned": {
          const n = m.keys[0]!.length, root = nodes.length;
          nodes.push({ name: `${b.name}_rig`, children: Array.from({ length: n }, (_, j) => root + 1 + j) });
          // The joints sit at their t = 0 key: with the inverse binds, the skin at rest is the identity.
          m.keys[0]!.forEach((key, j) => nodes.push({ name: `${b.name}_j${j}`, translation: key.t, rotation: key.r, scale: key.s }));
          const joints = Array.from({ length: n }, (_, j) => root + 1 + j);
          skins.push({ name: b.name, skeleton: root, joints, inverseBindMatrices: data(m.inverseBind, "MAT4", 16, false) });
          nodes[k]!.skin = skins.length - 1;
          const jv = view(new Uint8Array(m.joints.buffer, m.joints.byteOffset, m.joints.byteLength), ARRAY_BUFFER);
          accessors.push({ bufferView: jv, componentType: UNSIGNED_SHORT, count: m.joints.length / 4, type: "VEC4" });
          primitives[k]!.attributes = { ...(primitives[k]!.attributes as Record<string, number>), JOINTS_0: accessors.length - 1, WEIGHTS_0: attribute(m.weights, "VEC4", 4, false) };
          joints.forEach((node, j) => {
            keyed(node, "translation", Float32Array.from(m.keys.flatMap((ks) => ks[j]!.t)), "VEC3", 3);
            keyed(node, "rotation", Float32Array.from(m.keys.flatMap((ks) => ks[j]!.r)), "VEC4", 4);
            keyed(node, "scale", Float32Array.from(m.keys.flatMap((ks) => ks[j]!.s)), "VEC3", 3);
          });
          return;
        }
        case "morphed": {
          const T = m.deltas.length;
          primitives[k]!.targets = m.deltas.map((d) => ({ POSITION: attribute(d, "VEC3", 3, true) }));
          meshes[k]!.weights = new Array<number>(T).fill(0);
          // Key i shows target i alone; LINEAR weights between keys blend two neighbouring targets.
          const w = new Float32Array(T * T);
          for (let i = 0; i < T; i++) w[i * T + i] = 1;
          keyed(k, "weights", w, "SCALAR", 1);
          return;
        }
      }
    };
    motion.motions.forEach(write);
  }
  // Scene roots: every body's node, and every rig's root (its joints are its children).
  const children = new Set(nodes.flatMap((n) => (n.children as number[] | undefined) ?? []));

  const json = {
    asset: { version: "2.0", generator: "loft bake" },
    scene: 0,
    scenes: [{ name: "loft", nodes: nodes.map((_, i) => i).filter((i) => !children.has(i)) }],
    nodes, meshes, materials, accessors, bufferViews,
    ...(skins.length > 0 ? { skins } : {}),
    ...(channels.length > 0 ? { animations: [{ name: "loft", channels, samplers }] } : {}),
    buffers: [{ byteLength: offset }],
  };
  const jsonBytes = new TextEncoder().encode(JSON.stringify(json));
  const jsonLen = pad4(jsonBytes.byteLength);
  const total = 12 + 8 + jsonLen + 8 + offset;
  const out = new Uint8Array(total), dv = new DataView(out.buffer);
  dv.setUint32(0, MAGIC, true); dv.setUint32(4, VERSION, true); dv.setUint32(8, total, true);
  dv.setUint32(12, jsonLen, true); dv.setUint32(16, JSON_CHUNK, true);
  out.fill(0x20, 20, 20 + jsonLen);
  out.set(jsonBytes, 20);
  const binAt = 20 + jsonLen;
  dv.setUint32(binAt, offset, true); dv.setUint32(binAt + 4, BIN_CHUNK, true);
  let at = binAt + 8;
  for (const c of chunks) { out.set(c, at); at += c.byteLength; }
  return out;
}

/** One line per body: the counts the CLI prints. */
export function summary(baked: Pick<Baked, "bodies">): string[] {
  return baked.bodies.map((b) => `${b.name.padEnd(14)} ${(b.closed ? "solid" : "sheet").padEnd(7)} ${String(vertexCount(b)).padStart(7)} vertices ${String(triangleCount(b)).padStart(7)} triangles${b.paint.kind === "none" ? `  unpainted: ${b.paint.why}` : ""}`);
}

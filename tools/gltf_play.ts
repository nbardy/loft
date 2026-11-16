// Reads a .glb that `loft bake --animated` wrote and plays it on the CPU, from the file's bytes alone (no Loft types):
// the second reader of the format, so the writer cannot agree with itself by construction. `structure` lists what a
// viewer would trip on (counts, joint indices, weight sums, key counts); `pose` is every mesh's points at time t, skinned
// or morphed as a glTF viewer plays LINEAR keys.

type Json = {
  nodes: { name?: string; mesh?: number; skin?: number; children?: number[]; translation?: number[]; rotation?: number[]; scale?: number[] }[];
  meshes: { name?: string; weights?: number[]; primitives: { attributes: Record<string, number>; indices: number; targets?: Record<string, number>[] }[] }[];
  accessors: { bufferView: number; componentType: number; count: number; type: string; min?: number[]; max?: number[] }[];
  bufferViews: { byteOffset: number; byteLength: number }[];
  skins?: { joints: number[]; inverseBindMatrices: number; skeleton?: number }[];
  animations?: { channels: { sampler: number; target: { node: number; path: string } }[]; samplers: { input: number; output: number; interpolation: string }[] }[];
};
export type Glb = { readonly json: Json; readonly bin: DataView };

const WIDTH: Record<string, number> = { SCALAR: 1, VEC2: 2, VEC3: 3, VEC4: 4, MAT4: 16 };
const BYTES: Record<number, number> = { 5126: 4, 5125: 4, 5123: 2, 5121: 1 };

export function readGlb(bytes: Uint8Array): Glb {
  const dv = new DataView(bytes.buffer, bytes.byteOffset, bytes.byteLength);
  if (dv.getUint32(0, true) !== 0x46546c67) throw new Error("not a .glb");
  const jsonLen = dv.getUint32(12, true);
  const json = JSON.parse(new TextDecoder().decode(bytes.subarray(20, 20 + jsonLen))) as Json;
  const binAt = 20 + jsonLen;
  return { json, bin: new DataView(bytes.buffer, bytes.byteOffset + binAt + 8, dv.getUint32(binAt, true)) };
}

/** An accessor's values as numbers (tightly packed, as the writer packs them). */
export function read(g: Glb, id: number): number[] {
  const a = g.json.accessors[id]!, v = g.json.bufferViews[a.bufferView]!, n = a.count * WIDTH[a.type]!, size = BYTES[a.componentType]!;
  const at = (i: number) => v.byteOffset + i * size;
  switch (a.componentType) {
    case 5126: return Array.from({ length: n }, (_, i) => g.bin.getFloat32(at(i), true));
    case 5125: return Array.from({ length: n }, (_, i) => g.bin.getUint32(at(i), true));
    case 5123: return Array.from({ length: n }, (_, i) => g.bin.getUint16(at(i), true));
    case 5121: return Array.from({ length: n }, (_, i) => g.bin.getUint8(at(i)));
    default: throw new Error(`accessor ${id}: component type ${a.componentType}`);
  }
}

/** What a viewer would trip on, one line each; empty when the file holds together. */
export function structure(g: Glb): string[] {
  const out: string[] = [], j = g.json;
  j.accessors.forEach((a, i) => {
    const v = j.bufferViews[a.bufferView]!, want = a.count * WIDTH[a.type]! * BYTES[a.componentType]!;
    if (v.byteLength !== want) out.push(`accessor ${i}: ${v.byteLength} bytes for ${want}`);
  });
  j.meshes.forEach((m, k) => m.primitives.forEach((p) => {
    const count = j.accessors[p.attributes.POSITION!]!.count;
    for (const [name, id] of Object.entries(p.attributes)) if (j.accessors[id]!.count !== count) out.push(`mesh ${k} ${name}: ${j.accessors[id]!.count} for ${count} vertices`);
    for (const t of p.targets ?? []) if (j.accessors[t.POSITION!]!.count !== count) out.push(`mesh ${k}: a target of ${j.accessors[t.POSITION!]!.count} for ${count}`);
    if ((p.targets?.length ?? 0) !== (m.weights?.length ?? 0)) out.push(`mesh ${k}: ${p.targets?.length ?? 0} targets, ${m.weights?.length ?? 0} weights`);
    if (p.attributes.JOINTS_0 === undefined) return;
    const node = j.nodes.find((n) => n.mesh === k);
    const skin = node?.skin === undefined ? undefined : j.skins?.[node.skin];
    if (skin === undefined) { out.push(`mesh ${k}: JOINTS_0 without a skin`); return; }
    if (j.accessors[skin.inverseBindMatrices]!.count !== skin.joints.length) out.push(`mesh ${k}: ${j.accessors[skin.inverseBindMatrices]!.count} inverse binds for ${skin.joints.length} joints`);
    const js = read(g, p.attributes.JOINTS_0), ws = read(g, p.attributes.WEIGHTS_0!);
    for (let v = 0; v < count; v++) {
      const sum = ws[4 * v]! + ws[4 * v + 1]! + ws[4 * v + 2]! + ws[4 * v + 3]!;
      if (Math.abs(sum - 1) > 2e-6) { out.push(`mesh ${k} vertex ${v}: weights sum to ${sum}`); break; }
      if ([0, 1, 2, 3].some((c) => js[4 * v + c]! >= skin.joints.length)) { out.push(`mesh ${k} vertex ${v}: joint past ${skin.joints.length}`); break; }
    }
  }));
  for (const an of j.animations ?? []) for (const ch of an.channels) {
    const s = an.samplers[ch.sampler]!, keys = j.accessors[s.input]!.count, outA = j.accessors[s.output]!;
    const per = ch.target.path === "weights" ? (j.meshes[j.nodes[ch.target.node]!.mesh!]!.weights?.length ?? 0) : 1;
    if (outA.count !== keys * per) out.push(`channel ${ch.target.path} of node ${ch.target.node}: ${outA.count} values for ${keys} keys x ${per}`);
    const t = read(g, s.input);
    if (t.some((x, i) => i > 0 && !(x > t[i - 1]!))) out.push(`channel ${ch.target.path} of node ${ch.target.node}: key times not increasing`);
  }
  return out;
}

// ---- playing ----

type M4 = Float64Array;
function trs(t: readonly number[], r: readonly number[], s: readonly number[]): M4 {
  const [x, y, z, w] = r as [number, number, number, number], m = new Float64Array(16);
  const R = [1 - 2 * (y * y + z * z), 2 * (x * y + z * w), 2 * (x * z - y * w), 2 * (x * y - z * w), 1 - 2 * (x * x + z * z), 2 * (y * z + x * w), 2 * (x * z + y * w), 2 * (y * z - x * w), 1 - 2 * (x * x + y * y)];
  for (let c = 0; c < 3; c++) for (let row = 0; row < 3; row++) m[4 * c + row] = R[3 * c + row]! * s[c]!;
  m.set([t[0]!, t[1]!, t[2]!, 1], 12);
  return m;
}
function mul(a: M4, b: M4): M4 {
  const m = new Float64Array(16);
  for (let c = 0; c < 4; c++) for (let r = 0; r < 4; r++) { let s = 0; for (let k = 0; k < 4; k++) s += a[4 * k + r]! * b[4 * c + k]!; m[4 * c + r] = s; }
  return m;
}
const IDENTITY = trs([0, 0, 0], [0, 0, 0, 1], [1, 1, 1]);

/** A channel's value at t: LINEAR, with slerp for rotations; held at the ends. */
function sample(g: Glb, input: number, output: number, width: number, t: number, slerp: boolean): number[] {
  const times = read(g, input), values = read(g, output), n = times.length;
  const at = (i: number) => values.slice(i * width, i * width + width);
  if (t <= times[0]!) return at(0);
  if (t >= times[n - 1]!) return at(n - 1);
  let i = 0;
  while (times[i + 1]! < t) i++;
  const a = (t - times[i]!) / (times[i + 1]! - times[i]!), p = at(i), q = at(i + 1);
  if (!slerp) return p.map((x, k) => x + (q[k]! - x) * a);
  const d = p.reduce((s, x, k) => s + x * q[k]!, 0);
  if (d > 0.9995) { const l = p.map((x, k) => x + (q[k]! - x) * a), n2 = Math.hypot(...l); return l.map((x) => x / n2); }
  const th = Math.acos(Math.min(1, d)), sn = Math.sin(th);
  return p.map((x, k) => (Math.sin((1 - a) * th) * x + Math.sin(a * th) * q[k]!) / sn);
}

/** Every mesh node's points in the world at time t (one Float32Array per mesh, in mesh order). */
export function pose(g: Glb, t: number): Float32Array[] {
  const j = g.json, local: { t: number[]; r: number[]; s: number[] }[] = j.nodes.map((n) => ({ t: n.translation ?? [0, 0, 0], r: n.rotation ?? [0, 0, 0, 1], s: n.scale ?? [1, 1, 1] }));
  const weights = j.meshes.map((m) => m.weights?.slice() ?? []);
  for (const an of j.animations ?? []) for (const ch of an.channels) {
    const s = an.samplers[ch.sampler]!, node = ch.target.node;
    switch (ch.target.path) {
      case "translation": local[node]!.t = sample(g, s.input, s.output, 3, t, false); break;
      case "rotation": local[node]!.r = sample(g, s.input, s.output, 4, t, true); break;
      case "scale": local[node]!.s = sample(g, s.input, s.output, 3, t, false); break;
      case "weights": { const m = j.nodes[node]!.mesh!; weights[m] = sample(g, s.input, s.output, weights[m]!.length, t, false); break; }
    }
  }
  const parent = new Map<number, number>();
  j.nodes.forEach((n, i) => n.children?.forEach((c) => parent.set(c, i)));
  const world = (i: number): M4 => { const m = trs(local[i]!.t, local[i]!.r, local[i]!.s), p = parent.get(i); return p === undefined ? m : mul(world(p), m); };
  return j.meshes.map((m, k) => {
    const prim = m.primitives[0]!, node = j.nodes.findIndex((n) => n.mesh === k);
    const p = read(g, prim.attributes.POSITION!);
    (prim.targets ?? []).forEach((tg, i) => { const w = weights[k]![i]!; if (w !== 0) read(g, tg.POSITION!).forEach((d, v) => { p[v]! += w * d; }); });
    const out = new Float32Array(p.length);
    const skinId = j.nodes[node]!.skin;
    if (skinId === undefined) {
      const M = node >= 0 ? world(node) : IDENTITY;
      for (let v = 0; v < p.length; v += 3) for (let r = 0; r < 3; r++) out[v + r] = M[r]! * p[v]! + M[4 + r]! * p[v + 1]! + M[8 + r]! * p[v + 2]! + M[12 + r]!;
      return out;
    }
    const skin = j.skins![skinId]!, ibm = read(g, skin.inverseBindMatrices);
    const C = skin.joints.map((jn, q) => mul(world(jn), Float64Array.from(ibm.slice(16 * q, 16 * q + 16))));
    const js = read(g, prim.attributes.JOINTS_0!), ws = read(g, prim.attributes.WEIGHTS_0!);
    for (let v = 0; v < p.length / 3; v++) for (let c = 0; c < 4; c++) {
      const w = ws[4 * v + c]!;
      if (w === 0) continue;
      const M = C[js[4 * v + c]!]!;
      for (let r = 0; r < 3; r++) out[3 * v + r]! += w * (M[r]! * p[3 * v]! + M[4 + r]! * p[3 * v + 1]! + M[8 + r]! * p[3 * v + 2]! + M[12 + r]!);
    }
    return out;
  });
}

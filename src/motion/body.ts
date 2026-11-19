import { SceneError } from "../errors.ts";

export type V3 = readonly [number, number, number];
export const foot = (at: V3) => ({ kind: "foot" as const, at });
type Chain = { root: V3; lengths: V3; end: ReturnType<typeof foot> };
type Body = { trunk: { size: V3; height: number }; chains: readonly Chain[] };

/** Same-side adjacent roots plus the closest opposite root reflected across the midline. */
export function neighbours(chains: readonly Chain[]) {
  const masks = chains.map(() => 0);
  const join = (a: number, b: number) => { masks[a]! |= 1 << b; masks[b]! |= 1 << a; };
  const sides = [-1, 1].map(sign => chains.map((c, i) => ({ c, i }))
    .filter(({ c }) => Math.sign(c.root[2]) === sign).sort((a, b) => a.c.root[0] - b.c.root[0]));
  for (const side of sides) side.slice(1).forEach((e, i) => join(e.i, side[i]!.i));
  for (const { c, i } of sides[0]!) {
    const distance = (o: Chain) => Math.hypot(c.root[0]-o.root[0], c.root[1]-o.root[1], c.root[2]+o.root[2]);
    const mirror = [...sides[1]!].sort((a, b) => distance(a.c)-distance(b.c))[0];
    if (mirror) join(i, mirror.i);
  }
  return masks;
}

export function body(spec: Body, edges = neighbours(spec.chains)) {
  const {trunk, chains} = spec;
  const refuse = (why: string): never => { throw new SceneError("body", why, "finite dimensions and 1–8 reachable three-link chains", "adjust the body rows or supply another end solver"); };
  if (!chains.length || chains.length > 8 || edges.length !== chains.length) refuse("unsupported chain count");
  if (![...trunk.size,trunk.height].every(x=>Number.isFinite(x) && x>0)) refuse("invalid trunk dimensions");
  const plan = new Float32Array(26*4), state = new Float32Array(42*4);
  plan.set([...trunk.size,chains.length,trunk.height,0.6,1.3,0.25]);
  chains.forEach(({root,lengths,end},i)=>{
    if (![...root,...end.at].every(Number.isFinite) || !lengths.every(x=>Number.isFinite(x)&&x>0) || end.at[1]!==0) refuse(`invalid chain ${i}`);
    const distance=Math.hypot(root[0]-end.at[0],trunk.height+root[1]-end.at[1],root[2]-end.at[2]);
    if (distance>=lengths.reduce((a,b)=>a+b) || distance<1e-5) refuse(`unreachable rest site ${i}`);
    const mask=edges[i]!;
    if (!Number.isInteger(mask) || mask<0 || mask>=(1<<chains.length) || (mask&(1<<i))!==0) refuse(`invalid neighbours of chain ${i}`);
    plan.set([...root,0,...end.at,0,...lengths,mask],8+12*i);
    state.set([...end.at,-1,...end.at,0.12,...end.at,0.24,0,0,1/chains.length,0],40+16*i);
  });
  return { ...spec, plan, state };
}

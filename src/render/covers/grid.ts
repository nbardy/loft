import { SceneError } from "../../errors.ts";
import type { Source } from "../../record/passes.ts";

/** Grid vertex i lies at (i % (u+1), i / (u+1)). Only the index table has alignment padding. */
export function grid(u: number, v: number) {
  if (!Number.isSafeInteger(u) || !Number.isSafeInteger(v) || u < 1 || v < 1 || 6 * u * v > 0xffffffff)
    throw new SceneError("grid: use positive whole cell counts whose triangle index count fits u32");
  const words: number[] = [];
  for (let y = 0; y < v; y++)
    for (let x = 0; x < u; x++) {
      const a = x + y * (u + 1),
        c = a + u + 1;
      words.push(a, a + 1, c, c, a + 1, c + 1);
    }
  const indices = words.length;
  while (words.length % 4) words.push(0);
  return { indices, table: { lanes: 1, words } };
}

/** 'copies()' gives the candidate count and 'level(i)' chooses a cover (0 hides it).
 * Each entry writes an indexed indirect command in words 0..4 followed by original copy ids at word 8.
 * One group scans 64 copies at a time. The output ordering is independent of lane scheduling. */
export function compact(law: string, indexCounts: readonly number[]): Source {
  if (!indexCounts.length || indexCounts.some((n) => !Number.isInteger(n) || n < 1 || n > 0xffffffff))
    throw new SceneError("compact: give the index count of each nonempty cover, in level order starting at 1");
  const entries = indexCounts.map(
    (indices, i) => `
@compute @workgroup_size(64) fn level_${i + 1}(@builtin(local_invocation_index) lane:u32) {
  compact_level(lane,${i + 1}u,${indices}u);
}`,
  );
  return {
    opens: [],
    text:
      law +
      `
@group(2) @binding(0) var<storage,read_write> compacted:array<u32>;
var<workgroup> prefix:array<u32,64>;
var<workgroup> used:u32;
fn compact_level(lane:u32,want:u32,indices:u32) {
  if lane==0 { used=0; }
  workgroupBarrier();
  let n=copies();
  for(var block=0u;block<n;block+=64u) {
    let i=block+lane;var take=0u;
    if i<n {take=u32(level(i)==want);}
    prefix[lane]=take;
    workgroupBarrier();
    for(var stride=1u;stride<64u;stride*=2u) {
      var left=0u;if lane>=stride {left=prefix[lane-stride];}
      workgroupBarrier();prefix[lane]+=left;workgroupBarrier();
    }
    if take!=0u {compacted[8u+used+prefix[lane]-1u]=i;}
    workgroupBarrier();
    if lane==0 {used+=prefix[63];}
    workgroupBarrier();
  }
  if lane==0 {
    compacted[0]=indices;compacted[1]=used;compacted[2]=0;compacted[3]=0;compacted[4]=0;
  }
}` +
      entries.join("\n"),
  };
}

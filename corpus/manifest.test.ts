import { expect, test } from "bun:test";
import { execFileSync } from "node:child_process";
import corpus from "./manifest.json";
import parity from "../tools/parity/corpus.json";

test("the four corpus lanes partition all R0 examples and both seed sets", () => {
  const scenes = corpus.batches.flatMap(b => b.scenes);
  expect(scenes.length).toBe(295);
  expect(new Set(scenes).size).toBe(295);
  expect(corpus.batches.length).toBe(20);
  corpus.batches.forEach((batch, i) => {
    expect([batch.batch, batch.lane, batch.scenes.length]).toEqual([
      i + 1, ["l5c", "l5d", "l5e", "l5f"][Math.floor(i / 5)], i === 19 ? 10 : 15,
    ]);
  });
  const examples = execFileSync("git", ["ls-tree", "-r", "--name-only", corpus.examples_rev, "examples"],
    { cwd: import.meta.dir + "/..", encoding: "utf8" }).split("\n").filter(s => s.endsWith(".ts")).sort();
  expect(examples.length).toBe(125);
  expect(scenes.slice(0, 125)).toEqual(examples);
  for (const { set, rows } of parity.seeds.sets) {
    expect(scenes.filter(s => s.startsWith(set + "/")).length).toBe(rows);
  }
});

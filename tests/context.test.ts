// `loft context` is read whole before every scene, so its size is a cost paid on every sample. The bug this catches: a
// new export, a longer doc line or a new prelude function growing the print past its budget unnoticed (the API map and
// the text names are generated, so nobody edits them to see the growth). 2,850 words: 2,495 before the text-first
// rewrite (18 Nov 2026), plus the WGSL subset and the prelude it must now teach, less the prose text replaced.
import { expect, test } from "bun:test";
import { context } from "../src/cli/context.ts";

const BUDGET_WORDS = 2850;

test("loft context fits its word budget and teaches WGSL text", () => {
  const text = context();
  const words = text.split(/\s+/).filter(Boolean).length;
  expect(words).toBeLessThanOrEqual(BUDGET_WORDS);
  // The generated lines are present (a marker dropped from docs/context.md would throw, but a line printed empty would not).
  expect(text).toMatch(/^Builtins \(WGSL's meaning\): abs, /m);
  expect(text).toMatch(/^Prelude \(.*\): mod, pick, .*noise3.*; structs PaintAt \{ world, normal, copy \}/m);
});

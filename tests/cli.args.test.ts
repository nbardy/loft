import { expect, test } from "bun:test";
import { parseArgs } from "../src/cli/args.ts";

test("render defaults and repeated --param", () => {
  const c = parseArgs(["render", "a.ts", "--out", "o.webp", "--param", "x=1.5", "--param", "tube.girth=2"]);
  expect(c).toMatchObject({ kind: "render", views: ["front", "side", "top", "three"], time: 1, size: { width: 1280, height: 800 }, params: { x: 1.5, "tube.girth": 2 } });
});

test.each([
  [["render", "a.ts", "--out", "o.webp", "--bogus"], "--bogus"],
  [["render", "a.ts", "--out", "o.webp", "--size", "big"], "--size"],
  [["render", "a.ts", "--out", "o.webp", "--param", "nope"], "--param"],
  [["render", "a.ts", "--out", "o.png"], "--out"],
  [["clip", "a.ts", "--out", "o.mp4", "--fps", "0"], "--fps"],
  [["clip", "a.ts", "--out", "o.mp4", "--time", "1"], "--time"],
  [["frames", "a.ts", "--out", "o.webp"], "--times"],
  [["frames", "a.ts", "--out", "o.webp", "--times", "0,x"], "--times"],
  [["render", "--out", "o.webp"], "scene"],
  [["spin", "a.ts"], "spin"],
  [["dev", "a.ts", "--port"], "--port"],
])("%j is rejected naming %s", (argv, name) => {
  expect(() => parseArgs(argv)).toThrow(name);
});

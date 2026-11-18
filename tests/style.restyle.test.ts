// `--style toon` on a scene whose own style is toon with another outline (src/cli/load.ts `restyle`). Bug this catches:
// params were interned process-wide by name, so toon()'s `toon.outline` at 0.04 was refused against the scene's own,
// replaced toon at 0.03 ("param declared twice"). A param is now one scene's declaration, as an input is.
import { expect, test } from "bun:test";
import heron from "../examples/light/heron_lamp.ts";
import { compileScene } from "../src/compile/index.ts";
import { param } from "../src/index.ts";
import { toon, withStyle } from "../src/style/index.ts";

test("a style swapped in with another default for its param compiles, and one scene's two defaults are refused", () => {
  const c = compileScene(withStyle(heron, toon()));
  expect(c.artefact.params.find((p) => p.name === "toon.outline")?.def).toBe(0.04);
  const style = toon();
  const twice = withStyle(heron, { ...style, outline: param("toon.outline", 0.07), mark: () => param("toon.outline", 0.05) });
  expect(() => compileScene(twice)).toThrow(/declared twice in one scene/);
});

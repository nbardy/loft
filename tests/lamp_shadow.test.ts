// A lamp's shadow map (DESIGN.md 17), compile only. Catches: a scene compiled twice in a process making the pass's
// library function twice (bun threw "defined twice" in the bench and would in `loft dev`), and a lamp among its
// casters compiling into a picture that cannot hold them.
import { expect, test } from "bun:test";
import { compileScene } from "../src/compile/index.ts";
import { lamp, plane, scene, strand, toon, vec3 } from "../src/index.ts";
import { SceneError } from "../src/errors.ts";

const post = strand({ name: "post", spine: (m) => vec3(0, m.mul(2), 0), girth: 0.1, tips: "round", paint: [0.7, 0.6, 0.5] });
const lit = (at: [number, number, number]) => scene({ ground: plane, things: [post], style: toon(), sun: [-0.4, 0.8, 0.3], lights: [lamp(at, { colour: [1, 1, 1], reach: 2, shadow: true })] });

test("a shadowed lamp's scene compiles twice in one process to the same pass", () => {
  const s = lit([1.5, 4, 0]);
  const a = compileScene(s).artefact, b = compileScene(s).artefact;
  expect(a.passes?.map((p) => p.name)).toEqual(b.passes?.map((p) => p.name));
  expect(a.passes?.length).toBe(1);
});

test("a lamp its casters surround is refused by name", () => {
  expect(() => compileScene(lit([0, 1, 0]))).toThrow(SceneError);
  expect(() => compileScene(lit([0, 1, 0]))).toThrow(/spread .* degrees/);
});

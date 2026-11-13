// A strand written without paint, as a model reading `loft context` might: a scene refusal naming the fix, not a crash.
import { add, mul, plane, scene, strand, toon, vec3, type F } from "../../../src/index.ts";

export default scene({
  ground: plane,
  // @ts-expect-error: paint is required; this fixture is the mistake
  things: [strand({ name: "bare", spine: (m: F) => vec3(0, mul(m, 3), 0), girth: (m: F) => add(0.3, mul(m, -0.2)), stations: 8, detail: { along: 16, around: 8 } })],
  style: toon(),
  sun: [-0.4, 0.8, 0.45],
});

// mistake: a strand with 2 stations; a strand needs at least 4 transported frames
import { mix, scene, strand, vec3 } from "../../../src/index.ts";
import { cameras, style, water } from "./_parts.ts";

const stalk = strand({
  name: "stalk", spine: (m) => vec3(0, mix(0.5, 2.5, m), 0), stations: 2,
  girth: 0.1, tips: ["round", "round"], paint: [0.3, 0.6, 0.2], detail: { along: 12, around: 10 },
});
export default scene({ ground: water, things: [stalk], style, sun: [0.3, 0.8, 0.4], cameras });

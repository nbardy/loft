export { body, foot } from "./body.ts";
export { move } from "./move.ts";
export { roam } from "./roam.ts";
import hop from "./wgsl/plan.wgsl" with { type: "text" };
import gaze from "./wgsl/head.wgsl" with { type: "text" };
export { hop, gaze };

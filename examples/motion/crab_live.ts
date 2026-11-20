import { body, foot, move, roam } from "loft/motion";
import { input } from "../../src/expr.ts";
import { stage, stick } from "./stage.ts";

const legs = [[1.35,2.8,6.6],[0.5,1.1,7.4],[-0.4,-0.9,7.3],[-1.3,-2.7,6.4]] as const;
export const crab = body({
  trunk: { size:[4.8,1.05,5], height:2.5 },
  chains: [-1,1].flatMap(s=>legs.map(([x,fx,fz])=>({
    root:[x,0.1,2.5*s], lengths:[3,2.9,1.2], end:foot([fx,0,fz*s])
  }))),
});
export default stage(move(crab,roam(),{go:input("stick","vec2",[0,0]),script:stick}));

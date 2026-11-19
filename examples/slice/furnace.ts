import {scene,thing,type Scene} from "../../src/render/record.ts";
import {box,sphere} from "../../src/kinds/shapes.ts";
import {lamp,metal} from "../../src/kinds/materials.ts";
import {reference} from "../../src/engines/reference.ts";
const darkSun={towards:[0,1,0],power:[0,0,0]} as const;
export const closed:Scene={things:[thing(box([2,2,2]),lamp([0.5,0.5,0.5],[1,1,1]))],
  sun:darkSun,cameras:{front:{kind:"perspective",eye:[0,0,0],target:[0,0,-1],fovY:1}},engine:reference()};
export const white:Scene={things:[thing(sphere(1),metal([1,1,1],1))],sky:"fn sky(d:vec3f)->vec3f {return vec3f(0.5);}",
  sun:darkSun,cameras:{front:{kind:"orthographic",eye:[0,0,3],target:[0,0,0],height:2.2}},engine:reference()};
export default scene(closed);

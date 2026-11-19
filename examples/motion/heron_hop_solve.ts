// The heron's hop over the rail, solved (panel b of agent_notes/2026-11-19_motion.md): heron_leap.ts's motion, nothing
// keyed. Body x, y, pitch and the feet are `solve`d B-splines meeting the leap's holds (planted to 0.6 s, a parabola
// 0.6 to 1.2, both feet on the deck from 1.2, within reach, clear of the rail) with the least jerk.
import { leap, NUMBERS, stage } from "../second/heron_leap.ts";

export default stage(NUMBERS, leap(NUMBERS).bird.things);

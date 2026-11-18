// heron_leap.ts asked for the impossible: the rail raised to 3.2 m. Refused by `solve` (exit 4), naming the holds that
// conflict, each by its line in heron_leap.ts.
//
// Theory, before running (written for a rail at 3.6 m). The flight is fixed at 0.6 s between feet planted on the
// ground and feet on the deck, so the body's parabola rises only g T^2 / 8 = 0.44 m above the mean of its end heights,
// which are at most a leg's reach above the feet: the hips top out near 2.9 m. Predicted: no part clears a 3.6 m rail;
// refused, the conflict naming the flight, both plantings, the reach and one clearance of the rail.
//
// Scored: wrong, twice. At 3.6 m it solved: "clear of the rail" says nothing of which side, and the bird passed under
// the rail (holds only say what was written). At 3.2 m it is refused in 1.3 s, but the minimal conflict is two lines,
// not five: standing at rest at t = 0 (heron_leap.ts:71) and the bill's clearance (heron_leap.ts:83). The standing
// heron's bill is 3.2 to 3.3 m up at x 1.0 to 1.75, so it starts inside the rail; nothing about the flight is needed.
import { NUMBERS, sceneFor } from "./heron_leap.ts";

export default sceneFor({ ...NUMBERS, rail: { ...NUMBERS.rail, y: 3.2 } });

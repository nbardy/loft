// Checks layer: the measured verdicts on a built subject (reach, clearance, readability, budgets).
// Probes and checks run against what the other layers built.
export { clearance, landsInside, gripReaches, releasesAt, throwLands, fliesClear, letsGoClean, shoves, frontFirst, warned, tolerance, minScreenSize, litBy, budget, runChecks, formatChecks, Probe, type Check, type CheckTime, type Tolerance, type Verdict, type Measured } from "./check.ts";
export { expressionsRead, spansOnScreen, staysInView, limbsReach, type Mark, type Reaching } from "./check.ts";
// Ranged checks: a condition proved over a whole range of time or a param, not at sampled frames (src/ranged.ts).
export { always, eventually, type Ranged, type Range, type Settings as RangedSettings, type Decision } from "./ranged.ts";

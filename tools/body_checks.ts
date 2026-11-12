// A variant's checks with numbers overridden, for exploring a body plan before writing its numbers down (nothing is
// saved). Prints only what fails, with the reason, and the count.
//
//   bun tools/body_checks.ts <variant> '{"ride": 3.4, "legSize": {"length": [1.2, 1.2, 1.2, 1.2]}}'
import { formatChecks, runChecks } from "../src/check.ts";
import { design } from "../examples/bosses/reefback_titan.ts";

const [variant = "base", set = "{}"] = process.argv.slice(2);
const b = design.build(design.resolve({ variant, picks: [] }, JSON.parse(set)));
const verdicts = runChecks(b.scene, b.checks);
const lines = formatChecks(verdicts);
for (const l of (Array.isArray(lines) ? lines : String(lines).split("\n"))) if (/ fail /.test(l) || /FAIL/.test(l)) console.log(l.replace(/\s{2,}/g, "  ").slice(0, 330));
console.log(`${variant}: ${verdicts.filter((v) => v.pass).length} of ${verdicts.length} pass`);

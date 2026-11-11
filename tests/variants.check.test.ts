// Every named variant of a design passes the design's checks (docs/variants.md: the checks are the contract), run the
// way an artist runs them: `loft check <design>` with no --variant. It catches an option, a named variant or a rig edit
// that leaves some variant unable to land Claw Crush inside its warning or to close the cutter on the ship, and a
// variant that no longer builds (two things with one name, an option asking a number the design does not have).
import { expect, test } from "bun:test";

test("every named variant of the Reefback Titan passes its checks", () => {
  const r = Bun.spawnSync(["bun", "src/cli/main.ts", "check", "examples/bosses/reefback_titan.ts"], { stdout: "pipe", stderr: "pipe" });
  const out = r.stdout.toString();
  const rows = /variant\s+checks\n([\s\S]*?)\n\n(\d+) of (\d+) variants pass/.exec(out);
  expect(rows, `no summary in:\n${out}\n${r.stderr.toString()}`).not.toBeNull();
  for (const name of ["base", "stilt", "fiddler", "fiddlerBaseDisc", "mask"]) expect(rows![1]).toMatch(new RegExp(`^${name}\\s+4/4 pass$`, "m"));
  // The islander's face asks three more of itself (titan/face/dandy.ts): each eye's size from the game camera, and that
  // its expressions show and differ on screen.
  expect(rows![1]).toMatch(/^islander\s+7\/7 pass$/m);
  expect(rows![2]).toBe(rows![3]);
  expect(r.exitCode).toBe(0);
}, 600_000);

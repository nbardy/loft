// `loft check <scene.ts>`: a scene's checks (src/check.ts), run on the CPU against the scene they were built with,
// printed as a table. A scene with baked state (`recur`) is baked on the GPU first and also gets its seam and drift
// measured, with or without `checks` of its own; without a GPU it ends as an environment error (exit 6). A plain module: its exported `checks` against its default export. A design (docs/variants.md):
// the variant given, or else every named variant, or with `--combinations` every combination of slot options; one
// table per variant. Exits 5 when any check fails, or as the error of a variant that fails to build (README "Exit codes").
import type { Ranged } from "../ranged.ts";
import type { Command } from "./args.ts";
import { choose, subjectAt, type Chosen } from "./variant.ts";
import type { VariantAsk } from "../design.ts";
import { ChecksFailed } from "./verdict.ts";
import { UsageError } from "./args.ts";
import { LoadError } from "../errors.ts";

type Outcome = { readonly label: string; readonly about: string; readonly result: { readonly kind: "built"; readonly undecided: readonly string[]; readonly failed: number; readonly total: number; readonly failing: readonly string[] } | { readonly kind: "unbuilt"; readonly error: string; readonly thrown: unknown } };

export async function check(cmd: Extract<Command, { kind: "check" }>): Promise<void> {
  const subject = await subjectAt(cmd.path);
  const { runChecks, formatChecks, Probe } = await import("../check.ts");
  const { compileScene } = await import("../compile/index.ts");
  let device: GPUDevice | undefined;

  /** One variant's table; a variant that throws while it is built or compiled is a failure with its reason, and the rest go on. */
  const run = async (label: string, about: string, pick: () => Chosen): Promise<Outcome> => {
    const t = performance.now();
    try {
      const c = pick();
      const compiled = compileScene(c.scene);
      for (const n of compiled.artefact.notes ?? []) console.error(`note: ${n}`);
      const state = compiled.artefact.recurrences !== undefined;
      if (c.checks.kind === "absent" && !state) throw new LoadError(`${cmd.path} exports no \`checks\` array. Fix: export const checks = [...] beside the default export, e.g. export const checks = [budget({ triangles: 100_000 })]`);
      // Baked state is measured where it is drawn: baked on the GPU (a CPU bake of a chaotic step is another run).
      const list = c.checks.kind === "absent" ? [] : c.checks.list;
      // Ranged recurrence refusals need no bake. Print them before a mixed scene asks for a device.
      const ranged = state ? list.filter((check): check is Ranged => "formula" in check) : [];
      const probe = new Probe(compiled, {});
      const early = ranged.map((check) => check.run(probe));
      if (early.length) console.log(formatChecks(early));
      const remaining = state ? list.filter((check) => !("formula" in check)) : list;
      if (state && (remaining.length > 0 || c.scene.loop.kind === "every" || compiled.artefact.recurrences!.some((r) => r.kept.kind === "checkpoints"))) {
        const [{ headlessDevice }, { bakeRecurrences }] = await Promise.all([import("../host/headless.ts"), import("../host/bake.ts")]);
        device ??= await headlessDevice();
        await bakeRecurrences(device, compiled.artefact);
      }
      const rest = runChecks(c.scene, remaining, { compiled });
      const verdicts = [...early, ...rest];
      const failing = verdicts.filter((v) => v.holds === false).map((v) => v.name);
      const undecided = verdicts.filter((v) => v.holds === "undecided").map((v) => `${v.name} (${v.measured})`);
      if (rest.length || !early.length) console.log(formatChecks(rest));
      console.log(`\n${verdicts.length - failing.length - undecided.length} passed, ${failing.length} failed${undecided.length ? `, ${undecided.length} undecided` : ""}  (${(performance.now() - t).toFixed(0)} ms, including compile)`);
      return { label: c.label, about, result: { kind: "built", failed: failing.length, undecided, total: verdicts.length, failing } };
    } catch (e) {
      const error = e instanceof Error ? e.message : String(e);
      console.log(`NOT CHECKED: ${error}`);
      return { label, about, result: { kind: "unbuilt", error, thrown: e } };
    }
  };

  const outcomes: Outcome[] = [];
  const each = async (asks: readonly { readonly label: string; readonly about: string; readonly ask: VariantAsk }[]) => {
    for (const a of asks) {
      console.log(`\n== variant ${a.label}: ${a.about}`);
      outcomes.push(await run(a.label, a.about, () => choose(subject, cmd.path, { kind: "ask", ask: a.ask, text: a.label })));
    }
  };
  switch (subject.kind) {
    case "scene":
      if (cmd.combinations) throw new UsageError(`--combinations: ${cmd.path} exports a scene, not a design: it has no slot options to combine (drop --combinations)`);
      outcomes.push(await run("scene", "", () => choose(subject, cmd.path, cmd.variant)));
      break;
    case "design": {
      const d = subject.design;
      // Labelled from the picks, not resolved here: a combination whose options disagree on a number fails in its own row.
      if (cmd.combinations) await each(d.combinations().map((ask) => ({ label: ask.picks.length === 0 ? "base" : ask.picks.map(([s, o]) => `${s}=${o}`).join(","), about: "combination", ask })));
      else if (cmd.variant.kind === "ask") await each([{ label: cmd.variant.text, about: "as asked", ask: cmd.variant.ask }]);
      else await each(d.variants.map((v) => ({ label: v, about: d.about(v), ask: { variant: v, picks: [] } })));
      break;
    }
  }

  const bad = outcomes.filter((o) => o.result.kind === "unbuilt" || o.result.failed > 0 || o.result.undecided.length > 0);
  if (outcomes.length > 1) {
    console.log(`\nvariant${" ".repeat(34)}checks`);
    for (const o of outcomes) {
      const r = o.result;
      console.log(`${o.label.padEnd(40)} ${r.kind === "built" ? `${r.total - r.failed - r.undecided.length}/${r.total} pass${r.undecided.length ? `, ${r.undecided.length} undecided` : ""}` : `not checked: ${r.error.split("\n")[0]}`}`);
    }
    console.log(`\n${outcomes.length - bad.length} of ${outcomes.length} variants pass`);
  }
  // A variant that did not build ends the command as its error does (src/cli/verdict.ts); else failed checks.
  const unbuilt = outcomes.find((o) => o.result.kind === "unbuilt");
  if (unbuilt !== undefined && unbuilt.result.kind === "unbuilt") throw unbuilt.result.thrown;
  // The last line names each failing check, so a script (or a model) reads what failed without the table.
  const failures = bad.filter((o) => o.result.kind === "built" && o.result.failed > 0);
  const names = (which: "failing" | "undecided") => bad.flatMap((o) => o.result.kind === "built" ? o.result[which].map((v) => outcomes.length === 1 ? v : `${o.label}: ${v}`) : []).join(", ");
  const undecided = names("undecided");
  if (failures.length) throw new ChecksFailed(`${failures.length} of ${outcomes.length} ${outcomes.length === 1 ? "scene has" : "variants have"} failing checks: ${names("failing")}${undecided ? `; undecided: ${undecided}` : ""}`);
  if (undecided) throw new ChecksFailed(`undecided: ${undecided}`);
}

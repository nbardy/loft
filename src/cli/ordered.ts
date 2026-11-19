// Source and built native bundles use the same headless draw protocol. One request draws all times of a camera.
import { existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync } from "node:fs";
import { join, resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { type Command, type Source, UsageError } from "./args.ts";
import { canonical } from "../identity.ts";
import { type OrderedScene } from "../render/record.ts";
import { EnvironmentError, LoadError, SceneError } from "../errors.ts";
import { contactSheet, writeWebp, type Frame } from "./image.ts";
import { openVideo } from "./video.ts";

export const LOFT_DRAW = fileURLToPath(new URL("../../rust/target/lab/loft-draw", import.meta.url));

function scratch() {
  mkdirSync("out/native", { recursive: true });
  return mkdtempSync(resolve("out/native/job-"));
}

export async function orderedAt(source: Extract<Source, { kind: "file" }>) {
  const { subjectAt } = await import("./variant.ts");
  return orderedOf(await subjectAt(source.path), source);
}

/** The ordered scene a classified subject draws, with `--engine` applied; undefined for every other kind. */
export async function orderedOf(subject: import("../design.ts").Subject, source: Extract<Source, { kind: "file" }>) {
  if (subject.kind !== "ordered") {
    if (source.renderEngine)
      throw new UsageError("--engine applies to a scene from loft/render. Choose --style for a wire-4 scene");
    return;
  }
  if (source.variant.kind === "ask" || source.light.kind !== "scene" || source.style.kind !== "scene")
    throw new UsageError("ordered scene: choose the look and light in its engine");
  const name = source.renderEngine;
  if (!name) return subject.scene;
  const engines = await import("../render/index.ts");
  return { ...subject.scene, record: () => subject.scene.record(engines[name]()) };
}

export async function showOrdered(s: OrderedScene) {
  const dir = scratch();
  try {
    const file = join(dir, "scene.loft");
    const { buildOrdered } = await import("../core/ordered.ts");
    buildOrdered(s, file, { naga: "emit", precision: "f32" });
    const bytes = readFileSync(file);
    const end = 16 + Number(bytes.readBigUInt64LE(8));
    const head = JSON.parse(bytes.subarray(16, end).toString());
    let at = end;
    const sections: Buffer[] = head.sections.map((size: number) => {
      at = Math.ceil(at / 64) * 64;
      const text = bytes.subarray(at, at + size);
      at += size;
      return text;
    });
    return head.modules.map((m: { wgsl: number }, i: number) => {
      const passes = head.format.V2.passes.filter((p: { module: number }) => p.module === i);
      const entries = head.kernels.filter((k: { body: { Raw?: { module: number } } }) => k.body.Raw?.module === i);
      const label =
        [
          ...passes.map((p: { name: string }) => p.name),
          ...entries.map((k: { body: { Raw: { entry: number } } }) => head.names[k.body.Raw.entry]),
        ].join(" ") || "camera";
      const text = sections[m.wgsl].toString();
      return { label, text, lines: text.trimEnd().split("\n").length, sited: 0 };
    });
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

type PictureCommand = Extract<Command, { kind: "render" | "clip" | "frames" }>;
export async function drawNative(cmd: PictureCommand): Promise<boolean> {
  let source: OrderedScene | undefined;
  let bundle: string;
  switch (cmd.source.kind) {
    case "fixture":
      return false;
    case "artefact":
      bundle = resolve(cmd.source.dir, "scene.loft");
      if (
        !existsSync(bundle) ||
        (existsSync(resolve(cmd.source.dir, "manifest.json")) && process.env.LOFT_ENGINE !== "rust")
      )
        return false;
      break;
    case "file": {
      source = await orderedAt(cmd.source);
      if (!source) return false;
      bundle = "";
      break;
    }
  }
  if (cmd.still.kind === "still" || (cmd.kind === "render" && cmd.construction))
    throw new UsageError("native pass lists own sampling and geometry. Set these in the scene’s engine");
  if (cmd.kind !== "render" && cmd.sweep.kind !== "none")
    throw new UsageError("native frame sequences take fixed inputs. Supply the input in each draw request to vary it");
  if (!existsSync(LOFT_DRAW))
    throw new EnvironmentError(`build ${LOFT_DRAW} with tools/dev.sh build loft-draw --fast`, false);
  const dir = scratch();
  try {
    if (source) {
      const { buildOrdered } = await import("../core/ordered.ts");
      bundle = join(dir, "scene.loft");
      buildOrdered(source, bundle, { naga: "load", precision: cmd.precision ?? "f32" });
    }
    const { width, height } = cmd.size;
    const views = cmd.kind === "render" ? cmd.views : [cmd.view];
    const times =
      cmd.kind === "clip"
        ? Array.from({ length: Math.round(cmd.seconds * cmd.fps) }, (_, i) => i / cmd.fps)
        : cmd.kind === "frames"
          ? cmd.times
          : [cmd.time];
    const requests = views.map((name, i) =>
      canonical({
        bundle,
        camera: { kind: "named", name },
        times,
        size: [width, height],
        params: cmd.params,
        inputs: cmd.inputs,
        precision: cmd.precision ?? "f32",
        out: join(dir, String(i)),
      }),
    );
    const result = Bun.spawnSync([LOFT_DRAW, "serve"], { stdin: Buffer.from(requests.join("\n") + "\n") });
    if (result.exitCode !== 0) throw new EnvironmentError(result.stderr.toString(), false);
    const replies = result.stdout
      .toString()
      .trimEnd()
      .split("\n")
      .slice(1)
      .map((line) => JSON.parse(line));
    if (replies.length !== views.length)
      throw new EnvironmentError("loft-draw ended before returning all views", false);
    const files: string[] = [];
    for (const reply of replies) {
      if (!reply.ok) {
        const { kind, message } = reply.verdict;
        switch (kind) {
          case "usage":
            throw new UsageError(message);
          case "scene":
            throw new SceneError(message);
          case "load":
            throw new LoadError(message);
          default:
            throw new EnvironmentError(`loft-draw ${kind}: ${message}`, false);
        }
      }
      files.push(...reply.frames.map((f: { file: string }) => f.file));
    }
    if (cmd.kind === "clip") {
      const video = openVideo(cmd.out, width, height, cmd.fps, "av1");
      for (const file of files) await video.write(readFileSync(file));
      await video.finish();
    } else {
      const frames: Frame[] = files.map((file) => ({ width, height, rgba: readFileSync(file) }));
      writeWebp(cmd.out, contactSheet(frames), 95);
    }
    console.log(`wrote ${cmd.out}  ${files.length} frame(s)`);
    return true;
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
}

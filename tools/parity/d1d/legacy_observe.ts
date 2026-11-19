// Run from a scratch archive of the frozen legacy source. The legacy scene
// owns camera, animation, lighting and color. Add only a depth observation.
import { createHash } from "node:crypto";
import { mkdirSync, readFileSync, writeFileSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { pathToFileURL } from "node:url";

const [inventory, out, ...identities] = Bun.argv.slice(2);
if (!inventory || !out || identities.length === 0) {
  throw new Error("legacy_observe INVENTORY OUT EXAMPLE...");
}
const dir = import.meta.dir;
const fromArchive = (path: string) => import(pathToFileURL(join(dir, path)).href);
const { compile } = await fromArchive("src/index.ts");
const { headlessDevice } = await fromArchive("src/host/headless.ts");
const { createHost } = await fromArchive("src/host/host.ts");
const { encodePng } = await fromArchive("src/cli/image.ts");
const { loopTime } = await fromArchive("src/artefact.ts");
const { deviceOf } = await fromArchive("src/identity.ts");
const sha256 = (bytes: Uint8Array) => createHash("sha256").update(bytes).digest("hex");
const { rows, example_revision: revision } = JSON.parse(readFileSync(inventory, "utf8"));
const device = await headlessDevice();
const host = createHost(device, { precision: "f32" });

function depthObservation(plane: boolean) {
  const includePlane = plane ? `
    let dir = normalize(world - frame.eye.xyz);
    let ground = -frame.eye.y / dir.y;
    if ground > 0.0 { t = min(t, ground); }
  ` : "";
  return {
    label: "opaque distance witness",
    wgsl: `
      @fragment fn fs(in: PostIn) -> @location(0) vec4f {
        let px = in.px.xy;
        let world = scene_position(px);
        var t = select(1e30, length(world - frame.eye.xyz), scene_depth(px) < 1.0);
        ${includePlane}
        return vec4f(textureLoad(loft_post_colour, vec2i(px), 0).rgb, min(log2(1.0 + t) / 10.0, 1.0));
      }
    `,
  };
}

for (const identity of identities) {
  const row = rows.find((r: { id: string }) => r.id === identity);
  if (!row || row.source.path !== identity || row.source.revision !== revision || row.legacy_build.kind !== "built") {
    throw new Error(`no built standalone legacy scene for ${identity}`);
  }
  const source = resolve(dir, identity);
  if (sha256(readFileSync(source)) !== row.source.sha256) {
    throw new Error(`source identity differs: ${identity}`);
  }
  const { default: scene } = await fromArchive(identity);
  const ground = scene.ground.kind;
  if (ground !== "plane" && ground !== "nothing") {
    throw new Error(`ground ${ground} has no distance observation here: ${identity}`);
  }
  const style = { ...scene.style, post: [...(scene.style.post ?? []), depthObservation(ground === "plane")] };
  const artefact = compile({ ...scene, style });
  const loaded = await host.load(artefact);
  const observations = [];
  const shots = new Set<string>();
  try {
    for (const frame of row.archived_beauty) {
      if (frame.kind !== "drawn") continue;
      const [width, height] = frame.size;
      const key = `${frame.camera}@${frame.time}@${width}x${height}`;
      if (shots.has(key)) continue;
      shots.add(key);
      const camera = artefact.cameras[frame.camera];
      if (!camera) throw new Error(`missing legacy camera ${frame.camera}`);
      const time = loopTime(artefact.loop, frame.time);
      const rgba = await loaded.frame({ camera, width, height, time });
      const png = encodePng({ width, height, rgba });
      const file = join(out, identity, key + ".png");
      mkdirSync(dirname(file), { recursive: true });
      writeFileSync(file, png, { flag: "wx" });
      observations.push({
        camera: frame.camera, requested_seconds: frame.time, rendered_seconds: time,
        size: frame.size, original_beauty_sha256: frame.sha256,
        derived_file: file, png_sha256: sha256(png), rgba_sha256: sha256(rgba),
      });
    }
  } finally {
    loaded.destroy();
  }
  writeFileSync(join(out, identity, "observations.json"), JSON.stringify({
    identity, revision, source_sha256: row.source.sha256, device: deviceOf(device),
    executor: "frozen TS compiler and Dawn host",
    depth: ground === "plane" ? "minimum covered opaque distance and analytic plane" : "minimum covered opaque distance",
    recipe_sha256: sha256(readFileSync(import.meta.path)), provenance: "scratch observation",
    observations,
  }, null, 2) + "\n", { flag: "wx" });
  console.log(`${identity} ${observations.length} scratch observations`);
}
device.destroy();

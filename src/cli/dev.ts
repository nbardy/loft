// `loft dev`: bundle viewer/main.ts together with the scene for the browser, serve it, rebuild on change and tell
// the page over a WebSocket. The scene reaches the bundle through the virtual module "scene:current", so the viewer
// source never names a scene.
import { watch } from "node:fs";
import { dirname, resolve } from "node:path";
import type { Command } from "./args.ts";

const ROOT = resolve(import.meta.dir, "../..");

type DevCommand = Extract<Command, { kind: "dev" }>;
type Built = { readonly kind: "ok"; readonly js: string } | { readonly kind: "error"; readonly text: string };

/** Compile errors surface in the page, not at bundle time, so a scene that throws still loads the viewer. */
function sceneModule(cmd: DevCommand): string {
  const idx = JSON.stringify(resolve(ROOT, "src/index.ts"));
  switch (cmd.source.kind) {
    case "fixture":
      return `import { fixture } from ${JSON.stringify(resolve(ROOT, "src/host/fixture.ts"))};\nexport const sceneName = "fixture";\nexport const build = () => fixture;\n`;
    case "file":
      return `import scene from ${JSON.stringify(resolve(cmd.source.path))};\nimport { compile } from ${idx};\nexport const sceneName = ${JSON.stringify(cmd.source.path)};\nexport const build = () => compile(scene);\n`;
  }
}

async function bundle(cmd: DevCommand): Promise<Built> {
  const result = await Bun.build({
    entrypoints: [resolve(ROOT, "viewer/main.ts")],
    target: "browser",
    sourcemap: "inline",
    plugins: [{
      name: "loft-scene",
      setup(b) {
        b.onResolve({ filter: /^scene:current$/ }, () => ({ path: "scene:current", namespace: "loft" }));
        b.onLoad({ filter: /.*/, namespace: "loft" }, () => ({ contents: sceneModule(cmd), loader: "ts", resolveDir: ROOT }));
      },
    }],
    throw: false,
  });
  if (!result.success) return { kind: "error", text: result.logs.map((l) => l.message).join("\n") };
  return { kind: "ok", js: await result.outputs[0]!.text() };
}

export async function dev(cmd: DevCommand): Promise<void> {
  let built = await bundle(cmd);
  const sockets = new Set<{ send(s: string): void }>();
  const announce = () => {
    const msg = JSON.stringify(built.kind === "ok" ? { type: "reload" } : { type: "error", text: built.text });
    for (const s of sockets) s.send(msg);
  };

  const server = Bun.serve({
    port: cmd.port,
    fetch(req, srv) {
      const url = new URL(req.url);
      switch (url.pathname) {
        case "/": return new Response(Bun.file(resolve(ROOT, "viewer/index.html")));
        case "/style.css": return new Response(Bun.file(resolve(ROOT, "viewer/style.css")));
        case "/app.js":
          return built.kind === "ok"
            ? new Response(built.js, { headers: { "content-type": "text/javascript", "cache-control": "no-store" } })
            : new Response(`document.addEventListener("DOMContentLoaded",()=>{const e=document.getElementById("error");e.textContent=${JSON.stringify("build failed:\n" + built.text)};e.hidden=false;});`, { headers: { "content-type": "text/javascript" } });
        case "/favicon.ico": return new Response(null, { status: 204 });
        case "/ws": return srv.upgrade(req) ? undefined : new Response("expected a WebSocket", { status: 426 });
        default: return new Response("not found", { status: 404 });
      }
    },
    websocket: {
      open: (ws) => { sockets.add(ws); },
      close: (ws) => { sockets.delete(ws); },
      message() {},
    },
  });

  let timer: ReturnType<typeof setTimeout> | undefined;
  const rebuild = () => {
    clearTimeout(timer);
    timer = setTimeout(async () => {
      const t = performance.now();
      built = await bundle(cmd);
      console.log(`rebuild  ${(performance.now() - t).toFixed(0)} ms  ${built.kind === "ok" ? "ok" : "ERROR\n" + built.text}`);
      announce();
    }, 80);
  };
  const dirs = [resolve(ROOT, "src"), resolve(ROOT, "examples"), resolve(ROOT, "viewer")];
  if (cmd.source.kind === "file") dirs.push(dirname(resolve(cmd.source.path)));
  for (const d of new Set(dirs)) {
    try { watch(d, { recursive: true }, rebuild); } catch (e) { if ((e as NodeJS.ErrnoException).code !== "ENOENT") throw e; }
  }
  console.log(`loft dev  http://localhost:${server.port}/  (${cmd.source.kind === "file" ? cmd.source.path : "fixture"})  ctrl-c to stop`);
  await new Promise(() => {});
}

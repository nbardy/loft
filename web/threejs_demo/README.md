# Loft × Three.js

Three.js r180 draws the cube into the canvas and writes its depth. Loft attaches to
`renderer.backend.device`, loads the count-one Hermit bundle, and records its schedule on that
device. Its host present shader loads the canvas attachments, discards Vis misses and writes the
Loft hit depth. One perspective matrix and WebGPU's 0–1, less-is-nearer depth convention serve
both draws. Use **Cube depth** to compare front, intersecting and behind.

The **Sun** slider moves a Three.js directional light. Loft receives its normalized world-space
direction in a live vec4 parameter for Shade and uses the same direction to align its shadow
camera. The frame HUD reports CPU encoding and submission and the browser's frame interval.

## Run from the repo root

Use the repository's Rust toolchain, Chrome with WebGPU, pnpm and `wasm-bindgen-cli` matching
`wasm-bindgen` in `Cargo.lock` (0.2.130 on this branch).

```sh
mkdir -p web/threejs_demo/generated
CARGO_BUILD_JOBS=2 tools/dev.sh run loft-cli -- build stdlib/engines/hermit.loft --set count=1 -o web/threejs_demo/generated/hermit.loftb
CARGO_BUILD_JOBS=2 rustup run 1.99.1 cargo rustc --locked -p loft-wgpu --profile lab --target wasm32-unknown-unknown --features web-sys --lib --crate-type cdylib
wasm-bindgen --target web --out-dir web/threejs_demo/generated target/wasm32-unknown-unknown/lab/loft_wgpu.wasm
pnpm --dir web/threejs_demo install --frozen-lockfile
uv run --no-project python -m http.server 8080 --bind 127.0.0.1 --directory web/threejs_demo
```

Open `http://127.0.0.1:8080/` in Chrome. Keep the canvas at its bundle-defined 1280 × 800
backing resolution. The browser adapter supports the fixed schedule in this demo, and the shader
customization is local to it. Revisit these sources when changing Three revisions as the shared
canvas and depth handles come from Three's backend.

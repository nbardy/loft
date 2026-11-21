# Harbor style study

All seven styles use the same raster sail, SDF hulls, salt water, contact collars,
impact patch and small beacon at the `View` camera. The orange hulls share
the same pigment stripes, with roughness 0.72 on the left and 0.19 on the right.
The scene owns the timings and foam layers. The style owns the ink, and each
render module shows the chosen transport and post order.

`woodblock`, `toon` and `woodblock_restir` use the same print post.
`woodblock_reference` runs the same ink after averaging the path estimator's
incident light. Its thin trace sail follows the raster sail at the same
placement. `historical` uses the original `woodblock_pigment` schedule.

This revision uses std1b's noise and D's stage callbacks. D emits only its own
two payload structs. To run the study at D `c61c9ba`, use an isolated checkout
of that revision, apply B `bb1f16e`'s `stdlib/` diff, and copy this directory and
`stdlib/styles/` from std1c. From that checkout run:

```sh
uv run --no-project python scenes/showcase/std1c/d_overlay.py .
CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --bin loft
tools/gpu_lock.sh target/lab/loft render scenes/showcase/std1c/woodblock.loft \
  --out out/woodblock.png --size 384x240 --time 0
```

Use the other root scene filenames to select the look. The overlay specializes
only this lane's two value types and field accesses for D's emitter. The style
arithmetic, pinned B/D code and original ink stay in place. The harbor bridge
uses D's irradiance and an approximate sky gloss. Papercraft prepares the
normal in the estimator callback and repeats that calculation in the style
callback because D has no shared prepare result.

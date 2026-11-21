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

From the workspace root run:

```sh
CARGO_BUILD_JOBS=2 tools/dev.sh build loft-cli --fast --bin loft
tools/gpu_lock.sh target/lab/loft render scenes/showcase/std1c/woodblock.loft \
  --out out/woodblock.png --size 384x240 --time 0
```

Use the other root scene filenames to select the look. The Harbor bridge builds
the style records from the integrator's irradiance and an approximate sky gloss.
Papercraft prepares the normal in both the estimator and style callbacks.

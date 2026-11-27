# Moving-camera predictions before profiling

The measured two-second orbit keeps each fixture's target, eye height, horizontal distance and FOV. The stock orbit starts on +Z and advances 29.75 degrees in 120 frames at 60 Hz. The warmup uses the CLI's ordinary 30 frames. The prior same-resolution camera transfer and the forward/reverse hand guard supply the rates. The different azimuth is the largest extrapolation. Timing is immediate throughout. Record the first quiet run, or the first attempt with its flag after a contended retry.

Predicted GPU frame p50 / p95 in ms before timing the motion. For baselines, add the earlier non-Vis medians to the same-size Vis forecasts and allow 25% median-to-tail spread. For the guard, subtract the six per-field measured work reductions after scaling by the ratio of the same-size forecast to the first attributed frame. For the 1280 image, multiply by 1.48 (pixel area), ignoring the aspect change. The savings exclude the earlier retention result.

| Scene | Original 960 | Guard 960 | Original 1280 | Guard 1280 |
|---|---:|---:|---:|---:|
| Ball | 4.1 / 5.1 | 4.1 / 5.1 | — | — |
| Starter otter | 15.4 / 19.3 | 8.8 / 11.0 | — | — |
| Walrus | 27.2 / 34.0 | 19.0 / 23.8 | 40.3 / 50.3 | 28.1 / 35.2 |
| Heron | 27.8 / 34.8 | 4.0 / 5.0 | 41.1 / 51.5 | 5.9 / 7.4 |
| Final otter | 88.3 / 110.4 | 42.2 / 52.8 | 130.7 / 163.4 | 62.5 / 78.1 |

The production guard must match the corresponding orbit frames at every pixel. A smaller proxy experiment may change sampling and will be scored separately.

For the separate direct-proxy experiment at 960, predict another 2–5 ms off walrus, 1–3 off heron and 5–10 off otter frame p50 beyond the guard. The old interpolated ray and exact first march sample can both change. Predict at least one differing pixel in each moving clip. The probe fits only these fixtures' World queries and preserves the original shadow projection and shadow draw.

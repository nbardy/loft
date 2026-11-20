# Port1

The five cast rows drive a 64-count, 120 BPM loop with V, line, diagonal, canon, solos, slam and throw cues.
Wide and square cuts play on the native camera clock. Set `square=1` for square or `shot_time=SECONDS` to hold a lens.
`crab_song.ts` is audio. The port carries the dance score and leaves the audio out.

Crab Live uses the tagged stick cues and a 120 Hz fixed-point gait. Its instanced body and legs draw the sampled state.
CPU and Metal matched every packed row byte at 1, 2 and 4 ticks per submission through tick 480.
The lineup uses one shared baked pose table with spline interpolation in the renderer.

Hermit’s cream lip overhangs a dark throat, with the coil behind the body and three leg pairs coming forward.
The boats and detailed face and growth work in the tagged hermit reference remain distinct from this scene.

| Scene and sample | GPU p95 ms old → new | Modules old → new | Draws + dispatches old → new | Source tokens old → new |
| --- | ---: | ---: | ---: | ---: |
| Lineup button, 0 s, 1280×720 | 17.56 → 11.53 | 305 → 7 | 495 → 11 | 8,454 → 10,116 |
| Crab Live game, 1.6 s, 1280×800 | 9.05 → 15.62 | 5 → 6 | 4 → 9 | 2,373 → 2,528 |
| Hermit lowLeft, 1 s, 1280×800 | 16.58 → 11.68 | 84 → 6 | 144 → 9 | 26,642 → 7,605 |

Apple M4 Metal, 60 frames per sample under the GPU lock. OS contention was unmeasured.
Lineup and Hermit timings precede the horizon and sail polish. The Crab Live timing includes sampled motion.
Token counts use o200k_base over the scene and authored parts listed in `out/meta/port1/source-tokens.tsv`.
Five q95 WebP pairs in `out/meta/port1/approval/` cover button, throw, game, lowLeft and lowFront.
`tools/dev.sh check loft-cli --tests` and the two focused `motion_lane` tests passed.
Wrecker’s Lamp and Reefback Titan remain optional follow-up ports.

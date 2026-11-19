Pushed `loft/l5g` at `b87851b`. The compiler-free CPU loader repacked all 11 tested builds byte-for-byte, with a maximum warm load of **0.205 ms**. Both wasm32 checks passed, including `web-sys`.

Crab scramble replay had **0 differing bits after 481 ticks** in binary and JSON. The pin-drift replay exited `2`. [Fixture and lane notes](https://github.com/nbardy/loft/blob/b87851b/agent_notes/2026-11-20_v1/lanes/l5g.md)

Browser attachment compiled. Browser command encoding, device-side bundle loading, cross-device replay, and the regenerated corpus remain open.

Co-Authored-By: GPT (codex) <noreply@openai.com>
Implemented forward-Z and reverse-Z host rendering through `Runner::attach` and `encode_into`. Pushed `60ba7fb` to `loft/l5b`.

Type check, lane check, and all four CPU-only tests passed. The tested command path recorded zero caller clears, readbacks, or submissions. Naga validated the render shader.

Embedding uses base-format views, vertex pulling, and shaders that match the host’s depth convention. Live Keel and room-runner verification remains with the integrator. The render path extends the runner beyond its line budget. [Lane notes](https://github.com/nbardy/loft/blob/loft/l5b/agent_notes/2026-11-20_v1/lanes/l5b.md)

Co-Authored-By: GPT (codex) <noreply@openai.com>
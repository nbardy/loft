Pushed `1bb4fa4` to `loft/l4a`. It adds deterministic Bvh2 build and refit, CPU trace and visibility dispatch, and Coral Shrine with 3,069 analytic branches.

All five focused tests passed. The washer matched exactly **9,664** and **12,080** samples with zero shadow-mask differences. All four bore rays passed. Naga validated the traversal and scene shaders.

No GPU ran. Linking the BVH shader to the woodblock/reference pass and binding both scene snapshots remain integration work. [Lane notes](/Users/nicholasbardy/git/_wt/loft-l4a/agent_notes/2026-11-20_v1/lanes/l4a.md)

Co-Authored-By: GPT (codex) <noreply@openai.com>
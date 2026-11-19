You are an outside engineer evaluating a 3D rendering library you have never seen, at ~/git/loft, as a candidate
for a new project you are starting from scratch.

Rules: read CODE only — source (.ts, .rs, .wgsl), build and package files, tests and examples. Do NOT open any
Markdown or text documentation (*.md, *.txt, docs/, agent_notes/, READMEs, design notes) and do not trust comments
that make claims you cannot see in the code. Do not edit anything except your one output file.

Be exhaustive over the key code: the public API and how a user writes a scene, the compiler/IR, the TypeScript and
Rust engines (rust/), the GPU host and shaders, the examples, the tests. Work out from the code alone what the
library is for, how it works, how big it is (measure), what is solid, what is fragile, duplicated, dead or
over-engineered, how fast it is likely to be on the GPU and to build, how hard it is to learn, and what is missing.

Then answer plainly: would you use this for a new project, for what kind of project, and why or why not? Compare it
honestly with what you would otherwise use (three.js, Babylon, Bevy, raw wgpu, shader toys, etc.). Name the top
five things that would change your answer.

Write your review to OUTPUT (3,000-6,000 words, specific file:line evidence). Final message: a 200-word verdict.

<!--
The written half of `loft context` (src/cli/context.ts). OUTLINE, committed before the text (theory first).

Reader: a model (or a person) about to write Loft code, reading the whole output once, in its context window.
Size: about 8,000 words in all. Why: the catalogue alone is about 3,000 words if every exported value keeps its first
sentence (381 values; types by name only), and the written craft must be long enough to change what gets built
(examples pull hard; a one-line "try variety" does not). Above about 9,000 words a model starts skimming; below about
6,000 the craft sections become slogans.

Sections, in order; G = generated at run time, W = written here.

1. what      (W, 400)   What Loft is in six lines; the smallest complete scene (runnable); how to look at it.
2. layers    (W 600 + G 3,000)  The layers to reach for (expressions -> maps -> drawables -> shapes and motion ->
             actions and checks -> styles -> scene, compile, host), what draws a thing (the renderer, verified against
             src/draw), what a user can bring (Drawable, wgslFn, a style and post passes, a host from the files).
             Then the catalogue: per module, its line, each value with its first sentence and "cannot" lines, types by
             name; then src/lib (names only); then the undocumented list.
3. range     (W 1,300)  How to get range, not just accuracy. Far-apart candidates first; change the law, not the
             number (spiralTube's relatives as the model); a target from outside the library; do not copy the nearest
             example's proportions; vary style by composing and writing parts; one contact sheet; the pull of examples
             and presets and the ways out.
4. motion    (W 800)    Rich animation, each lesson with its function.
5. shapes    (W 600)    Shapes that read.
6. loop      (W 300 + G 500)  Predict, render once, score; the commands (USAGE from args.ts, generated); checks; budgets.
7. limits    (W 400)    What Loft cannot do today.

`loft context <section>` prints one section; `loft context components` prints the full catalogue (every type with its
sentence, every cannot line), which is also docs/components.md.
-->

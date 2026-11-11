Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to explore lighting and shading treatments over a render of a 3D model. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. The built-in tool accepts at most five reference images per call: `in/` holds two, pass only those. If the tool fails on one picture, say so and go on to the next; do not stop.

Pictures in `in/`:
- `in/current_3d_render.png`: two renders of our 3D sea-monster crab today (left: from the game's high camera; right: a lower three-quarter view). It is drawn as a woodblock print with flat colour and black key lines. The owner likes the cel-shaded look but finds it too flat: forms do not turn, parts do not sit on each other, nothing feels lit.
- `in/concept_style.png`: a concept drawing of the same character, for the drawing style and line quality.

Keep in every picture: the exact composition, camera, pose, colours and every shape of the right-hand render (the lower three-quarter view). Change ONLY the lighting and shading treatment. The sun is high and to the left. Everything must remain a print: flat areas with hard edges, no soft airbrush gradients, no photographic lighting, no glow or bloom.

## Pictures to make (each a paint-over of the right-hand render, landscape)
1. `shade_flat.png`: as it is now, redrawn cleanly: flat colour and key lines only. The baseline.
2. `shade_two_tone.png`: classic two-tone cel shading: every form gets one hard-edged core shadow shape in a darker flat tone of its own colour on the side away from the sun; cast shadows of the claw, legs and island onto the body and onto each other as flat dark shapes; a dark flat shadow of the whole creature on the water.
3. `shade_three_tone.png`: three tones: the two-tone above plus a small hard-edged highlight shape in a lighter flat tone on the sunward shoulder of each large form (claw, shell plates, leg segments), like a wet shell catching light.
4. `shade_bokashi.png`: woodblock bokashi: flat colour, with a graded wash only at the edges of large forms (the underside of the shell and claw darkening smoothly into indigo, the island's cliffs darkening toward the water), as a printer wipes ink on the block; no core shadows.
5. `shade_hatched.png`: shadow carried by line, not tone: the shadow side of each form filled with parallel printed hatching or pattern lines that follow the form, heavier key lines on the shadow side and thin ones on the lit side; colours stay flat.
6. `shade_contact.png`: flat colour plus only contact and occlusion shadows: a thin dark flat band wherever one part meets or overlaps another (leg into body, claw over shell, island on shell, eyes in sockets, creature on water); no directional light.
7. `shade_rim.png`: flat colour with a dark overall value and a thin bright cream rim of light along the sunward silhouette edges of every form, the shadow sides merging into indigo; a dramatic, back-lit print.
8. `shade_best.png`: your own recommendation: the combination of the above that makes this the most striking and readable boss at game size while still plainly a woodblock print.
9. `shade_best_game.png`: that same recommended treatment applied to the left-hand render (the game's high camera), same composition as the source.

Make each picture once.

## Final message
List the files you saved, with one line each on anything that drifted from the source render. Then at most six lines: which treatments you combined in the recommendation and why, written as instructions to a shader programmer.

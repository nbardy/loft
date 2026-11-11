Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to art-direct a 3D model by painting over renders of it. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. The built-in tool accepts at most five reference images per call: `in/` holds three, pass only those. If the tool fails on one picture, say so and go on to the next; do not stop.

Pictures in `in/`:
- `in/current_3d_turnaround.png`: four renders of our 3D sea-monster crab today, from four sides. It carries an island on its back with a small lighthouse on a rocky point.
- `in/current_3d_far_back_game_face.png`: four more renders: far, from behind and above, from the game's high camera, and the face.
- `in/design_to_match.png`: a paint-over already made of the top-right panel of the turnaround, with the island redesigned: three broad red shell plates sweep up into terraces and cliffs with pale stone strata between them, two groves of trees on a green crown, a larger leaning lighthouse on a barnacled point, coral, mussels, kelp and barnacles in clusters at the low seams, a crescent beach, three tapered sea stacks. THIS is the island design; every picture below that shows the redesigned island must match it.

## The style (use in every prompt)
Japanese ukiyo-e woodblock print. Bold black key lines of varied weight round every form, flat colour with no gradients, a limited palette of deep indigo sea, cream paper, brick red, moss green, coral pink, pale stone and black, pattern lines instead of shading, cream foam with curled fingers where it breaks the water. Shapes must be simple enough to build as 3D geometry with flat paint: large clear forms first, detail in clusters, nothing that depends on painterly texture. No text or labels in any picture.

## Keep
The crab itself (brick red, one giant claw shading to cream, eight legs, low in the water), its pose and the camera angle of each render, the sea, the boats. A lighthouse on a point at one side of the island must remain, and may be larger and more characterful.


## Keep
The crab itself (brick red, one giant claw shading to cream, eight legs, low in the water), its pose and the camera angle of each source render, the sea, the boats.

## Pictures to make
1. `back_paintover_side.png`: a paint-over of the bottom-right panel of `in/current_3d_turnaround.png`, the island redesigned to match `in/design_to_match.png`.
2. `back_paintover_game.png`: a paint-over of the third panel of `in/current_3d_far_back_game_face.png` (the game's high camera), the island redesigned to match; it must read clearly at this small size, lighthouse and point visible.
3. `back_model_sheet.png` (landscape): that island design as a clean model sheet on plain cream paper, the island alone on the shell drawn three times: from directly above (a plan), from the side (an elevation), and from the rear three-quarter; coast features clearly separated so a modeller can copy the outline.
4. `back_alternatives.png` (landscape): three DIFFERENT designs for the whole back, side by side as three equal panels, each the crab from behind and above: (a) a stepped shell-island, the shell's plates stepping up like rice terraces into a green crown; (b) a coral atoll, a ring reef with a lagoon in the middle and the lighthouse on the ring; (c) a single great barnacle-crusted spiral shell, like a hermit crab's, overgrown with turf and trees, the lighthouse growing from its tip.
5. `whole_paintover_game.png`: your own art direction over the game-camera render (third panel of `in/current_3d_far_back_game_face.png`), same camera and pose: everything you would change to make this a more striking boss at this size (silhouette, colour blocking, leg and claw shapes, pattern on the shell, the waterline foam), with the island as in the design.
6. `whole_paintover_front.png`: the same over the top-left panel of `in/current_3d_turnaround.png`, at close range.
7. `legs_and_claw.png`: a close-up of two legs and the giant claw redesigned with more character: joint shapes, plates, spines, barnacle clusters, pattern lines.

Make each picture once.

## Final message
List the files you saved, with one line each on anything off model. Then at most eight lines on the changes you made in pictures 5 to 7 and why, written as instructions to a 3D modeller, most important first.

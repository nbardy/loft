Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to make model sheets for a 3D modeller. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. The built-in tool accepts at most five reference images per call: `in/` holds two, pass only those. If the tool fails on one picture, say so and go on to the next; do not stop.

Pictures in `in/`:
- `in/spiral_shell_pick.png`: THE design. A colossal hermit crab seen from behind and above: on its back one great spiral shell, banded brick red and cream, its whorl turning to a tight curl at the side; clusters of round cream barnacles strung along the bands; two small groves of round trees on patches of green turf growing on the shell; a red-brick lighthouse with a black lantern room standing on a barnacled base at the shell's high outer end, leaning; two eyes on ringed stalks; one giant claw raised. Every picture below is this same creature and this same shell.
- `in/current_3d_turnaround.png`: four renders of our 3D model today from four sides, for the crab's body, legs, claw, eyes on stalks, proportions and the cameras. Its back (a round island) is what the spiral shell replaces.

## The style (use in every prompt)
Japanese ukiyo-e woodblock print. Bold black key lines of varied weight round every form, flat colour with no gradients, a limited palette of deep indigo sea, cream paper, brick red, moss green and black, pattern lines instead of shading, cream foam where it breaks the water. Shapes simple enough to build as 3D geometry with flat paint. No text or labels.

## Pictures to make (landscape about 3:2)
1. `spiral_turnaround.png`: the whole creature with the spiral shell drawn four times in a row at the same size on plain cream paper: from the front, from the side with the curl toward us, from behind, and from high above at a three-quarter angle. Consistent in all four: where the whorl's curl sits (on the creature's left flank), how many bands, where the lighthouse stands, where the two groves are.
2. `spiral_shell_sheet.png`: the shell alone, off the crab, as a clean model sheet on plain cream paper: from directly above, from the left side (looking at the curl), from the right side, and from behind; the band edges, the opening where the crab's body enters, and the lighthouse's base clearly drawn.
3. `spiral_paintover_rear.png`: a paint-over of the top-right panel of `in/current_3d_turnaround.png` (same camera and pose): the round island replaced by the spiral shell.
4. `spiral_paintover_front.png`: a paint-over of the top-left panel of `in/current_3d_turnaround.png` (same camera and pose): the round island replaced by the spiral shell, the eyes on stalks in front of it.
5. `spiral_game.png`: the creature from far away and high above at a three-quarter angle, small in a wide indigo sea, one tiny sailing ship nearby: the spiral must read at this size as a spiral.
6. `spiral_details.png`: three close-ups in three panels: the tight curl at the centre of the whorl with barnacles along its bands; the lighthouse's barnacled base growing out of the shell's high end; the rim where the shell meets the crab's body, the body tucked inside the shell's opening.
7. `spiral_withdraw.png`: a storyboard of four frames left to right: the hermit crab pulls back into its shell to defend itself (eyes and legs drawing in, the giant claw closing the opening like a door), then bursts back out.

Make each picture once.

## Final message
List the files you saved, and one line per picture on anything off model. Nothing else.

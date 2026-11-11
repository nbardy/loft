Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to make character reference pictures for a 3D modeller. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. If the built-in tool is unavailable or fails, stop and say so in your final message; do not switch to another path.

Two pictures are in `in/` for you to look at first:
- `in/flux_faces_sheet.png`: twelve rough concept pictures of the character's face in six moods. Their head shape and expressions are what we want: a broad, heavy head with a thick brow ridge, big round eyes set in the head under the brows, and a wide mouth that opens. Ignore their diffusion artefacts (extra limbs, stray antennae, smeared claws).
- `in/current_3d_crab.png`: the 3D model today, bottom row ("ISLANDER"). Keep from it: the colours, the island on the back, the one giant claw, and the black handlebar moustache. Its eyes on tall stalks are what we are replacing.

If the tool accepts reference images, pass these so the pictures stay on model.

## The character (use this description in every prompt)
A colossal fiddler crab, a sea monster the size of an island, low in the water. Brick-red shell with darker red mottling. One gigantic claw as long as its body, brick red shading to cream at the fingers; the other claw small. On its back a green island with pale stone cliffs, a few round trees, and a small leaning lighthouse on a rocky point off to one side. Its head is a broad, heavy mass at the front of the shell: a thick overhanging brow ridge, two big round cream eyes with green irises and black pupils set in the head under the brows (not on stalks), a wide mouth with two cream plate-like mandibles that can clamp shut or gape open, a black handlebar moustache curling up at both ends over the mouth, and two long thin red antennae sweeping up and back from the top of the head. Temperament: vain, a show-off, a bully.

## The style (use in every prompt)
Japanese ukiyo-e woodblock print. Bold black key lines of varied weight round every form, flat colour with no gradients on the creature, a limited palette of deep indigo sea, cream paper, brick red, moss green and black, pattern lines instead of shading, cream foam with curled fingers where it breaks the water. Clean shapes that a modeller can read; no text or labels in the picture unless asked.

## Pictures to make
1. `turnaround.png` (wide, landscape): a model sheet of the head and front of the body in one neutral, haughty expression, drawn four times in a row at the same size on plain cream paper: straight from the front, three-quarter view, side view, and from high above at a three-quarter angle (the angle a top-down game camera sees it from). The same character in all four; consistent proportions.
2. `expressions_sheet.png` (wide, landscape): a model sheet of six front close-ups of the face on plain cream paper, in two rows of three, the same character in each, each with a clearly different expression: (a) posing, smug, one brow raised, eyes half lidded; (b) greedy, eyes wide and fixed on something to one side, mouth slightly open; (c) straining, brows clamped down, eyes squeezed to slits, mandibles clenched; (d) gleeful, eyes wide, mouth gaping in a laugh; (e) furious, brows in a deep V, eyes narrowed, mouth open in a roar; (f) wincing, both eyes screwed shut, mouth pulled into a grimace.
3. Six single close-ups, one per expression above, each a front three-quarter close-up of the face filling the picture with a strip of indigo sea and the island behind: `face_posing.png`, `face_greedy.png`, `face_straining.png`, `face_gleeful.png`, `face_furious.png`, `face_wincing.png`.
4. `game_view.png` (landscape): the whole creature seen from far away and high above at a three-quarter angle, small in a wide indigo sea with long wave lines, raising its giant claw, furious; one tiny sailing ship with blue sails nearby for scale. The face must still read at this size: brows, eyes and open mouth visible.
5. `stalk_hybrid.png` (landscape): one alternative for comparison: the same face and expressions idea, but with the two eyes on short thick stalks rising from the brow, each eye hooded by its own heavy brow lid; front three-quarter close-up, furious.

Landscape pictures should be about 3:2. Make each picture once; if one comes out clearly off model (wrong number of claws, no moustache, eyes on tall stalks where not asked), remake it once.

## Final message
List the files you saved, and one line per picture on anything off model. Nothing else.

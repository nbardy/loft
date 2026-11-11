Use the $imagegen skill in its default built-in tool mode (the built-in `image_gen` tool; never the CLI fallback, never an API key) to make character reference pictures for a 3D modeller and animator. Save every picture as a PNG in the current directory with exactly the file names below. Touch nothing outside this directory, run no git commands, and do not write code. If the built-in tool is unavailable or fails, stop and say so in your final message; do not switch to another path.

Pictures in `in/` to look at first (pass them to the tool as references if it accepts reference images):
- `in/current_3d_models.png`: four 3D sea-monster crabs today, one per row, through three cameras. Rows from the top: the original, FIDDLER, MASK (samurai), STILT (spider crab). Keep each character's colours, silhouette and what it carries on its back.
- `in/fiddler_concept_face.png`, `in/fiddler_concept_wide.png`, `in/mask_concept_face.png`, `in/mask_concept_wide.png`, `in/stilt_concept_face.png`, `in/stilt_concept_wide.png`: the rough concept pictures those three were modelled from.
- `in/islander_expressions_example.png`: an expression sheet already made for a fourth crab. Match its drawing style, layout and quality exactly; the three new characters must look like they belong in the same set, with clearly different faces and temperaments.

## The style (use in every prompt)
Japanese ukiyo-e woodblock print. Bold black key lines of varied weight round every form, flat colour with no gradients on the creature, a limited palette of deep indigo sea, cream paper, brick red, moss green and black plus each character's own accent, pattern lines instead of shading, cream foam with curled fingers where it breaks the water. Clean shapes a modeller can read. No text or labels in the picture.

## The three characters
FIDDLER. A colossal fiddler crab, a sea monster the size of an island. A smooth dark oxblood pebble-shaped shell, slender legs, one gigantic vermilion claw shading to cream at the fingers and as long as its body, the other claw tiny. Two very tall thin eyestalks ringed like periscopes with round cream eyes and big black pupils that swivel independently. A small mouth of two cream plates. On its back one leaning ruined brick lighthouse with gulls circling. Temperament: manic, twitchy, bug-eyed, a jittery show-off who cannot keep still.

MASK. A colossal samurai crab (a heikegani). The whole front of its brick-red shell is shaped like a scowling samurai war mask: heavy angular brow plates, deep-set eyes, a grimacing mouth plate with fangs, cheek guards. Layered black-and-red lacquered armour plates tied with cream cords on every leg and on both heavy gauntlet claws. On its back a small castle keep with curved roofs and two crooked pine trees. Temperament: a stern, proud warrior; its expressions are kabuki poses, held and theatrical, including the cross-eyed glare.

STILT. A colossal spider crab wading through the sea: a small knobbly thorny olive-tan body carried very high above the water on eight extremely long thin jointed legs like bamboo poles with swollen knuckles, long slender claws like tongs, a small face with long drooping eyestalks and heavy sad eyelids, and on its back a red shrine gate with hanging paper lanterns. Temperament: patient, melancholy, eerie and slow; a mourner.

## Pictures to make, for each character (9 pictures in all)
Replace NAME with `fiddler`, `mask`, `stilt`.
1. `NAME_expressions.png` (landscape, about 3:2): a model sheet of six front close-ups of the face on plain cream paper, two rows of three, the same character in each, each a clearly different expression in that character's own temperament: (a) at rest, in character; (b) wanting something it has seen off to one side; (c) straining with effort; (d) triumphant; (e) furious; (f) wincing at an impact.
2. `NAME_turnaround.png` (landscape): the head and front of the body in its resting expression, drawn four times in a row at the same size on plain cream paper: straight from the front, three-quarter view, side view, and from high above at a three-quarter angle (the angle a top-down game camera sees it from).
3. `NAME_taunt.png` (landscape): the whole creature standing in the sea, drawn three times in a row as three key poses of its victory taunt, a dance in its own temperament, on plain cream paper with a strip of indigo sea: fiddler waves its giant claw overhead and jitters side to side; mask strikes a slow kabuki pose, one gauntlet raised, one planted, head turned and held; stilt bows low and sways, lifting its long tongs like a slow ceremony.

Make each picture once; if one comes out clearly off model (wrong character, wrong number of claws, the wrong thing on its back), remake it once.

## Final message
List the files you saved, and one line per picture on anything off model. Nothing else.

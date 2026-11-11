"""Expression references for the island fiddler's face: one face in six moods, each as a front close-up and as a small
figure at game distance, from the Klein 1-step model (the pattern of tools/crab_concepts.py).
Writes WebPs, prompts.json and a labelled sheet into the output directory given as argv[1].

    uv run --project ~/git/flux-klein-fast python tools/crab_faces.py docs/art_direction/crab_round3
"""
import json
import shutil
import sys
from pathlib import Path
from time import perf_counter

import mlx.core as mx
from PIL import Image, ImageDraw

from flux_klein_fast import Pipeline

OUT = Path(sys.argv[1])
W, H = 768, 576
# Stop if the disk has less than this free (the converted weights are cached, so the job should not grow it).
MIN_FREE_GB = 8

STYLE = ("Japanese ukiyo-e woodblock print. Bold black key lines of varied weight around every form, flat colour "
         "with no gradients on the creature, a limited palette of deep indigo sea, cream paper and brick red, "
         "pattern lines instead of shading, cream claw-shaped foam where it breaks the water.")

SUBJECT = ("a colossal brick-red fiddler crab, low in the water, carrying a small mossy green island with stone cliffs "
           "on its back, one gigantic vermilion claw and one tiny claw, and two tall thick eyestalks ending in big "
           "round cream-white eyes with black pupils and heavy dark eyelids, the left eye a little lower and smaller "
           "than the right")

# One temperament (vain, showy, a bully) in the six moods of its fight.
MOODS = {
    "posing": ("vain and posing: chin up, eyelids half lowered in smug disdain, pupils turned sideways toward the "
               "viewer, one eyestalk cocked higher than the other, a small smirk"),
    "greedy": ("greedy: both eyes wide open with small pupils fixed on a tiny sailing ship, the eyestalks leaning "
               "toward it and close together, mouth plates parted, hungry"),
    "straining": ("straining with effort as it squeezes something: eyes squinted to narrow slits under lowered slanted "
                  "lids, eyestalks bent and trembling, mouth clenched tight"),
    "gleeful": ("gleeful, laughing as it throws something away: eyes wide and round with big pupils looking up, "
                "eyestalks flung apart in a V, mouth plates wide open"),
    "furious": ("furious, about to slam its claw down: eyelids slanted steeply down toward the middle in a hard scowl, "
                "tiny pinpoint pupils, eyestalks pulled low and forward, mouth open in a roar"),
    "wincing": ("wincing at an impact: both eyes screwed tightly shut, eyestalks pulled down and bent, the whole face "
                "flinching, spray flying"),
}

VIEWS = {
    "close": "Extreme close-up of its face and eyestalks seen from the front and a little above, filling the picture, a plain indigo sea behind.",
    "game": ("Seen from far away and high above at a three-quarter angle, the whole creature small in a wide empty "
             "indigo sea with long wave lines, one tiny sailing ship with blue sails beside it; the expression of its "
             "eyes still readable at this distance."),
}

jobs = []
for m, (mood, look) in enumerate(MOODS.items()):
    for v, (view, framing) in enumerate(VIEWS.items()):
        jobs.append(dict(name=f"{mood}_{view}", mood=mood, view=view, seed=500 + 10 * m + v,
                         prompt=f"{STYLE} The subject is {SUBJECT}. Its expression is {look}. {framing}"))

OUT.mkdir(parents=True, exist_ok=True)
free_gb = lambda: shutil.disk_usage("/").free / 1024**3
if free_gb() < MIN_FREE_GB:
    sys.exit(f"only {free_gb():.1f} GB free; need {MIN_FREE_GB}")
mx.set_cache_limit(256 * 1024**2)
t0 = perf_counter()
pipe = Pipeline(bits=4)
print(f"model ready in {perf_counter() - t0:.1f}s", flush=True)
for job in jobs:
    # A picture already there is kept (same prompt, same seed): a run that was cut short picks up where it stopped.
    if (OUT / f"{job['name']}.webp").exists():
        continue
    if free_gb() < MIN_FREE_GB:
        sys.exit(f"stopped before {job['name']}: only {free_gb():.1f} GB free")
    t = perf_counter()
    r = pipe.generate(job["prompt"], seed=job["seed"], width=W, height=H)
    r.image.save(OUT / f"{job['name']}.webp", quality=90)
    print(f"{job['name']} {perf_counter() - t:.1f}s", flush=True)
(OUT / "prompts.json").write_text(json.dumps(jobs, indent=2) + "\n")

tw, th, label = 384, 288, 24
sheet = Image.new("RGB", (tw * len(MOODS), (th + label) * len(VIEWS)), (16, 21, 28))
draw = ImageDraw.Draw(sheet)
for row, v in enumerate(VIEWS):
    for col, mood in enumerate(MOODS):
        im = Image.open(OUT / f"{mood}_{v}.webp").resize((tw, th), Image.LANCZOS)
        sheet.paste(im, (col * tw, row * (th + label) + label))
        draw.text((col * tw + 6, row * (th + label) + 5), f"{mood} / {v}", fill=(236, 226, 196))
sheet.save(OUT / "sheet.webp", quality=88)
print(f"sheet {OUT / 'sheet.webp'}  total {perf_counter() - t0:.1f}s", flush=True)

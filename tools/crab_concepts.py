"""Concept pictures for new art directions of the crab boss: six designs by four views, from the Klein 1-step model.
Writes WebPs, prompts.json and a labelled sheet into the output directory given as argv[1]."""
import json
import sys
from pathlib import Path
from time import perf_counter

import mlx.core as mx
from PIL import Image, ImageDraw

from flux_klein_fast import Pipeline

OUT = Path(sys.argv[1])
ONLY = set(sys.argv[2].split(",")) if len(sys.argv) > 2 else None
W, H = 768, 576

STYLE = ("Japanese ukiyo-e woodblock print. Bold black key lines of varied weight around every form, flat colour "
         "with no gradients on the creature, a limited palette of deep indigo sea, cream paper and brick red, "
         "pattern lines instead of shading, cream claw-shaped foam where it breaks the water.")

DESIGNS = {
    "mask": (
        "a colossal samurai crab, the heikegani: the whole front of its carapace is shaped like a scowling samurai "
        "war mask with heavy brow ridges and a grimacing mouth plate, its legs are layered lacquered armour plates "
        "like a samurai's shoulder guards, tied with cords, and on its back it carries a small castle keep with "
        "curved roofs and two crooked pine trees"),
    "stilt": (
        "a colossal spider crab wading through the sea: a small knobbly thorny body carried very high above the water "
        "on eight extremely long thin jointed legs like bamboo poles with swollen knuckle joints, long slender claws "
        "like tongs, small patient face with long eyestalks, and on its back a red shrine gate and hanging lanterns"),
    "fortress": (
        "a colossal box crab that is a living fortress: a broad round domed carapace like a stone drum tower, two "
        "huge flat shield-shaped claws folded shut over its face like castle gates with its eyes peering over the "
        "top, short thick legs tucked under like buttresses, battlements all round the rim of its back and a ruined "
        "fort inside the wall"),
    "fiddler": (
        "a colossal fiddler crab: one gigantic claw as large as its whole body held high like a banner and the other "
        "claw tiny, two very tall thin periscope eyestalks, a smooth pebble-shaped carapace, proud comic posture, and "
        "on its back one leaning ruined lighthouse with gulls"),
    "garden": (
        "a colossal decorator crab: its carapace is completely overgrown and dressed with branching coral, sea "
        "anemones, long kelp banners, broken shipwreck masts and a tilted stone ruin, its legs are shaggy with weed "
        "and crusted with barnacle clusters, and its face is half hidden under a fringe of kelp with two bright eyes "
        "looking out"),
    "aqueduct": (
        "a colossal crab made of ancient masonry: each leg is a tall arched stone pier of stacked blocks like the "
        "arches of a Roman aqueduct standing in the sea, its carapace is a slab of hexagonal basalt columns carrying "
        "a ruined amphitheatre, and its face is a carved keystone grimace with two narrow glowing red eye slits"),
}

VIEWS = {
    "wide": ("Seen from far away and high above at a three-quarter angle, the whole creature small in a wide empty "
             "indigo sea with long wave lines, and one tiny sailing ship with blue sails beside it for scale."),
    "face": "Extreme close-up of its face seen from the front at water level, filling the picture.",
    "legs": ("Close-up from the side at water level of its legs and one claw, showing the shape of each joint and "
             "the foam where the legs stand in the sea."),
    "back": "Seen from directly above, looking straight down on its back and what it carries, legs spread around it.",
}

jobs = []
for d, (name, subject) in enumerate(DESIGNS.items()):
    if ONLY and name not in ONLY:
        continue
    for v, (view, framing) in enumerate(VIEWS.items()):
        jobs.append(dict(name=f"{name}_{view}", design=name, view=view, seed=200 + 10 * d + v,
                         prompt=f"{STYLE} The subject is {subject}. {framing}"))
    # A second look at the wide shot: the silhouette is what the game camera sees.
    jobs.append(dict(name=f"{name}_wide2", design=name, view="wide2", seed=300 + d,
                     prompt=f"{STYLE} The subject is {subject}. {VIEWS['wide']}"))

OUT.mkdir(parents=True, exist_ok=True)
mx.set_cache_limit(256 * 1024**2)
t0 = perf_counter()
pipe = Pipeline(bits=4)
print(f"model ready in {perf_counter() - t0:.1f}s", flush=True)
for job in jobs:
    t = perf_counter()
    r = pipe.generate(job["prompt"], seed=job["seed"], width=W, height=H)
    r.image.save(OUT / f"{job['name']}.webp", quality=90)
    print(f"{job['name']} {perf_counter() - t:.1f}s", flush=True)
(OUT / "prompts.json").write_text(json.dumps(jobs, indent=2) + "\n")

cols = ["wide", "wide2", "face", "legs", "back"]
names = [n for n in DESIGNS if not ONLY or n in ONLY]
tw, th, label = 480, 360, 26
sheet = Image.new("RGB", (tw * len(cols), (th + label) * len(names)), (16, 21, 28))
draw = ImageDraw.Draw(sheet)
for row, n in enumerate(names):
    for col, v in enumerate(cols):
        im = Image.open(OUT / f"{n}_{v}.webp").resize((tw, th), Image.LANCZOS)
        sheet.paste(im, (col * tw, row * (th + label) + label))
        draw.text((col * tw + 8, row * (th + label) + 6), f"{n} / {v}", fill=(230, 237, 243))
sheet.save(OUT / "sheet.webp", quality=88)
print("wrote", OUT / "sheet.webp", flush=True)

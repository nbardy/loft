---
name: build-a-character
description: Build a posed creature or character in Loft from a description or reference pictures - a boss, an animal, a personified figure. Use when asked to model a new character, give one a face, costume, fur or materials, or when a character "does not read" or "looks like a blob".
---

# Building a character

Three workers each built a character in a day (an otter, a heron, a walrus; 25 Nov 2026, 307 renders). All three
got the engineering right and the look wrong: tidy, buildable, and not the animal. The order below is what would
have caught it. Their trails are under `out/meta/boss/*/trail.jsonl`.

## Order of work

1. **Write who it is before what it is made of.** One page: personality, the silhouette in words, the one prop,
   the one accent colour, the pose and where the weight sits. Get reference or concept pictures and look at them
   beside every sheet. They are for shape and mood, not a specification.
2. **Start from the line of the pose, not from parts.** A heron is an S-curve and a walrus is a sack with tusks
   that break its outline. Build the spine and the weight first. A column with a head on top, or a hill with a
   hat, is what you get by assembling parts and posing them later.
3. **Block out in one flat clay colour, from the side, the front and three quarters.** Render a black mask and
   shrink it to 200 px: is it this character and no other animal? A shape judged from one camera fails from the
   others. Nothing else is added until the mask reads, and the owner sees the mask sheet before you go on.
4. **Then the face.** Eyes that read at 200 px (dark, one catchlight), brows as part of the skull and not as
   rolls stuck on it, a mouth line that carries the attitude. Drive it with named numbers and render one sheet
   of expressions from one camera. Keep props out of the face's frame.
5. **Then costume and the prop.** Big planes, a few folds as geometry, edges and hems you can see.
6. **Then materials, as channels.** Base colour, roughness, normal detail, a material mask, each a named function
   you can render alone, on the stdlib lobes. Fur and feathers are fibres with a direction, not a bumpy skin
   (`docs/context.md`, the plastic-or-clay mistake). Render the channel sheet (`SHEET_AOVS=albedo,normal` plus
   your own roughness and mask views) before judging the final picture.
7. **Then light and setting.** Light it so the figure can be seen: a back light alone leaves the front black.
8. **Keep every render.** Rounds go through `tools/render_sheet.py`: two comment lines per scene make the trail,
   and a rejected variant is data. Fan out early with clearly different builds, not tunings of one.

## What went wrong, so you can check for it

- **Parts floating free of the body** in the views nobody looked at. Check every view before the next stage.
- **A second animal.** The walrus's face was a bear's; the heron's bill a plank. Compare the mask and the face
  with the reference each round and name the animal you actually see.
- **Whiskers as a brush.** A thousand straight rods of one radius and one colour. Whiskers and quills taper,
  curve, vary and are few.
- **Tusks, ears and tails inside the outline.** If the identifying feature does not break the silhouette, it
  does not identify.
- **Dark renders passed as mood.** If the coat cannot be seen it cannot be judged.
- **"It reads" from the builder.** The lead's picks matched the owner's votes about half the time, and a
  worker's less. Show options and let the owner's glance decide; say what is wrong before what is right.

## Cost

Measure with `loft profile` (see `frame-time`); the number on a sheet tile is one cold frame. Until the
visibility pass is fixed (`OPEN.md`, lane `loft/perf`), cost follows how much screen each instance's bounding
box covers and how many overlap, not how many shapes there are: prefer capsules, spheres and round boxes, split
a blended form into parts with tight boxes, cut long diagonal rods into short ones, and keep strands few.

# Styles: parts, styles, looks

## Plan (written before building, 2026-11-12)

Three layers, each built only from the one below: **parts** (functions a style is composed from, importable on their
own), **styles** (named ones, each a short file that composes parts), **looks** (named presets of a style). The rule: a
built-in style may use nothing a user cannot import (`tools/style_imports.ts` proves it).

### The parts (smallest set: each composed by at least two styles, or the lighting recipe's own machinery)

| Part | File | Law in one line | Composed by |
|---|---|---|---|
| lighting recipe: `light`, `LIGHT`, `lit`, `litBodies` | `light.ts` | treatments give a tone, darkest wins, printed once as a block; `litBodies` turns a recipe and a colour into a style's `shade`, `mark`, `block` | woodblock, toon, sketch |
| sky: `skyOf(sky, ctx)` | `sky.ts` | a horizon ink with washes wiped over it by view height, a sun disc, cloud bands, `below` under the horizon | woodblock, papercraft, neon |
| swell lines: `swellLines(ctx, o)` | `sea.ts` | lines of constant phase of the largest swell, bent by the height, heavier at crests, turned to their tone where they crowd | woodblock, sketch, neon |
| contact foam and marks: `contactFoam`, `marksFoam`, `printLayers` | `foam.ts` | each pattern returns coverage per ink (fill, line, edge) in printing order; a style chooses the inks | woodblock, sketch, neon |
| white water: `rollingFoam`, `curledHeads`, `crown` | `whitewater.ts` (as is) | a mass sizes hooks, lobes and droplets drawn as curves in the world | foam (`roll`, `heads`), impact kit |
| paint treatments: `warmed`, `mottled`, `weighted`, `standsOut` | `paint.ts` | repaint before the print rule; a line inside the hull whose width follows the light (or wanders) | woodblock, sketch |
| post passes: `paperGrain`, `bloom`, `reflection` | `post.ts` | full-screen WGSL with its numbers as options | woodblock and sketch (paper); biolume and neon (bloom, reflection) |
| shared: `edgeOf`, `rgb`, `pixelsInside`, `POST_COMMON` | `shared.ts` | an antialiased edge; pixels in from the silhouette | everything |

Not parts, and why: the print rule (bands, keys, overprint: the palette quantiser) is composed only by woodblock and
stays its decision; papercraft's facets and studio pass, biolume's glow from paint, its halo and papercraft's collar
are one style's each. A second user would promote them.

### Each style as a composition

- **woodblock**: print rule (own) over `warmed`/`mottled` paint; `litBodies` under a recipe or the rule's two tones;
  `weighted` line; `skyOf` (horizon cream; washes haze 0..0.12, indigo 0.18..0.5); water = two blues split at the
  swell + `swellLines` (5 a wave, widths 0.025..0.075) + `contactFoam` (default `ridge`) + distance haze +
  `marksFoam` printed in (cream, black, indigo); `paperGrain` (fibre 0.07, tooth 0.035).
- **toon**: its own bands, rim and gloss; under a recipe `litBodies` over that colour. Sky and sea its own (power-curve
  grades are not washes).
- **biolume**: its own glow and water; `reflection` and `bloom` (radius 30 px, gain 1.6) from `post.ts`.
- **papercraft**: its own facets, sheets and collar; `skyOf` (horizon backdrop, one wash to the top over 0..0.6).
- **sketch** (new): bodies are paper; `litBodies` with a hatch (spacing 0.14, ragged 0.25) in a graphite block; dark
  paints (luminance under 0.12) filled in graphite; `weighted` with a new `wandering` line weight (width from noise
  over the anchor and the light); a thin graphite hull. Water: paper, `swellLines` (3 a wave) in graphite, foam
  `roll` printed paper fill with graphite lines; shadows hatched on the water. Sky: plain paper. `paperGrain` with
  more tooth.
- **neon** (new): near-black bodies whose paint becomes a saturated light; the hull is the tube (its `ink` is that
  light); a glow `pixelsInside` 0..5 px and a fresnel term in the same light. Water near black with violet
  `swellLines` and `rings` foam in cyan; sky `skyOf` (violet horizon washed to black over 0..0.35); `reflection` and
  `bloom`.

### Predictions

Line counts after (before): woodblock 360 (857), toon 115 (in `style.ts`, 100), biolume 95 (167), papercraft 205
(218), sketch 80, neon 85. Parts: sky 90, sea 45, foam 300, paint 100, post 110, light 495 (471).

Pictures: woodblock, toon, biolume, papercraft byte-identical (the hash list in the report). The one risk is a part
that builds the same value through a different expression tree; every extraction keeps the tree.

Cost at the crab's `game` camera, 1280x800, beside woodblock in the same timing rounds: woodblock unchanged (same
shaders); sketch within 0.2 ms of woodblock (its hatch costs what `hatched` does, nothing measurable; its foam and paper
are woodblock's); neon +0.6 ms in the fast clock state (40-tap bloom over 1 M pixels and a 24-tap reflection on the
water pixels), about three times that in the slow state.

Weaker of the two, predicted: sketch, because one direction of hatching on paper may read as woodblock's engraving
without its colour, not as a pencil drawing.

Inherited: cel bands and rim light (cel animation), hatching and the engraver's swelling line, overprinted blocks,
bokashi washes and print registration (the woodblock print), bloom (film). Ours: foam as coverage per ink so one
pattern prints in any style's inks; the sky as washes (a gradient's stops are washes that abut, a print's bokashi
washes that overlap); the terminator round a form's section and line counts that follow the screen (from
`docs/lighting.md`).

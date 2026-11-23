# Crab shell texture, from references (lead, 23 Nov)

References: 12 Wikimedia Commons photos in `out/meta/refs/crab/` (`sources.txt` has the title, licence and URL of each; `sheet.png` is the contact sheet, `closeups.png` the shell close-ups). The div2 "chitin" seeds failed because they painted isotropic, asymmetric, mid-frequency noise into albedo and removed all highlights. The owner called it "digital camo at best". Every observation below is from the photos; the numbers are the reference images' own.

## What the photos show
1. **The pattern is anatomical and bilaterally symmetric.** Coconut crab (01): the orange flush sits on plate edges and sutures, and the pale dashes run in rows along the plate curvature. Edible crab (10): darker toward the grooves and centre, lighter at the rim. Ghost crab (04): dark smudges fill the H-groove. Shore crab (07): lighter H-groove regions on an olive ground. Nothing is uniform noise.
2. **Pattern, where a species has one, is crisp at a single characteristic scale.** Blue swimmer (06): sharp pale spots and vermiculated lines on a dark ground, mirrored about the midline, at about 1/15–1/25 of carapace width. It is a thresholded field (hard edges), not soft fBm.
3. **Fine granularity is seen mostly in specular, not albedo.** On the edible crab (10) the dense pinhead granules show as sparkle in the sheen while the base colour stays a smooth brick orange. Same on the swimmer.
4. **Most species are glossy.** Coconut, swimmer, edible and Sally Lightfoot (08) all show clear, shape-revealing highlights. The ghost crab is the matte exception, and even it has soft sheen on the claws.
5. **Legs are a separate colour system.** Transverse bands per segment, dark joints and dactyl tips, a different hue from the carapace (coconut: blue legs on a dark body; shore crab: orange legs on olive), and a pale underside.
6. **The silhouette carries detail.** A toothed or lobed anterolateral margin ("pie-crust" on the edible crab) and dark tooth tips.
7. **Sparse life and wear:** occasional algae or barnacle patches and scratches (edible crab), a few per animal.

## Texture model (replaces div2's chitin material)
Work in carapace coordinates: `u` along the midline (front to back) and `s = |x|` across it. Using `|x|` makes every pattern bilaterally symmetric by construction.

| Layer | Drives | How |
|---|---|---|
| Anatomical tone | albedo (low frequency) | A groove field (distance to the H-groove and plate-suture curves) darkens and adds AO. Rim distance lightens. A dorsal-to-ventral gradient. Species flush colour concentrated at sutures and edges. |
| Species pattern | albedo (crisp, one scale) | Optional per species. Spots are thresholded Worley cells; vermiculation is a thin band `abs(n) < w` of domain-warped low-octave noise. Both are sampled in `(u, s)` with smoothstep edges about 1–2 px wide. The scale comes from the reference species. |
| Granules | normal and roughness (high frequency) | Worley bumps on the normal only, plus a slight roughness dip on the granule tips. They show as specular sparkle. Albedo is changed ≤3%. |
| Gloss | BSDF | Wet species: roughness 0.18–0.32 with a thin coat. Ghost crab: 0.45–0.6. Highlights must reveal the dome and the plate edges. |
| Legs | separate material | Per-segment hue and banding, dark joints and dactyl tips, a pale ventral side, the same granule sparkle. |
| Life and wear | sparse decals | 0–3 algae or barnacle patches or scratches, placed asymmetrically on purpose (life is not symmetric). |

## Guards (two-sided, calibrated on the references, checked to reject div2 first)
- Bilateral symmetry of the carapace albedo, ignoring the wear decals: high.
- Spectral energy fraction at wavelengths < 1/12 of carapace width, in **albedo**: low. In **luminance including specular**: within the reference range.
- Highlight coverage inside the matte: inside the reference band for that species (glossy species clearly above zero).
- None of these replaces the side-by-side visual judgement against the photos.

## Negatives
Every superseded program is a rejected example for the Improve model and is kept byte-exact:
- the 16 template seeds from build_crab_v15 (recoloured templates);
- div1's 16 seeds (shape fixed, plastic look);
- div2's 16 (camo chitin, blotchy watercolour, unchanged woodblock spider);
- every losing imp1 candidate.

The lead's reason for each rejection is recorded with it.

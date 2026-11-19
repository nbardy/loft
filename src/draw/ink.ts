// The ink outline as a full-screen pass from the opaque scene's depth (`Scene.outline` "screen"), the alternative to
// drawing every outlined body a second time pushed out along its normal ("hull", src/draw/surface.ts).
//
// Which surfaces want ink travels in the picture's alpha, which otherwise carries the style's `mark`: an outlined
// surface writes 0.49 mark, any other 0.51 + 0.49 mark (MARK_WGSL). The pass decodes it and writes the mark back, so
// the style's own post passes (woodblock's key line reads it) see what they always saw, at 7 bits.
//
// A pixel p is ink where some outlined surface q within the ink width of p (the hull's width: the style's world
// width at q, at least 1.5 px) is nearer than p's own surface by more than a step: p lies just outside q's silhouette,
// where the hull's back faces would have shown. The ink colour is the style's `ink` with q's shaded colour as the base
// (a post pass cannot know q's paint) and q's depth below the water. Traced solids write their mark through
// `ctx.markOut` as not outlined, which is right (they ink themselves). They used to write the raw mark, which agreed
// only while the mark was 1: under a style whose mark is below 0.5 the pass read them as outlined and decoded their
// mark wrong (tested 2026-11-14, out/geometry/outline_test: mark 0.3 moved 240 pixels by up to 14/255; mark 1 none).
//
// What it cannot do as the hull does: ink a silhouette that is under the water (the water blended over q raises its
// alpha above 0.5, so q reads as not outlined), and see an outline wider than the shader's search radius.

import markWgsl from "../wgsl/mark.wgsl" with { type: "text" };
import inkPost from "../wgsl/ink_post.wgsl" with { type: "text" };

/** WGSL for opaque fragments: the alpha that carries `mark` and whether the surface wants screen ink. */
export const MARK_WGSL = markWgsl;

/** The shader clamps ink width to its farthest search radius (above the hull's close-up widths in the examples). */
export const INK_POST = inkPost;

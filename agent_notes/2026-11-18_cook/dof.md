## Your lane: depth of field and film grain (worktree `~/git/_wt/loft-dof`)

Product shots read as photographs because of shallow focus. `DESIGN.md` mentions a lens disc for accumulated stills;
check what exists. Build a real-time post pass:
- Depth of field from the scene depth: physically based circle of confusion from focus distance, aperture (f-stop) and
  focal length of the camera, near and far fields, no haloing at depth edges (gather with depth-aware weights, e.g. a
  scatter-as-gather bokeh), bokeh shape option only if cheap. Transparent things (soft medium, spray, volume) need a
  depth: say what you use.
- Film grain as a tiny post pass (luminance-dependent, deterministic per frame).
- These are post passes a style composes (`post: [...]`, see `src/style/post.ts`), not new camera kinds unless the
  camera must carry focus/aperture (then put them on the camera spec).
- Example: refocus an existing scene; GPU test of a measured property (in-focus plane sharp, background blur radius
  matching the CoC formula within a stated tolerance). Report ms/frame at 1280x720.

# Writing Loft

Run `loft context` for the authoring guide, a complete custom schedule with a motion law, and the source-generated stdlib signatures. Use `loft context --section engines`, `--section motion`, `--section styles` or `--section passes` to inspect one part.

Each row is an instance. Give its `kind` a space and columns for per-instance variation, then use `build for` and `emit` to populate it. A trace returns signed distance in metres. A material returns a BSDF. Bind a style or integrator in the schedule to change how those surfaces are lit.

The engine, schedule, integrator, style and motion library are editable Loft source. Run `loft fork engines/realtime my_engine.loft` and use the printed import from a sibling scene. Rewrite the fork's passes or lighting, author a new schedule, or replace a motion solver when the shot or frame budget demands it.

Run `loft check`, `loft lint` and `loft show --plan` on the scene. Use `loft still` and `loft clip` to judge the look and motion, and `loft profile` to measure p95 and pass cost at the intended resolution. When a surface looks wrong, add `--aov albedo` or `--aov normal` to `loft still` or `loft clip` to see its base colour and its surface normal without light, and name the channel that is wrong before editing the material. Change the layer responsible, then run the same comparisons again.

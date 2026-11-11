# Art research prompts

Two prompts for handing this work to another model. Each stands alone. Written 2026-11-11.

| File | Asks for | How tightly it is specified |
|---|---|---|
| `01_art_tool_from_first_principles.md` | The whole tool for agents who make 3D art: the language, the graph, the kit round it | Loosely. Goals, reasons and questions; no design. The previous attempt is offered as one data point |
| `02_render_math_and_algorithms.md` | The mathematics and algorithms of getting shapes onto the screen | More tightly. A problem statement, measured results, candidate results to test or break, and a method |

The split: prompt 1 designs what an agent writes and uses (API, data format, methods, tooling). Prompt 2 works on
how things are drawn, either as implementations for the existing tool or as new algorithms that may call for a
different tool. Each tells its reader to override the other's limiting decisions and record what it overrode. Only
the owner's constraints and budgets bind both.

To reuse one: give the model the file and access to the repositories it points to. For a model you expect to do
better than the previous attempt, hand over prompt 1 without `DESIGN.md` first, and let it read that afterwards.

The measured figures in both are dated. Re-measure before relying on them.

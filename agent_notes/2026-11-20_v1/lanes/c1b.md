# C1b · Narrow physical registers for native compute edits

Base `173d6e9a2b4fa36fcb45c0957f75e9c44f8e6dfa` from fetched `origin/main`.

## Prediction before implementation

For the four-word promotion tape, each encoded register field is in 0–2. OR the words in
a tape of up to eight instructions and test the upper three bits of each five-bit register
field, including the low five bits of the immediate. A zero mask proves all four register
indices fit, including the eager reads of otherwise unused operands. A nonzero mask selects
32 registers. An immediate such as a swizzle or loop PC can conservatively select the wide
VM. The count and words come from the existing bucket buffer, so the choice is uniform and
takes effect on the first dispatch after upload. This makes at most eight extra word reads
in the short path and reduces the register array from 512 to 64 bytes per invocation.
Both widths share one WGSL instruction body. The tape keeps its count word, 56 opcodes,
logical register IDs 0–31 and existing host validation.

Predict **+0.250 ms** paired p95 overhead, a pass in each of three observations against
**+0.500 ms**, upload **≤2 ms**, and zero foreground pipeline creates. The archived del2
observations were **+0.581/+0.667/+0.796 ms**. C1's full bank measured
**+0.779/+0.545 ms**, the four-register control **+0.063/+0.033 ms**, and the combined
shader **+0.228 ms**. Each planned observation runs the original 140,000-point test,
4,375 workgroups of 32, 120 interleaved pairs with the first 20 discarded, and the
original paired p95 statistic and promotion assertions.

Predict CPU and compute field agreement within 1e-5 at the 4/5-register and 8/9-word
boundaries, register 31, loop execution and an immediate above 3. The non-sphere edit
will use a short box tape. Predict ≥99.9% of promotion pixels within 1/255.

[Archived C1 probes](../../../out/meta/c1/) · [Lane evidence](../../../out/meta/c1b/)

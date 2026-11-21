# Phase 5 corpus

The Cargo workspace compiles the ported `.loft` scenes. The grammar gate checks their source,
including quarantined entries. The bead gate renders `l5c/bead.loft` against nine frozen observations.

`manifest.json` assigns the original 295 identities to 20 batches. For an example, read its
source with `git show REV:PATH` at `examples_rev`. For a seed, split the identity at the first
slash and look up the remaining ID in the JSONL file pinned in `sources.json`.
Run `uv run --no-project python tools/check_corpus.py` to verify the partition.

From the repository root, build a port with
`tools/dev.sh run loft-cli -- build corpus/l5c/bead.loft -o out/bead.loftb`.

Replay the 170 procedural seeds with `tools/regenerate_corpus.sh`. It checks
both JSONL pins and runs the historical CPU sampler in an isolated directory.
The seven sampled knot times and original views are preserved. Generated build
loops import ring samples and sparse paint and exponent overrides. The kind
declarations hold the modal paint and exponent. `tools/check.sh lint` checks
Loft lexical tokens and repeated numeric blocks. It enforces the budget on
regenerated ports and reports warnings on the retained legacy ports.

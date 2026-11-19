# Phase 5 corpus

Each batch in `manifest.json` belongs to the named lane. A scene starting with
`examples/` is a source path at `examples_rev` (`git show REV:PATH`).
A seed is `SET/ID`. Split at the first slash and look up the complete ID in
the JSONL pinned for that set by `tools/parity/corpus.json`.
The order is R0's sorted example paths followed by the two pinned files in row order.

From a lane workspace, use `~/git/loft/target/release/loft build SCENE.loft`
for a CPU build. The integrator runs the training command from 06 §17 under
`tools/gpu_lock.sh`. Clip directories are published by rename to a new or empty
destination. Frame percentiles use nearest rank across all requested frames.

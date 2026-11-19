#!/usr/bin/env bash
# The gate's cargo flags in one file, sourced by tools/dev.sh and tools/check.sh. Clippy's `-D warnings` and
# --all-targets are part of each unit's fingerprint: Keel 2026-10-30, a warm script without `-D warnings` re-linted 175
# units on every switch to the gate. A crate lint, quick and full must spell these the same way, so only here.
LOFT_GATE_CLIPPY_ARGS=(--locked --all-targets)
LOFT_GATE_LINT_ARGS=(-D warnings)
# --no-tests=warn: a member with no tests yet is not a failure of the workspace run.
LOFT_GATE_TEST_ARGS=(--locked --no-tests=warn)
LOFT_GATE_DOCTEST_ARGS=(--locked --doc)
LOFT_GATE_WASM_CHECK_ARGS=(--locked --target wasm32-unknown-unknown)

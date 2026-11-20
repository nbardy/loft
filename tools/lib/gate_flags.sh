# Cargo flags shared by the edit loop and native checks.
LOFT_GATE_CLIPPY_ARGS=(--locked --all-targets)
LOFT_GATE_LINT_ARGS=(-D warnings)
LOFT_GATE_TEST_ARGS=(--locked --no-tests=warn)
LOFT_GATE_DOCTEST_ARGS=(--locked --doc)

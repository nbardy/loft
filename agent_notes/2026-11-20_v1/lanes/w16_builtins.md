# Wave 16 builtin catalog

79 call spellings in 362 physical source lines. The crate has zero dependencies.
One declaration generates the enum, table and exhaustive ID lookup.
The rows follow checker, evaluator, WGSL, motion and AD behavior, including
aliases, packed math and immutable lookups. SDF and noise helpers are authored
Loft functions. Motion/AD-only names carry an unchecked signature.
Expected verification is four passing table tests with zero runtime or GPU
impact because the catalog has no consumer in this phase.

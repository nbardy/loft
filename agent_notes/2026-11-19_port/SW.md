# SW sharing, chunk names and bundle writing

The pre-implementation estimate used the plan's 295 entries and CORE.md's 233 builds. The harness now discovers
302 entries. With the LW stub, the prediction is that each built reference first differs at `files.manifest.format`
because the Rust CLI returns the LW refusal before writing an artefact.

Sharing has two passes over the code tokens and retains only the first shader's template for each group. The
decimal writer needs at most 17 significant digits per binary64 and compares exact midpoint distances to choose
the last digit. The chunk hash runs exactly two wrapping updates per UTF-16 code unit.

## Translation

- `src/host/share.ts` (318 lines) to `rust/crates/loft-lower/src/share.rs` (912 lines including tests). The lexer
  follows the original regex alternatives and keeps UTF-16 code units in the comparison key. Fragment decimals
  round through binary64 before binary32. Groups, row dedupe, and WGSL replacements follow TS encounter order.
- `src/host/files.ts` (`writeBundle`, 30 lines) and `src/host/lower.ts` (`chunkName`, 9 lines) to
  `rust/crates/loft-lower/src/files.rs` (810 lines including the serializer and tests). The stable object order and
  numeric spelling are implemented in the serializer. Rust `&str` carries Unicode scalar values. `JsString` accepts
  UTF-16 code units so unpaired surrogates can be serialized as JSON escapes.

## Focused checks

- `manifest_json_matches_ts_bytes` catches reordered object fields, different decimal or exponent spellings,
  wrong string escapes, an appended newline, and non-finite numbers serialized as `null`.
- `chunk_names_match_ts_files` catches hashing UTF-8 or Unicode scalar values in place of UTF-16 code units, and
  using the updated second hash in the shift.
- `bundle_replacement_checks_first` catches modifying an earlier build before either refusal and leaving stale files.
- `sharing_matches_ts_chunks_and_rows` catches grouping and WGSL differences, changes in row order or dedupe, and
  planned draws using the wrong row or constants. Its midpoint fixture catches a decimal parsed directly to f32.
- The saved decimal reference check matched 119,973 finite binary64 bit patterns against Bun's `JSON.stringify`.

Clippy, the fast build, and all four tests passed. The tests re-emitted 241 manifests byte for byte, checked 7,758
chunk names, matched 1,323 shared modules and 39,315 row floats, and checked each emitted planned draw's row and
constants against the TypeScript manifest.

## Scoped gate

The resumed unfiltered `lower` run with scope `SW` and `--reuse-ts` exited 1 with **60/302 passing**. All 60 passes
were author-stage refusals. For the 241 built entries, the first missing section was `files.manifest.format`
because the Rust CLI returned `unported: src/host/lower.ts (lane LW)`. `examples__serpent_clip` failed before any
section comparison when TypeScript tried to read `scene.style.post` from `undefined`. The report is
`out/port/sw/report.json`.

The direct checks found zero byte differences on the available references. The whole-bundle comparison still
requires LW to construct the artefact and call SW. The `serpent_clip` TypeScript crash also remains open.

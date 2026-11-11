# ADR-008: Canonical IR serialization

Status: accepted. Plan reference: D8.

## Context

Two of the project's load-bearing tests are **byte comparisons on the IR file**: the
fixpoint check `almc.2.ir == almc.3.ir` that *is* self-hosting (M7), and the
`almc0`-vs-`almc` differential that replaces an interpreter for the whole front end
(ADR-013.1). A byte comparison is only meaningful if the bytes are fully determined by
the source.

## Decision

The IR file's content is a function of the source alone. Specifically:

- **Constant pool and string table in first-occurrence order.** Not sorted, not hashed —
  first occurrence in a defined traversal.
- **Module order is the deterministic `share`-init traversal of [4.2]** (depth-first
  over the import graph), which the language already defines.
- **No timestamps.** None, anywhere, including in comments.
- **No host paths.** Module names only. A build of the same source in two directories
  produces identical bytes.
- **No map-iteration order anywhere in either compiler.** Every iteration that reaches
  the output iterates an ordered structure — a `Vec` in source order, or a `BTreeMap`.
  In `almc` this is the insertion-order iteration of [8.5.3].

`documentation/ir.md` is normative for the format, for both compilers and the backend.

## Consequences

- `std::collections::HashMap` must not be iterated anywhere on a path that reaches the
  IR, the diagnostics or the report. Prefer `BTreeMap`, or a `Vec` plus an index.
- Diagnostics are rendered with the module path as given, never an absolute path
  (`alm-core::diag`), for the same reason and because the conformance suite matches
  diagnostic text exactly.
- The option set is part of the program's identity, and its canonical form excludes
  `--path` for exactly this reason (`alm-core::options::Options::identity`).
- A debugging field added "temporarily" to the IR output is a fixpoint break. Any field
  that is not a function of the source must go to a side channel, never into the artifact.

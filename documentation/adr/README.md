# Architecture decision records

One file per decision, numbered in the order they were taken. ADR-001 to ADR-014
are the decisions the implementation plan (§1, D1–D14) requires before code is
written; the point of writing them down is the reasoning, not the choice.

Bracketed section numbers point into `../language_reference.md`. Where a decision is
*forced* by a spec rule rather than merely preferred, the ADR says which rule, because
that is what stops a later "simplification" from quietly breaking the language.

Status values: `accepted`, `superseded by ADR-NNN`.

| ADR | Decision | Status |
|---|---|---|
| [001](001-native-via-textual-llvm-ir.md) | Native code through textual LLVM IR | accepted |
| [002](002-host-language-and-crates.md) | Rust, one workspace, four crates | accepted |
| [003](003-ir-shape.md) | Typed three-address IR over explicit frames | accepted |
| [004](004-precise-gc-over-frame-list.md) | Precise GC walking the frame list | accepted |
| [005](005-step-charges-are-ir-level.md) | Step charges are IR-level, not coalescable | accepted |
| [006](006-exceptions-by-status-and-branch.md) | Exceptions by status-and-branch | accepted |
| [007](007-decimal-representation.md) | Base-10^18 limbs plus a scale | accepted |
| [008](008-canonical-ir-serialization.md) | Canonical IR serialization | accepted |
| [009](009-tasks-as-os-threads.md) | Tasks as OS threads with a round barrier | accepted |
| [010](010-replay-log-format.md) | Replay log format | accepted |
| [011](011-collections-in-support.md) | Compiler collections in `support/`, not `std` | accepted |
| [012](012-rng-algorithm.md) | RNG algorithm, pinned | accepted |
| [013](013-no-interpreter.md) | No interpreter; four substitutes for an oracle | accepted |
| [014](014-strict-steps-invariance.md) | `--strict-steps` × `--steps` invariance in CI | accepted |

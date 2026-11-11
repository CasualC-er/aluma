# ADR-013: No interpreter, and the four substitutes for an oracle

Status: accepted. Plan reference: D13.

## Context

The toolchain compiles to native code only. There is therefore no second, independent
execution of the semantics to compare against: a codegen bug surfaces only as a wrong
answer. An interpreter was considered and rejected — it would be a second
implementation of the hardest parts of the spec (rounds, copying, views), and the
effort is better spent on the four substitutes below.

## Decision

No interpreter. Four deliberate substitutes, each built before it is needed:

1. **`almc0` vs `almc` on canonical IR.** A genuine differential pair for the entire
   front end, free, and byte-exact thanks to ADR-008. Covers everything up to the IR.
   From Phase 10.
2. **`-O0` vs `-O2` differential execution.** The same program at two optimization
   levels must behave identically. [8.7.2] is what makes "identically" a meaningful
   assertion, and this is the cheapest strong test of the backend and of LLVM itself.
   In CI from the first running program (Phase 3b).
3. **An IR verifier in `alm-core`**, run on every compile in CI: types, place legality,
   slot-map coverage, charge-site presence, `try`-region nesting.
4. **The conformance suite as the primary check**, built **first** (Phase 0) and kept
   exhaustive. With no oracle, coverage *is* the safety net.

## The suite's shape

A test is a **directory**, not a file, because the spec's own examples are not single
files: the `Stack` + `Lookup` + `Demo` set of [11] is three modules and a `--path`, and
the negative tests of Phase 2 and Phase 8 expect a diagnostic and no run at all. A test
directory holds a manifest plus every module of the program; `src/almtest/src/lib.rs`
documents the format.

[11] also states compile errors **inline inside otherwise-valid examples**, so a
whole-program verdict cannot express them. Such a test annotates the offending line in
the source with `#!error` or `#!error: text`. `#` starts a comment [2], so the
annotation does not change the program, and the harness requires the set of error lines
to be exactly the set of annotated lines — an unannotated error fails the test too.

## The `-O0`/`-O2` corpus is restricted

[8.7.2] guarantees determinism only *given the same outside events, taking effect in the
same rounds*. A program that calls `readLine`, `now`, `sleep`, `random` or `Net` may
legitimately differ between two runs, so comparing two optimization levels of it is a
flaky test, not a strong one. The differential corpus therefore contains only programs
with no outside events, or else runs both levels under a pinned replay log (ADR-010) and
an explicit `--seed`.

## Consequences

- The suite must be populated mechanically from the docs ([11] is effectively an
  acceptance suite), and the conformance matrix — every numbered rule → its tests — is
  checked in CI.
- Until `almbe` and `almrt` exist, harness runs report `pending`, not `pass`. A
  `pending` count that is not falling is a schedule signal.

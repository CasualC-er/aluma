# ADR-014: `--strict-steps` × `--steps` invariance is a CI property test

Status: accepted. Plan reference: D14.

## Context

[8.4] states: under `--strict-steps`, "a run that raises no `OutOfSteps` gives the same
results for every value of `--steps`". Unlike most determinism claims, this one is
cheaply falsifiable by a machine.

## Decision

A CI property test, from Phase 6: take each program in the conformance corpus, run it
under `--strict-steps` with a sweep of `--steps` values, and assert that every run that
raises no `OutOfSteps` produces identical stdout, stderr and exit status.

## Reasons

This is the cheapest real check on the budget machinery: the refresh, the cap, the
reset to `N` under `--strict-steps`, and the rule that a task never idles for debt. An
error in any of those shows up as two `--steps` values disagreeing.

**What it does not catch.** It does not validate where charge sites sit. Under
`--strict-steps`, a run that raises no `OutOfSteps` is not supposed to depend on the
budget at all, so moving a charge site — or coalescing several — changes nothing this
test compares. Charge-site placement is covered instead by:

- the **threshold** runs: the smallest `--steps` at which a program raises `OutOfSteps`,
  and the site it is raised at, are both reproducible, and are asserted;
- the `-O0`/`-O2` differential (ADR-013.2), which catches a charge the optimizer moved
  or elided;
- the IR verifier (ADR-013.3), which checks that every chargeable event has a site.

Pair it with the randomized thread-interleaving test (ADR-009): one varies the budget,
the other the scheduling, and between them they cover both halves of [8.7.2].

## Consequences

- The sweep must include values small enough that some runs *do* raise `OutOfSteps`;
  those runs are excluded from the comparison, but the fact that the threshold is
  reproducible is itself worth asserting.
- Programs with outside events are excluded for the reason given in ADR-013, unless run
  under a pinned log and seed.

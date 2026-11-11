# ADR-009: Tasks are OS threads with a barrier at the round boundary

Status: accepted. Plan reference: D9.

## Context

Alúma's concurrency is round-based: within a round tasks may run in any order or at
once with an identical outcome [8.3], and [8.7.1] guarantees that no mutable memory is
reachable from two tasks.

## Decision

One OS thread per task, with a barrier at the round boundary. No coroutines, no CPS
transform, no stack switching.

## Reasons

- Because no mutable memory is shared [8.7.1], real threads need **no locks on program
  data**. The barrier is the only synchronisation, and it is exactly where the spec puts
  one.
- "Budget exhausted" becomes "park at the barrier", which is what a thread does anyway.
- Each task has its own heap [8], so the collector is per-task and needs no
  stop-the-world coordination beyond the boundary.

## Consequences

- One stack per task, so a few thousand tasks is the practical ceiling. Above that,
  small-stack green threads are needed; design `almrt`'s task representation so that
  the swap is contained in one module.
- The round boundary's ten steps [8.3] must happen in order while every task is parked,
  which is a single-threaded phase. That is where output is flushed, messages are
  delivered, task numbers are assigned and cancel closure runs.
- Thread scheduling is a source of nondeterminism that the language forbids being
  observable, so the randomized-interleaving property test (Phase 6, Phase 8) is not
  optional: it is the evidence for [8.7.2].

# ADR-005: Step charges are decided in the IR and are not coalescable

Status: accepted. Plan reference: D5.

## Context

A task yields exactly where its budget crosses zero [8.4], and under `--strict-steps`
`OutOfSteps` lands at a specific charge site. The step table in [8.4] charges per
event: one step per simple statement, one per condition evaluation, one per procedure
entry, one per unwound frame, limb counts for decimal arithmetic, copy price for
`new`/`send`/`answer`.

## Decision

One charge site per event, each a decrement-and-branch of two or three instructions,
emitted in the IR by the compiler. `almbe` only lowers them; it never merges, moves or
elides one.

## Reasons

Fold a basic block's charges into a single subtraction and the yield point moves. Move
the yield point and a following `send` or `write` lands in a different round, which is
observable through [8.3]. So the usual optimization — hoist the counter out of the loop,
charge once per block — is a semantic change here, not a speedup.

Because the charges are decided in the IR, they are decided by the self-hosted part of
the compiler. Every determinism-relevant decision therefore lives inside `almc`, which
is where it belongs: the host-language backend cannot change the language's behaviour.

## Consequences

- Roughly two or three instructions per charged event. Measured against the fact that
  the alternative is a language whose observable behaviour depends on `-O`.
- The step counter must not be hoisted out of a loop by LLVM, which constrains call
  attributes (plan §2.4) and is exactly what the `-O0`/`-O2` differential (ADR-013.2)
  is there to catch.
- `alm-core::steps` holds the one copy of the table, shared by compiler, backend and
  runtime (ADR-002).

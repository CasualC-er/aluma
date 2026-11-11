# ADR-003: The IR is typed, three-address, over explicit frames

Status: accepted. Plan reference: D3.

## Context

The IR shape is not a free choice. [7.2] says **all parameters are passed by
reference, aliasing is real, and assignments through aliases take effect in program
order.** Any representation that assumes a value can be copied into a register and
written back later is wrong for this language.

## Decision

A typed, three-address IR over explicit frames. Not SSA, not a stack machine.

A parameter is a **place**: a tagged reference to one of

- a frame slot,
- a record field,
- an array element,
- a read-only object in the shared region [4.2] or in a version [8.12], which rejects
  writes with `ReadOnly`.

Everything whose address can be taken lives in an addressable frame slot; LLVM's
`mem2reg` promotes the rest. A non-variable argument materialises a caller temporary
and passes a place to it [7.2].

The IR carries explicitly: charge sites (ADR-005), safepoints, place computations with
their bounds and `nil` checks, `try` regions, per-procedure slot maps (ADR-004), and
the site table with its source snippets (see ADR-004 and `../ir.md`).

`defer` records a closed list of places plus pre-evaluated temporaries — not a closure,
which the language does not have [1] — and one record per `defer` statement, which
bounds the frame [5.1].

## Consequences

- Reference-holding locals stay in memory. Accepted; ADR-004 explains why.
- Alias analysis in LLVM sees ordinary loads and stores through pointers, which is what
  the semantics actually are, so it cannot "optimize" an aliasing write away.
- The IR is verifiable: types, place legality, slot-map coverage, charge-site presence
  and `try`-region nesting are all checkable properties (ADR-013.3).

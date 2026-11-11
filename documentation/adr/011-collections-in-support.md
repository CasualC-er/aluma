# ADR-011: The compiler's collections live in `support/`, not in `std`

Status: accepted. Plan reference: D11.

## Context

`std` has no list, map or set; collections are deferred to a later version of the
language [12.1]. `almc` needs all three, plus string building and sorting.

## Decision

Generic modules in `lib/support/`, shipped with the compiler, not part of `std`:
`Vec(T)`, `Map(K, V, equal, hash)`, `Set(T, equal, hash)`, `Builder`, `Sort(T, less)`,
and the decimal bignum of plan §2.6.

## Reasons

Putting them in `std` now would pre-empt a language decision that [12.1] deliberately
left open: the eventual `std` collections should be designed as a language feature, with
a spec amendment and an ADR of their own, not back-formed from whatever the compiler
happened to need. Keeping them in `support/` also means their API can change freely
while `almc` is being written.

They are built over `new(array of T, n)` with copy-grow — the `Lookup` example of [11]
is the pattern — and iterate in insertion order [8.5.3], which ADR-008 requires of
anything whose iteration can reach the IR.

## Consequences

- `support/` is on the module path for the compiler's own build, and not for user
  programs.
- Promotion to `std` later is a spec amendment, which is the right amount of friction.
- Each module gets a `*Test` module per style guide §13.

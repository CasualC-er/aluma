# `support`

The collections the compiler needs and `std` does not have: `Vec(T)`,
`Map(K, V, equal, hash)`, `Set(T, equal, hash)`, `Builder`, `Sort(T, less)`, and the
arbitrary-precision decimal of plan §2.6.

These are deliberately **not** `std` — see ADR-011. [12] "Deferred", item 1 leaves the
library's own collections unspecified, and shipping these as `std` would pre-empt that
decision.

Populated at Phase 9 (M5), before the compiler is ported to Alúma.

# `std`

The parts of the standard library that are written **in Alúma** rather than built into
the runtime: the string procedures of [9.4] (`concat`, `substring`, `find`, `split`,
`join`, `trim`, …) and whatever else needs no runtime support.

Everything here is compiled by the same compiler as a user program, which is the point:
it exercises the language early and keeps the host-language surface small (plan Phase 4).
A `*Test` module accompanies each, per style guide §13.

Populated at Phase 4, after M1.

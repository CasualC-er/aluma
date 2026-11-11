# ADR-004: Precise GC walking an explicit frame list

Status: accepted. Plan reference: D4.

## Context

[8.5.1] requires a task's live size to be recounted **exactly**, with `OutOfMemory`
firing exactly when the limit would be exceeded.

## Decision

Each procedure has a **frame object** with a compile-time **slot map** saying which
slots hold references, linked to its caller's frame. The collector walks the frame
list. Nothing depends on scanning the machine stack.

## Reasons

- **Conservative scanning is excluded by two rules together.** [8.5.1] requires that
  `OutOfMemory` be raised "exactly when, immediately after an allocation, the live size
  would exceed the limit", and that the size be recounted exactly whenever it could, "so
  collector timing is never observable". [8.7.2] then requires every run to produce the
  same output whatever the timing or the number of processors. A conservative stack scan
  retains garbage a precise one would free, and how much it retains depends on register
  allocation and stack layout — so the live-size count, and therefore whether
  `OutOfMemory` is raised at all, would vary with how the optimizer happened to allocate
  registers. That is exactly the observability those two rules rule out. It is a
  derivation, not a reading of one sentence: neither rule names garbage collection.
- LLVM's `gc.statepoint` would give real stack maps, but it is fiddly and constrains
  which passes may run. Explicit frames cost speed in a predictable way instead.
- **The same frame list pays for three things at once.** It gives the collector its
  roots, and — *at the moment an exception is raised*, which is when the chain is
  captured, since unwinding then pops the frames (ADR-006, `../ir.md` §6.3) — it gives
  the report format its call chain and the current site index that [7.5] needs — `uncaught Overflow in Stats::sum (task 1)`, the
  `called from:` chain, `evaluating: total + x`, `assert`'s condition text — with no
  dependence on DWARF or LLVM debug info.

## The site table carries source text, not spans

A site table of "site index → source span" would resolve only against the original
source files, which are not present at run time and whose host paths ADR-008 forbids
recording. So lowering copies the **snippet text** of each site into a per-procedure
string table in the binary. `documentation/ir.md` §6 is normative for its layout.

## Consequences

- Reference-holding locals stay in memory; `mem2reg` still promotes non-reference,
  non-aliased locals. Measure before optimizing.
- Frame slot stores must be visible before any call that can collect, which constrains
  the LLVM call attributes `almbe` emits (plan §2.4).
- The binary grows by the size of the embedded snippets. That is the price of [7.5]
  being a specified output format rather than a debugging aid.

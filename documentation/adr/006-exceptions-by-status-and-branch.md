# ADR-006: Exceptions by explicit status-and-branch

Status: accepted. Plan reference: D6.

## Context

Alúma has exceptions with `try`/`catch`, `raise`, bare re-`raise` and `defer` [5.1,
7.5]. The obvious implementation is LLVM's unwinding: `invoke`, landing pads, a
personality function, libunwind.

## Decision

No LLVM unwinding. Every call that can raise returns a **status** that the caller
tests, branching to its handler or to its cleanup chain.

## Reasons

- **[8.4] charges one step per unwound frame.** An explicit unwinder counts frames as
  it walks them; a foreign unwinder does not know about the step budget and cannot.
- **The `defer` rules are ordinary control flow this way.** Deferred calls run in
  reverse order, run on return, fallthrough *and* exception, the remaining ones still
  run if one raises, and a raising deferred call replaces the outgoing exception [5.1].
  Expressing that through a personality function is far harder than expressing it as
  branches.
- It keeps `almbe` free of platform ABI machinery, which matters for ADR-001's goal of
  a small, portable text emitter.

## The report record comes first

Because unwinding pops frames, and the [7.5] report is written only when the exception
leaves the task, the chain must be captured **before** the first frame is popped. The
report record — exception, task or initializer, and one (procedure, site) pair per live
frame — is built at the raise and travels with the exception; a bare re-`raise` keeps the
one it already has. See `../ir.md` §6.3.

## Consequences

- A branch after each call that can raise, next to a step poll that is already there.
- The IR must mark which calls can raise, and the verifier checks `try`-region nesting.
- Runtime entry points that can raise must not be marked `readnone` or `willreturn`
  (plan §2.4).

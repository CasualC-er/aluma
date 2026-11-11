# The Alúma IR

**Normative** for `almc0`, `almc` and `almbe`. The canonical serialization of this IR
is the self-hosting boundary (plan §0): the fixpoint test of M7 and the
`almc0`-vs-`almc` differential (ADR-013) are byte comparisons on a file in this format,
so anything underspecified here is a source of spurious diffs.

Status: skeleton. Sections marked *(Phase 3)* are filled in as lowering is written.

Bracketed section numbers point into `language_reference.md`.

## 1. Shape

Typed, three-address, over explicit frames (ADR-003). Not SSA: [7.2] makes aliasing
real and ordered, so a value cannot be assumed to live in a register across a call.

A **place** is a tagged reference to one of:

| Place | Meaning |
|---|---|
| frame slot | a local or parameter of a live frame |
| field | a record field of an object |
| element | an array element, after a bounds check |
| read-only | an object in the shared region [4.2] or in a version [8.12]; a write raises `ReadOnly` |

All parameters are places [7.2]. A non-variable argument materialises a caller
temporary and passes a place to it.

## 2. Types *(Phase 3)*

Name-equivalent [7.1], so a type is an identifier into the type table, not a structure.
The table is written in first-occurrence order (ADR-008).

## 3. Instructions *(Phase 3)*

Three-address. Each instruction names its result place and its operand places. Includes
explicitly: place computations with their bounds and `nil` checks, charge sites
(ADR-005), safepoints, `try` region boundaries, `raise`, and `defer` record
construction.

## 4. Frames and slot maps

Every procedure declares a frame: the number of slots, and a **slot map** marking which
slots hold references. The collector walks the frame list and reads the slot maps
(ADR-004); the verifier checks that the map covers every slot the body uses.

## 5. Charge sites

One per event in the step table of [8.4], never merged (ADR-005). A charge site carries
its constant cost, or a marker that the cost is computed at run time (copy price, limb
counts). The verifier checks that every chargeable event has one.

## 6. Site tables, source snippets and the report record

**Normative, and the reason is not debugging.** [7.5] requires the *runtime* to print
the source text of what was being evaluated:

```
uncaught Overflow in Stats::sum (task 1)
  evaluating: total + x
  called from: Stats::average
  called from: App::start
```

and `assert(c)` must show the text of `c` [9].

### 6.1 Why a span is not enough

A table of "site index → source span" cannot satisfy this. A span resolves only against
the original source file, which is not present when the compiled program runs, and whose
host path ADR-008 forbids recording in the artifact. So each procedure carries a **site
table**: site index → an offset into a per-procedure **string table holding the snippet
text itself**, which `almbe` emits as a data section and `almrt` reads when formatting a
report. A separate per-module table holds the **module-qualified procedure names**
(`Stats::sum`) and the declared exception names (`IntList::Empty`), which the report also
needs and which are likewise absent at run time.

The snippets are therefore part of the bytes the fixpoint test compares, which is
correct: they are a function of the source.

### 6.2 Snippet form

A snippet is the source text of the span, with comments removed, every run of
whitespace — newlines included — collapsed to one space, and the result trimmed. The
report format is line-oriented (`  evaluating: <text>`), so a snippet can never contain
a newline, and a statement spread over four lines prints as one line.

### 6.3 The report record is captured at the raise, not at the report

This is the part that is easy to get wrong. Exceptions unwind by explicit
status-and-branch (ADR-006), and unwinding **pops frames as it goes**, charging a step
per frame [8.4]. The report, however, is written much later: for the initial task when
the exception leaves it, and for any other task at the boundary at which the task ends
[8.6]. By then the frames the chain describes no longer exist. ADR-004's claim that the
frame list supplies the `called from:` chain is therefore only true *at the moment of the
raise*.

So, at the point an exception is raised — whether by an explicit `raise` or by a runtime
check — and **before any frame is popped**, the runtime captures a **report record**:

- the exception's identity (a predefined one, or a module-qualified declared one),
- the task number, or the initializer's `Module::name` when running in pseudo-task 0
  [4.2], which is the `(initialization of Module::Name)` form of [7.5],
- and, walking the still-live frame list innermost-first, one entry per frame: the
  procedure's qualified-name id and that frame's **current site index**.

The record travels with the in-flight exception. Capturing it costs one pass over the
frames, which the unwinder is about to make anyway.

**A bare re-`raise` keeps the original record.** [5.1] allows re-raising with a bare
`raise`, and [7.5] does not say which raise point the report then names. Naming the
original is the useful answer — it is where the fault occurred — and it is what keeping
the record gives for free. Logged as amendment A-003, because the reference should say
so explicitly.

**A deferred `OutOfMemory` needs no extra machinery.** When growth at a boundary pushes a
task over its limit, [8.5.1] says the exception is raised from "its next return from
`sync` or `await`" — so the raise point, and therefore the site in the record, is that
call, not the allocation or the `send` that caused the growth. Nothing has to remember
the earlier site.

## 7. Canonical serialization

See ADR-008. In short: first-occurrence ordering for the constant pool and string table,
module order = the `share`-init traversal of [4.2], no timestamps, no host paths, and no
map-iteration order anywhere in either compiler.

Two forms exist: a textual form for debugging, and the binary form that is the artifact.
Only the binary form is compared.

## 8. Verification

`alm-core` verifies every IR it loads (ADR-013.3): types, place legality, slot-map
coverage, charge-site presence, `try`-region nesting. Run on every compile in CI.

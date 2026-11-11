# Amendments

Places where implementing the language forced a decision the Language Reference does
not settle: an ambiguity, a conflict between its prose and its grammar, or a rule the
implementation found to be unworkable as written. Each entry states the decision taken
and where it is implemented, so that the reference can be corrected deliberately rather
than drifting to match the code.

An entry here is **not** a licence to diverge. It is a record that a divergence exists.

| # | Subject | Status |
|---|---|---|
| A-001 | Hexadecimal digits in a `fixed` literal | decided |
| A-002 | `--seed` is not fixed at compile time | decided |
| A-003 | Which raise point a bare re-`raise` reports | decided |

---

## A-001 — Hexadecimal digits in a `fixed` literal

**Where.** [2] and the grammar in [10].

**The conflict.** The prose in [2] gives a fixed literal as "digits `v` digits", and its
example is `12v50`. The grammar in [10] has

```
fixedlit    = number "v" number .
number      = digit { digit } | "0x" hexdigit { hexdigit } .
```

and `number` admits hexadecimal, so the grammar accepts `0xFFv10` and `12v0xA` while
the prose does not.

**Decision.** The prose governs: **a fixed literal is decimal digits, `v`, decimal
digits.** `0xFFv10` is a compile error.

**Why.** The scale of a fixed literal is "the number of digits after `v`" [2], which is
a decimal notion — `0v0A` would have to mean scale 2 with the value 10/100, and the
whole point of `fixed` is that the written form is the value. Hexadecimal exists in the
language for bit patterns in `int`, where there is no scale.

**Consequence for the reference.** `fixedlit` should read
`fixedlit = digit { digit } "v" digit { digit } .`

**Implemented in.** The lexer, Phase 1. A negative test pins `0xFFv10`.

---

## A-002 — `--seed` is not fixed at compile time

**Where.** [8.9], and [8.7] for replay.

**The conflict.** [8.9] opens by saying its options "are fixed when the program is
compiled and are part of the program's identity, so a replay uses the same values", and
then gives `--seed`'s default as "chosen from the operating system and recorded". A
value chosen from the operating system cannot have been fixed when the program was
compiled, and "and recorded" only makes sense for something chosen per run.

**Decision.** The seed is **bound at startup, and always recorded**:

- `--seed N` given: the value is baked into the binary and is part of its identity. Every
  run uses it.
- `--seed` not given: the runtime draws a seed at startup and writes it to the replay log
  as a record. The binary's identity carries `seed=os`, not a value.

Every other option in [8.9] is fixed at compile time as the text says.

**Why.** It is the only reading under which both halves of the sentence are true, and it
is the one that makes replay work: a recording and its replay have the same identity
string, and the drawn value travels in the log where the other outside events are.

**Consequence for the reference.** [8.9] should exempt `--seed` from the opening
sentence, and say that an unspecified seed is drawn at startup and recorded.

**Implemented in.** `alm-core::options` (`Seed::FromOs` / `Seed::Fixed`, and
`Options::identity`). ADR-010 and ADR-012.

---

## A-003 — Which raise point a bare re-`raise` reports

**Where.** [7.5] (the report format) and [5.1] (`raise` with no exception name).

**The gap.** [5.1] allows a handler to re-raise the exception it caught with a bare
`raise`. [7.5] says the report's first line names "the procedure that raised" and its
second "the source text of the expression or statement being evaluated at that point
(for an explicit `raise`, the `raise` statement)". After a bare re-`raise` there are two
candidates — the original fault, and the `raise` in the handler — and the reference does
not say which is reported.

**Decision.** The report names the **original** raise point: the procedure, site and call
chain captured when the exception was first raised. A bare re-`raise` does not replace
them.

**Why.** It is the useful answer — the original site is where the fault is — and it falls
out of the implementation for nothing: the report record is captured at the raise, before
unwinding pops the frames (`ir.md` §6.3), so a bare re-`raise` simply keeps the record it
is already carrying. Reporting the handler's `raise` instead would mean capturing a
second record and discarding the first, which is both more work and less informative.

Note that a `raise` naming an exception explicitly is a *new* exception even if it names
the same one, and gets a new record. Only the bare form is affected.

**Consequence for the reference.** [7.5] should add that a bare re-`raise` reports the
original raise point and chain.

**Implemented in.** `documentation/ir.md` §6.3, ADR-006. Phase 3 lowering and Phase 4
report formatting.

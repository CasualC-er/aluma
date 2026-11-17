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
| A-004 | A procedure call is not a constant expression | decided |

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

---

## A-004 — A procedure call is not a constant expression

**Where.** [4] ("Constants are compile-time constant expressions"), the grammar of [10],
and the three places that require a constant *argument*: `strFixed(s, k)` [9.2],
`divide(a, b, k)` and `rescale(x, k)` [7.4], and `power`'s exponent [9.5].

**The gap.** The grammar puts calls inside `expr`, so `const Limit is power(2, 10);`
parses. [4] says a constant is a "compile-time constant expression" without saying
whether a call is one. The three requirements above are about arguments being constant,
not about calls folding, so nothing settles it either way.

**Decision.** A procedure call is **not** a constant expression. Every call in a
constant position is a compile error, including a call of a library procedure whose
arguments are all constants.

**Why.** A constant must be determined by the program text alone. If a call could
appear, a constant's value would come from *executing* something — the library's
semantics, or a user procedure's body — and the compiler would have to agree with the
runtime about that execution to agree about the constant. Two consequences follow, and
both are worse than the inconvenience this rule causes:

- A constant would depend on run-time behaviour to be known at compile time, which is
  the opposite of what "compile-time constant" means. Sizes [4] and the `p` and `s` of
  a `fixed` type [7.4] are constants precisely so that a type is fixed by the source.
- `almc` and `almc0` are compared on byte-identical IR (ADR-013.1), so any folding rule
  is a rule both must implement identically. A folding rule that reaches into the
  library makes the IR depend on two implementations of the library agreeing, which is
  a far larger surface than the front end the differential is meant to cover.

**What it costs.** `type Buf is array [power(2, 10)] of int;` must be written with a
named constant: `const Size is 1024; type Buf is array [Size] of int;`. No example in
the reference, the guide or the style guide puts a call in a constant — the only two
constant declarations in all three documents are `const Size is 100;` and
`const Keep is 64;` — so nothing existing is affected.

**The direction is deliberate.** Permitting more folding later breaks no program that
can be written today; retracting permission would break every program that relied on
it. If a case for folding a named set of pure built-ins ever arises, it is an amendment
of its own with that set written out.

**Consequence for the reference.** [4] should say that a constant expression is built
from literals, named constants, enumeration names and operators, and that it contains no
calls.

**Implemented in.** `almc0::consts` rejects every call, naming the rule.

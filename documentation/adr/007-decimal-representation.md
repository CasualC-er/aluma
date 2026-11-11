# ADR-007: Decimal as sign and magnitude in base-10^18 limbs, plus a scale

Status: accepted. Plan reference: D7.

## Context

`fixed(p, s)` is a decimal type with precision `p` and scale `s` [7.4]. The size table
of [8.4] says a `fixed(p, s)` occupies **⌈p / 18⌉ units**, and the step table charges
a decimal add or subtract the **limb count of the larger operand** and a multiply or
divide the **product of the operands' limb counts**.

## Decision

Sign plus magnitude in **base-10^18 limbs**, with a scale.

**Where the code lives.** The arithmetic itself is implemented in `alm-core`, so that
`almc0`'s constant folding and the runtime cannot disagree about a rounding mode or an
overflow boundary (ADR-002). `almrt` links it and exposes the entry points generated
code calls; `alm-core::steps` holds the cost tables. One implementation, used from two
places.

## Reasons

**18 digits per limb is forced, not chosen.** The size table charges one unit per 18
digits and the step table charges per limb; if a limb held any other number of digits,
the published costs would be wrong and every step count in every program would differ
from the specification. The radix is part of the language's observable behaviour.

Base 10 rather than binary because the arithmetic is specified in decimal: scales
propagate by decimal digit count (`+`/`-` take the maximum scale, `*` the sum, `/` the
sum), and rounding happens at decimal positions [7.4]. A binary representation would
make every one of those an approximation to correct afterwards.

## The limbs hold the unscaled integer

A `fixed(p, s)` is **one integer — the value multiplied by 10^s — held in base-10^18
limbs, plus the scale `s`**. The limbs are *not* aligned at the decimal point, and the
integer and fractional parts are not stored separately.

**A stored value occupies a fixed `⌈p / 18⌉` limbs, decided by its declared precision,
not by the magnitude it happens to hold.** A `fixed(30, 2)` variable is two limbs whether
it holds 1 or 10^29. This matters twice over: the size table charges by the declared
type, so a value-dependent length would make a task's live size depend on its data
rather than on its declarations; and the step table charges a decimal add "the limb count
of the larger operand", which is then a **compile-time constant** for any two declared
operands. Variable-length magnitudes appear only in the unbounded intermediates of
`+ - *`, which are temporaries that no size or step rule measures.

This is forced by the size table. If limbs were aligned at the decimal point, a
`fixed(30, 1)` would need two limbs for its 29 integer digits and one for its single
fractional digit — three limbs — against the `⌈30 / 18⌉` = **two** units the size table
charges, and the add and multiply charges of [8.4] would then disagree with the size of
the thing being added. One scaled integer makes limb count and unit count the same
number by construction.

Aligning limbs at the decimal point, which would make `rescale` cheaper, is therefore a
stated non-goal.

## The compile-time bignum is a different thing

The paragraph above is about a **stored** `fixed(p, s)`, whose radix is observable
through the size and step tables. The compiler also needs unbounded decimal arithmetic
to fold constants, where no size or step rule applies: an intermediate of `+`, `-` or
`*` is "exact however many digits it needs" [7.4] and is measured by nothing.

`alm_core::decimal` is therefore free to use whatever radix is easiest to get right, and
uses base 10⁹ so that every limb product fits a `u64`. That is not a second
representation of the same thing: it is the unbounded intermediate, and the two meet
only at an assignment, where the value is range-checked against a declared `p` and `s`.

## Consequences

- Multiplication of two limbs needs `u128`; available in Rust, no bignum dependency.
- Intermediates of `+`, `-` and `*` are unbounded: only assignment range-checks, raising
  `Overflow` or `LostPrecision` [7.4]. The representation must therefore grow freely and
  be narrowed only at an assignment.
- `int` is a separate thing: checked `i64` with Euclidean `div`/`mod` [7.3], not a
  one-limb decimal.
- `almc` cannot use this for its own constant folding: it must fold `fixed(p, s)` with
  unbounded `p`, while its own declarations need a constant `p`. Hence the separate
  decimal bignum in `support/`, base 10^9 so products fit an `int` (plan §2.6).

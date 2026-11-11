//! The step and size tables of [8.4].
//!
//! Both tables are shared because three different pieces have to agree about them:
//! `almc0`/`almc` decide where a charge site goes and what constant it charges
//! (ADR-005), `almbe` lowers those sites, and `almrt` charges what it cannot know
//! statically (a copy price depends on the object graph at run time).
//!
//! Charges are **per event and are never coalesced** (ADR-005): a task yields
//! exactly where its budget crosses zero, and under `--strict-steps` `OutOfSteps`
//! lands at a specific site, so folding a block's charges into one subtraction moves
//! an observable yield point.

/// Steps charged for the fixed-cost events of the step table.
///
/// `int` arithmetic, comparison and indexing are free, and so are expressions in
/// general: their size is fixed by the program text, and everything that loops is
/// charged through a condition or a call.
pub mod cost {
    /// A simple statement: assignment, call, `return`, `raise`, `defer`.
    pub const SIMPLE_STATEMENT: u64 = 1;
    /// Evaluating the condition of `if`, `elsif` or `while`, each time.
    pub const CONDITION: u64 = 1;
    /// Entering a user-defined procedure.
    pub const PROCEDURE_ENTRY: u64 = 1;
    /// Unwinding one procedure frame. Counted by our own unwinder (ADR-006).
    pub const FRAME_UNWIND: u64 = 1;
    /// `round`, `replyTo`, `link`, `cancel`, `awaitEnd`, `disown`, `ended`, `failed`,
    /// `closing`, `newView`, `latest`, `version`, `awaitChange`.
    pub const TASK_OPERATION: u64 = 1;
    /// The fixed part of `spawn` (1 + the copy price of the arguments).
    pub const SPAWN_BASE: u64 = 1;
    /// The fixed part of `publish` (1 + the copy price of the graph).
    pub const PUBLISH_BASE: u64 = 1;
}

/// Size units for a `fixed(p, s)`: one unit per 18 digits, `⌈p / 18⌉`.
///
/// 18 is not a free choice: it is why ADR-007 stores decimals in base 10^18 limbs.
/// A unit *is* a limb, so the published add/subtract and multiply costs below are
/// only correct for that radix.
pub const DIGITS_PER_LIMB: u32 = 18;

/// Limb count of a `fixed(p, s)` value, which is also its size in units.
pub const fn fixed_limbs(precision: u32) -> u64 {
    // `p` is at least 1 for any declarable type, but be defensive: a zero-digit
    // number still occupies one limb.
    if precision == 0 {
        1
    } else {
        precision.div_ceil(DIGITS_PER_LIMB) as u64
    }
}

/// Steps for a `fixed` add or subtract: the limb count of the larger operand.
pub const fn fixed_add_cost(left_limbs: u64, right_limbs: u64) -> u64 {
    if left_limbs > right_limbs {
        left_limbs
    } else {
        right_limbs
    }
}

/// Steps for a `fixed` multiply or divide: the product of the operands' limb counts.
pub const fn fixed_mul_cost(left_limbs: u64, right_limbs: u64) -> u64 {
    left_limbs.saturating_mul(right_limbs)
}

/// Units charged per 16 units of copied payload, per the copy-price rule.
pub const UNITS_PER_COPY_STEP: u64 = 16;

/// The copy price of a set of objects: one step per object, plus one step per 16
/// units, rounded up.
///
/// Charged by `new`, `send` and `answer`, and as the variable part of `spawn` and
/// `publish`. A reference into the shared region or into a version counts as one
/// unit and is not followed, so `units` must already have been computed under that
/// rule — see [8.4] and [8.12].
pub const fn copy_price(objects: u64, units: u64) -> u64 {
    objects.saturating_add(units.div_ceil(UNITS_PER_COPY_STEP))
}

/// Size in units of the one-unit values: `int`, `bool`, `char`, any reference,
/// `mailbox`, `reply`, `publisher`, `task`, `File`, `Listener`, and a `view` whose
/// owner is still live.
pub const SCALAR_UNITS: u64 = 1;

/// Units of a record: 1 for the record itself plus the sum of its fields.
pub fn record_units(field_units: impl IntoIterator<Item = u64>) -> u64 {
    1 + field_units.into_iter().sum::<u64>()
}

/// Units of a fixed-length `array [n] of T`: `n × size(T)`, with no header.
pub const fn array_units(n: u64, element_units: u64) -> u64 {
    n.saturating_mul(element_units)
}

/// Units of a `ref array of T` of length `n`: `1 + n × size(T)`.
pub const fn ref_array_units(n: u64, element_units: u64) -> u64 {
    1 + n.saturating_mul(element_units)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limbs_are_eighteen_digits_each() {
        assert_eq!(fixed_limbs(1), 1);
        assert_eq!(fixed_limbs(18), 1);
        assert_eq!(fixed_limbs(19), 2);
        assert_eq!(fixed_limbs(36), 2);
        assert_eq!(fixed_limbs(37), 3);
    }

    #[test]
    fn add_takes_the_larger_and_multiply_the_product() {
        assert_eq!(fixed_add_cost(fixed_limbs(20), fixed_limbs(5)), 2);
        assert_eq!(fixed_add_cost(fixed_limbs(5), fixed_limbs(20)), 2);
        assert_eq!(fixed_mul_cost(fixed_limbs(20), fixed_limbs(40)), 2 * 3);
    }

    #[test]
    fn copy_price_rounds_the_unit_part_up() {
        // One object of 1 unit: 1 + ⌈1/16⌉ = 2.
        assert_eq!(copy_price(1, 1), 2);
        assert_eq!(copy_price(1, 16), 2);
        assert_eq!(copy_price(1, 17), 3);
        // Nothing copied costs nothing.
        assert_eq!(copy_price(0, 0), 0);
    }

    #[test]
    fn the_unit_part_is_rounded_over_the_total_not_per_object() {
        // [8.4]: "one step for each object plus one step for each 16 units, rounded
        // up". The rounding applies once, to the total — two 1-unit objects cost
        // 2 + ⌈2/16⌉ = 3, not 2 + 1 + 1 = 4. This test exists to pin that reading,
        // because both are grammatical readings of the sentence and they differ.
        assert_eq!(copy_price(2, 2), 3);
        assert_eq!(copy_price(3, 33), 6);
        // A graph of many small objects is charged mostly per object.
        assert_eq!(copy_price(100, 100), 100 + 7);
    }

    #[test]
    fn a_zero_digit_precision_still_occupies_one_limb() {
        // Not reachable from a declarable type — `fixed(0, s)` is not one — but the
        // branch exists, so it is pinned rather than left to a future refactor.
        assert_eq!(fixed_limbs(0), 1);
    }

    #[test]
    fn the_sizes_the_spec_works_out_by_hand() {
        // "a tree node with a key and two children is 4 units" [8.4].
        assert_eq!(record_units([SCALAR_UNITS, SCALAR_UNITS, SCALAR_UNITS]), 4);
        // "a 1000-character string is 1001" — a string is a `ref array of char` [3.4].
        assert_eq!(ref_array_units(1000, SCALAR_UNITS), 1001);
        // A fixed-length array has no header of its own.
        assert_eq!(array_units(1000, SCALAR_UNITS), 1000);
    }

    #[test]
    fn a_wide_decimal_array_does_not_overflow_the_counter() {
        // Saturating, not wrapping: an absurd size must report OutOfMemory, not wrap
        // to a small live size and be allowed.
        assert_eq!(array_units(u64::MAX, 2), u64::MAX);
        assert_eq!(fixed_mul_cost(u64::MAX, 2), u64::MAX);
    }
}

//! Exact decimal arithmetic [7.4].
//!
//! `fixed(p, s)` is "an exact decimal number with at most `p` digits in total, of which
//! `s` follow the decimal point ... stored as an arbitrary-precision integer scaled by
//! 10ˢ". Intermediate results of `+`, `-` and `*` are exact however many digits they
//! need; only an assignment is range-checked. So the type here is **unbounded**: a
//! sign, a magnitude and a scale, with no precision limit of its own. Precision is a
//! property of a declared type, checked at an assignment, not of a value.
//!
//! One implementation, used from two places (ADR-002): the compiler folds constants
//! with it, and the runtime computes with it, so the two cannot disagree about a
//! rounding mode or a boundary.
//!
//! ## Representation
//!
//! Sign and magnitude, the magnitude in base 10⁹ limbs, little-endian, plus a scale.
//! Base 10⁹ keeps every limb product inside a `u64`.
//!
//! This is *not* the storage layout of a `fixed` at run time, which ADR-007 fixes at
//! base 10¹⁸ because the size table of [8.4] charges `⌈p / 18⌉` units and the step
//! table charges per limb. That radix is observable through those tables; this one is
//! not, because an unbounded intermediate is measured by neither. Keeping them apart
//! lets the compile-time arithmetic use whatever radix is easiest to get right.
//!
//! ## Division
//!
//! Long division, one decimal digit at a time, each digit found by trial subtraction.
//! Knuth's algorithm D would be faster and is a classic source of subtle bugs; these
//! are compile-time constants and runtime values of a few limbs, where the difference
//! cannot matter and correctness can be read off the code.

use std::cmp::Ordering;

/// The rounding modes of [7.4].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Rounding {
    /// Nearest, ties away from zero. What `rounded` does.
    HalfAway,
    /// Nearest, ties to the even digit.
    HalfEven,
    /// Toward zero.
    Down,
    /// Away from zero.
    Up,
    /// Toward minus infinity.
    Floor,
    /// Toward plus infinity.
    Ceiling,
}

impl Rounding {
    /// The name [9.5] gives it, as a program writes it.
    pub const fn name(self) -> &'static str {
        match self {
            Rounding::HalfAway => "RoundHalfAway",
            Rounding::HalfEven => "RoundHalfEven",
            Rounding::Down => "RoundDown",
            Rounding::Up => "RoundUp",
            Rounding::Floor => "RoundFloor",
            Rounding::Ceiling => "RoundCeiling",
        }
    }

    pub fn from_name(name: &str) -> Option<Rounding> {
        [
            Rounding::HalfAway,
            Rounding::HalfEven,
            Rounding::Down,
            Rounding::Up,
            Rounding::Floor,
            Rounding::Ceiling,
        ]
        .into_iter()
        .find(|m| m.name() == name)
    }
}

/// Base of a limb. Chosen so that a product of two limbs fits a `u64`.
const BASE: u64 = 1_000_000_000;
const BASE_DIGITS: u32 = 9;

/// An exact decimal: sign, magnitude and scale.
///
/// The value is `(-1)^negative × magnitude × 10^-scale`. Zero is always positive, so
/// there is no negative zero to compare wrongly.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Decimal {
    negative: bool,
    /// Base-10⁹ limbs, least significant first, no leading zero limbs.
    limbs: Vec<u64>,
    scale: u32,
}

impl Decimal {
    pub fn zero() -> Decimal {
        Decimal {
            negative: false,
            limbs: Vec::new(),
            scale: 0,
        }
    }

    pub fn is_zero(&self) -> bool {
        self.limbs.is_empty()
    }

    pub fn is_negative(&self) -> bool {
        self.negative
    }

    pub fn scale(&self) -> u32 {
        self.scale
    }

    /// From an `int` [7.3], which "acts as a `fixed` of scale 0".
    pub fn from_i64(v: i64) -> Decimal {
        let negative = v < 0;
        // `unsigned_abs` so that `i64::MIN` works.
        let mut magnitude = v.unsigned_abs();
        let mut limbs = Vec::new();
        while magnitude > 0 {
            limbs.push(magnitude % BASE);
            magnitude /= BASE;
        }
        Decimal {
            negative,
            limbs,
            scale: 0,
        }
        .normalized()
    }

    /// From the digits and scale of a `fixed` literal [2]: `12v50` is `("1250", 2)`.
    ///
    /// The digits are the unscaled integer, most significant first. Returns `None` if
    /// anything other than ASCII digits appears.
    pub fn from_digits(digits: &str, scale: u32, negative: bool) -> Option<Decimal> {
        if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
            return None;
        }
        let mut limbs = Vec::new();
        let bytes = digits.as_bytes();
        // Chunk from the least significant end, nine digits at a time.
        let mut end = bytes.len();
        while end > 0 {
            let start = end.saturating_sub(BASE_DIGITS as usize);
            let chunk = std::str::from_utf8(&bytes[start..end]).ok()?;
            limbs.push(chunk.parse::<u64>().ok()?);
            end = start;
        }
        Some(
            Decimal {
                negative,
                limbs,
                scale,
            }
            .normalized(),
        )
    }

    /// Parse the number text of [9.2]: ASCII digits, an optional leading `-`, and for a
    /// fractional part a `.` with at least one digit on each side.
    ///
    /// "There is no `+`, no spaces, no exponent and no locale; empty text is a
    /// `FormatError`. Leading zeros are accepted."
    pub fn parse(text: &str) -> Option<Decimal> {
        let (negative, rest) = match text.strip_prefix('-') {
            Some(r) => (true, r),
            None => (false, text),
        };
        if rest.is_empty() {
            return None;
        }
        match rest.split_once('.') {
            None => Decimal::from_digits(rest, 0, negative),
            Some((whole, frac)) => {
                // "at least one digit on each side"
                if whole.is_empty() || frac.is_empty() {
                    return None;
                }
                let joined = format!("{whole}{frac}");
                Decimal::from_digits(&joined, frac.len() as u32, negative)
            }
        }
    }

    /// Drop leading zero limbs, and never keep a negative zero.
    fn normalized(mut self) -> Decimal {
        while self.limbs.last() == Some(&0) {
            self.limbs.pop();
        }
        if self.limbs.is_empty() {
            self.negative = false;
        }
        self
    }

    /// The number of digits in the magnitude: the `p` an assignment checks against.
    ///
    /// Zero has one digit, which matters because `fixed(1, 0)` must hold 0.
    pub fn digits(&self) -> u32 {
        match self.limbs.last() {
            None => 1,
            Some(top) => {
                let top_digits = top.to_string().len() as u32;
                (self.limbs.len() as u32 - 1) * BASE_DIGITS + top_digits
            }
        }
    }

    /// The digits before the decimal point, which is what `Overflow` is about: "too
    /// many integer digits raises `Overflow`".
    pub fn integer_digits(&self) -> u32 {
        self.digits().saturating_sub(self.scale).max(1)
    }

    // -- magnitude helpers --------------------------------------------------

    fn cmp_magnitude(a: &[u64], b: &[u64]) -> Ordering {
        if a.len() != b.len() {
            return a.len().cmp(&b.len());
        }
        for (x, y) in a.iter().rev().zip(b.iter().rev()) {
            if x != y {
                return x.cmp(y);
            }
        }
        Ordering::Equal
    }

    fn add_magnitude(a: &[u64], b: &[u64]) -> Vec<u64> {
        let mut out = Vec::with_capacity(a.len().max(b.len()) + 1);
        let mut carry = 0;
        for i in 0..a.len().max(b.len()) {
            let sum = a.get(i).copied().unwrap_or(0) + b.get(i).copied().unwrap_or(0) + carry;
            out.push(sum % BASE);
            carry = sum / BASE;
        }
        if carry > 0 {
            out.push(carry);
        }
        out
    }

    /// `a - b`, which the caller must know is not negative.
    fn sub_magnitude(a: &[u64], b: &[u64]) -> Vec<u64> {
        debug_assert!(Decimal::cmp_magnitude(a, b) != Ordering::Less);
        let mut out = Vec::with_capacity(a.len());
        let mut borrow = 0i64;
        for (i, limb) in a.iter().enumerate() {
            let mut d = *limb as i64 - b.get(i).copied().unwrap_or(0) as i64 - borrow;
            if d < 0 {
                d += BASE as i64;
                borrow = 1;
            } else {
                borrow = 0;
            }
            out.push(d as u64);
        }
        debug_assert_eq!(borrow, 0);
        out
    }

    fn mul_magnitude(a: &[u64], b: &[u64]) -> Vec<u64> {
        if a.is_empty() || b.is_empty() {
            return Vec::new();
        }
        let mut out = vec![0u64; a.len() + b.len()];
        for (i, &x) in a.iter().enumerate() {
            let mut carry = 0u64;
            for (j, &y) in b.iter().enumerate() {
                // x, y < 10^9, so the product is under 10^18 and fits a u64 with room
                // for the carry and the accumulator.
                let cur = out[i + j] + x * y + carry;
                out[i + j] = cur % BASE;
                carry = cur / BASE;
            }
            let mut k = i + b.len();
            while carry > 0 {
                let cur = out[k] + carry;
                out[k] = cur % BASE;
                carry = cur / BASE;
                k += 1;
            }
        }
        out
    }

    /// Multiply the magnitude by 10^k.
    fn shift_magnitude(limbs: &[u64], k: u32) -> Vec<u64> {
        if limbs.is_empty() {
            return Vec::new();
        }
        let whole = (k / BASE_DIGITS) as usize;
        let rest = k % BASE_DIGITS;
        let mut out = vec![0u64; whole];
        out.extend_from_slice(limbs);
        if rest > 0 {
            let factor = 10u64.pow(rest);
            let mut carry = 0;
            for limb in out.iter_mut().skip(whole) {
                let cur = *limb * factor + carry;
                *limb = cur % BASE;
                carry = cur / BASE;
            }
            while carry > 0 {
                out.push(carry % BASE);
                carry /= BASE;
            }
        }
        out
    }

    /// This value with its scale raised to `scale`, which loses nothing.
    fn with_scale_at_least(&self, scale: u32) -> Decimal {
        debug_assert!(scale >= self.scale);
        let k = scale - self.scale;
        if k == 0 {
            return self.clone();
        }
        Decimal {
            negative: self.negative,
            limbs: Decimal::shift_magnitude(&self.limbs, k),
            scale,
        }
        .normalized()
    }

    // -- arithmetic ---------------------------------------------------------

    pub fn negated(&self) -> Decimal {
        if self.is_zero() {
            return self.clone();
        }
        Decimal {
            negative: !self.negative,
            limbs: self.limbs.clone(),
            scale: self.scale,
        }
    }

    /// Exact addition, "with the larger of the two scales" [7.4].
    pub fn add(&self, other: &Decimal) -> Decimal {
        let scale = self.scale.max(other.scale);
        let a = self.with_scale_at_least(scale);
        let b = other.with_scale_at_least(scale);
        if a.negative == b.negative {
            return Decimal {
                negative: a.negative,
                limbs: Decimal::add_magnitude(&a.limbs, &b.limbs),
                scale,
            }
            .normalized();
        }
        match Decimal::cmp_magnitude(&a.limbs, &b.limbs) {
            Ordering::Equal => Decimal {
                negative: false,
                limbs: Vec::new(),
                scale,
            },
            Ordering::Greater => Decimal {
                negative: a.negative,
                limbs: Decimal::sub_magnitude(&a.limbs, &b.limbs),
                scale,
            }
            .normalized(),
            Ordering::Less => Decimal {
                negative: b.negative,
                limbs: Decimal::sub_magnitude(&b.limbs, &a.limbs),
                scale,
            }
            .normalized(),
        }
    }

    /// Exact subtraction, with the larger of the two scales [7.4].
    pub fn sub(&self, other: &Decimal) -> Decimal {
        self.add(&other.negated())
    }

    /// Exact multiplication: "the scales add" [7.4].
    pub fn mul(&self, other: &Decimal) -> Decimal {
        Decimal {
            negative: self.negative != other.negative,
            limbs: Decimal::mul_magnitude(&self.limbs, &other.limbs),
            scale: self.scale + other.scale,
        }
        .normalized()
    }

    /// Comparison "by numeric value, so `1v5 = 1v50`" [7.4].
    pub fn compare(&self, other: &Decimal) -> Ordering {
        if self.is_zero() && other.is_zero() {
            return Ordering::Equal;
        }
        match (self.negative, other.negative) {
            (false, true) => return Ordering::Greater,
            (true, false) => return Ordering::Less,
            _ => {}
        }
        let scale = self.scale.max(other.scale);
        let a = self.with_scale_at_least(scale);
        let b = other.with_scale_at_least(scale);
        let magnitudes = Decimal::cmp_magnitude(&a.limbs, &b.limbs);
        if self.negative {
            magnitudes.reverse()
        } else {
            magnitudes
        }
    }

    /// Divide the magnitude by another, returning (quotient, remainder).
    ///
    /// Decimal long division: one digit of the quotient at a time, found by trial
    /// subtraction. See the note at the top of this file on why not Knuth D.
    fn divmod_magnitude(a: &[u64], b: &[u64]) -> (Vec<u64>, Vec<u64>) {
        debug_assert!(!b.is_empty(), "the caller checks for a zero divisor");
        if Decimal::cmp_magnitude(a, b) == Ordering::Less {
            return (Vec::new(), a.to_vec());
        }
        // Work in decimal digits, most significant first.
        let digits = Decimal::to_digits(a);
        let mut quotient: Vec<u8> = Vec::with_capacity(digits.len());
        let mut remainder: Vec<u64> = Vec::new();
        for d in digits {
            // remainder = remainder * 10 + d
            remainder = Decimal::shift_magnitude(&remainder, 1);
            remainder = Decimal::add_magnitude(&remainder, &[d as u64]);
            while remainder.last() == Some(&0) {
                remainder.pop();
            }
            let mut q = 0u8;
            while Decimal::cmp_magnitude(&remainder, b) != Ordering::Less {
                remainder = Decimal::sub_magnitude(&remainder, b);
                while remainder.last() == Some(&0) {
                    remainder.pop();
                }
                q += 1;
            }
            quotient.push(q);
        }
        (Decimal::from_decimal_digits(&quotient), remainder)
    }

    /// The magnitude as decimal digits, most significant first.
    fn to_digits(limbs: &[u64]) -> Vec<u8> {
        if limbs.is_empty() {
            return vec![0];
        }
        let mut s = String::new();
        for (i, limb) in limbs.iter().enumerate().rev() {
            if i == limbs.len() - 1 {
                s.push_str(&limb.to_string());
            } else {
                s.push_str(&format!("{limb:0>9}"));
            }
        }
        s.bytes().map(|b| b - b'0').collect()
    }

    fn from_decimal_digits(digits: &[u8]) -> Vec<u64> {
        let text: String = digits.iter().map(|d| (d + b'0') as char).collect();
        let mut limbs = Vec::new();
        let bytes = text.as_bytes();
        let mut end = bytes.len();
        while end > 0 {
            let start = end.saturating_sub(BASE_DIGITS as usize);
            let chunk = std::str::from_utf8(&bytes[start..end]).expect("digits");
            limbs.push(chunk.parse::<u64>().expect("nine digits"));
            end = start;
        }
        while limbs.last() == Some(&0) {
            limbs.pop();
        }
        limbs
    }

    /// Divide to exactly `scale` digits, rounding as `mode` says.
    ///
    /// Returns `None` for a zero divisor, which is `DivideByZero` [7.5]. The `bool` is
    /// whether anything was lost: the caller turns that into `LostPrecision` when the
    /// statement does not say `rounded` [7.4].
    pub fn divide(&self, other: &Decimal, scale: u32, mode: Rounding) -> Option<(Decimal, bool)> {
        if other.is_zero() {
            return None;
        }
        if self.is_zero() {
            return Some((
                Decimal {
                    negative: false,
                    limbs: Vec::new(),
                    scale,
                },
                false,
            ));
        }
        // (a / 10^as) / (b / 10^bs) at scale s  =  a * 10^(s + bs - as) / b
        let shift = scale as i64 + other.scale as i64 - self.scale as i64;
        let (numerator, denominator) = if shift >= 0 {
            (
                Decimal::shift_magnitude(&self.limbs, shift as u32),
                other.limbs.clone(),
            )
        } else {
            (
                self.limbs.clone(),
                Decimal::shift_magnitude(&other.limbs, (-shift) as u32),
            )
        };
        let (quotient, remainder) = Decimal::divmod_magnitude(&numerator, &denominator);
        let negative = self.negative != other.negative;
        let exact = remainder.is_empty();
        let rounded = Decimal::round_magnitude(quotient, &remainder, &denominator, negative, mode);
        Some((
            Decimal {
                negative,
                limbs: rounded,
                scale,
            }
            .normalized(),
            !exact,
        ))
    }

    /// Apply a rounding mode to a truncated quotient, given its remainder.
    fn round_magnitude(
        quotient: Vec<u64>,
        remainder: &[u64],
        divisor: &[u64],
        negative: bool,
        mode: Rounding,
    ) -> Vec<u64> {
        if remainder.is_empty() {
            return quotient;
        }
        // Compare 2 × remainder with the divisor to place the halfway point.
        let twice = Decimal::add_magnitude(remainder, remainder);
        let half = Decimal::cmp_magnitude(&twice, divisor);
        let round_away = match mode {
            Rounding::Down => false,
            Rounding::Up => true,
            Rounding::Floor => negative,
            Rounding::Ceiling => !negative,
            Rounding::HalfAway => half != Ordering::Less,
            Rounding::HalfEven => match half {
                Ordering::Greater => true,
                Ordering::Less => false,
                // A tie: away only if the last digit would otherwise be odd.
                Ordering::Equal => {
                    let last = quotient.first().copied().unwrap_or(0) % 10;
                    last % 2 == 1
                }
            },
        };
        if round_away {
            Decimal::add_magnitude(&quotient, &[1])
        } else {
            quotient
        }
    }

    /// Change the scale, rounding as `mode` says. "fills in zeros when `k` is larger"
    /// [7.4]. The `bool` is whether anything was lost.
    pub fn rescale(&self, scale: u32, mode: Rounding) -> (Decimal, bool) {
        if scale >= self.scale {
            return (self.with_scale_at_least(scale), false);
        }
        // Dividing by 1 at the lower scale does the rounding, with the same rules.
        let one = Decimal::from_i64(1);
        let (value, lost) = self.divide(&one, scale, mode).expect("1 is not zero");
        (value, lost)
    }

    /// The text of [9.2]: the decimal digits "with all its scale digits (`-1.50`,
    /// `0.00`, `7`)", with a leading `-` for a negative number.
    pub fn to_text(&self) -> String {
        let digits = Decimal::to_digits(&self.limbs);
        let mut text: String = digits.iter().map(|d| (d + b'0') as char).collect();
        if self.scale > 0 {
            // Pad so that there is at least one digit before the point.
            while text.len() <= self.scale as usize {
                text.insert(0, '0');
            }
            text.insert(text.len() - self.scale as usize, '.');
        }
        if self.negative {
            text.insert(0, '-');
        }
        text
    }

    /// The value as an `int`, or `None` if it does not fit or has a fractional part.
    pub fn to_i64(&self) -> Option<i64> {
        if self.scale > 0 {
            let (whole, lost) = self.rescale(0, Rounding::Down);
            if lost {
                return None;
            }
            return whole.to_i64();
        }
        let mut value: i128 = 0;
        for limb in self.limbs.iter().rev() {
            value = value
                .checked_mul(BASE as i128)?
                .checked_add(*limb as i128)?;
            if value > u64::MAX as i128 {
                return None;
            }
        }
        if self.negative {
            (-value).try_into().ok()
        } else {
            value.try_into().ok()
        }
    }
}

impl std::fmt::Display for Decimal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.to_text())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn d(text: &str) -> Decimal {
        Decimal::parse(text).unwrap_or_else(|| panic!("{text:?} should parse"))
    }

    #[test]
    fn the_worked_examples_of_section_seven_four() {
        // "`0v10 + 0v20 = 0v30` exactly"
        assert_eq!(d("0.10").add(&d("0.20")).to_text(), "0.30");
        // "`6v00 / 2v00` is `3v0000`" — the scales add.
        let (q, lost) = d("6.00").divide(&d("2.00"), 4, Rounding::HalfAway).unwrap();
        assert_eq!(q.to_text(), "3.0000");
        assert!(!lost);
        // "`1v00 / 4v00` is `0v2500`"
        let (q, lost) = d("1.00").divide(&d("4.00"), 4, Rounding::HalfAway).unwrap();
        assert_eq!(q.to_text(), "0.2500");
        assert!(!lost);
        // "`1v00 / 3v00` raises `LostPrecision` without `rounded` and gives `0v3333`
        // with it" — the loss is reported, and the caller decides.
        let (q, lost) = d("1.00").divide(&d("3.00"), 4, Rounding::HalfAway).unwrap();
        assert_eq!(q.to_text(), "0.3333");
        assert!(lost, "the caller turns this into LostPrecision [7.4]");
        // "`divide(1v0, 8v0, 3)` is `0v125`"
        let (q, lost) = d("1.0").divide(&d("8.0"), 3, Rounding::HalfAway).unwrap();
        assert_eq!(q.to_text(), "0.125");
        assert!(!lost);
        // "`rescale(2v675, 2, RoundHalfEven)` is `2v68`"
        assert_eq!(
            d("2.675").rescale(2, Rounding::HalfEven).0.to_text(),
            "2.68"
        );
        // "`rescale(_2v5, 0, RoundFloor)` is `_3`"
        assert_eq!(d("-2.5").rescale(0, Rounding::Floor).0.to_text(), "-3");
        // "`divide(1v00, 3v00, 4, RoundHalfAway)` is `0v3333`"
        let (q, _) = d("1.00").divide(&d("3.00"), 4, Rounding::HalfAway).unwrap();
        assert_eq!(q.to_text(), "0.3333");
    }

    #[test]
    fn addition_and_subtraction_take_the_larger_scale() {
        assert_eq!(d("1.5").add(&d("2.25")).to_text(), "3.75");
        assert_eq!(d("1.5").add(&d("2.25")).scale(), 2);
        assert_eq!(d("1").add(&d("0.001")).to_text(), "1.001");
        assert_eq!(d("5").sub(&d("8")).to_text(), "-3");
        // A zero keeps the scale it was computed at, and [9.2] shows "all its scale
        // digits", so this is `0.0` rather than `0`.
        assert_eq!(d("0.1").sub(&d("0.1")).to_text(), "0.0");
        assert_eq!(d("1").sub(&d("1")).to_text(), "0");
        // Zero keeps the larger scale but prints as `0`, since the magnitude is empty.
        assert!(d("0.10").sub(&d("0.10")).is_zero());
    }

    #[test]
    fn multiplication_adds_the_scales() {
        assert_eq!(d("1.5").mul(&d("1.5")).to_text(), "2.25");
        assert_eq!(d("1.50").mul(&d("2.000")).scale(), 5);
        assert_eq!(d("1.50").mul(&d("2.000")).to_text(), "3.00000");
        assert_eq!(d("-2").mul(&d("3")).to_text(), "-6");
        assert_eq!(d("-2").mul(&d("-3")).to_text(), "6");
        assert_eq!(d("0").mul(&d("5")).to_text(), "0");
    }

    #[test]
    fn comparison_is_by_value_not_by_scale() {
        // [7.4]: "Comparison is by numeric value, so `1v5 = 1v50`."
        assert_eq!(d("1.5").compare(&d("1.50")), Ordering::Equal);
        assert_eq!(d("1.5").compare(&d("1.51")), Ordering::Less);
        assert_eq!(d("-1.5").compare(&d("1.5")), Ordering::Less);
        assert_eq!(d("-1.5").compare(&d("-1.50")), Ordering::Equal);
        assert_eq!(d("-1").compare(&d("-2")), Ordering::Greater);
        assert_eq!(d("0").compare(&d("0.000")), Ordering::Equal);
        assert_eq!(
            d("0").compare(&d("-0")),
            Ordering::Equal,
            "there is no negative zero"
        );
    }

    #[test]
    fn every_rounding_mode_of_section_seven_four() {
        // The modes as [7.4] defines them, on the cases that tell them apart.
        let cases: &[(&str, u32, Rounding, &str)] = &[
            ("2.5", 0, Rounding::HalfAway, "3"),
            ("-2.5", 0, Rounding::HalfAway, "-3"),
            ("2.5", 0, Rounding::HalfEven, "2"),
            ("3.5", 0, Rounding::HalfEven, "4"),
            ("-2.5", 0, Rounding::HalfEven, "-2"),
            ("2.7", 0, Rounding::Down, "2"),
            ("-2.7", 0, Rounding::Down, "-2"),
            ("2.1", 0, Rounding::Up, "3"),
            ("-2.1", 0, Rounding::Up, "-3"),
            ("2.7", 0, Rounding::Floor, "2"),
            ("-2.1", 0, Rounding::Floor, "-3"),
            ("2.1", 0, Rounding::Ceiling, "3"),
            ("-2.7", 0, Rounding::Ceiling, "-2"),
            // A tie at a non-zero scale, and a non-tie that must not move.
            ("1.005", 2, Rounding::HalfAway, "1.01"),
            ("1.004", 2, Rounding::HalfAway, "1.00"),
            ("1.015", 2, Rounding::HalfEven, "1.02"),
            ("1.025", 2, Rounding::HalfEven, "1.02"),
        ];
        for (value, scale, mode, want) in cases {
            let (got, lost) = d(value).rescale(*scale, *mode);
            assert_eq!(
                got.to_text(),
                *want,
                "{value} at scale {scale} under {mode:?}"
            );
            assert!(lost, "{value} at scale {scale} loses digits");
        }
    }

    #[test]
    fn rescaling_upward_fills_in_zeros_and_loses_nothing() {
        // [7.4]: "`rescale` ... fills in zeros when `k` is larger."
        let (got, lost) = d("1.5").rescale(4, Rounding::Down);
        assert_eq!(got.to_text(), "1.5000");
        assert!(!lost);
        let (got, lost) = d("7").rescale(2, Rounding::Down);
        assert_eq!(got.to_text(), "7.00");
        assert!(!lost);
    }

    #[test]
    fn a_zero_divisor_is_reported_rather_than_panicking() {
        // [7.5]: `DivideByZero`. The caller raises it; the arithmetic says `None`.
        assert!(d("1").divide(&d("0"), 2, Rounding::HalfAway).is_none());
        assert!(d("0").divide(&d("0.00"), 2, Rounding::HalfAway).is_none());
    }

    #[test]
    fn precision_is_unbounded() {
        // [7.4]: "`p` may be any positive constant with no upper limit", and
        // intermediates are "exact however many digits they need". Forty digits is
        // past `i128`, so this fails for any fixed-width implementation.
        let big = d("1234567890123456789012345678901234567890");
        assert_eq!(big.digits(), 40);
        let squared = big.mul(&big);
        assert_eq!(
            squared.digits(),
            79,
            "a 40-digit square is 79 or 80 digits; this one is 79"
        );
        assert!(
            squared
                .to_text()
                .starts_with("1524157875323883675049535156256668194500")
        );
        // And the scales add without limit.
        let small = d("0.000000000000000000000000000000000000001");
        assert_eq!(small.scale(), 39);
        assert_eq!(small.mul(&small).scale(), 78);
    }

    #[test]
    fn the_number_text_rules_of_nine_two() {
        // "ASCII digits with an optional leading `-`, and for `fixed` a `.` as the
        // decimal point, with at least one digit on each side. There is no `+`, no
        // spaces, no exponent and no locale; empty text is a `FormatError`. Leading
        // zeros are accepted."
        assert_eq!(d("007").to_text(), "7");
        assert_eq!(d("-0.50").to_text(), "-0.50");
        assert_eq!(d("0.00").to_text(), "0.00", "a zero keeps its scale digits");
        for bad in [
            "", "-", "+1", " 1", "1 ", "1e5", ".5", "5.", "1.2.3", "1,5", "abc",
        ] {
            assert!(Decimal::parse(bad).is_none(), "{bad:?} must not parse");
        }
    }

    #[test]
    fn the_text_of_a_value_shows_all_its_scale_digits() {
        // [9.2] gives the examples: `-1.50`, `0.00`, `7`.
        assert_eq!(d("-1.50").to_text(), "-1.50");
        assert_eq!(d("0.00").to_text(), "0.00");
        assert_eq!(d("7").to_text(), "7");
    }

    #[test]
    fn conversion_to_and_from_int() {
        // [7.4]: "An `int` acts as a `fixed` of scale 0."
        assert_eq!(Decimal::from_i64(0).to_text(), "0");
        assert_eq!(Decimal::from_i64(-7).to_text(), "-7");
        assert_eq!(Decimal::from_i64(i64::MAX).to_text(), "9223372036854775807");
        // `i64::MIN` has no positive counterpart, which a naive `abs` would panic on.
        assert_eq!(
            Decimal::from_i64(i64::MIN).to_text(),
            "-9223372036854775808"
        );
        assert_eq!(Decimal::from_i64(i64::MIN).to_i64(), Some(i64::MIN));
        assert_eq!(Decimal::from_i64(i64::MAX).to_i64(), Some(i64::MAX));
        // A fractional part is not an `int`, and neither is something too large.
        assert_eq!(d("1.5").to_i64(), None);
        assert_eq!(d("1.0").to_i64(), Some(1), "a zero fraction is exact");
        assert_eq!(d("99999999999999999999999").to_i64(), None);
    }

    #[test]
    fn digit_counts_are_what_an_assignment_checks() {
        // [7.4]: `p` is the total digits, and "too many integer digits raises
        // `Overflow`", so both counts matter.
        assert_eq!(
            d("0").digits(),
            1,
            "zero needs one digit, so fixed(1,0) holds it"
        );
        assert_eq!(d("0.00").integer_digits(), 1);
        assert_eq!(
            d("99999.99").digits(),
            7,
            "the fixed(7,2) of the spec's example"
        );
        assert_eq!(d("99999.99").integer_digits(), 5);
        assert_eq!(d("-123.4").digits(), 4, "the sign is not a digit");
        assert_eq!(d("1000000000").digits(), 10, "exactly one limb boundary");
        assert_eq!(d("999999999").digits(), 9);
    }

    #[test]
    fn a_fixed_literal_keeps_its_scale() {
        // The lexer hands over digits and a scale [2]: `12v50` is ("1250", 2).
        let v = Decimal::from_digits("1250", 2, false).unwrap();
        assert_eq!(v.to_text(), "12.50");
        assert_eq!(v.scale(), 2);
        // `0v05` is 0.05 with scale 2, and `5v0` is 5.0 with scale 1.
        assert_eq!(
            Decimal::from_digits("005", 2, false).unwrap().to_text(),
            "0.05"
        );
        assert_eq!(
            Decimal::from_digits("50", 1, false).unwrap().to_text(),
            "5.0"
        );
        assert!(Decimal::from_digits("", 0, false).is_none());
        assert!(Decimal::from_digits("12a", 0, false).is_none());
    }

    #[test]
    fn the_rounding_mode_names_are_the_ones_programs_write() {
        for mode in [
            Rounding::HalfAway,
            Rounding::HalfEven,
            Rounding::Down,
            Rounding::Up,
            Rounding::Floor,
            Rounding::Ceiling,
        ] {
            assert_eq!(Rounding::from_name(mode.name()), Some(mode));
        }
        assert_eq!(
            Rounding::from_name("RoundHalfUp"),
            None,
            "not a mode in [7.4]"
        );
    }

    #[test]
    fn division_across_many_limbs_is_exact_when_it_can_be() {
        // A quotient that spans several limbs, checked by multiplying back.
        let a = d("123456789012345678901234567890");
        let b = d("987654321");
        let (q, lost) = a.divide(&b, 0, Rounding::Down).unwrap();
        assert!(lost, "this division is not exact");
        // q * b <= a < (q + 1) * b
        assert!(q.mul(&b).compare(&a) != Ordering::Greater);
        assert!(q.add(&Decimal::from_i64(1)).mul(&b).compare(&a) == Ordering::Greater);
        // And an exact one loses nothing.
        let exact = b.mul(&d("1000000007"));
        let (q, lost) = exact.divide(&b, 0, Rounding::Down).unwrap();
        assert!(!lost);
        assert_eq!(q.to_text(), "1000000007");
    }

    #[test]
    fn dividing_by_an_int_keeps_the_dividends_scale() {
        // [7.4]: "dividing by an `int` keeps the dividend's scale."
        let (q, _) = d("7.50")
            .divide(&Decimal::from_i64(2), 2, Rounding::HalfAway)
            .unwrap();
        assert_eq!(q.to_text(), "3.75");
    }
}

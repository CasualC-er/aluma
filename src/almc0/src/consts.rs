//! The constant expression evaluator [4].
//!
//! "Constants are compile-time constant expressions, usable wherever a size is needed
//! (`array [Size] of int`, `fixed(Digits, 2)`)", and "`enum A, B, C;` declares the
//! `int` constants 0, 1 and 2 in order".
//!
//! Arithmetic is the same arithmetic the runtime uses (`alm_core::decimal`), so a
//! folded constant and a computed value cannot disagree. `int` is checked `i64` [7.3];
//! a `fixed` intermediate is unbounded [7.4].
//!
//! Errors reported here are the runtime exceptions of [7.5] turned into compile errors,
//! which is what they are when the operands are constants: `Overflow`, `DivideByZero`
//! and `LostPrecision` cannot be caught at compile time, so a program that would raise
//! one in a constant is rejected.
//!
//! ## What is not constant
//!
//! A procedure call is rejected. The reference says constants are "compile-time
//! constant expressions" without saying which calls, if any, fold — `strFixed`'s `k`
//! and `power`'s exponent are described as constant expressions, which is about their
//! *arguments*, not about the calls being foldable. Rejecting them is the choice that
//! cannot be wrong later: allowing a call and then finding the spec meant otherwise
//! would break programs, whereas allowing more folding later breaks nothing. Logged in
//! QUESTIONS.md.

use std::collections::BTreeMap;

use alm_core::decimal::{Decimal, Rounding};
use alm_core::diag::Diagnostics;
use alm_core::span::Span;

use crate::ast::*;
use crate::load::Loaded;

/// The value of a constant expression.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ConstValue {
    /// A checked `i64` [7.3]. Untagged: "literals have no tag" [7.4].
    Int(i64),
    /// An exact decimal of some scale [7.4].
    Fixed(Decimal),
    Bool(bool),
    Char(char),
}

impl ConstValue {
    pub const fn describe(&self) -> &'static str {
        match self {
            ConstValue::Int(_) => "an `int`",
            ConstValue::Fixed(_) => "a `fixed` value",
            ConstValue::Bool(_) => "a `bool`",
            ConstValue::Char(_) => "a `char`",
        }
    }

    /// The value as a count, for `array [n] of T` and `fixed(p, s)`.
    pub fn as_int(&self) -> Option<i64> {
        match self {
            ConstValue::Int(v) => Some(*v),
            _ => None,
        }
    }

    /// `int` and `fixed` mix: "an `int` acts as a `fixed` of scale 0" [7.4].
    fn as_decimal(&self) -> Option<Decimal> {
        match self {
            ConstValue::Int(v) => Some(Decimal::from_i64(*v)),
            ConstValue::Fixed(d) => Some(d.clone()),
            _ => None,
        }
    }
}

/// Every constant of the program, by module and name.
#[derive(Clone, Debug, Default)]
pub struct Consts {
    values: BTreeMap<(String, String), ConstValue>,
}

impl Consts {
    pub fn get(&self, module: &str, name: &str) -> Option<&ConstValue> {
        self.values.get(&(module.to_owned(), name.to_owned()))
    }

    fn set(&mut self, module: &str, name: &str, value: ConstValue) {
        self.values
            .insert((module.to_owned(), name.to_owned()), value);
    }
}

/// Evaluate every constant in the program.
///
/// Modules come in dependency order from the loader, and declarations within a module
/// are in source order with declaration-before-use already checked, so one pass in that
/// order is enough: a constant's operands are always evaluated by the time it is.
pub fn evaluate(loaded: &Loaded, diags: &mut Diagnostics) -> Consts {
    let mut consts = Consts::default();
    for m in &loaded.modules {
        let Some(program) = &m.program else { continue };
        for decl in &program.decls {
            match &decl.kind {
                DeclKind::Const { name, value } => {
                    let mut ev = Evaluator {
                        module: &m.name,
                        consts: &consts,
                        diags,
                    };
                    if let Some(v) = ev.expr(value) {
                        consts.set(&m.name, &name.text, v);
                    }
                }
                DeclKind::Enum { names } => {
                    // "`enum A, B, C;` declares the `int` constants 0, 1 and 2 in
                    // order" [4].
                    for (i, name) in names.iter().enumerate() {
                        consts.set(&m.name, &name.text, ConstValue::Int(i as i64));
                    }
                }
                _ => {}
            }
        }
    }
    consts
}

/// Evaluate one expression in the context of a module's constants.
pub fn evaluate_expr(
    module: &str,
    consts: &Consts,
    expr: &Expr,
    diags: &mut Diagnostics,
) -> Option<ConstValue> {
    Evaluator {
        module,
        consts,
        diags,
    }
    .expr(expr)
}

struct Evaluator<'a, 'd> {
    module: &'a str,
    consts: &'a Consts,
    diags: &'d mut Diagnostics,
}

impl Evaluator<'_, '_> {
    fn expr(&mut self, e: &Expr) -> Option<ConstValue> {
        match &e.kind {
            ExprKind::Int { value } => Some(ConstValue::Int(*value)),
            ExprKind::Fixed { digits, scale } => Decimal::from_digits(digits, *scale, false)
                .map(ConstValue::Fixed)
                .or_else(|| {
                    self.error(e.span, "this `fixed` literal is not a number");
                    None
                }),
            ExprKind::Bool(b) => Some(ConstValue::Bool(*b)),
            ExprKind::Char(c) => Some(ConstValue::Char(*c)),
            ExprKind::Paren(inner) => self.expr(inner),
            ExprKind::Not(inner) => match self.expr(inner)? {
                ConstValue::Bool(b) => Some(ConstValue::Bool(!b)),
                other => {
                    self.error(
                        inner.span,
                        format!("`not` needs a `bool`, and this is {}", other.describe()),
                    );
                    None
                }
            },
            ExprKind::Negate(inner) => match self.expr(inner)? {
                ConstValue::Int(v) => match v.checked_neg() {
                    Some(n) => Some(ConstValue::Int(n)),
                    None => {
                        self.error(e.span, "this negation is out of range for an `int`");
                        None
                    }
                },
                ConstValue::Fixed(d) => Some(ConstValue::Fixed(d.negated())),
                other => {
                    self.error(
                        inner.span,
                        format!("`_` needs a number, and this is {}", other.describe()),
                    );
                    None
                }
            },
            ExprKind::Binary { op, left, right } => {
                let l = self.expr(left)?;
                let r = self.expr(right)?;
                self.binary(*op, &l, &r, e.span, left.span, right.span)
            }
            ExprKind::Designator(d) => self.designator(d),
            ExprKind::Call { call, .. } => {
                self.error(
                    call.span,
                    "a constant expression cannot call a procedure; it is evaluated at \
                     compile time [4]",
                );
                None
            }
            ExprKind::Nil => {
                // `nil` is a reference, and a constant holds a value.
                self.error(e.span, "`nil` is not a constant: a constant is a value [4]");
                None
            }
            ExprKind::Str(_) => {
                // A string literal is "a new, mutable `String` each time it is
                // evaluated" [3.4], so it cannot be a compile-time value.
                self.error(
                    e.span,
                    "a string literal is a new array each time it is evaluated, so it \
                     cannot be a constant [3.4]",
                );
                None
            }
            ExprKind::New(_) | ExprKind::NewArray { .. } => {
                self.error(
                    e.span,
                    "`new` creates an object at run time, so it is not a constant",
                );
                None
            }
        }
    }

    /// A named constant, which may be qualified.
    fn designator(&mut self, d: &Designator) -> Option<ConstValue> {
        if !d.selectors.is_empty() {
            self.error(
                d.span,
                "a constant expression cannot index or select a field: there is no \
                 object to read at compile time",
            );
            return None;
        }
        let module = d
            .base
            .module
            .as_ref()
            .map(|m| m.text.as_str())
            .unwrap_or(self.module);
        match self.consts.get(module, &d.base.name.text) {
            Some(v) => Some(v.clone()),
            None => {
                // Resolution has already reported an undeclared name, so this is a
                // name that exists but is not a constant.
                self.error(d.span, format!("`{}` is not a constant", d.base.name.text));
                None
            }
        }
    }

    fn binary(
        &mut self,
        op: BinOp,
        l: &ConstValue,
        r: &ConstValue,
        whole: Span,
        left: Span,
        right: Span,
    ) -> Option<ConstValue> {
        use BinOp::*;
        match op {
            And | Or => {
                let (a, b) = match (l, r) {
                    (ConstValue::Bool(a), ConstValue::Bool(b)) => (*a, *b),
                    _ => {
                        self.error(whole, format!("`{}` needs two `bool` values", op.text()));
                        return None;
                    }
                };
                Some(ConstValue::Bool(if op == And { a && b } else { a || b }))
            }
            Equal => self.equality(l, r, whole),
            Less | LessEqual | Greater | GreaterEqual => self.ordering(op, l, r, whole),
            Identical => {
                // [7.1]: `~` is defined only on references, mailboxes and views.
                self.error(
                    whole,
                    "`~` compares references, so it is not a constant [7.1]",
                );
                None
            }
            Add | Subtract | Multiply => self.arithmetic(op, l, r, whole, left, right),
            Divide => self.divide(l, r, whole, left, right),
            Div | Mod => self.int_division(op, l, r, whole, left, right),
        }
    }

    fn equality(&mut self, l: &ConstValue, r: &ConstValue, whole: Span) -> Option<ConstValue> {
        // [7.1]: `=` is defined on `int`, `bool`, `char` and `fixed`.
        match (l, r) {
            (ConstValue::Bool(a), ConstValue::Bool(b)) => Some(ConstValue::Bool(a == b)),
            (ConstValue::Char(a), ConstValue::Char(b)) => Some(ConstValue::Bool(a == b)),
            _ => match (l.as_decimal(), r.as_decimal()) {
                // Comparison is by value, so `1v5 = 1v50` [7.4].
                (Some(a), Some(b)) => Some(ConstValue::Bool(a.compare(&b).is_eq())),
                _ => {
                    self.error(
                        whole,
                        format!("`=` cannot compare {} with {}", l.describe(), r.describe()),
                    );
                    None
                }
            },
        }
    }

    fn ordering(
        &mut self,
        op: BinOp,
        l: &ConstValue,
        r: &ConstValue,
        whole: Span,
    ) -> Option<ConstValue> {
        let order = match (l, r) {
            (ConstValue::Char(a), ConstValue::Char(b)) => a.cmp(b),
            _ => match (l.as_decimal(), r.as_decimal()) {
                (Some(a), Some(b)) => a.compare(&b),
                _ => {
                    self.error(
                        whole,
                        format!(
                            "`{}` cannot compare {} with {}",
                            op.text(),
                            l.describe(),
                            r.describe()
                        ),
                    );
                    return None;
                }
            },
        };
        Some(ConstValue::Bool(match op {
            BinOp::Less => order.is_lt(),
            BinOp::LessEqual => order.is_le(),
            BinOp::Greater => order.is_gt(),
            BinOp::GreaterEqual => order.is_ge(),
            _ => unreachable!("only the four orderings reach here"),
        }))
    }

    fn arithmetic(
        &mut self,
        op: BinOp,
        l: &ConstValue,
        r: &ConstValue,
        whole: Span,
        left: Span,
        right: Span,
    ) -> Option<ConstValue> {
        // Two `int`s stay `int`, with the checked arithmetic of [7.3].
        if let (ConstValue::Int(a), ConstValue::Int(b)) = (l, r) {
            let result = match op {
                BinOp::Add => a.checked_add(*b),
                BinOp::Subtract => a.checked_sub(*b),
                BinOp::Multiply => a.checked_mul(*b),
                _ => unreachable!(),
            };
            return match result {
                Some(v) => Some(ConstValue::Int(v)),
                None => {
                    self.error(
                        whole,
                        format!(
                            "this `{}` is out of range for an `int`, which would raise \
                             `Overflow` [7.3]",
                            op.text()
                        ),
                    );
                    None
                }
            };
        }
        let (Some(a), Some(b)) = (l.as_decimal(), r.as_decimal()) else {
            self.error(
                if l.as_decimal().is_none() {
                    left
                } else {
                    right
                },
                format!(
                    "`{}` needs numbers, and this is {}",
                    op.text(),
                    if l.as_decimal().is_none() {
                        l.describe()
                    } else {
                        r.describe()
                    }
                ),
            );
            return None;
        };
        // Exact, with the scale rules of [7.4].
        Some(ConstValue::Fixed(match op {
            BinOp::Add => a.add(&b),
            BinOp::Subtract => a.sub(&b),
            BinOp::Multiply => a.mul(&b),
            _ => unreachable!(),
        }))
    }

    fn divide(
        &mut self,
        l: &ConstValue,
        r: &ConstValue,
        whole: Span,
        left: Span,
        right: Span,
    ) -> Option<ConstValue> {
        // [7.4]: "`/` with two `int` operands is a compile error; use `div`."
        if matches!((l, r), (ConstValue::Int(_), ConstValue::Int(_))) {
            self.error(
                whole,
                "`/` with two `int` operands is a compile error; use `div` [7.4]",
            );
            return None;
        }
        let (Some(a), Some(b)) = (l.as_decimal(), r.as_decimal()) else {
            self.error(
                if l.as_decimal().is_none() {
                    left
                } else {
                    right
                },
                "`/` needs numbers",
            );
            return None;
        };
        // "Division `/` gives the quotient at the sum of the two scales" [7.4].
        let scale = a.scale() + b.scale();
        let Some((value, lost)) = a.divide(&b, scale, Rounding::HalfAway) else {
            self.error(
                right,
                "this divisor is zero, which would raise `DivideByZero` [7.5]",
            );
            return None;
        };
        if lost {
            // A division is a rounding step, and a constant has no `rounded` to say
            // otherwise [7.4].
            self.error(
                whole,
                format!(
                    "this division is not exact at scale {scale}, which would raise \
                     `LostPrecision`; a constant cannot say `rounded`, so write the \
                     value out or use `divide(a, b, k, mode)` [7.4]"
                ),
            );
            return None;
        }
        Some(ConstValue::Fixed(value))
    }

    fn int_division(
        &mut self,
        op: BinOp,
        l: &ConstValue,
        r: &ConstValue,
        whole: Span,
        left: Span,
        right: Span,
    ) -> Option<ConstValue> {
        let (ConstValue::Int(a), ConstValue::Int(b)) = (l, r) else {
            self.error(
                if l.as_int().is_none() { left } else { right },
                format!("`{}` needs two `int` values [7.3]", op.text()),
            );
            return None;
        };
        if *b == 0 {
            self.error(
                right,
                "this divisor is zero, which would raise `DivideByZero` [7.5]",
            );
            return None;
        }
        // [7.3]: `div` and `mod` are Euclidean, so the remainder is never negative.
        let (q, m) = euclidean(*a, *b);
        let value = if op == BinOp::Div { q } else { m };
        match value {
            Some(v) => Some(ConstValue::Int(v)),
            None => {
                self.error(
                    whole,
                    format!(
                        "this `{}` is out of range for an `int`, which would raise \
                         `Overflow` [7.3]",
                        op.text()
                    ),
                );
                None
            }
        }
    }

    fn error(&mut self, span: Span, message: impl Into<String>) {
        self.diags.error(span, message);
    }
}

/// Euclidean division [7.3]: the remainder is never negative.
///
/// Returns `(quotient, remainder)`, either of which is `None` if it overflows — which
/// happens for `i64::MIN div _1`.
fn euclidean(a: i64, b: i64) -> (Option<i64>, Option<i64>) {
    let Some(q) = a.checked_div_euclid(b) else {
        return (None, None);
    };
    let m = a.checked_rem_euclid(b);
    (Some(q), m)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alm_core::diag::SourceMap;

    /// Evaluate `const Value is <expr>;` and return the value or the diagnostics.
    fn eval(expr: &str) -> Result<ConstValue, Vec<String>> {
        eval_source(&format!("const Value is {expr};\n"))
    }

    fn eval_source(source: &str) -> Result<ConstValue, Vec<String>> {
        let dir = std::env::temp_dir().join(format!(
            "almc0-consts-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a writable temporary directory");
        std::fs::write(dir.join("Main.alm"), source).expect("writable");
        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let loaded = crate::load::load(&dir.join("Main.alm"), &[], &mut sources, &mut diags);
        assert!(
            !diags.has_errors(),
            "the test source must parse: {:?}",
            diags
                .items()
                .iter()
                .map(|d| d.render(&sources))
                .collect::<Vec<_>>()
        );
        let consts = evaluate(&loaded, &mut diags);
        let _ = std::fs::remove_dir_all(&dir);
        if diags.has_errors() {
            return Err(diags.items().iter().map(|d| d.render(&sources)).collect());
        }
        Ok(consts
            .get("Main", "Value")
            .cloned()
            .expect("Value was evaluated"))
    }

    fn int(expr: &str) -> i64 {
        match eval(expr) {
            Ok(ConstValue::Int(v)) => v,
            other => panic!("{expr} should be an int, got {other:?}"),
        }
    }

    fn fixed(expr: &str) -> String {
        match eval(expr) {
            Ok(ConstValue::Fixed(d)) => d.to_text(),
            other => panic!("{expr} should be a fixed value, got {other:?}"),
        }
    }

    fn fails(expr: &str, wanted: &str) {
        match eval(expr) {
            Err(d) => assert!(
                d.iter().any(|x| x.contains(wanted)),
                "{expr}: expected {wanted:?}, got {d:#?}"
            ),
            Ok(v) => panic!("{expr} should have been rejected, gave {v:?}"),
        }
    }

    #[test]
    fn integer_arithmetic_is_checked() {
        assert_eq!(int("1 + 2 * 3"), 7);
        assert_eq!(int("(1 + 2) * 3"), 9);
        assert_eq!(int("_5 + 2"), -3);
        assert_eq!(int("9223372036854775807"), i64::MAX);
        assert_eq!(int("_9223372036854775808"), i64::MIN);
        // [7.3]: arithmetic overflow is an error, so a constant that would overflow is
        // rejected at compile time rather than wrapping.
        fails("9223372036854775807 + 1", "out of range for an `int`");
        fails("9223372036854775807 * 2", "out of range for an `int`");
        fails("_9223372036854775808 - 1", "out of range for an `int`");
    }

    #[test]
    fn div_and_mod_are_euclidean() {
        // [7.3]: "`div`/`mod` are Euclidean", so the remainder is never negative.
        assert_eq!(int("7 div 2"), 3);
        assert_eq!(int("7 mod 2"), 1);
        assert_eq!(int("_7 div 2"), -4);
        assert_eq!(int("_7 mod 2"), 1);
        assert_eq!(int("7 div _2"), -3);
        assert_eq!(int("7 mod _2"), 1);
        fails("1 div 0", "would raise `DivideByZero`");
        fails("1 mod 0", "would raise `DivideByZero`");
    }

    #[test]
    fn slash_on_two_ints_is_a_compile_error() {
        // [7.4]: "`/` with two `int` operands is a compile error; use `div`."
        fails("6 / 2", "use `div`");
    }

    #[test]
    fn fixed_arithmetic_follows_the_scale_rules() {
        // Add and subtract take the larger scale; multiply adds them [7.4].
        assert_eq!(fixed("0v10 + 0v20"), "0.30");
        assert_eq!(fixed("1v5 + 2v25"), "3.75");
        assert_eq!(fixed("1v50 * 2v000"), "3.00000");
        // An `int` acts as a `fixed` of scale 0 [7.4].
        assert_eq!(fixed("1v50 + 2"), "3.50");
        assert_eq!(fixed("2 * 1v5"), "3.0");
        // Division gives the sum of the scales, when it is exact.
        assert_eq!(fixed("6v00 / 2v00"), "3.0000");
        assert_eq!(fixed("1v00 / 4v00"), "0.2500");
    }

    #[test]
    fn an_inexact_constant_division_is_rejected() {
        // A division is a rounding step, and a constant has no `rounded` to permit the
        // loss [7.4], so this would raise `LostPrecision` and cannot be folded.
        fails("1v00 / 3v00", "would raise `LostPrecision`");
        fails("1v0 / 0v0", "would raise `DivideByZero`");
    }

    #[test]
    fn comparisons_and_logic() {
        assert_eq!(eval("1 < 2"), Ok(ConstValue::Bool(true)));
        assert_eq!(
            eval("1v5 = 1v50"),
            Ok(ConstValue::Bool(true)),
            "by value [7.4]"
        );
        assert_eq!(eval("'a' < 'b'"), Ok(ConstValue::Bool(true)));
        assert_eq!(eval("true and false"), Ok(ConstValue::Bool(false)));
        assert_eq!(eval("true or false"), Ok(ConstValue::Bool(true)));
        assert_eq!(eval("not true"), Ok(ConstValue::Bool(false)));
        assert_eq!(
            eval("not 1 = 2"),
            Ok(ConstValue::Bool(true)),
            "`not` is looser [2]"
        );
        fails("true and 1", "needs two `bool` values");
        fails("not 1", "needs a `bool`");
        fails("'a' = 1", "cannot compare");
    }

    #[test]
    fn a_constant_may_name_an_earlier_constant_or_an_enum_name() {
        assert_eq!(
            eval_source("const Size is 10;\nconst Value is Size * 2;\n"),
            Ok(ConstValue::Int(20))
        );
        // "`enum A, B, C;` declares the `int` constants 0, 1 and 2 in order" [4].
        assert_eq!(
            eval_source("enum Get, Put, Delete;\nconst Value is Delete;\n"),
            Ok(ConstValue::Int(2))
        );
        assert_eq!(
            eval_source("enum Get, Put;\nconst Value is Get;\n"),
            Ok(ConstValue::Int(0))
        );
    }

    #[test]
    fn what_is_not_a_constant() {
        fails("f(1)", "cannot call a procedure");
        fails("nil", "`nil` is not a constant");
        fails("\"text\"", "new array each time");
        fails("new(Node)", "not a constant");
        // A name that exists but is not a constant.
        match eval_source("type T is int;\nconst Value is T;\n") {
            Err(d) => assert!(d.iter().any(|x| x.contains("is not a constant")), "{d:#?}"),
            Ok(v) => panic!("a type is not a constant, got {v:?}"),
        }
    }

    #[test]
    fn a_constant_of_another_module() {
        let dir = std::env::temp_dir().join(format!(
            "almc0-consts-cross-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("writable");
        std::fs::write(
            dir.join("Main.alm"),
            "import Limits;\nconst Value is Limits::Max * 2;\nproc start()\nbegin\nend;\n",
        )
        .expect("writable");
        std::fs::write(dir.join("Limits.alm"), "const Max is 50;\n").expect("writable");
        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let loaded = crate::load::load(&dir.join("Main.alm"), &[], &mut sources, &mut diags);
        assert!(!diags.has_errors());
        let consts = evaluate(&loaded, &mut diags);
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            !diags.has_errors(),
            "{:?}",
            diags
                .items()
                .iter()
                .map(|d| d.render(&sources))
                .collect::<Vec<_>>()
        );
        // The loader puts imports first, so `Limits::Max` is evaluated before it is
        // named — which is why one pass in that order is enough.
        assert_eq!(consts.get("Main", "Value"), Some(&ConstValue::Int(100)));
        assert_eq!(consts.get("Limits", "Max"), Some(&ConstValue::Int(50)));
    }

    #[test]
    fn euclidean_handles_the_min_int_case() {
        // `i64::MIN div _1` has no `i64` result [7.3].
        assert_eq!(euclidean(7, 2), (Some(3), Some(1)));
        assert_eq!(euclidean(-7, 2), (Some(-4), Some(1)));
        assert_eq!(euclidean(i64::MIN, -1), (None, None));
    }
}

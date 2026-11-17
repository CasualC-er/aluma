//! Type checking, part one: designators, operands and conditions.
//!
//! What this part decides, using the type graph:
//!
//! - the type of a designator — a variable, then its index and field selectors [10] —
//!   which is where the field rules of [4.1] finally bite: "a private field can be read
//!   and assigned only in the module that declares the record type, and a use anywhere
//!   else is a compile error";
//! - that `=` is used only on types that have it, and `~` only on references,
//!   mailboxes and views [7.1], with a note saying *which* part of a record stops `=`
//!   working;
//! - that `and`, `or`, `not` and the conditions of `if`, `elsif` and `while` are
//!   `bool`.
//!
//! What it does not do yet: calls and their arguments [7.2], the assignment rules of
//! [7.4] with `rounded` and single rounding, and the tag rules in arithmetic. Those are
//! the next slices, and until they land an expression whose type this part cannot
//! determine is `Unknown`, which silences everything downstream of it rather than
//! guessing.
//!
//! The library's procedures are the main source of `Unknown`: `std` has no source to
//! read a signature from, and [9] makes sixteen of its names polymorphic built-ins that
//! the compiler checks specially — a separate Phase 2 item. So a call to `len(a)` has
//! an unknown result here, deliberately, rather than a wrong one.

use std::collections::BTreeMap;

use alm_core::diag::{Diagnostic, Diagnostics};
use alm_core::span::Span;

use crate::ast::*;
use crate::library;
use crate::load::Loaded;
use crate::resolve::{SymbolKind, Symbols};
use crate::types::{TypeId, TypeKind, Types};

/// What kind of number a value is, for the arithmetic rules of [7.4].
///
/// An `int` "acts as a `fixed` of scale 0 when mixed with fixed values", so the two
/// live in one enum.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Num {
    Int,
    /// A decimal of this scale. **No precision**, deliberately: "intermediate results
    /// of `+`, `-` and `*` are exact however many digits they need; only an assignment
    /// to a variable of a declared `fixed(p, s)` type is range-checked" [7.4]. An
    /// intermediate therefore has no `p`, and inventing one would either reject a legal
    /// expression or claim a bound the language does not have.
    Fixed {
        scale: u32,
    },
}

impl Num {
    /// The scale, with an `int` counting as 0 [7.4].
    const fn scale(self) -> u32 {
        match self {
            Num::Int => 0,
            Num::Fixed { scale } => scale,
        }
    }

    const fn describe(self) -> &'static str {
        match self {
            Num::Int => "an `int`",
            Num::Fixed { .. } => "a `fixed` value",
        }
    }
}

/// The type of an expression, or `Unknown` when this part cannot say.
///
/// `Unknown` is not an error: it means "not determined yet", and every check treats it
/// as acceptable so that one unfinished area does not produce a cascade of complaints
/// about correct code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Typed {
    Known(TypeId),
    /// A number with no declared type: a literal, or the result of arithmetic. Carries
    /// its scale, and its tag once [7.4]'s named number types are checked.
    Number(Num),
    Unknown,
}

impl Typed {
    fn id(self) -> Option<TypeId> {
        match self {
            Typed::Known(id) => Some(id),
            Typed::Number(_) | Typed::Unknown => None,
        }
    }
}

/// Check every module. Reports into `diags`; returns nothing, because nothing
/// downstream needs the results until lowering.
pub fn check(
    loaded: &Loaded,
    symbols: &Symbols,
    types: &Types,
    consts: &crate::consts::Consts,
    diags: &mut Diagnostics,
) {
    for m in &loaded.modules {
        let Some(program) = &m.program else { continue };
        for decl in &program.decls {
            if let DeclKind::Proc { params, body, .. } = &decl.kind {
                let mut locals: BTreeMap<String, TypeId> = BTreeMap::new();
                let mut builder = crate::types::Lookup {
                    module: &m.name,
                    types,
                    consts,
                };
                for p in params {
                    if let ParamType::Value(t) = &p.ty
                        && let Some(id) = builder.resolve(t)
                    {
                        locals.insert(p.name.text.clone(), id);
                    }
                }
                for v in &body.vars {
                    if let Some(id) = builder.resolve(&v.ty) {
                        locals.insert(v.name.text.clone(), id);
                    }
                }
                let mut c = Checker {
                    module: &m.name,
                    symbols,
                    types,
                    locals,
                    diags,
                };
                for s in &body.body {
                    c.stmt(s);
                }
            }
        }
    }
}

struct Checker<'a, 'd> {
    module: &'a str,
    symbols: &'a Symbols,
    types: &'a Types,
    /// Parameters and locals of the procedure being checked.
    locals: BTreeMap<String, TypeId>,
    diags: &'d mut Diagnostics,
}

impl Checker<'_, '_> {
    fn stmt(&mut self, stmt: &Stmt) {
        match &stmt.kind {
            StmtKind::Assign { target, value, .. } => {
                self.designator(target);
                self.expr(value);
            }
            StmtKind::Call { call, .. } | StmtKind::Defer { call, .. } => {
                for a in &call.args {
                    self.expr(a);
                }
            }
            StmtKind::If { arms, otherwise } => {
                for (cond, body) in arms {
                    self.condition(cond, "an `if` or `elsif`");
                    for s in body {
                        self.stmt(s);
                    }
                }
                if let Some(body) = otherwise {
                    for s in body {
                        self.stmt(s);
                    }
                }
            }
            StmtKind::While { condition, body } => {
                self.condition(condition, "a `while`");
                for s in body {
                    self.stmt(s);
                }
            }
            StmtKind::Return(Some(e)) => {
                self.expr(e);
            }
            StmtKind::Return(None) | StmtKind::Exit | StmtKind::Next => {}
            StmtKind::Raise(_) => {}
            StmtKind::Try { body, handlers } => {
                for s in body {
                    self.stmt(s);
                }
                for h in handlers {
                    for s in &h.body {
                        self.stmt(s);
                    }
                }
            }
        }
    }

    /// A condition must be `bool` [10]: `if expr then`, `while expr do`.
    fn condition(&mut self, e: &Expr, what: &str) {
        let got = self.expr(e);
        if let Some(id) = got.id()
            && !matches!(self.types.kind(self.types.underlying(id)), TypeKind::Bool)
        {
            self.diags.error(
                e.span,
                format!(
                    "the condition of {what} must be a `bool`, and this is {}",
                    self.types.describe(id)
                ),
            );
        }
    }

    fn expr(&mut self, e: &Expr) -> Typed {
        match &e.kind {
            // A literal's type: `int`, `fixed` of its own scale, `char`, `bool`, and a
            // string literal is a `String`, which is `ref array of char` [3.4, 9].
            // A literal has no tag [7.4], and a `fixed` literal's scale is the digits
            // after `v` [2].
            ExprKind::Int { .. } => Typed::Number(Num::Int),
            ExprKind::Fixed { scale, .. } => Typed::Number(Num::Fixed { scale: *scale }),
            ExprKind::Char(_) => self.predefined("char"),
            ExprKind::Bool(_) => self.predefined("bool"),
            ExprKind::Str(_) => Typed::Unknown,
            // `nil` has no type of its own: it is a value of every reference type
            // [3.3], so what it may be compared with is decided by the other operand.
            ExprKind::Nil => Typed::Unknown,
            ExprKind::Paren(inner) => self.expr(inner),
            ExprKind::Designator(d) => self.designator(d),
            ExprKind::Call { call, selectors } => {
                for a in &call.args {
                    self.expr(a);
                }
                // A call's result type needs the signature table, which is the next
                // slice; the selectors on it are then checkable too [6].
                let _ = selectors;
                Typed::Unknown
            }
            ExprKind::New(_) | ExprKind::NewArray { .. } => Typed::Unknown,
            ExprKind::Not(inner) => {
                self.condition(inner, "`not`");
                self.predefined("bool")
            }
            // `_` negates a number and changes nothing else [2, 7.4].
            ExprKind::Negate(inner) => {
                let inner_type = self.expr(inner);
                match self.numeric(inner_type) {
                    Some(n) => Typed::Number(n),
                    None => {
                        if let Some(id) = inner_type.id() {
                            self.diags.error(
                                inner.span,
                                format!(
                                    "`_` negates a number, and {} is not one [7.4]",
                                    self.types.describe(id)
                                ),
                            );
                        }
                        Typed::Unknown
                    }
                }
            }
            ExprKind::Binary { op, left, right } => self.binary(*op, left, right, e.span),
        }
    }

    fn predefined(&mut self, name: &str) -> Typed {
        // The predefined types are interned first, so they are always present.
        match name {
            "int" => self.find_predefined(TypeKind::Int),
            "bool" => self.find_predefined(TypeKind::Bool),
            "char" => self.find_predefined(TypeKind::Char),
            _ => Typed::Unknown,
        }
    }

    fn find_predefined(&self, want: TypeKind) -> Typed {
        for i in 0..self.types.len() {
            let id = TypeId(i as u32);
            if *self.types.kind(id) == want {
                return Typed::Known(id);
            }
        }
        Typed::Unknown
    }

    fn binary(&mut self, op: BinOp, left: &Expr, right: &Expr, whole: Span) -> Typed {
        let l = self.expr(left);
        let r = self.expr(right);
        match op {
            BinOp::And | BinOp::Or => {
                self.condition(left, &format!("`{}`", op.text()));
                self.condition(right, &format!("`{}`", op.text()));
                self.predefined("bool")
            }
            BinOp::Equal => {
                self.check_equality(l, r, left, right, whole);
                self.predefined("bool")
            }
            BinOp::Identical => {
                // One mistake, one diagnostic: `a ~ b` on two ints is a single misuse
                // of `~`, not one fault per operand.
                if !self.check_identity(l, left) {
                    self.check_identity(r, right);
                }
                self.predefined("bool")
            }
            BinOp::Less | BinOp::LessEqual | BinOp::Greater | BinOp::GreaterEqual => {
                self.ordering(op, l, r, left, right, whole);
                self.predefined("bool")
            }
            BinOp::Add | BinOp::Subtract | BinOp::Multiply | BinOp::Divide => {
                self.arithmetic(op, l, r, left, right, whole)
            }
            BinOp::Div | BinOp::Mod => {
                self.int_only(op, l, left);
                self.int_only(op, r, right);
                Typed::Number(Num::Int)
            }
        }
    }

    /// The numeric view of a type, if it is a number [7.4].
    fn numeric(&self, t: Typed) -> Option<Num> {
        match t {
            Typed::Number(n) => Some(n),
            Typed::Unknown => None,
            Typed::Known(id) => match self.types.kind(self.types.underlying(id)) {
                TypeKind::Int => Some(Num::Int),
                TypeKind::Fixed { scale, .. } => Some(Num::Fixed { scale: *scale }),
                _ => None,
            },
        }
    }

    /// Report a side that should be a number but is not. Returns whether it reported.
    fn want_number(&mut self, op: BinOp, side: Typed, expr: &Expr) -> bool {
        if self.numeric(side).is_some() || side == Typed::Unknown {
            return false;
        }
        let described = side
            .id()
            .map(|id| self.types.describe(id))
            .unwrap_or_else(|| "this".to_owned());
        self.diags.error(
            expr.span,
            format!(
                "`{}` works on numbers, and {described} is not one [7.4]",
                op.text()
            ),
        );
        true
    }

    /// `div` and `mod` take two `int` values [7.3].
    fn int_only(&mut self, op: BinOp, side: Typed, expr: &Expr) {
        match self.numeric(side) {
            Some(Num::Int) | None => {}
            Some(other) => {
                self.diags.error(
                    expr.span,
                    format!(
                        "`{}` takes two `int` values, and this is {} [7.3]; use `/` for \
                         decimal division",
                        op.text(),
                        other.describe()
                    ),
                );
            }
        }
    }

    /// `<` `<=` `>` `>=`: numbers of any scales, or two `char` values.
    fn ordering(&mut self, op: BinOp, l: Typed, r: Typed, left: &Expr, right: &Expr, whole: Span) {
        // Two `char`s order by code point, which [7.1] allows alongside numbers.
        let is_char = |t: Typed, s: &Self| {
            t.id()
                .is_some_and(|id| matches!(s.types.kind(s.types.underlying(id)), TypeKind::Char))
        };
        if is_char(l, self) && is_char(r, self) {
            return;
        }
        if self.want_number(op, l, left) || self.want_number(op, r, right) {
            return;
        }
        let _ = whole;
    }

    /// `+` `-` `*` `/` and their scale rules [7.4].
    fn arithmetic(
        &mut self,
        op: BinOp,
        l: Typed,
        r: Typed,
        left: &Expr,
        right: &Expr,
        whole: Span,
    ) -> Typed {
        if self.want_number(op, l, left) || self.want_number(op, r, right) {
            return Typed::Unknown;
        }
        let (Some(a), Some(b)) = (self.numeric(l), self.numeric(r)) else {
            return Typed::Unknown;
        };
        match op {
            // "Addition and subtraction are exact, with the larger of the two scales."
            // Two `int`s stay an `int`; an `int` mixed with a `fixed` is scale 0.
            BinOp::Add | BinOp::Subtract => {
                if a == Num::Int && b == Num::Int {
                    Typed::Number(Num::Int)
                } else {
                    Typed::Number(Num::Fixed {
                        scale: a.scale().max(b.scale()),
                    })
                }
            }
            // "Multiplication is exact, and the scales add."
            BinOp::Multiply => {
                if a == Num::Int && b == Num::Int {
                    Typed::Number(Num::Int)
                } else {
                    Typed::Number(Num::Fixed {
                        scale: a.scale().saturating_add(b.scale()),
                    })
                }
            }
            // "`/` with two `int` operands is a compile error; use `div`." Otherwise
            // "division `/` gives the quotient at the sum of the two scales".
            BinOp::Divide => {
                if a == Num::Int && b == Num::Int {
                    self.diags.error(
                        whole,
                        "`/` with two `int` operands is a compile error; use `div` [7.4]",
                    );
                    return Typed::Unknown;
                }
                Typed::Number(Num::Fixed {
                    scale: a.scale().saturating_add(b.scale()),
                })
            }
            _ => unreachable!("only the four arithmetic operators reach here"),
        }
    }

    /// `=` "is defined on `int`, `bool`, `char`, `fixed`, and on records and arrays
    /// that contain no references, mailboxes, replies, views, publishers, tasks or
    /// files. It is not defined on those types." [7.1]
    fn check_equality(&mut self, l: Typed, r: Typed, left: &Expr, right: &Expr, whole: Span) {
        // Two numbers compare by value whatever their scales [7.4], so neither the
        // equality blocker nor the same-type rule below applies to them.
        if self.numeric(l).is_some() && self.numeric(r).is_some() {
            return;
        }
        for (side, expr) in [(l, left), (r, right)] {
            let Some(id) = side.id() else { continue };
            if let Some(blocker) = self.types.equality_blocker(id) {
                let mut d = Diagnostic::error(
                    expr.span,
                    format!("`=` is not defined on {} [7.1]", self.types.describe(id)),
                );
                if blocker != self.types.underlying(id) {
                    // Say which part of the record or array is the problem: the
                    // programmer sees a record name, not the reference inside it.
                    d = d.note(
                        expr.span,
                        format!(
                            "it contains {}, and `=` is not defined on that; compare the \
                             parts, or use a library procedure such as `stringcmp` [7.1]",
                            self.types.describe(blocker)
                        ),
                    );
                } else {
                    d = d.note(
                        expr.span,
                        "use `~` to ask whether two references are the same object [7.1]"
                            .to_owned(),
                    );
                }
                self.diags.push(d);
                return;
            }
        }
        // Both sides must be the same type: identity is by name [3.5].
        if let (Some(a), Some(b)) = (l.id(), r.id())
            && self.types.underlying(a) != self.types.underlying(b)
        {
            self.diags.error(
                whole,
                format!(
                    "`=` compares two values of one type, and these are {} and {} [3.5]",
                    self.types.describe(a),
                    self.types.describe(b)
                ),
            );
        }
    }

    /// `~` "is defined only on references, mailboxes and views (and `nil`)" [7.1].
    /// Returns whether anything was reported.
    fn check_identity(&mut self, side: Typed, expr: &Expr) -> bool {
        let Some(id) = side.id() else { return false };
        if !self.types.has_identity(id) {
            let mut d = Diagnostic::error(
                expr.span,
                format!(
                    "`~` asks whether two references are the same object, and {} is not \
                     a reference, a mailbox or a view [7.1]",
                    self.types.describe(id)
                ),
            );
            if self.types.has_equality(id) {
                d = d.note(expr.span, "use `=` to compare values [7.1]".to_owned());
            }
            self.diags.push(d);
            return true;
        }
        false
    }

    /// The type of a designator: a variable, then its selectors [10].
    fn designator(&mut self, d: &Designator) -> Typed {
        let mut current = self.base(&d.base);
        for selector in &d.selectors {
            current = match selector {
                Selector::Index(e) => {
                    self.expr(e);
                    self.index(current, e.span)
                }
                Selector::Field(name) => self.field(current, name),
            };
        }
        current
    }

    /// The type of the name a designator starts from.
    fn base(&mut self, q: &QIdent) -> Typed {
        if q.module.is_none()
            && let Some(id) = self.locals.get(&q.name.text)
        {
            return Typed::Known(*id);
        }
        // A module-level name: a `share` has a type, a constant has one too, but
        // neither is reachable without the signature table of the next slice. The
        // library's names are unknown for the same reason.
        let module = q
            .module
            .as_ref()
            .map(|m| m.text.as_str())
            .unwrap_or(self.module);
        if let Some(names) = self.symbols.modules.get(module)
            && let Some(symbol) = names.by_name.get(&q.name.text)
            && symbol.kind == SymbolKind::Type
        {
            // Resolution already reports a type used as a value.
            return Typed::Unknown;
        }
        let _ = library::lookup(&q.name.text);
        Typed::Unknown
    }

    /// Indexing: the operand must be an array, and the result is its element type.
    fn index(&mut self, target: Typed, span: Span) -> Typed {
        let Some(id) = target.id() else {
            return Typed::Unknown;
        };
        match self.types.kind(self.types.underlying(id)) {
            TypeKind::Array { element, .. } | TypeKind::RefArray(element) => Typed::Known(*element),
            _ => {
                self.diags.error(
                    span,
                    format!(
                        "{} cannot be indexed: it is not an array",
                        self.types.describe(id)
                    ),
                );
                Typed::Unknown
            }
        }
    }

    /// A field: the record must have it, and a private field is reachable only in the
    /// module that declares the record type [4.1].
    fn field(&mut self, target: Typed, name: &Ident) -> Typed {
        let Some(id) = target.id() else {
            return Typed::Unknown;
        };
        // A `ref R` is followed: `c.value` where `c: ref Cell` reads the record.
        let holder = match self.types.kind(self.types.underlying(id)) {
            TypeKind::Ref(inner) => self.types.underlying(*inner),
            _ => self.types.underlying(id),
        };
        let TypeKind::Record { module, fields } = self.types.kind(holder) else {
            self.diags.error(
                name.span,
                format!(
                    "{} has no fields: a field is selected from a record [10]",
                    self.types.describe(id)
                ),
            );
            return Typed::Unknown;
        };
        let Some(field) = fields.iter().find(|f| f.name == name.text) else {
            let known: Vec<&str> = fields.iter().map(|f| f.name.as_str()).collect();
            let mut d = Diagnostic::error(
                name.span,
                format!("{} has no field `{}`", self.types.describe(id), name.text),
            );
            if !known.is_empty() {
                d = d.note(name.span, format!("its fields are {}", known.join(", ")));
            }
            self.diags.push(d);
            return Typed::Unknown;
        };
        // [4.1]: "a private field can be read and assigned only in the module that
        // declares the record type, and a use anywhere else is a compile error."
        if field.private && module != self.module {
            self.diags.push(
                Diagnostic::error(
                    name.span,
                    format!(
                        "`{}` is a private field of a record declared in `{module}`, so \
                         it can only be read and assigned there [4.1]",
                        name.text
                    ),
                )
                .note(field.span, "declared `private` here"),
            );
        }
        Typed::Known(field.ty)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alm_core::diag::SourceMap;

    /// Run the whole front end over a program and return the diagnostics.
    fn check_modules(files: &[(&str, &str)]) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!(
            "almc0-check-{}-{:?}",
            std::process::id(),
            std::thread::current().id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).expect("a writable temporary directory");
        for (name, text) in files {
            std::fs::write(dir.join(name), text).expect("writable");
        }
        let mut sources = SourceMap::new();
        let mut diags = Diagnostics::new();
        let loaded = crate::load::load(&dir.join(files[0].0), &[], &mut sources, &mut diags);
        assert!(
            !diags.has_errors(),
            "the test source must parse: {:?}",
            diags
                .items()
                .iter()
                .map(|d| d.render(&sources))
                .collect::<Vec<_>>()
        );
        let symbols = crate::resolve::resolve(&loaded, &mut diags);
        let consts = crate::consts::evaluate(&loaded, &mut diags);
        let options = alm_core::options::Options::default();
        let types = crate::types::build(&loaded, &consts, &options, &mut diags);
        assert!(
            !diags.has_errors(),
            "the test source must resolve: {:?}",
            diags
                .items()
                .iter()
                .map(|d| d.render(&sources))
                .collect::<Vec<_>>()
        );
        check(&loaded, &symbols, &types, &consts, &mut diags);
        let out = diags.items().iter().map(|d| d.render(&sources)).collect();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    fn check_one(source: &str) -> Vec<String> {
        check_modules(&[("Main.alm", source)])
    }

    fn only(source: &str, wanted: &str) -> String {
        let d = check_one(source);
        assert_eq!(d.len(), 1, "expected one diagnostic, got {d:#?}");
        assert!(d[0].contains(wanted), "expected {wanted:?} in {:?}", d[0]);
        d[0].clone()
    }

    #[test]
    fn a_field_must_exist_on_the_record() {
        let d = only(
            "type Cell is record value: int; link: ref Cell; end;\n\
             proc start()\n  var c: ref Cell;\nbegin\n  c.nope := 1;\nend;\n",
            "has no field `nope`",
        );
        assert!(d.contains("its fields are value, link"), "list them: {d}");
    }

    #[test]
    fn a_ref_is_followed_to_select_a_field() {
        // `c.value` where `c: ref Cell` reads the record the reference points at.
        assert!(
            check_one(
                "type Cell is record value: int; link: ref Cell; end;\n\
                 proc start()\n  var c: ref Cell;\nbegin\n  \
                 c.value := 1;\n  c.link.value := 2;\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn only_a_record_has_fields_and_only_an_array_is_indexed() {
        only(
            "proc start()\n  var n: int;\nbegin\n  n.field := 1;\nend;\n",
            "has no fields",
        );
        only(
            "proc start()\n  var n: int;\nbegin\n  n[0] := 1;\nend;\n",
            "cannot be indexed",
        );
    }

    #[test]
    fn indexing_gives_the_element_type() {
        assert!(
            check_one(
                "type Cell is record value: int; end;\n\
                 proc start()\n  var a: array [4] of Cell;\n  var r: ref array of Cell;\n\
                 begin\n  a[0].value := 1;\n  r[1].value := 2;\nend;\n"
            )
            .is_empty()
        );
        only(
            "type Cell is record value: int; end;\n\
             proc start()\n  var a: array [4] of Cell;\nbegin\n  a[0].nope := 1;\nend;\n",
            "has no field `nope`",
        );
    }

    #[test]
    fn a_private_field_is_reachable_only_in_its_own_module() {
        // [4.1]: "a private field can be read and assigned only in the module that
        // declares the record type, and a use anywhere else is a compile error."
        // Inside its own module it is fine.
        assert!(
            check_one(
                "type Account is record name: String; private balance: int; end;\n\
                 proc credit(a: ref Account, n: int)\nbegin\n  \
                 a.balance := a.balance + n;\nend;\n"
            )
            .is_empty()
        );
        // From another module it is not.
        let d = check_modules(&[
            (
                "Main.alm",
                "import Bank;\nproc start()\n  var a: ref Bank::Account;\n  var n: int;\n\
                 begin\n  n := a.balance;\nend;\n",
            ),
            (
                "Bank.alm",
                "type Account is record name: String; private balance: int; end;\n",
            ),
        ]);
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(
            d[0].contains("is a private field of a record declared in `Bank`"),
            "{d:?}"
        );
        assert!(
            d[0].contains("declared `private` here"),
            "point at it: {d:?}"
        );
    }

    #[test]
    fn a_visible_field_of_another_modules_record_is_fine() {
        // "Other modules can still hold, pass, copy and compare whole records with
        // private fields" [4.1] — and read the visible ones.
        assert!(
            check_modules(&[
                (
                    "Main.alm",
                    "import Bank;\nproc start()\n  var a: ref Bank::Account;\n  var s: String;\n\
                     begin\n  s := a.name;\nend;\n",
                ),
                (
                    "Bank.alm",
                    "type Account is record name: String; private balance: int; end;\n",
                ),
            ])
            .is_empty()
        );
    }

    #[test]
    fn a_private_type_may_appear_in_a_visible_signature() {
        // [4.1]: "A private type may appear in the signature of a visible procedure:
        // other modules can pass its values around but cannot name the type." The
        // permissive half is the part worth a test — nothing may reject it.
        assert!(
            check_modules(&[
                (
                    "Main.alm",
                    "import Dep;\nproc start()\n  var n: int;\nbegin\n                       n := Dep::idOf(Dep::make(7));\nend;\n",
                ),
                (
                    "Dep.alm",
                    "private type Handle is record id: int; end;\n\
                     proc make(n: int): ref Handle\n  var h: ref Handle;\nbegin\n                       h := new(Handle);\n  h.id := n;\n  return h;\nend;\n\
                     proc idOf(h: ref Handle): int\nbegin\n  return h.id;\nend;\n",
                ),
            ])
            .is_empty()
        );
    }

    #[test]
    fn equality_is_rejected_where_the_spec_forbids_it() {
        // [7.1]: not defined on references, mailboxes, replies, views, publishers,
        // tasks or files, nor on a record or array containing one.
        let d = only(
            "type Cell is record value: int; link: ref Cell; end;\n\
             proc start()\n  var a: Cell;\n  var b: Cell;\n  var same: bool;\nbegin\n  \
             same := a = b;\nend;\n",
            "`=` is not defined on",
        );
        assert!(
            d.contains("it contains ref") || d.contains("it contains"),
            "say which part stops it: {d}"
        );
        // A reference itself.
        let d = only(
            "type Cell is record value: int; end;\n\
             proc start()\n  var a: ref Cell;\n  var b: ref Cell;\n  var same: bool;\n\
             begin\n  same := a = b;\nend;\n",
            "`=` is not defined on",
        );
        assert!(d.contains("use `~`"), "suggest the right operator: {d}");
    }

    #[test]
    fn equality_is_allowed_on_reference_free_values() {
        assert!(
            check_one(
                "type Point is record x: int; y: int; end;\n\
                 proc start()\n  var a: Point;\n  var b: Point;\n  var same: bool;\n\
                 begin\n  same := a = b;\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn equality_needs_two_values_of_one_type() {
        // Type identity is by name [3.5], so two different declared types do not
        // compare even when they are declared the same way.
        let d = only(
            "type A is record x: int; end;\n\
             type B is record x: int; end;\n\
             proc start()\n  var a: A;\n  var b: B;\n  var same: bool;\nbegin\n  \
             same := a = b;\nend;\n",
            "`=` compares two values of one type",
        );
        assert!(d.contains("Main::A") && d.contains("Main::B"), "{d}");
    }

    #[test]
    fn identity_is_rejected_where_the_spec_forbids_it() {
        // [7.1]: "`~` is defined only on references, mailboxes and views (and `nil`)."
        let d = only(
            "proc start()\n  var a: int;\n  var b: int;\n  var same: bool;\nbegin\n  \
             same := a ~ b;\nend;\n",
            "is not a reference, a mailbox or a view",
        );
        assert!(d.contains("use `=`"), "suggest the right operator: {d}");
    }

    #[test]
    fn identity_is_allowed_on_references_mailboxes_and_views() {
        assert!(
            check_one(
                "type R is record x: int; end;\n\
                 proc start()\n  var a: ref R;\n  var b: ref R;\n  var m: mailbox of R;\n\
                 var v: view of R;\n  var same: bool;\nbegin\n  \
                 same := a ~ b;\n  same := a ~ nil;\n  same := m ~ m;\n  same := v ~ v;\n\
                 end;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn the_scale_rules_of_section_seven_four() {
        // "Addition and subtraction are exact, with the larger of the two scales.
        // Multiplication is exact, and the scales add ... Division `/` gives the
        // quotient at the sum of the two scales."
        //
        // The scales are not observable until an assignment is range-checked, so what
        // is asserted here is that every legal combination is accepted and the illegal
        // one is not.
        assert!(
            check_one(
                "type Money is fixed(9, 2);\n\
                 proc start()\n  var a: Money;\n  var b: Money;\n  var n: int;\n                   var m: fixed(9, 4);\nbegin\n                   m := a + b;\n  m := a - b;\n  m := a * b;\n  m := a / b;\n                   m := a + n;\n  m := n * a;\n  m := a / n;\n  n := n + 1;\n                   n := n * 2;\n  n := n div 2;\n  n := n mod 2;\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn slash_on_two_ints_is_rejected_in_an_expression() {
        // [7.4]: "`/` with two `int` operands is a compile error; use `div`." The
        // constant evaluator already refused this; now an ordinary expression does.
        let d = only(
            "proc start()\n  var n: int;\n  var m: fixed(9, 2);\nbegin\n               m := n / 2;\nend;\n",
            "use `div`",
        );
        assert!(d.contains("two `int` operands"), "{d}");
        // One `fixed` operand makes it legal, including an `int` literal divisor.
        assert!(
            check_one(
                "proc start()\n  var x: fixed(9, 2);\n  var m: fixed(9, 4);\nbegin\n                   m := x / 2;\n  m := 1v0 / 3v0;\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn div_and_mod_take_two_ints() {
        // [7.3]. The message points at `/`, which is the decimal operator.
        let d = only(
            "proc start()\n  var x: fixed(9, 2);\n  var n: int;\nbegin\n               n := x div 2;\nend;\n",
            "takes two `int` values",
        );
        assert!(d.contains("use `/`"), "suggest the right operator: {d}");
        only(
            "proc start()\n  var x: fixed(9, 2);\n  var n: int;\nbegin\n               n := 7 mod x;\nend;\n",
            "takes two `int` values",
        );
    }

    #[test]
    fn arithmetic_needs_numbers() {
        only(
            "proc start()\n  var b: bool;\n  var n: int;\nbegin\n  n := b + 1;\nend;\n",
            "`+` works on numbers",
        );
        only(
            "type R is record x: int; end;\n\
             proc start()\n  var r: R;\n  var n: int;\nbegin\n  n := r * 2;\nend;\n",
            "`*` works on numbers",
        );
        // `_` too.
        only(
            "proc start()\n  var b: bool;\n  var n: int;\nbegin\n  n := _b;\nend;\n",
            "`_` negates a number",
        );
    }

    #[test]
    fn ordering_compares_numbers_or_two_chars() {
        assert!(
            check_one(
                "proc start()\n  var x: fixed(9, 2);\n  var n: int;\n  var c: char;\n                   var d: char;\n  var b: bool;\nbegin\n                   b := n < 1;\n  b := x <= 2v5;\n  b := x > n;\n  b := c < d;\nend;\n"
            )
            .is_empty()
        );
        only(
            "proc start()\n  var b: bool;\n  var c: bool;\nbegin\n  b := b < c;\nend;\n",
            "`<` works on numbers",
        );
    }

    #[test]
    fn two_numbers_compare_with_equals_whatever_their_scales() {
        // [7.4]: "Comparison is by numeric value, so `1v5 = 1v50`." So `=` on two
        // numbers is not subject to the same-type rule that applies to records.
        assert!(
            check_one(
                "type Money is fixed(9, 2);\n\
                 proc start()\n  var a: Money;\n  var x: fixed(9, 4);\n  var n: int;\n                   var b: bool;\nbegin\n                   b := a = x;\n  b := a = n;\n  b := n = 1;\n  b := 1v5 = 1v50;\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn a_condition_must_be_a_bool() {
        only(
            "proc start()\n  var n: int;\nbegin\n  if n then\n    n := 1;\n  end;\nend;\n",
            "the condition of an `if` or `elsif` must be a `bool`",
        );
        only(
            "proc start()\n  var n: int;\nbegin\n  while n do\n    n := 1;\n  end;\nend;\n",
            "the condition of a `while` must be a `bool`",
        );
        only(
            "proc start()\n  var n: int;\n  var b: bool;\nbegin\n  b := not n;\nend;\n",
            "the condition of `not` must be a `bool`",
        );
        only(
            "proc start()\n  var n: int;\n  var b: bool;\nbegin\n  b := b and n;\nend;\n",
            "the condition of `and` must be a `bool`",
        );
    }

    #[test]
    fn a_bool_condition_is_accepted_in_every_form() {
        assert!(
            check_one(
                "proc start()\n  var b: bool;\n  var c: bool;\nbegin\n  \
                 if b and c then\n    b := not c;\n  elsif b or c then\n    b := false;\n  \
                 end;\n  while b do\n    b := false;\n  end;\nend;\n"
            )
            .is_empty()
        );
    }
}

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

/// The type of an expression, or `Unknown` when this part cannot say.
///
/// `Unknown` is not an error: it means "not determined yet", and every check treats it
/// as acceptable so that one unfinished area does not produce a cascade of complaints
/// about correct code.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Typed {
    Known(TypeId),
    Unknown,
}

impl Typed {
    fn id(self) -> Option<TypeId> {
        match self {
            Typed::Known(id) => Some(id),
            Typed::Unknown => None,
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
            ExprKind::Int { .. } => self.predefined("int"),
            ExprKind::Fixed { .. } => Typed::Unknown,
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
            ExprKind::Negate(inner) => self.expr(inner),
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
            // Ordering, arithmetic and the tag rules need the number rules of [7.4],
            // which is the next slice.
            _ => Typed::Unknown,
        }
    }

    /// `=` "is defined on `int`, `bool`, `char`, `fixed`, and on records and arrays
    /// that contain no references, mailboxes, replies, views, publishers, tasks or
    /// files. It is not defined on those types." [7.1]
    fn check_equality(&mut self, l: Typed, r: Typed, left: &Expr, right: &Expr, whole: Span) {
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
        let types = crate::types::build(&loaded, &consts, &mut diags);
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

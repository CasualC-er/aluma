//! The type graph [3.5, 4.3, 7.1, 7.4].
//!
//! Types are interned and compared by identity, because type compatibility is **by
//! name**: "two types are the same only if they are the same declared type (or both
//! spell out the same predefined type). Every `record` declaration is its own type"
//! [3.5]. So a declared type gets an id of its own and two spellings of the same
//! declaration share one; nothing here compares structures.
//!
//! A declared `type D is fixed(p, s);` or `type D is int;` is a **named number type**
//! whose name is a *tag* [7.4]: `price + fare` of different tags is a compile error even
//! though both are decimals. The tag is therefore part of the type's identity, which
//! falls out of name equivalence rather than needing a rule of its own.
//!
//! What this module computes:
//!
//! - an id per declared type, and per predefined or constructed type;
//! - the zero value of every type [3.3];
//! - whether `=` is defined on a type, which [7.1] makes a structural question: "records
//!   and arrays that contain no references, mailboxes, replies, views, publishers, tasks
//!   or files";
//! - the by-value containment cycles and alias loops that [4.3] forbids, reported with
//!   the cycle named.

use std::collections::BTreeMap;

use alm_core::diag::{Diagnostic, Diagnostics};
use alm_core::span::Span;

use crate::ast::{self, DeclKind};
use crate::consts::{ConstValue, Consts, evaluate_expr};
use crate::load::Loaded;

/// An interned type.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct TypeId(pub u32);

/// What a type is.
///
/// A `Named` type is its own type whatever it is declared as, which is what name
/// equivalence means [3.5]. Its `underlying` says how it behaves.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypeKind {
    Int,
    Bool,
    Char,
    /// `fixed(p, s)`: at most `p` digits, `s` after the point [7.4].
    Fixed {
        precision: u32,
        scale: u32,
    },
    /// A declared type: `type D is T;`. Its own type, and a *tag* when `T` is a number
    /// type [7.4].
    Named {
        module: String,
        name: String,
        underlying: TypeId,
    },
    /// `record ... end`. Every declaration is its own type [3.5], so two records are
    /// the same type only through the same `Named`.
    Record {
        module: String,
        fields: Vec<Field>,
    },
    /// `array [n] of T`
    Array {
        length: u64,
        element: TypeId,
    },
    /// `ref R`, `ref array of T`
    Ref(TypeId),
    RefArray(TypeId),
    Inbox(TypeId),
    Mailbox(TypeId),
    Reply(TypeId),
    View(TypeId),
    Publisher(TypeId),
    /// `task [of R]`
    Task(Option<TypeId>),
    /// A predefined opaque reference type: `File` [9.3]. `Listener` is `Net`'s [9.7].
    Opaque {
        name: String,
    },
    /// A generic module's type parameter [4.4]: opaque inside the module.
    Parameter {
        module: String,
        name: String,
    },
    /// Reported already; stops one mistake becoming many.
    Error,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Field {
    pub name: String,
    /// "A record field is visible unless its declaration starts with `private`" [4.1].
    pub private: bool,
    pub ty: TypeId,
    pub span: Span,
}

/// The zero value of a type [3.3].
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Zero {
    Int,
    Bool,
    Char,
    Fixed,
    /// A reference-like type: `ref`, `mailbox`, `reply`, `view`, `publisher`, `task`,
    /// `File` and `Listener` all start `nil` [3.3].
    Nil,
    /// "aggregates zero in every part" [3.3].
    Aggregate,
}

/// Every type in the program, interned.
#[derive(Clone, Debug, Default)]
pub struct Types {
    kinds: Vec<TypeKind>,
    /// Structural types that may be shared: the predefined ones and the constructed
    /// ones. A `Named` or `Record` is never interned by structure.
    interned: BTreeMap<String, TypeId>,
    /// `module::name` of every declared type.
    declared: BTreeMap<(String, String), TypeId>,
}

impl Types {
    pub fn kind(&self, id: TypeId) -> &TypeKind {
        &self.kinds[id.0 as usize]
    }

    pub fn len(&self) -> usize {
        self.kinds.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kinds.is_empty()
    }

    pub fn declared(&self, module: &str, name: &str) -> Option<TypeId> {
        self.declared
            .get(&(module.to_owned(), name.to_owned()))
            .copied()
    }

    fn add(&mut self, kind: TypeKind) -> TypeId {
        let id = TypeId(self.kinds.len() as u32);
        self.kinds.push(kind);
        id
    }

    /// Intern a structural type by a key, so that two spellings of `ref array of char`
    /// are one id. Declared types and records never come here.
    fn intern(&mut self, key: String, kind: TypeKind) -> TypeId {
        if let Some(id) = self.interned.get(&key) {
            return *id;
        }
        let id = self.add(kind);
        self.interned.insert(key, id);
        id
    }

    /// Follow `Named` types to what they are declared as.
    pub fn underlying(&self, id: TypeId) -> TypeId {
        match self.kind(id) {
            TypeKind::Named { underlying, .. } => self.underlying(*underlying),
            _ => id,
        }
    }

    /// The tag of a value of this type, if it has one [7.4].
    ///
    /// "A value whose type is a named type has that name as its tag. Plain `int` and
    /// `fixed` values, and literals, have no tag."
    pub fn tag(&self, id: TypeId) -> Option<(&str, &str)> {
        match self.kind(id) {
            TypeKind::Named {
                module,
                name,
                underlying,
            } => {
                // Only a *number* type carries a tag; `type S is String;` names a
                // reference type, which the tag rules do not mention.
                match self.kind(self.underlying(*underlying)) {
                    TypeKind::Int | TypeKind::Fixed { .. } => Some((module, name)),
                    _ => None,
                }
            }
            _ => None,
        }
    }

    /// The zero value [3.3].
    pub fn zero(&self, id: TypeId) -> Zero {
        match self.kind(id) {
            // A named type zeroes as what it is declared as.
            TypeKind::Named { underlying, .. } => self.zero(*underlying),
            TypeKind::Int => Zero::Int,
            TypeKind::Bool => Zero::Bool,
            TypeKind::Char => Zero::Char,
            TypeKind::Fixed { .. } => Zero::Fixed,
            TypeKind::Record { .. } | TypeKind::Array { .. } => Zero::Aggregate,
            TypeKind::Ref(_)
            | TypeKind::RefArray(_)
            | TypeKind::Inbox(_)
            | TypeKind::Mailbox(_)
            | TypeKind::Reply(_)
            | TypeKind::View(_)
            | TypeKind::Publisher(_)
            | TypeKind::Task(_)
            | TypeKind::Opaque { .. } => Zero::Nil,
            // A generic parameter's zero is the argument's; inside the module it is
            // opaque and nothing may be assumed [4.4].
            TypeKind::Parameter { .. } => Zero::Aggregate,
            TypeKind::Error => Zero::Int,
        }
    }

    /// Is `=` defined on this type? [7.1]
    ///
    /// "`=` compares values. It is defined on `int`, `bool`, `char`, `fixed`, and on
    /// records and arrays that contain no references, mailboxes, replies, views,
    /// publishers, tasks or files."
    pub fn has_equality(&self, id: TypeId) -> bool {
        self.equality_blocker(id).is_none()
    }

    /// What stops `=` working on this type, if anything: the offending type, so that a
    /// diagnostic can say *why* a record cannot be compared.
    pub fn equality_blocker(&self, id: TypeId) -> Option<TypeId> {
        match self.kind(self.underlying(id)) {
            TypeKind::Int | TypeKind::Bool | TypeKind::Char | TypeKind::Fixed { .. } => None,
            TypeKind::Record { fields, .. } => {
                fields.iter().find_map(|f| self.equality_blocker(f.ty))
            }
            TypeKind::Array { element, .. } => self.equality_blocker(*element),
            // A generic parameter is opaque: `=` on it is rejected [4.4], and the
            // instance's argument is checked where it is instantiated.
            TypeKind::Parameter { .. } => Some(self.underlying(id)),
            TypeKind::Error => None,
            // Everything else is a reference, a handle or a file.
            _ => Some(self.underlying(id)),
        }
    }

    /// Is `~` defined on this type? [7.1]: "only on references, mailboxes and views
    /// (and `nil`)".
    pub fn has_identity(&self, id: TypeId) -> bool {
        matches!(
            self.kind(self.underlying(id)),
            TypeKind::Ref(_)
                | TypeKind::RefArray(_)
                | TypeKind::Mailbox(_)
                | TypeKind::View(_)
                | TypeKind::Error
        )
    }

    /// How a type is written in a diagnostic.
    pub fn describe(&self, id: TypeId) -> String {
        match self.kind(id) {
            TypeKind::Int => "int".to_owned(),
            TypeKind::Bool => "bool".to_owned(),
            TypeKind::Char => "char".to_owned(),
            TypeKind::Fixed { precision, scale } => format!("fixed({precision}, {scale})"),
            TypeKind::Named { module, name, .. } => format!("{module}::{name}"),
            TypeKind::Record { .. } => "a record".to_owned(),
            TypeKind::Array { length, element } => {
                format!("array [{length}] of {}", self.describe(*element))
            }
            TypeKind::Ref(inner) => format!("ref {}", self.describe(*inner)),
            TypeKind::RefArray(inner) => format!("ref array of {}", self.describe(*inner)),
            TypeKind::Inbox(inner) => format!("inbox of {}", self.describe(*inner)),
            TypeKind::Mailbox(inner) => format!("mailbox of {}", self.describe(*inner)),
            TypeKind::Reply(inner) => format!("reply of {}", self.describe(*inner)),
            TypeKind::View(inner) => format!("view of {}", self.describe(*inner)),
            TypeKind::Publisher(inner) => format!("publisher of {}", self.describe(*inner)),
            TypeKind::Task(None) => "task".to_owned(),
            TypeKind::Task(Some(inner)) => format!("task of {}", self.describe(*inner)),
            TypeKind::Opaque { name } => name.clone(),
            TypeKind::Parameter { module, name } => format!("{module}::{name}"),
            TypeKind::Error => "an unknown type".to_owned(),
        }
    }
}

/// Build the program's type graph.
pub fn build(
    loaded: &Loaded,
    consts: &Consts,
    options: &alm_core::options::Options,
    diags: &mut Diagnostics,
) -> Types {
    let mut types = Types::default();

    // The predefined types first, so that their ids are stable and small.
    types.intern("int".to_owned(), TypeKind::Int);
    types.intern("bool".to_owned(), TypeKind::Bool);
    types.intern("char".to_owned(), TypeKind::Char);
    // `File` is a predefined opaque reference type [9.3]; `String` is `ref array of
    // char` [9], which is a constructed type and interns itself on first use.
    types.intern(
        "File".to_owned(),
        TypeKind::Opaque {
            name: "File".to_owned(),
        },
    );

    // Every declared type gets an id before any body is built, so that a group's
    // members can refer to each other [4.3] and a type can refer to itself through
    // `ref` [4].
    let mut pending: Vec<(String, String, TypeId, ast::Type)> = Vec::new();
    for m in &loaded.modules {
        let Some(program) = &m.program else { continue };
        for param in &program.generics {
            if let ast::GenericParam::Type { name, .. } = param {
                let id = types.add(TypeKind::Parameter {
                    module: m.name.clone(),
                    name: name.text.clone(),
                });
                types
                    .declared
                    .insert((m.name.clone(), name.text.clone()), id);
            }
        }
        for decl in &program.decls {
            if let DeclKind::Type { name, ty } = &decl.kind {
                // A placeholder, filled in once every name has an id.
                let id = types.add(TypeKind::Error);
                types
                    .declared
                    .insert((m.name.clone(), name.text.clone()), id);
                pending.push((m.name.clone(), name.text.clone(), id, ty.clone()));
            }
        }
    }

    // Now build each declared type's body.
    for (module, name, id, ty) in &pending {
        let underlying = {
            let mut builder = Builder {
                module,
                types: &mut types,
                consts,
                options,
                diags,
            };
            builder.build(ty)
        };
        types.kinds[id.0 as usize] = TypeKind::Named {
            module: module.clone(),
            name: name.clone(),
            underlying,
        };
    }

    // The bodies of procedures and shared values use types too, and building them
    // interns anything new, which the later phases need.
    for m in &loaded.modules {
        let Some(program) = &m.program else { continue };
        let mut builder = Builder {
            module: &m.name,
            types: &mut types,
            consts,
            options,
            diags,
        };
        for decl in &program.decls {
            builder.walk_decl(decl);
        }
    }

    check_cycles(&types, &pending, diags);
    types
}

/// Resolves a type's syntax to an already-interned id, without interning anything.
///
/// The graph is built once (`build`); a later phase asking "what type is this `var`?"
/// must find the same id, not make a new one, or two spellings of one type would
/// compare unequal and name equivalence would be broken [3.5].
pub struct Lookup<'a> {
    pub module: &'a str,
    pub types: &'a Types,
    /// Needed to rebuild the key of an `array [n] of T` or a `fixed(p, s)`, whose keys
    /// hold the *values* of their constant expressions.
    pub consts: &'a Consts,
}

impl Lookup<'_> {
    /// The id of this type, if the graph holds it. `None` when it does not — a type
    /// whose construction reported an error, or one mentioning a name that does not
    /// resolve — in which case the caller treats the type as unknown rather than
    /// guessing.
    pub fn resolve(&mut self, ty: &ast::Type) -> Option<TypeId> {
        // A named type is found by name, not by a key: that is what name equivalence
        // means [3.5].
        if let ast::TypeKind::Named(q) = &ty.kind {
            return self.named(q);
        }
        let key = self.key(ty)?;
        self.types.interned.get(&key).copied()
    }

    /// The interning key of a type, matching the keys `Builder` writes.
    fn key(&mut self, ty: &ast::Type) -> Option<String> {
        Some(match &ty.kind {
            ast::TypeKind::Int => "int".to_owned(),
            ast::TypeKind::Bool => "bool".to_owned(),
            ast::TypeKind::Char => "char".to_owned(),
            ast::TypeKind::Ref(q) => format!("ref{}", self.named(q)?.0),
            ast::TypeKind::RefArray(inner) => {
                format!("refarrayof{}", self.id(inner)?.0)
            }
            ast::TypeKind::Inbox(q) => format!("inboxof{}", self.named(q)?.0),
            ast::TypeKind::Mailbox(q) => format!("mailboxof{}", self.named(q)?.0),
            ast::TypeKind::Reply(q) => format!("replyof{}", self.named(q)?.0),
            ast::TypeKind::View(q) => format!("viewof{}", self.named(q)?.0),
            ast::TypeKind::Publisher(q) => format!("publisherof{}", self.named(q)?.0),
            ast::TypeKind::Task(None) => "task".to_owned(),
            ast::TypeKind::Task(Some(q)) => format!("taskof{}", self.named(q)?.0),
            ast::TypeKind::Fixed { precision, scale } => {
                let p = self.constant(precision)?;
                let s = self.constant(scale)?;
                format!("fixed({p},{s})")
            }
            ast::TypeKind::Array { length, element } => {
                let n = self.constant(length)?;
                format!("array[{n}]of{}", self.id(element)?.0)
            }
            // A record is its own type [3.5], so it cannot be found by structure at
            // all: an inline `var r: record ... end;` has no key, and the caller treats
            // its type as unknown. Declared records are found by name instead.
            ast::TypeKind::Record { .. } | ast::TypeKind::Named(_) => return None,
        })
    }

    fn id(&mut self, ty: &ast::Type) -> Option<TypeId> {
        self.resolve(ty)
    }

    /// A constant that must be a non-negative `int`, evaluated silently: any error in
    /// it was reported when the graph was built, and reporting again here would double
    /// every such diagnostic.
    fn constant(&mut self, expr: &ast::Expr) -> Option<i64> {
        let mut quiet = Diagnostics::new();
        match crate::consts::evaluate_expr(self.module, self.consts, expr, &mut quiet) {
            Some(crate::consts::ConstValue::Int(v)) if v >= 0 => Some(v),
            _ => None,
        }
    }

    fn named(&mut self, q: &ast::QIdent) -> Option<TypeId> {
        let module = q
            .module
            .as_ref()
            .map(|m| m.text.as_str())
            .unwrap_or(self.module);
        if let Some(id) = self.types.declared(module, &q.name.text) {
            return Some(id);
        }
        // The library's types [9].
        if q.module.as_ref().is_none_or(|m| m.text == "std") {
            let key = match q.name.text.as_str() {
                "String" => {
                    let ch = *self.types.interned.get("char")?;
                    format!("refarrayof{}", ch.0)
                }
                "File" => "File".to_owned(),
                "Version" | "Round" => "fixed(36,0)".to_owned(),
                _ => return None,
            };
            return self.types.interned.get(&key).copied();
        }
        None
    }
}

/// Builds types from their syntax, interning as it goes.
struct Builder<'a, 'd> {
    module: &'a str,
    types: &'a mut Types,
    consts: &'a Consts,
    options: &'a alm_core::options::Options,
    diags: &'d mut Diagnostics,
}

impl Builder<'_, '_> {
    fn build(&mut self, ty: &ast::Type) -> TypeId {
        match &ty.kind {
            ast::TypeKind::Int => self.types.intern("int".to_owned(), TypeKind::Int),
            ast::TypeKind::Bool => self.types.intern("bool".to_owned(), TypeKind::Bool),
            ast::TypeKind::Char => self.types.intern("char".to_owned(), TypeKind::Char),
            ast::TypeKind::Fixed { precision, scale } => {
                let p = self.constant(precision, "the precision of a `fixed` type");
                let s = self.constant(scale, "the scale of a `fixed` type");
                match (p, s) {
                    (Some(p), Some(s)) => self.fixed(p, s, ty.span),
                    _ => self.types.add(TypeKind::Error),
                }
            }
            ast::TypeKind::Array { length, element } => {
                let element = self.build(element);
                match self.constant(length, "the length of an array") {
                    Some(n) if n < 0 => {
                        self.diags
                            .error(length.span, format!("an array cannot have {n} elements"));
                        self.types.add(TypeKind::Error)
                    }
                    Some(n) => {
                        let key = format!("array[{n}]of{}", element.0);
                        self.types.intern(
                            key,
                            TypeKind::Array {
                                length: n as u64,
                                element,
                            },
                        )
                    }
                    None => self.types.add(TypeKind::Error),
                }
            }
            ast::TypeKind::Record { fields } => {
                // Every `record` declaration is its own type [3.5], so this is never
                // interned: two identical records are different types.
                let mut built = Vec::with_capacity(fields.len());
                let mut seen: BTreeMap<String, Span> = BTreeMap::new();
                for f in fields {
                    let ty = self.build(&f.ty);
                    if let Some(first) = seen.get(&f.name.text) {
                        self.diags.push(
                            Diagnostic::error(
                                f.name.span,
                                format!("this record already has a field `{}`", f.name.text),
                            )
                            .note(*first, "the first one is here"),
                        );
                        continue;
                    }
                    seen.insert(f.name.text.clone(), f.name.span);
                    built.push(Field {
                        name: f.name.text.clone(),
                        private: f.private,
                        ty,
                        span: f.name.span,
                    });
                }
                self.types.add(TypeKind::Record {
                    module: self.module.to_owned(),
                    fields: built,
                })
            }
            ast::TypeKind::Named(q) => self.named(q),
            ast::TypeKind::Ref(q) => {
                let inner = self.named(q);
                self.types
                    .intern(format!("ref{}", inner.0), TypeKind::Ref(inner))
            }
            ast::TypeKind::RefArray(element) => {
                let element = self.build(element);
                self.types.intern(
                    format!("refarrayof{}", element.0),
                    TypeKind::RefArray(element),
                )
            }
            ast::TypeKind::Inbox(q) => self.of(q, "inbox", TypeKind::Inbox),
            ast::TypeKind::Mailbox(q) => self.of(q, "mailbox", TypeKind::Mailbox),
            ast::TypeKind::Reply(q) => self.of(q, "reply", TypeKind::Reply),
            ast::TypeKind::View(q) => self.of(q, "view", TypeKind::View),
            ast::TypeKind::Publisher(q) => self.of(q, "publisher", TypeKind::Publisher),
            ast::TypeKind::Task(None) => self.types.intern("task".to_owned(), TypeKind::Task(None)),
            ast::TypeKind::Task(Some(q)) => {
                let inner = self.named(q);
                self.types
                    .intern(format!("taskof{}", inner.0), TypeKind::Task(Some(inner)))
            }
        }
    }

    fn of(&mut self, q: &ast::QIdent, label: &str, wrap: fn(TypeId) -> TypeKind) -> TypeId {
        let inner = self.named(q);
        self.types
            .intern(format!("{label}of{}", inner.0), wrap(inner))
    }

    fn fixed(&mut self, precision: i64, scale: i64, span: Span) -> TypeId {
        // [7.4]: "`p` may be any positive constant with no upper limit, and `s` must
        // satisfy 0 <= s <= p."
        if precision < 1 {
            self.diags.error(
                span,
                format!("a `fixed` type needs at least one digit, not {precision} [7.4]"),
            );
            return self.types.add(TypeKind::Error);
        }
        if scale < 0 || scale > precision {
            self.diags.error(
                span,
                format!(
                    "the scale of a `fixed` type must satisfy 0 <= s <= p, and {scale} \
                     is not in 0 to {precision} [7.4]"
                ),
            );
            return self.types.add(TypeKind::Error);
        }
        // [7.4] puts no upper limit on `p`, and nor does this compiler in any way a
        // program could notice: the bound below is about the *representation* of a
        // scale, which is a `u32`. Casting an i64 straight to u32 wrapped, so
        // `fixed(5000000000, 5000000000)` became a narrow type rather than an error.
        let max = self.options.max_digits as i64;
        if precision > max {
            self.diags.error(
                span,
                format!(
                    "this compiler accepts a `fixed` precision up to {max} digits, and \
                     this one asks for {precision}; raise `--max-digits` if you need \
                     more ([7.4] sets no limit of its own)"
                ),
            );
            return self.types.add(TypeKind::Error);
        }
        let (p, s) = (precision as u32, scale as u32);
        self.types.intern(
            format!("fixed({p},{s})"),
            TypeKind::Fixed {
                precision: p,
                scale: s,
            },
        )
    }

    /// A named type, possibly qualified, possibly the library's.
    fn named(&mut self, q: &ast::QIdent) -> TypeId {
        let module = q
            .module
            .as_ref()
            .map(|m| m.text.as_str())
            .unwrap_or(self.module);
        if let Some(id) = self.types.declared(module, &q.name.text) {
            return id;
        }
        // The library's types [9]: `String` is `ref array of char`, `File` is opaque,
        // and `Version` and `Round` are `fixed(36, 0)`.
        if q.module.as_ref().is_none_or(|m| m.text == "std") {
            match q.name.text.as_str() {
                "String" => {
                    let ch = self.types.intern("char".to_owned(), TypeKind::Char);
                    return self
                        .types
                        .intern(format!("refarrayof{}", ch.0), TypeKind::RefArray(ch));
                }
                "File" => {
                    return self.types.intern(
                        "File".to_owned(),
                        TypeKind::Opaque {
                            name: "File".to_owned(),
                        },
                    );
                }
                "Version" | "Round" => {
                    return self.types.intern(
                        "fixed(36,0)".to_owned(),
                        TypeKind::Fixed {
                            precision: 36,
                            scale: 0,
                        },
                    );
                }
                _ => {}
            }
        }
        // Resolution has already reported an unknown name; this is a name that exists
        // but is not a type, or a module whose source is not read yet.
        self.types.add(TypeKind::Error)
    }

    /// A constant that must be an `int`.
    fn constant(&mut self, expr: &ast::Expr, what: &str) -> Option<i64> {
        let value = evaluate_expr(self.module, self.consts, expr, self.diags)?;
        match value {
            ConstValue::Int(v) => Some(v),
            other => {
                self.diags.error(
                    expr.span,
                    format!("{what} must be an `int`, and this is {}", other.describe()),
                );
                None
            }
        }
    }

    /// Build the types a declaration's signature and body mention.
    fn walk_decl(&mut self, decl: &ast::Decl) {
        match &decl.kind {
            DeclKind::Share { ty, .. } => {
                self.build(ty);
            }
            DeclKind::Proc {
                params,
                result,
                body,
                ..
            } => {
                for p in params {
                    match &p.ty {
                        ast::ParamType::Value(t) => {
                            self.build(t);
                        }
                        ast::ParamType::Proc { params, result, .. } => {
                            for t in params {
                                self.build(t);
                            }
                            if let Some(r) = result {
                                self.build(r);
                            }
                        }
                    }
                }
                if let Some(r) = result {
                    self.build(r);
                }
                for v in &body.vars {
                    self.build(&v.ty);
                }
            }
            _ => {}
        }
    }
}

/// Report the by-value containment cycles and alias loops [4.3] forbids.
///
/// "A type group cannot contain a cycle of containment by value: a record or
/// fixed-length array may hold another member only through `ref`, unless the other
/// member does not hold it back. An alias loop, such as `type A is B; also type B is
/// A;`, is also an error. Both are compile errors that name the cycle."
fn check_cycles(
    types: &Types,
    declared: &[(String, String, TypeId, ast::Type)],
    diags: &mut Diagnostics,
) {
    // A cycle is one mistake, so it is reported once. Without this, `A` contains `B`
    // contains `A` is reported from both ends: two diagnostics for one fault, which
    // the conformance harness's "one annotation per error line" rule would reject.
    let mut already: Vec<TypeId> = Vec::new();

    for (module, name, id, syntax) in declared {
        if already.contains(id) {
            continue;
        }
        // Walk what this type contains *by value*, following `Named` types and the
        // insides of records and fixed arrays, but never through a `ref` or any other
        // reference: that is exactly what breaks a cycle [4.3].
        let mut path = Vec::new();
        if let Some(cycle) = reaches_by_value(types, *id, *id, &mut path) {
            already.extend(
                cycle
                    .iter()
                    .filter(|t| matches!(types.kind(**t), TypeKind::Named { .. })),
            );
            // Name the cycle [4.3] in terms of *declared* types. The path also runs
            // through the anonymous records and arrays between them, which have no
            // name to give and would only clutter the message.
            let mut named: Vec<String> = Vec::new();
            for step in &cycle {
                if let TypeKind::Named { module, name, .. } = types.kind(*step) {
                    let text = format!("{module}::{name}");
                    if !named.contains(&text) {
                        named.push(text);
                    }
                }
            }
            let what = if named.len() <= 1 {
                format!("`{module}::{name}` contains itself by value")
            } else {
                format!(
                    "these types contain each other by value: {}",
                    named
                        .iter()
                        .map(|n| format!("`{n}`"))
                        .collect::<Vec<_>>()
                        .join(" contains ")
                )
            };
            diags.push(
                Diagnostic::error(
                    syntax.span,
                    format!(
                        "{what}, which cannot be laid out; hold one of them through \
                         `ref` [4.3]"
                    ),
                )
                .note(syntax.span, format!("`{module}::{name}` is declared here")),
            );
        }
    }
}

/// Does `from` reach `target` by value? Returns the path if so.
fn reaches_by_value(
    types: &Types,
    from: TypeId,
    target: TypeId,
    path: &mut Vec<TypeId>,
) -> Option<Vec<TypeId>> {
    if path.len() > 256 {
        // A guard, not a rule: the graph is finite, but a bug here must not hang.
        return None;
    }
    path.push(from);
    let result = match types.kind(from) {
        // An alias chain: `type A is B;` holds a `B` by value, whatever `B` is.
        TypeKind::Named { underlying, .. } => step(types, *underlying, target, path),
        TypeKind::Record { fields, .. } => {
            fields.iter().find_map(|f| step(types, f.ty, target, path))
        }
        TypeKind::Array { element, .. } => step(types, *element, target, path),
        // Everything else either holds nothing by value or holds it through a
        // reference, which breaks the cycle.
        _ => None,
    };
    path.pop();
    result
}

fn step(
    types: &Types,
    next: TypeId,
    target: TypeId,
    path: &mut Vec<TypeId>,
) -> Option<Vec<TypeId>> {
    if next == target {
        return Some(path.clone());
    }
    // Only keep walking through types that can hold something by value, and never
    // revisit, so a shared record is not mistaken for a cycle.
    if path.contains(&next) {
        return None;
    }
    reaches_by_value(types, next, target, path)
}

#[cfg(test)]
mod tests {
    use super::*;
    use alm_core::diag::SourceMap;

    /// Build the type graph of a one-module program.
    fn graph(source: &str) -> (Types, Vec<String>) {
        let dir = std::env::temp_dir().join(format!(
            "almc0-types-{}-{:?}",
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
        let consts = crate::consts::evaluate(&loaded, &mut diags);
        let options = alm_core::options::Options::default();
        let types = build(&loaded, &consts, &options, &mut diags);
        let rendered = diags.items().iter().map(|d| d.render(&sources)).collect();
        let _ = std::fs::remove_dir_all(&dir);
        (types, rendered)
    }

    fn clean(source: &str) -> Types {
        let (types, errs) = graph(source);
        assert!(errs.is_empty(), "unexpected diagnostics: {errs:#?}");
        types
    }

    fn errors(source: &str) -> Vec<String> {
        let (_, errs) = graph(source);
        assert!(!errs.is_empty(), "expected a diagnostic for {source:?}");
        errs
    }

    #[test]
    fn type_identity_is_by_name() {
        // [3.5]: "two types are the same only if they are the same declared type (or
        // both spell out the same predefined type). Every `record` declaration is its
        // own type."
        let t = clean(
            "type A is record x: int; end;\n\
             type B is record x: int; end;\n\
             type C is int;\n\
             type D is int;\n",
        );
        let a = t.declared("Main", "A").unwrap();
        let b = t.declared("Main", "B").unwrap();
        let c = t.declared("Main", "C").unwrap();
        let d = t.declared("Main", "D").unwrap();
        assert_ne!(a, b, "two identical records are different types");
        assert_ne!(c, d, "two named types of `int` are different types");
        // But the predefined `int` they are declared as is one type.
        assert_eq!(t.underlying(c), t.underlying(d));
    }

    #[test]
    fn a_constructed_type_is_interned_so_two_spellings_are_one_type() {
        // "or both spell out the same predefined type" [3.5]: `ref array of char`
        // written twice is one type, which is what makes `String` usable at all.
        let t =
            clean("proc f(a: ref array of char, b: ref array of char, c: String)\nbegin\nend;\n");
        // Three spellings of the same thing must have produced one id, so the graph
        // holds no duplicate.
        let count = (0..t.len())
            .filter(|i| matches!(t.kind(TypeId(*i as u32)), TypeKind::RefArray(_)))
            .count();
        assert_eq!(count, 1, "ref array of char should be interned once");
    }

    #[test]
    fn a_named_number_type_carries_a_tag() {
        // [7.4]: "`type D is fixed(p, s);` and `type D is int;` make named types. A
        // value whose type is a named type has that name as its tag. Plain `int` and
        // `fixed` values, and literals, have no tag."
        let t = clean(
            "type Price is fixed(10, 2);\n\
             type Count is int;\n\
             type Name is String;\n",
        );
        let price = t.declared("Main", "Price").unwrap();
        let count = t.declared("Main", "Count").unwrap();
        let name = t.declared("Main", "Name").unwrap();
        assert_eq!(t.tag(price), Some(("Main", "Price")));
        assert_eq!(t.tag(count), Some(("Main", "Count")));
        // A named *reference* type is not a number type, so the tag rules of [7.4] do
        // not apply to it.
        assert_eq!(t.tag(name), None);
        // And the underlying types are untagged.
        assert_eq!(t.tag(t.underlying(price)), None);
    }

    #[test]
    fn zero_values_follow_section_three_three() {
        // "`int` 0, `bool` false, `char` code 0, `fixed` 0, `ref`, `mailbox`, `reply`,
        // `view`, `publisher`, `task`, `File` and `Listener` `nil`, and aggregates zero
        // in every part."
        let t = clean(
            "type R is record x: int; end;\n\
             type Zs is record\n\
               i: int; b: bool; c: char; f: fixed(5, 2);\n\
               r: ref R; m: mailbox of R; q: reply of R;\n\
               v: view of R; p: publisher of R; h: task; fl: File;\n\
               arr: array [3] of int;\n\
             end;\n",
        );
        let zs = t.declared("Main", "Zs").unwrap();
        let TypeKind::Record { fields, .. } = t.kind(t.underlying(zs)) else {
            panic!("a record")
        };
        let zero_of = |name: &str| {
            let f = fields.iter().find(|f| f.name == name).expect(name);
            t.zero(f.ty)
        };
        assert_eq!(zero_of("i"), Zero::Int);
        assert_eq!(zero_of("b"), Zero::Bool);
        assert_eq!(zero_of("c"), Zero::Char);
        assert_eq!(zero_of("f"), Zero::Fixed);
        for name in ["r", "m", "q", "v", "p", "h", "fl"] {
            assert_eq!(zero_of(name), Zero::Nil, "{name} starts nil");
        }
        assert_eq!(zero_of("arr"), Zero::Aggregate);
        assert_eq!(t.zero(zs), Zero::Aggregate);
    }

    #[test]
    fn equality_is_defined_only_on_reference_free_values() {
        // [7.1]: "`=` ... is defined on `int`, `bool`, `char`, `fixed`, and on records
        // and arrays that contain no references, mailboxes, replies, views,
        // publishers, tasks or files."
        let t = clean(
            "type Plain is record a: int; b: char; end;\n\
             type Nested is record p: Plain; arr: array [2] of bool; end;\n\
             type HasRef is record a: int; r: ref Plain; end;\n\
             type HasTask is record h: task; end;\n\
             type HasFile is record f: File; end;\n\
             type Arr is array [3] of Plain;\n\
             type ArrOfRef is array [3] of ref Plain;\n",
        );
        for name in ["Plain", "Nested", "Arr"] {
            let id = t.declared("Main", name).unwrap();
            assert!(t.has_equality(id), "`=` should be defined on {name}");
        }
        for name in ["HasRef", "HasTask", "HasFile", "ArrOfRef"] {
            let id = t.declared("Main", name).unwrap();
            assert!(!t.has_equality(id), "`=` should not be defined on {name}");
            // And the blocker is reported, so a diagnostic can say which field is the
            // problem rather than just refusing.
            assert!(t.equality_blocker(id).is_some());
        }
    }

    #[test]
    fn identity_is_defined_only_on_references_mailboxes_and_views() {
        // [7.1]: "`~` is defined only on references, mailboxes and views (and `nil`)."
        let t = clean(
            "type R is record x: int; end;\n\
             type Rf is ref R;\n\
             type Mb is mailbox of R;\n\
             type Vw is view of R;\n\
             type Rp is reply of R;\n\
             type Pb is publisher of R;\n\
             type Tk is task;\n\
             type I is int;\n",
        );
        for name in ["Rf", "Mb", "Vw"] {
            assert!(
                t.has_identity(t.declared("Main", name).unwrap()),
                "`~` should be defined on {name}"
            );
        }
        for name in ["Rp", "Pb", "Tk", "I", "R"] {
            assert!(
                !t.has_identity(t.declared("Main", name).unwrap()),
                "`~` should not be defined on {name}"
            );
        }
    }

    #[test]
    fn the_fixed_precision_and_scale_rules() {
        // [7.4]: "`p` may be any positive constant with no upper limit, and `s` must
        // satisfy 0 <= s <= p."
        let t = clean("const D is 7;\ntype Money is fixed(D, 2);\n");
        let money = t.declared("Main", "Money").unwrap();
        assert_eq!(
            t.kind(t.underlying(money)),
            &TypeKind::Fixed {
                precision: 7,
                scale: 2
            },
            "the precision came from a constant"
        );
        assert!(errors("type T is fixed(0, 0);\n")[0].contains("at least one digit"));
        assert!(errors("type T is fixed(4, 5);\n")[0].contains("0 <= s <= p"));
        // A precision past what a `u32` scale can represent is refused rather than
        // wrapping into a narrow type.
        let too_wide = errors("type T is fixed(5000000000, 5000000000);\n");
        assert!(
            too_wide[0].contains("accepts a `fixed` precision up to"),
            "{too_wide:?}"
        );
        // The limit is the compiler's, not the language's, so the message says how to
        // raise it (QUESTIONS.md 6).
        assert!(too_wide[0].contains("raise `--max-digits`"), "{too_wide:?}");
        assert!(errors("type T is fixed(4, _1);\n")[0].contains("0 <= s <= p"));
        // A non-constant, and a constant of the wrong kind.
        assert!(errors("type T is fixed(true, 0);\n")[0].contains("must be an `int`"));
    }

    #[test]
    fn an_array_length_comes_from_a_constant() {
        let t = clean("const N is 4;\ntype Buf is array [N * 2] of int;\n");
        let buf = t.declared("Main", "Buf").unwrap();
        let TypeKind::Array { length, .. } = t.kind(t.underlying(buf)) else {
            panic!("an array")
        };
        assert_eq!(*length, 8);
        assert!(errors("type T is array [_1] of int;\n")[0].contains("cannot have -1 elements"));
    }

    #[test]
    fn a_record_cannot_declare_one_field_twice() {
        let d = errors("type R is record x: int; x: bool; end;\n");
        assert!(d[0].contains("already has a field `x`"), "{d:?}");
        assert!(d[0].contains("note:"), "point at the first one: {d:?}");
    }

    #[test]
    fn a_containment_cycle_by_value_is_an_error_naming_the_cycle() {
        // [4.3]: "a record or fixed-length array may hold another member only through
        // `ref`, unless the other member does not hold it back ... compile errors that
        // name the cycle."
        let d = errors(
            "type Tree is record kids: Forest; end;\n\
             also type Forest is record trees: Tree; end;\n",
        );
        assert!(
            d.iter().any(|x| x.contains("contain each other by value")),
            "{d:#?}"
        );
        assert!(
            d.iter().any(|x| x.contains("through `ref`")),
            "say how to fix it: {d:#?}"
        );
    }

    #[test]
    fn a_type_that_holds_itself_by_value_is_an_error() {
        let d = errors("type Loop is record me: Loop; end;\n");
        assert!(d[0].contains("contains itself by value"), "{d:?}");
    }

    #[test]
    fn an_alias_loop_is_an_error() {
        // [4.3]: "An alias loop, such as `type A is B; also type B is A;`, is also an
        // error."
        let d = errors("type A is B;\nalso type B is A;\n");
        assert!(!d.is_empty(), "{d:#?}");
        assert!(
            d.iter().any(|x| x.contains("by value")),
            "an alias loop is a containment cycle: {d:#?}"
        );
    }

    #[test]
    fn a_cycle_through_ref_is_allowed() {
        // The ordinary recursive type, which is the whole point of the `ref` exception.
        assert!(
            clean("type Node is record key: int; left: ref Node; right: ref Node; end;\n")
                .declared("Main", "Node")
                .is_some()
        );
        // And the mutual case from [4.3]'s own example.
        clean(
            "type Tree is record label: int; kids: ref Forest; end;\n\
             also type Forest is record trees: ref array of ref Tree; end;\n",
        );
    }

    #[test]
    fn a_shared_record_is_not_mistaken_for_a_cycle() {
        // Two fields of the same record type is a diamond, not a cycle. A naive walk
        // that does not track the path reports this wrongly.
        clean(
            "type Leaf is record x: int; end;\n\
             type Pair is record a: Leaf; b: Leaf; end;\n\
             type Triple is record p: Pair; q: Pair; r: Leaf; end;\n",
        );
    }

    #[test]
    fn the_library_types_resolve() {
        // [9]: `String` is `ref array of char`, `File` is opaque, `Version` and `Round`
        // are `fixed(36, 0)`.
        let t = clean("type S is String;\ntype F is File;\ntype V is Version;\ntype R is Round;\n");
        let s = t.underlying(t.declared("Main", "S").unwrap());
        assert!(matches!(t.kind(s), TypeKind::RefArray(_)));
        let f = t.underlying(t.declared("Main", "F").unwrap());
        assert!(matches!(t.kind(f), TypeKind::Opaque { .. }));
        for name in ["V", "R"] {
            let id = t.underlying(t.declared("Main", name).unwrap());
            assert_eq!(
                t.kind(id),
                &TypeKind::Fixed {
                    precision: 36,
                    scale: 0
                }
            );
        }
    }

    #[test]
    fn a_generic_type_parameter_is_opaque() {
        // [4.4]: the parameter is a type of the module, and nothing may be assumed
        // about it — in particular `=` is rejected on it.
        let (t, errs) = graph("generic type T;\nproc f(a: T)\nbegin\nend;\n");
        assert!(errs.is_empty(), "{errs:#?}");
        let param = t.declared("Main", "T").unwrap();
        assert!(matches!(t.kind(param), TypeKind::Parameter { .. }));
        assert!(
            !t.has_equality(param),
            "`=` is rejected on an opaque parameter [4.4]"
        );
    }
}

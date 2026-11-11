//! Name resolution [4.1, 4.3].
//!
//! One pass over the loaded program. It builds a table of the top-level names of each
//! module, then walks every declaration and body resolving each name use, and reports:
//!
//! - a name that is not declared anywhere in scope;
//! - a use of a top-level name before its declaration, which [4.1] says is an error
//!   "not a use of the library name";
//! - two declarations of one name in a module, since a module has one flat namespace;
//! - a declaration that collides with an imported module's name, which occupies the
//!   namespace too;
//! - a declaration, parameter or local that would hide one of the unhidable library
//!   names [4.1];
//! - a qualified name whose module is not imported, since imports are not transitive;
//! - a qualified name that the module does not declare, or declares `private`;
//! - an `also` that cannot join a group [4.3].
//!
//! What it does not do: types. Whether `a.b` is a field of a record, whether a private
//! *field* is reached from outside its module, and whether an argument fits a parameter
//! all need the type graph, which is the next step.

use std::collections::BTreeMap;

use alm_core::diag::{Diagnostic, Diagnostics};
use alm_core::span::Span;

use crate::ast::*;
use crate::library;
use crate::load::Loaded;

/// What a name denotes.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SymbolKind {
    Const,
    /// One name of an `enum` declaration: a plain `int` constant [4.1].
    EnumName,
    Type,
    Exception,
    Share,
    Proc,
    /// An imported module's name, which occupies the namespace [4.1].
    Module,
    /// A parameter of a generic module [4.4].
    GenericType,
    GenericConst,
    GenericProc,
}

impl SymbolKind {
    pub const fn describe(self) -> &'static str {
        match self {
            SymbolKind::Const => "a constant",
            SymbolKind::EnumName => "an enumeration name",
            SymbolKind::Type => "a type",
            SymbolKind::Exception => "an exception",
            SymbolKind::Share => "a shared value",
            SymbolKind::Proc => "a procedure",
            SymbolKind::Module => "an imported module",
            SymbolKind::GenericType => "a generic type parameter",
            SymbolKind::GenericConst => "a generic constant parameter",
            SymbolKind::GenericProc => "a generic procedure parameter",
        }
    }

    /// Can this name be used as a type?
    pub const fn is_type(self) -> bool {
        matches!(self, SymbolKind::Type | SymbolKind::GenericType)
    }
}

/// A top-level name of a module.
#[derive(Clone, Debug)]
pub struct Symbol {
    pub name: String,
    pub kind: SymbolKind,
    pub span: Span,
    /// Hidden from other modules [4.1].
    pub private: bool,
    /// Position of the declaration in the module, for declaration-before-use.
    pub index: usize,
    /// Index of the first declaration of this symbol's `also` group [4.3]. Equal to
    /// `index` for a declaration that starts no group.
    pub group: usize,
}

/// The top-level names of one module, in declaration order.
#[derive(Clone, Debug, Default)]
pub struct ModuleSymbols {
    pub by_name: BTreeMap<String, Symbol>,
    /// Modules this one imports, by the name they are bound to.
    pub imports: BTreeMap<String, Span>,
}

/// Every module's symbols, by module name.
#[derive(Clone, Debug, Default)]
pub struct Symbols {
    pub modules: BTreeMap<String, ModuleSymbols>,
}

/// Resolve the whole program.
pub fn resolve(loaded: &Loaded, diags: &mut Diagnostics) -> Symbols {
    let mut symbols = Symbols::default();
    // Collect every module's names first: a module may refer to a name of an imported
    // module declared later in that module's file, since across modules there is no
    // declaration order.
    for m in &loaded.modules {
        let Some(program) = &m.program else {
            // An implementation module. Its names are not known from source; `std` is
            // handled by the library table and `Net`'s are checked in a later phase.
            symbols
                .modules
                .insert(m.name.clone(), ModuleSymbols::default());
            continue;
        };
        symbols
            .modules
            .insert(m.name.clone(), collect(&m.name, program, diags));
    }
    for m in &loaded.modules {
        if let Some(program) = &m.program {
            let mut r = Resolver {
                module: &m.name,
                symbols: &symbols,
                diags,
                scopes: Vec::new(),
            };
            r.program(program);
        }
    }
    symbols
}

/// Build one module's table, reporting what can be seen from the declarations alone.
fn collect(module: &str, program: &Program, diags: &mut Diagnostics) -> ModuleSymbols {
    let mut out = ModuleSymbols::default();

    // An imported module's name is entered in the namespace [4.1].
    for line in &program.imports {
        for item in &line.items {
            declare(
                &mut out,
                module,
                &item.name,
                SymbolKind::Module,
                false,
                0,
                0,
                diags,
            );
            out.imports.insert(item.name.text.clone(), item.name.span);
        }
    }
    // The generic parameters of a generic module [4.4].
    for (i, g) in program.generics.iter().enumerate() {
        let (name, kind) = match g {
            GenericParam::Type { name, .. } => (name, SymbolKind::GenericType),
            GenericParam::Const { name, .. } => (name, SymbolKind::GenericConst),
            GenericParam::Proc { name, .. } => (name, SymbolKind::GenericProc),
        };
        declare(&mut out, module, name, kind, false, i, i, diags);
    }

    // Group membership [4.3]: a declaration with `also` joins the group of the one
    // before it. The group's index is the index of its first member.
    let mut group_start = 0usize;
    let mut previous_kind: Option<SymbolKind> = None;
    for (i, decl) in program.decls.iter().enumerate() {
        let kind = match &decl.kind {
            DeclKind::Const { .. } => SymbolKind::Const,
            DeclKind::Enum { .. } => SymbolKind::EnumName,
            DeclKind::Type { .. } => SymbolKind::Type,
            DeclKind::Exception { .. } => SymbolKind::Exception,
            DeclKind::Share { .. } => SymbolKind::Share,
            DeclKind::Proc { .. } => SymbolKind::Proc,
            DeclKind::Instance(line) => {
                for item in &line.items {
                    declare(
                        &mut out,
                        module,
                        &item.name,
                        SymbolKind::Module,
                        false,
                        i,
                        i,
                        diags,
                    );
                    out.imports.insert(item.name.text.clone(), item.name.span);
                }
                previous_kind = None;
                continue;
            }
        };

        if decl.also {
            // [4.3]: `also` cannot come first in a file, after any other kind of
            // declaration, or after a declaration of the other kind.
            match previous_kind {
                None => {
                    diags.error(
                        decl.span,
                        "`also` must follow another declaration, so it cannot come first [4.3]",
                    );
                }
                // The order of these two matters: "after any other kind of
                // declaration" is the more precise diagnosis, so it is tried first.
                // Reported the other way round, `const C is 1; also type A is int;`
                // would complain that a type cannot join a group of "a constant",
                // which invites the reader to think constants form groups.
                Some(previous) if !groupable(previous) => {
                    diags.error(
                        decl.span,
                        format!(
                            "only types and procedures form groups, so `also` cannot \
                             follow {} [4.3]",
                            previous.describe()
                        ),
                    );
                }
                Some(previous) if previous != kind => {
                    diags.push(
                        Diagnostic::error(
                            decl.span,
                            format!(
                                "a group is all types or all procedures, so {} cannot \
                                 join a group of {} [4.3]",
                                kind.describe(),
                                plural(previous)
                            ),
                        )
                        .note(
                            program.decls[i - 1].span,
                            format!("the group before it declares {}", previous.describe()),
                        ),
                    );
                }
                Some(_) => {}
            }
        } else {
            group_start = i;
        }

        match &decl.kind {
            DeclKind::Enum { names } => {
                // An enumeration declares all its names; `private` hides all of them.
                for name in names {
                    declare(
                        &mut out,
                        module,
                        name,
                        kind,
                        decl.private,
                        i,
                        group_start,
                        diags,
                    );
                }
            }
            DeclKind::Const { name, .. }
            | DeclKind::Type { name, .. }
            | DeclKind::Exception { name }
            | DeclKind::Share { name, .. }
            | DeclKind::Proc { name, .. } => {
                declare(
                    &mut out,
                    module,
                    name,
                    kind,
                    decl.private,
                    i,
                    group_start,
                    diags,
                );
            }
            DeclKind::Instance(_) => unreachable!("handled above"),
        }
        previous_kind = Some(kind);
    }
    out
}

const fn groupable(kind: SymbolKind) -> bool {
    matches!(kind, SymbolKind::Type | SymbolKind::Proc)
}

const fn plural(kind: SymbolKind) -> &'static str {
    match kind {
        SymbolKind::Type => "types",
        SymbolKind::Proc => "procedures",
        other => other.describe(),
    }
}

/// Enter a name in a module's namespace, reporting a collision or an unhidable name.
#[allow(clippy::too_many_arguments)]
fn declare(
    out: &mut ModuleSymbols,
    module: &str,
    name: &Ident,
    kind: SymbolKind,
    private: bool,
    index: usize,
    group: usize,
    diags: &mut Diagnostics,
) {
    // A name the parser could not read; it has already been reported.
    if name.text.is_empty() {
        return;
    }
    // [2]: `std` names the standard library and cannot be declared.
    if name.text == "std" {
        diags.error(
            name.span,
            "`std` names the standard library and cannot be declared [2]",
        );
        return;
    }
    // [4.1]: the unhidable names "cannot be hidden".
    if library::is_unhidable(&name.text) {
        diags.error(
            name.span,
            format!(
                "`{}` is one of the library names the language relies on and cannot be \
                 hidden [4.1]",
                name.text
            ),
        );
        return;
    }
    if let Some(previous) = out.by_name.get(&name.text) {
        // One flat namespace per module [4.1], so this is a collision whatever the two
        // kinds are — including a declaration against an imported module's name.
        let message = if previous.kind == SymbolKind::Module {
            format!(
                "`{}` is already the name of an imported module in `{module}`, so it \
                 cannot also name {} [4.1]",
                name.text,
                kind.describe()
            )
        } else {
            format!(
                "`{}` is already declared in `{module}`: a module has one namespace [4.1]",
                name.text
            )
        };
        diags.push(Diagnostic::error(name.span, message).note(
            previous.span,
            format!("the first declaration of `{}` is here", name.text),
        ));
        return;
    }
    out.by_name.insert(
        name.text.clone(),
        Symbol {
            name: name.text.clone(),
            kind,
            span: name.span,
            private,
            index,
            group,
        },
    );
}

/// One procedure's parameters and locals.
struct Scope {
    names: BTreeMap<String, Span>,
}

struct Resolver<'a, 'd> {
    module: &'a str,
    symbols: &'a Symbols,
    diags: &'d mut Diagnostics,
    scopes: Vec<Scope>,
}

impl Resolver<'_, '_> {
    fn here(&self) -> &ModuleSymbols {
        self.symbols
            .modules
            .get(self.module)
            .expect("the module was collected")
    }

    fn program(&mut self, program: &Program) {
        for g in &program.generics {
            match g {
                GenericParam::Type { .. } => {}
                GenericParam::Const { ty, .. } => self.ty(ty, 0),
                GenericParam::Proc { params, result, .. } => {
                    for p in params {
                        self.param_type(&p.ty, 0);
                    }
                    if let Some(r) = result {
                        self.ty(r, 0);
                    }
                }
            }
        }
        for (i, decl) in program.decls.iter().enumerate() {
            self.decl(decl, i);
        }
    }

    fn decl(&mut self, decl: &Decl, index: usize) {
        match &decl.kind {
            DeclKind::Const { value, .. } => self.expr(value, index),
            DeclKind::Enum { .. } | DeclKind::Exception { .. } | DeclKind::Instance(_) => {}
            DeclKind::Type { ty, .. } => self.ty(ty, index),
            DeclKind::Share { ty, value, .. } => {
                self.ty(ty, index);
                self.expr(value, index);
            }
            DeclKind::Proc {
                params,
                result,
                body,
                ..
            } => {
                let mut scope = Scope {
                    names: BTreeMap::new(),
                };
                for p in params {
                    self.param_type(&p.ty, index);
                    self.bind(&mut scope, &p.name, "parameter");
                }
                if let Some(r) = result {
                    self.ty(r, index);
                }
                for v in &body.vars {
                    self.ty(&v.ty, index);
                    self.bind(&mut scope, &v.name, "local variable");
                }
                self.scopes.push(scope);
                for s in &body.body {
                    self.stmt(s, index);
                }
                self.scopes.pop();
            }
        }
    }

    /// Bind a parameter or local, reporting a repeat or an unhidable name [4.1].
    fn bind(&mut self, scope: &mut Scope, name: &Ident, what: &str) {
        if name.text.is_empty() {
            return;
        }
        if library::is_unhidable(&name.text) {
            self.diags.error(
                name.span,
                format!(
                    "`{}` is one of the library names the language relies on and a {what} \
                     cannot hide it [4.1]",
                    name.text
                ),
            );
            return;
        }
        if let Some(previous) = scope.names.get(&name.text) {
            self.diags.push(
                Diagnostic::error(
                    name.span,
                    format!("`{}` is already declared in this procedure", name.text),
                )
                .note(
                    *previous,
                    format!("the first declaration of `{}` is here", name.text),
                ),
            );
            return;
        }
        scope.names.insert(name.text.clone(), name.span);
    }

    fn in_scope(&self, name: &str) -> bool {
        self.scopes.iter().any(|s| s.names.contains_key(name))
    }

    fn param_type(&mut self, ty: &ParamType, index: usize) {
        match ty {
            ParamType::Value(t) => self.ty(t, index),
            ParamType::Proc { params, result, .. } => {
                for t in params {
                    self.ty(t, index);
                }
                if let Some(r) = result {
                    self.ty(r, index);
                }
            }
        }
    }

    fn ty(&mut self, ty: &Type, index: usize) {
        match &ty.kind {
            TypeKind::Named(q) => self.qident(q, index, Expect::Type),
            TypeKind::Int | TypeKind::Bool | TypeKind::Char => {}
            TypeKind::Fixed { precision, scale } => {
                self.expr(precision, index);
                self.expr(scale, index);
            }
            TypeKind::Array { length, element } => {
                self.expr(length, index);
                self.ty(element, index);
            }
            TypeKind::Record { fields } => {
                for f in fields {
                    self.ty(&f.ty, index);
                }
            }
            TypeKind::Ref(q)
            | TypeKind::Inbox(q)
            | TypeKind::Mailbox(q)
            | TypeKind::Reply(q)
            | TypeKind::View(q)
            | TypeKind::Publisher(q) => self.qident(q, index, Expect::Type),
            TypeKind::RefArray(t) => self.ty(t, index),
            TypeKind::Task(Some(q)) => self.qident(q, index, Expect::Type),
            TypeKind::Task(None) => {}
        }
    }

    fn stmt(&mut self, stmt: &Stmt, index: usize) {
        match &stmt.kind {
            StmtKind::Assign { target, value, .. } => {
                self.designator(target, index);
                self.expr(value, index);
            }
            StmtKind::Call { call, .. } | StmtKind::Defer { call, .. } => {
                self.call(call, index);
            }
            StmtKind::If { arms, otherwise } => {
                for (cond, body) in arms {
                    self.expr(cond, index);
                    for s in body {
                        self.stmt(s, index);
                    }
                }
                if let Some(body) = otherwise {
                    for s in body {
                        self.stmt(s, index);
                    }
                }
            }
            StmtKind::While { condition, body } => {
                self.expr(condition, index);
                for s in body {
                    self.stmt(s, index);
                }
            }
            StmtKind::Return(Some(e)) => self.expr(e, index),
            StmtKind::Return(None) | StmtKind::Exit | StmtKind::Next => {}
            StmtKind::Raise(Some(q)) => self.qident(q, index, Expect::Exception),
            StmtKind::Raise(None) => {}
            StmtKind::Try { body, handlers } => {
                for s in body {
                    self.stmt(s, index);
                }
                for h in handlers {
                    for q in &h.exceptions {
                        self.qident(q, index, Expect::Exception);
                    }
                    for s in &h.body {
                        self.stmt(s, index);
                    }
                }
            }
        }
    }

    fn call(&mut self, call: &Call, index: usize) {
        self.qident(&call.callee, index, Expect::Value);
        for a in &call.args {
            self.expr(a, index);
        }
    }

    fn designator(&mut self, d: &Designator, index: usize) {
        self.qident(&d.base, index, Expect::Value);
        for s in &d.selectors {
            if let Selector::Index(e) = s {
                self.expr(e, index);
            }
            // A field name is resolved against the record's type, which needs the type
            // graph; that check belongs to the next step.
        }
    }

    fn expr(&mut self, e: &Expr, index: usize) {
        match &e.kind {
            ExprKind::Int { .. }
            | ExprKind::Fixed { .. }
            | ExprKind::Char(_)
            | ExprKind::Str(_)
            | ExprKind::Bool(_)
            | ExprKind::Nil => {}
            ExprKind::Designator(d) => self.designator(d, index),
            ExprKind::Call { call, selectors } => {
                self.call(call, index);
                for s in selectors {
                    if let Selector::Index(e) = s {
                        self.expr(e, index);
                    }
                }
            }
            ExprKind::New(t) => self.ty(t, index),
            ExprKind::NewArray { element, length } => {
                self.ty(element, index);
                self.expr(length, index);
            }
            ExprKind::Not(inner) | ExprKind::Negate(inner) | ExprKind::Paren(inner) => {
                self.expr(inner, index);
            }
            ExprKind::Binary { left, right, .. } => {
                self.expr(left, index);
                self.expr(right, index);
            }
        }
    }

    /// Resolve one name, qualified or not.
    fn qident(&mut self, q: &QIdent, index: usize, expect: Expect) {
        match &q.module {
            Some(module) => self.qualified(module, &q.name, expect),
            None => self.unqualified(&q.name, index, expect),
        }
    }

    /// `A::name`, or `std::name` for the library [4.1].
    fn qualified(&mut self, module: &Ident, name: &Ident, expect: Expect) {
        if name.text.is_empty() {
            return;
        }
        // `std::name` always reaches the library one [4.1].
        if module.text == "std" {
            match library::lookup(&name.text) {
                Some(l) => self.check_library_kind(l, name, expect),
                None => self.diags.error(
                    name.span,
                    format!("the standard library has no name `{}` [9]", name.text),
                ),
            }
            return;
        }
        // Imports are not transitive: a module must import what it names [4.1].
        let imported = self.here().imports.contains_key(&module.text);
        if !imported {
            let known = self.symbols.modules.contains_key(&module.text);
            let message = if known {
                format!(
                    "`{}` is not imported by `{}`; imports are not transitive, so this \
                     module must import it too [4.1]",
                    module.text, self.module
                )
            } else {
                format!("`{}` is not an imported module [4.1]", module.text)
            };
            self.diags.error(module.span, message);
            return;
        }
        let Some(other) = self.symbols.modules.get(&module.text) else {
            return;
        };
        // An implementation module's names are not known from source yet.
        if other.by_name.is_empty() && other.imports.is_empty() {
            return;
        }
        match other.by_name.get(&name.text).cloned() {
            None => self.diags.error(
                name.span,
                format!("`{}` does not declare `{}`", module.text, name.text),
            ),
            Some(symbol) if symbol.private => {
                self.diags.push(
                    Diagnostic::error(
                        name.span,
                        format!(
                            "`{}::{}` is private to `{}` [4.1]",
                            module.text, name.text, module.text
                        ),
                    )
                    .note(symbol.span, "declared `private` here"),
                );
            }
            Some(symbol) => self.check_kind(&symbol, name, expect),
        }
    }

    /// An unqualified name: a parameter or local, then the module, then the library.
    fn unqualified(&mut self, name: &Ident, index: usize, expect: Expect) {
        if name.text.is_empty() {
            return;
        }
        if self.in_scope(&name.text) {
            return;
        }
        // Cloned because reporting borrows `self` mutably; a symbol is small.
        if let Some(symbol) = self.here().by_name.get(&name.text).cloned() {
            // [4.1]: "A use of the name before its declaration is an error, not a use
            // of the library name." Within an `also` group every member is in scope
            // throughout [4.3].
            let visible = symbol.index < index || symbol.group == group_of(self.here(), index);
            if !visible {
                let hides_library = library::lookup(&name.text).is_some();
                let mut d = Diagnostic::error(
                    name.span,
                    format!("`{}` is used before it is declared [4.1]", name.text),
                )
                .note(symbol.span, format!("`{}` is declared here", name.text));
                if hides_library {
                    d = d.note(
                        symbol.span,
                        format!(
                            "this declaration hides the library's `{}` throughout the \
                             module, so the use above is not the library one [4.1]",
                            name.text
                        ),
                    );
                }
                self.diags.push(d);
                return;
            }
            self.check_kind(&symbol, name, expect);
            return;
        }
        match library::lookup(&name.text) {
            Some(l) => self.check_library_kind(l, name, expect),
            None => self
                .diags
                .error(name.span, format!("`{}` is not declared", name.text)),
        }
    }

    fn check_kind(&mut self, symbol: &Symbol, name: &Ident, expect: Expect) {
        let ok = match expect {
            Expect::Type => symbol.kind.is_type(),
            Expect::Exception => symbol.kind == SymbolKind::Exception,
            Expect::Value => !matches!(symbol.kind, SymbolKind::Module),
        };
        if !ok {
            let symbol_span = symbol.span;
            let described = symbol.kind.describe();
            self.diags.push(
                Diagnostic::error(
                    name.span,
                    format!("`{}` is {described}, not {}", name.text, expect.describe()),
                )
                .note(symbol_span, format!("`{}` is declared here", name.text)),
            );
        }
    }

    fn check_library_kind(
        &mut self,
        l: &'static library::LibraryName,
        name: &Ident,
        expect: Expect,
    ) {
        let ok = match expect {
            Expect::Type => l.kind == library::LibraryKind::Type,
            // The library declares no exceptions: the predefined ones of [7.5] are not
            // library names, and a `catch` naming one is checked elsewhere.
            Expect::Exception => false,
            Expect::Value => l.kind != library::LibraryKind::Type,
        };
        if !ok && expect != Expect::Exception {
            self.diags.error(
                name.span,
                format!(
                    "the library's `{}` is {}, not {}",
                    name.text,
                    match l.kind {
                        library::LibraryKind::Type => "a type",
                        library::LibraryKind::Constant => "a constant",
                        library::LibraryKind::Procedure => "a procedure",
                    },
                    expect.describe()
                ),
            );
        }
    }
}

/// The group index of the declaration at `index`.
fn group_of(module: &ModuleSymbols, index: usize) -> usize {
    module
        .by_name
        .values()
        .find(|s| s.index == index)
        .map(|s| s.group)
        .unwrap_or(index)
}

/// What a name is expected to denote where it appears.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Expect {
    Type,
    Exception,
    Value,
}

impl Expect {
    const fn describe(self) -> &'static str {
        match self {
            Expect::Type => "a type",
            Expect::Exception => "an exception",
            Expect::Value => "a value or a procedure",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alm_core::diag::SourceMap;

    /// Resolve a one-module program, returning the rendered diagnostics.
    fn check(source: &str) -> Vec<String> {
        check_modules(&[("Main.alm", source)])
    }

    /// Resolve a program of several modules. The first is the main module.
    fn check_modules(files: &[(&str, &str)]) -> Vec<String> {
        let dir = std::env::temp_dir().join(format!(
            "almc0-resolve-{}-{:?}",
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
            "the test source must load and parse cleanly: {:?}",
            diags
                .items()
                .iter()
                .map(|d| d.render(&sources))
                .collect::<Vec<_>>()
        );
        resolve(&loaded, &mut diags);
        let out = diags.items().iter().map(|d| d.render(&sources)).collect();
        let _ = std::fs::remove_dir_all(&dir);
        out
    }

    fn only(source: &str, wanted: &str) -> String {
        let d = check(source);
        assert_eq!(d.len(), 1, "expected one diagnostic, got {d:#?}");
        assert!(d[0].contains(wanted), "expected {wanted:?} in {:?}", d[0]);
        d[0].clone()
    }

    #[test]
    fn a_clean_module_resolves() {
        assert!(
            check(
                "const Size is 10;\n\
                 type Node is record key: int; link: ref Node; end;\n\
                 exception Empty;\n\
                 proc len2(a: ref array of int): int\n\
                 begin\n\
                   return len(a) * Size;\n\
                 end;\n\
                 proc start()\n\
                   var n: ref Node;\n\
                 begin\n\
                   n := new(Node);\n\
                   n.key := 1;\n\
                   writeLine(intStr(len2(new(array of int, Size))));\n\
                   raise Empty;\n\
                 end;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn an_undeclared_name_is_reported() {
        only(
            "proc start()\n  var x: int;\nbegin\n  x := missing(1);\nend;\n",
            "`missing` is not declared",
        );
    }

    #[test]
    fn the_library_is_in_scope_unqualified_and_qualified() {
        // [4.1]: "Library names can be written unqualified, as in `len(a)`, or
        // qualified, as in `std::len(a)`."
        assert!(
            check(
                "proc start()\n  var a: ref array of int;\nbegin\n  \
                 writeLine(intStr(len(a)));\n  writeLine(intStr(std::len(a)));\nend;\n"
            )
            .is_empty()
        );
        only(
            "proc start()\n  var x: int;\nbegin\n  x := std::nosuch(1);\nend;\n",
            "the standard library has no name `nosuch`",
        );
    }

    #[test]
    fn a_declaration_hides_a_library_name_but_std_still_reaches_it() {
        // [4.1]: a top-level declaration hides a library name, and `std::name` always
        // reaches the library one.
        assert!(
            check(
                "proc intStr(n: int): int\nbegin\n  return n;\nend;\n\
                 proc start()\nbegin\n  writeLine(std::intStr(intStr(1)));\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn using_a_hidden_library_name_before_its_declaration_is_an_error() {
        // [4.1]: "A use of the name before its declaration is an error, not a use of
        // the library name." This is the subtle one: without the rule, the call below
        // would silently mean the library's `intStr`.
        let d = only(
            "proc start()\nbegin\n  writeLine(intStr(1));\nend;\n\
             proc intStr(n: int): int\nbegin\n  return n;\nend;\n",
            "is used before it is declared",
        );
        assert!(
            d.contains("not the library one"),
            "the note must explain why: {d}"
        );
    }

    #[test]
    fn an_unhidable_library_name_cannot_be_declared_or_shadowed() {
        // [4.1] names them: `len` and the task, handle and view operations.
        only(
            "proc len(a: int): int\nbegin\n  return a;\nend;\n",
            "cannot be hidden",
        );
        only("const spawn is 1;\n", "cannot be hidden");
        only(
            "proc start(send: int)\nbegin\nend;\n",
            "a parameter cannot hide it",
        );
        only(
            "proc start()\n  var await: int;\nbegin\nend;\n",
            "a local variable cannot hide it",
        );
        // A hidable library name is fine in all those positions.
        assert!(check("proc start(intStr: int)\nbegin\nend;\n").is_empty());
    }

    #[test]
    fn std_cannot_be_declared() {
        // [2]: "`std` names the standard library and cannot be declared."
        only("const std is 1;\n", "cannot be declared");
    }

    #[test]
    fn a_module_has_one_namespace() {
        // [4.1]: "Within a module, all declared names ... share one namespace."
        let d = only(
            "const Thing is 1;\ntype Thing is int;\n",
            "`Thing` is already declared",
        );
        assert!(
            d.contains("note:"),
            "the first declaration must be pointed at: {d}"
        );
        // Across kinds, and for enumeration names too.
        only("enum Red, Green;\nconst Red is 1;\n", "already declared");
        only("exception E;\nproc E()\nbegin\nend;\n", "already declared");
    }

    #[test]
    fn an_imported_module_name_occupies_the_namespace() {
        // [4.1]: "An imported module's name is entered in the importing module's
        // namespace, so it cannot also be used for something declared there."
        let d = check_modules(&[
            (
                "Main.alm",
                "import Dep;\nconst Dep is 1;\nproc start()\nbegin\nend;\n",
            ),
            ("Dep.alm", "const X is 1;\n"),
        ]);
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(
            d[0].contains("already the name of an imported module"),
            "{d:?}"
        );
    }

    #[test]
    fn imports_are_not_transitive() {
        // [4.1]: "if `A` imports `B`, a module that imports `A` must also import `B` to
        // use `B`'s names."
        let d = check_modules(&[
            (
                "Main.alm",
                "import Middle;\nproc start()\n  var x: int;\nbegin\n  x := Leaf::value();\nend;\n",
            ),
            ("Middle.alm", "import Leaf;\nproc go()\nbegin\nend;\n"),
            ("Leaf.alm", "proc value(): int\nbegin\n  return 1;\nend;\n"),
        ]);
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(d[0].contains("imports are not transitive"), "{d:?}");
    }

    #[test]
    fn a_private_name_cannot_be_used_from_another_module() {
        let d = check_modules(&[
            (
                "Main.alm",
                "import Dep;\nproc start()\n  var x: int;\nbegin\n  x := Dep::hidden();\nend;\n",
            ),
            (
                "Dep.alm",
                "private proc hidden(): int\nbegin\n  return 1;\nend;\n",
            ),
        ]);
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(d[0].contains("is private to `Dep`"), "{d:?}");
        assert!(
            d[0].contains("declared `private` here"),
            "the note must point at it: {d:?}"
        );
    }

    #[test]
    fn a_visible_name_of_an_imported_module_resolves() {
        assert!(
            check_modules(&[
                (
                    "Main.alm",
                    "import Dep;\nproc start()\nbegin\n  writeLine(Dep::text());\nend;\n",
                ),
                (
                    "Dep.alm",
                    "proc text(): String\nbegin\n  return \"hi\";\nend;\n",
                ),
            ])
            .is_empty()
        );
    }

    #[test]
    fn a_module_that_does_not_declare_the_name() {
        let d = check_modules(&[
            (
                "Main.alm",
                "import Dep;\nproc start()\n  var x: int;\nbegin\n  x := Dep::nope();\nend;\n",
            ),
            ("Dep.alm", "proc yes()\nbegin\nend;\n"),
        ]);
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(d[0].contains("`Dep` does not declare `nope`"), "{d:?}");
    }

    #[test]
    fn a_group_lets_its_members_name_each_other() {
        // [4.3]: "Inside a group, every member name can be used in every member,
        // wherever the member is written, including in signatures and in the types of
        // fields."
        assert!(
            check(
                "type Tree is record label: int; kids: ref Forest; end;\n\
                 also type Forest is record trees: ref array of ref Tree; end;\n\
                 proc start()\nbegin\nend;\n"
            )
            .is_empty()
        );
        assert!(
            check(
                "proc even(n: int): bool\nbegin\n  return odd(n - 1);\nend;\n\
                 also proc odd(n: int): bool\nbegin\n  return even(n - 1);\nend;\n\
                 proc start()\nbegin\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn without_also_a_forward_reference_is_an_error() {
        // The same pair without `also`: declaration before use applies [4.3].
        let d = check(
            "proc even(n: int): bool\nbegin\n  return odd(n - 1);\nend;\n\
             proc odd(n: int): bool\nbegin\n  return even(n - 1);\nend;\n",
        );
        assert_eq!(d.len(), 1, "{d:#?}");
        assert!(
            d[0].contains("`odd` is used before it is declared"),
            "{d:?}"
        );
    }

    #[test]
    fn also_cannot_come_first_or_follow_the_wrong_kind() {
        // [4.3]: "`also` cannot come first in a file, after any other kind of
        // declaration, or after a declaration of the other kind."
        only("also type A is int;\n", "cannot come first");
        let d = only(
            "type A is int;\nalso proc f()\nbegin\nend;\n",
            "all types or all procedures",
        );
        assert!(
            d.contains("note:"),
            "point at the group it tried to join: {d}"
        );
        only(
            "const C is 1;\nalso type A is int;\n",
            "only types and procedures form groups",
        );
    }

    #[test]
    fn group_members_must_differ() {
        // [4.3]: "Member names must differ from each other." The flat namespace
        // already gives this, and the error says so.
        only("type A is int;\nalso type A is bool;\n", "already declared");
    }

    #[test]
    fn a_name_must_denote_the_right_kind_of_thing() {
        let d = only(
            "const C is 1;\nproc start()\n  var x: C;\nbegin\nend;\n",
            "not a type",
        );
        assert!(d.contains("`C` is a constant"), "{d:?}");
        only(
            "type T is int;\nproc start()\nbegin\n  raise T;\nend;\n",
            "not an exception",
        );
        // A library type used as a value, and a library procedure used as a type.
        only(
            "proc start()\n  var x: int;\nbegin\n  x := String;\nend;\n",
            "is a type, not a value",
        );
        only(
            "proc start()\n  var x: writeLine;\nbegin\nend;\n",
            "is a procedure, not a type",
        );
    }

    #[test]
    fn a_parameter_or_local_shadows_a_module_name_inside_its_procedure() {
        // The namespace is flat per module, but a procedure's own names come first.
        assert!(
            check(
                "const Size is 10;\n\
                 proc start(Size: int)\nbegin\n  writeLine(intStr(Size));\nend;\n"
            )
            .is_empty()
        );
    }

    #[test]
    fn a_name_cannot_be_declared_twice_in_one_procedure() {
        let d = only(
            "proc start(a: int)\n  var a: int;\nbegin\nend;\n",
            "already declared in this procedure",
        );
        assert!(d.contains("note:"), "{d}");
    }

    #[test]
    fn two_modules_may_declare_the_same_name() {
        // [4.1]: "Two modules may each declare `Node` or `Empty`."
        assert!(
            check_modules(&[
                (
                    "Main.alm",
                    "import Dep;\ntype Node is int;\nexception Empty;\n\
                     proc start()\nbegin\n  raise Empty;\nend;\n",
                ),
                ("Dep.alm", "type Node is bool;\nexception Empty;\n"),
            ])
            .is_empty()
        );
    }

    #[test]
    fn a_generic_modules_parameters_are_in_scope() {
        // [4.4]: `generic type T;` and the rest are names of the module.
        assert!(
            check_modules(&[
                (
                    "Main.alm",
                    "import Inst is Gen(int);\nproc start()\nbegin\nend;\n",
                ),
                (
                    "Gen.alm",
                    "generic type T;\ngeneric const Count: int;\n\
                     generic proc less(a: T, b: T): bool;\n\
                     proc first(items: ref array of T): T\nbegin\n  return items[0];\nend;\n\
                     proc howMany(): int\nbegin\n  return Count;\nend;\n",
                ),
            ])
            .is_empty()
        );
    }
}

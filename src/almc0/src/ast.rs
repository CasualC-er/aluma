//! The abstract syntax tree of [10].
//!
//! Every node carries a span, because the [7.5] report format needs the source text of
//! the expression being evaluated and the diagnostics need positions.
//!
//! The tree is deliberately close to the grammar: one variant per production, nothing
//! desugared. Phase 2 resolves names and Phase 3 lowers; both are easier to check
//! against the specification when the tree still looks like [10].
//!
//! ## The dump format
//!
//! `dump` writes a parenthesised form of the tree. It exists for a specific job: Phase
//! 10 ports the parser to Alúma and the two parsers are compared by **identical AST
//! dumps** (ADR-013.1), so the format is an interface, not a debugging aid. It is
//! therefore fully determined by the tree: no spans, no addresses, no map iteration
//! (ADR-008).

use std::fmt::Write as _;

use alm_core::Span;

/// `ident` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Ident {
    pub text: String,
    pub span: Span,
}

/// `qident = ident [ "::" ident ]` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct QIdent {
    /// `Some` for `Module::name`.
    pub module: Option<Ident>,
    pub name: Ident,
    pub span: Span,
}

/// A whole source file. A file is a module [4.1].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Program {
    /// `generic` lines, which come first [4.4].
    pub generics: Vec<GenericParam>,
    /// `import` lines, which come next.
    pub imports: Vec<Import>,
    /// Declarations in source order, including instance imports [4.4].
    pub decls: Vec<Decl>,
}

/// `genparam` [10]: a generic module's parameters.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GenericParam {
    Type {
        name: Ident,
        span: Span,
    },
    Const {
        name: Ident,
        ty: Type,
        span: Span,
    },
    Proc {
        name: Ident,
        params: Vec<Param>,
        result: Option<Type>,
        span: Span,
    },
}

/// `importdecl` / `instdecl` [10]. An instance import may also stand among the
/// declarations [4.4], which is why `Decl` can hold one too.
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Import {
    pub items: Vec<ImportItem>,
    pub span: Span,
}

/// `importitem = ident [ "is" ident "(" garg { "," garg } ")" ]` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct ImportItem {
    /// The name bound in this module.
    pub name: Ident,
    /// `Some` for an instance: `import Names is Lookup(String, int, stringEqual)`.
    pub instance: Option<Instance>,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Instance {
    pub generic: Ident,
    pub args: Vec<GenericArg>,
}

/// `garg = type | expr` [10].
///
/// The two are not distinguishable here: `Count` could be a type name or a constant.
/// Phase 2 decides, once names are resolved, which is why the parser keeps whichever it
/// managed to read and does not guess.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum GenericArg {
    Type(Type),
    Expr(Expr),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Decl {
    /// `also` groups a mutually recursive set [4.3].
    pub also: bool,
    /// `private` hides the declaration outside its module [4.1].
    pub private: bool,
    pub kind: DeclKind,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum DeclKind {
    /// `const ident is expr ;`
    Const { name: Ident, value: Expr },
    /// `enum ident { "," ident } ;` — a plain list of `int` constants [4.1], with no
    /// type of its own.
    Enum { names: Vec<Ident> },
    /// `type ident is type ;`
    Type { name: Ident, ty: Type },
    /// `exception ident ;`
    Exception { name: Ident },
    /// `share ident : type is expr ;`
    Share { name: Ident, ty: Type, value: Expr },
    /// `proc ident ( params ) [ : type ] block ;`
    Proc {
        name: Ident,
        params: Vec<Param>,
        result: Option<Type>,
        body: Block,
    },
    /// An instance import standing among the declarations [4.4].
    Instance(Import),
}

/// `param = ident ":" ( type | proctype )` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Param {
    pub name: Ident,
    pub ty: ParamType,
    pub span: Span,
}

/// A parameter's type. Procedure parameters are second-class [7.2], so a procedure type
/// can appear here and nowhere else.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ParamType {
    Value(Type),
    /// `proctype = "proc" "(" [ type { "," type } ] ")" [ ":" type ]`
    Proc {
        params: Vec<Type>,
        result: Option<Box<Type>>,
        span: Span,
    },
}

/// `block = { vardecl } "begin" stmts "end"` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Block {
    pub vars: Vec<VarDecl>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// `vardecl = "var" ident ":" type ";"` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct VarDecl {
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Type {
    pub kind: TypeKind,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TypeKind {
    /// A named type, possibly module-qualified.
    Named(QIdent),
    Int,
    Bool,
    Char,
    /// `fixed ( precision , scale )`, both constant expressions [7.4].
    Fixed {
        precision: Box<Expr>,
        scale: Box<Expr>,
    },
    /// `array [ n ] of T`
    Array {
        length: Box<Expr>,
        element: Box<Type>,
    },
    /// `record { [ private ] ident : type ; } end`
    Record {
        fields: Vec<Field>,
    },
    /// `ref R`
    Ref(QIdent),
    /// `ref array of T`
    RefArray(Box<Type>),
    /// `inbox of M`
    Inbox(QIdent),
    /// `mailbox of M`
    Mailbox(QIdent),
    /// `reply of M`
    Reply(QIdent),
    /// `view of V`
    View(QIdent),
    /// `publisher of V`
    Publisher(QIdent),
    /// `task [ of R ]`
    Task(Option<QIdent>),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Field {
    /// A record field may be `private` [4.1].
    pub private: bool,
    pub name: Ident,
    pub ty: Type,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Stmt {
    pub kind: StmtKind,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum StmtKind {
    /// `designator := [ rounded ] expr`
    Assign {
        target: Designator,
        rounded: bool,
        value: Expr,
    },
    /// `[ void ] qident ( args )`
    Call {
        void: bool,
        call: Call,
    },
    If {
        /// The `if` condition and each `elsif`, in order.
        arms: Vec<(Expr, Vec<Stmt>)>,
        /// `else` statements, if any.
        otherwise: Option<Vec<Stmt>>,
    },
    While {
        condition: Expr,
        body: Vec<Stmt>,
    },
    Return(Option<Expr>),
    Exit,
    Next,
    /// `defer [ void ] qident ( args )` [5.1].
    Defer {
        void: bool,
        call: Call,
    },
    /// `raise [ qident ]`. Bare `raise` re-raises [5.1, 7.5].
    Raise(Option<QIdent>),
    /// `try stmts handler { handler } end`
    Try {
        body: Vec<Stmt>,
        handlers: Vec<Handler>,
    },
}

/// `handler = "catch" [ qident { "," qident } ] "do" stmts` [10].
///
/// An empty `exceptions` list is the nameless handler, which must come last [5].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Handler {
    pub exceptions: Vec<QIdent>,
    pub body: Vec<Stmt>,
    pub span: Span,
}

/// `qident ( [ args ] )`
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Call {
    pub callee: QIdent,
    pub args: Vec<Expr>,
    pub span: Span,
}

/// `designator = qident { "[" expr "]" | "." ident }` [10].
#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Designator {
    pub base: QIdent,
    pub selectors: Vec<Selector>,
    pub span: Span,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum Selector {
    /// `[ expr ]`
    Index(Expr),
    /// `. ident`
    Field(Ident),
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Expr {
    pub kind: ExprKind,
    pub span: Span,
}

/// Binary operators, at the precedence levels of [6].
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum BinOp {
    Or,
    And,
    Equal,
    Less,
    LessEqual,
    Greater,
    GreaterEqual,
    Identical,
    Add,
    Subtract,
    Multiply,
    Divide,
    Div,
    Mod,
}

impl BinOp {
    pub const fn text(self) -> &'static str {
        match self {
            BinOp::Or => "or",
            BinOp::And => "and",
            BinOp::Equal => "=",
            BinOp::Less => "<",
            BinOp::LessEqual => "<=",
            BinOp::Greater => ">",
            BinOp::GreaterEqual => ">=",
            BinOp::Identical => "~",
            BinOp::Add => "+",
            BinOp::Subtract => "-",
            BinOp::Multiply => "*",
            BinOp::Divide => "/",
            BinOp::Div => "div",
            BinOp::Mod => "mod",
        }
    }

    /// A relation is **non-associative** [6]: `a < b < c` is a syntax error, not a
    /// chain, which is why `rel` in [10] takes at most one operator.
    pub const fn is_relation(self) -> bool {
        matches!(
            self,
            BinOp::Equal
                | BinOp::Less
                | BinOp::LessEqual
                | BinOp::Greater
                | BinOp::GreaterEqual
                | BinOp::Identical
        )
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum ExprKind {
    /// An integer literal. `min_int_magnitude` marks the one over-max value [2] allows
    /// under `_`; the parser rejects it anywhere else.
    Int {
        value: i64,
    },
    /// A `fixed` literal, as digits and scale: its precision is unbounded [7.4].
    Fixed {
        digits: String,
        scale: u32,
    },
    Char(char),
    /// A fresh, mutable `String` each time it is evaluated [3.4].
    Str(String),
    Bool(bool),
    Nil,
    /// A variable, field or element.
    Designator(Designator),
    /// A call, with the selectors [6] allows on its result.
    Call {
        call: Call,
        selectors: Vec<Selector>,
    },
    /// `new ( R )`
    New(Type),
    /// `new ( array of T , n )`
    NewArray {
        element: Type,
        length: Box<Expr>,
    },
    /// `not e`
    Not(Box<Expr>),
    /// `_ e`, the negation glyph [2].
    Negate(Box<Expr>),
    Binary {
        op: BinOp,
        left: Box<Expr>,
        right: Box<Expr>,
    },
    /// `( e )`. Kept because the single-rounding rule of [7.4] is syntactic — "a single
    /// division ... possibly in parentheses" — so Phase 2 must be able to see them.
    Paren(Box<Expr>),
}

// ---------------------------------------------------------------------------
// The dump format
// ---------------------------------------------------------------------------

/// Write a program as a parenthesised tree. Determined entirely by the tree (ADR-008).
pub fn dump(program: &Program) -> String {
    let mut out = String::new();
    let mut w = Dumper {
        out: &mut out,
        depth: 0,
    };
    w.program(program);
    out
}

struct Dumper<'a> {
    out: &'a mut String,
    depth: usize,
}

impl Dumper<'_> {
    fn line(&mut self, text: &str) {
        for _ in 0..self.depth {
            self.out.push_str("  ");
        }
        self.out.push_str(text);
        self.out.push('\n');
    }

    fn nest(&mut self, head: &str, body: impl FnOnce(&mut Self)) {
        self.line(head);
        self.depth += 1;
        body(self);
        self.depth -= 1;
    }

    fn program(&mut self, p: &Program) {
        self.nest("program", |w| {
            for g in &p.generics {
                w.generic(g);
            }
            for i in &p.imports {
                w.import(i);
            }
            for d in &p.decls {
                w.decl(d);
            }
        });
    }

    fn generic(&mut self, g: &GenericParam) {
        match g {
            GenericParam::Type { name, .. } => self.line(&format!("generic-type {}", name.text)),
            GenericParam::Const { name, ty, .. } => {
                self.nest(&format!("generic-const {}", name.text), |w| w.ty(ty));
            }
            GenericParam::Proc {
                name,
                params,
                result,
                ..
            } => {
                self.nest(&format!("generic-proc {}", name.text), |w| {
                    for p in params {
                        w.param(p);
                    }
                    if let Some(r) = result {
                        w.nest("result", |w| w.ty(r));
                    }
                });
            }
        }
    }

    fn import(&mut self, i: &Import) {
        self.nest("import", |w| {
            for item in &i.items {
                match &item.instance {
                    None => w.line(&format!("module {}", item.name.text)),
                    Some(inst) => {
                        w.nest(
                            &format!("instance {} is {}", item.name.text, inst.generic.text),
                            |w| {
                                for a in &inst.args {
                                    match a {
                                        GenericArg::Type(t) => w.ty(t),
                                        GenericArg::Expr(e) => w.expr(e),
                                    }
                                }
                            },
                        );
                    }
                }
            }
        });
    }

    fn decl(&mut self, d: &Decl) {
        let mut head = String::new();
        if d.also {
            head.push_str("also ");
        }
        if d.private {
            head.push_str("private ");
        }
        match &d.kind {
            DeclKind::Const { name, value } => {
                let _ = write!(head, "const {}", name.text);
                self.nest(&head, |w| w.expr(value));
            }
            DeclKind::Enum { names } => {
                let list: Vec<&str> = names.iter().map(|n| n.text.as_str()).collect();
                let _ = write!(head, "enum {}", list.join(" "));
                self.line(&head);
            }
            DeclKind::Type { name, ty } => {
                let _ = write!(head, "type {}", name.text);
                self.nest(&head, |w| w.ty(ty));
            }
            DeclKind::Exception { name } => {
                let _ = write!(head, "exception {}", name.text);
                self.line(&head);
            }
            DeclKind::Share { name, ty, value } => {
                let _ = write!(head, "share {}", name.text);
                self.nest(&head, |w| {
                    w.ty(ty);
                    w.expr(value);
                });
            }
            DeclKind::Proc {
                name,
                params,
                result,
                body,
            } => {
                let _ = write!(head, "proc {}", name.text);
                self.nest(&head, |w| {
                    for p in params {
                        w.param(p);
                    }
                    if let Some(r) = result {
                        w.nest("result", |w| w.ty(r));
                    }
                    w.block(body);
                });
            }
            DeclKind::Instance(import) => self.import(import),
        }
    }

    fn param(&mut self, p: &Param) {
        match &p.ty {
            ParamType::Value(t) => self.nest(&format!("param {}", p.name.text), |w| w.ty(t)),
            ParamType::Proc { params, result, .. } => {
                self.nest(&format!("param-proc {}", p.name.text), |w| {
                    for t in params {
                        w.ty(t);
                    }
                    if let Some(r) = result {
                        w.nest("result", |w| w.ty(r));
                    }
                });
            }
        }
    }

    fn block(&mut self, b: &Block) {
        self.nest("block", |w| {
            for v in &b.vars {
                w.nest(&format!("var {}", v.name.text), |w| w.ty(&v.ty));
            }
            for s in &b.body {
                w.stmt(s);
            }
        });
    }

    fn qident(&self, q: &QIdent) -> String {
        match &q.module {
            Some(m) => format!("{}::{}", m.text, q.name.text),
            None => q.name.text.clone(),
        }
    }

    fn ty(&mut self, t: &Type) {
        match &t.kind {
            TypeKind::Named(q) => self.line(&format!("named {}", self.qident(q))),
            TypeKind::Int => self.line("int"),
            TypeKind::Bool => self.line("bool"),
            TypeKind::Char => self.line("char"),
            TypeKind::Fixed { precision, scale } => self.nest("fixed", |w| {
                w.expr(precision);
                w.expr(scale);
            }),
            TypeKind::Array { length, element } => self.nest("array", |w| {
                w.expr(length);
                w.ty(element);
            }),
            TypeKind::Record { fields } => self.nest("record", |w| {
                for f in fields {
                    let head = if f.private {
                        format!("private field {}", f.name.text)
                    } else {
                        format!("field {}", f.name.text)
                    };
                    w.nest(&head, |w| w.ty(&f.ty));
                }
            }),
            TypeKind::Ref(q) => self.line(&format!("ref {}", self.qident(q))),
            TypeKind::RefArray(t) => self.nest("ref-array", |w| w.ty(t)),
            TypeKind::Inbox(q) => self.line(&format!("inbox {}", self.qident(q))),
            TypeKind::Mailbox(q) => self.line(&format!("mailbox {}", self.qident(q))),
            TypeKind::Reply(q) => self.line(&format!("reply {}", self.qident(q))),
            TypeKind::View(q) => self.line(&format!("view {}", self.qident(q))),
            TypeKind::Publisher(q) => self.line(&format!("publisher {}", self.qident(q))),
            TypeKind::Task(None) => self.line("task"),
            TypeKind::Task(Some(q)) => self.line(&format!("task {}", self.qident(q))),
        }
    }

    fn stmt(&mut self, s: &Stmt) {
        match &s.kind {
            StmtKind::Assign {
                target,
                rounded,
                value,
            } => {
                let head = if *rounded { "assign-rounded" } else { "assign" };
                self.nest(head, |w| {
                    w.designator(target);
                    w.expr(value);
                });
            }
            StmtKind::Call { void, call } => {
                let head = if *void { "void-call" } else { "call" };
                self.nest(head, |w| w.call(call));
            }
            StmtKind::If { arms, otherwise } => self.nest("if", |w| {
                for (cond, body) in arms {
                    w.nest("arm", |w| {
                        w.expr(cond);
                        for s in body {
                            w.stmt(s);
                        }
                    });
                }
                if let Some(body) = otherwise {
                    w.nest("else", |w| {
                        for s in body {
                            w.stmt(s);
                        }
                    });
                }
            }),
            StmtKind::While { condition, body } => self.nest("while", |w| {
                w.expr(condition);
                for s in body {
                    w.stmt(s);
                }
            }),
            StmtKind::Return(None) => self.line("return"),
            StmtKind::Return(Some(e)) => self.nest("return", |w| w.expr(e)),
            StmtKind::Exit => self.line("exit"),
            StmtKind::Next => self.line("next"),
            StmtKind::Defer { void, call } => {
                let head = if *void { "defer-void" } else { "defer" };
                self.nest(head, |w| w.call(call));
            }
            StmtKind::Raise(None) => self.line("raise"),
            StmtKind::Raise(Some(q)) => self.line(&format!("raise {}", self.qident(q))),
            StmtKind::Try { body, handlers } => self.nest("try", |w| {
                for s in body {
                    w.stmt(s);
                }
                for h in handlers {
                    let names: Vec<String> = h.exceptions.iter().map(|q| w.qident(q)).collect();
                    let head = if names.is_empty() {
                        "catch".to_owned()
                    } else {
                        format!("catch {}", names.join(" "))
                    };
                    w.nest(&head, |w| {
                        for s in &h.body {
                            w.stmt(s);
                        }
                    });
                }
            }),
        }
    }

    fn call(&mut self, c: &Call) {
        self.nest(&format!("callee {}", self.qident(&c.callee)), |w| {
            for a in &c.args {
                w.expr(a);
            }
        });
    }

    fn designator(&mut self, d: &Designator) {
        self.nest(&format!("designator {}", self.qident(&d.base)), |w| {
            for s in &d.selectors {
                w.selector(s);
            }
        });
    }

    fn selector(&mut self, s: &Selector) {
        match s {
            Selector::Index(e) => self.nest("index", |w| w.expr(e)),
            Selector::Field(f) => self.line(&format!("field {}", f.text)),
        }
    }

    fn expr(&mut self, e: &Expr) {
        match &e.kind {
            ExprKind::Int { value } => self.line(&format!("int {value}")),
            ExprKind::Fixed { digits, scale } => self.line(&format!("fixed {digits} {scale}")),
            // The char is written as its code point so that the dump is plain ASCII and
            // cannot depend on how a terminal renders it.
            ExprKind::Char(c) => self.line(&format!("char {}", *c as u32)),
            ExprKind::Str(s) => self.line(&format!("string {}", s.escape_default())),
            ExprKind::Bool(b) => self.line(&format!("bool {b}")),
            ExprKind::Nil => self.line("nil"),
            ExprKind::Designator(d) => self.designator(d),
            ExprKind::Call { call, selectors } => self.nest("call", |w| {
                w.call(call);
                for s in selectors {
                    w.selector(s);
                }
            }),
            ExprKind::New(t) => self.nest("new", |w| w.ty(t)),
            ExprKind::NewArray { element, length } => self.nest("new-array", |w| {
                w.ty(element);
                w.expr(length);
            }),
            ExprKind::Not(e) => self.nest("not", |w| w.expr(e)),
            ExprKind::Negate(e) => self.nest("negate", |w| w.expr(e)),
            ExprKind::Binary { op, left, right } => {
                self.nest(&format!("binary {}", op.text()), |w| {
                    w.expr(left);
                    w.expr(right);
                });
            }
            ExprKind::Paren(e) => self.nest("paren", |w| w.expr(e)),
        }
    }
}

//! The parser [10].
//!
//! Recursive descent with one token of lookahead, as the grammar allows: it is LL(1)
//! with explicit `end` and no dangling-else, so there are no tables, no generator and
//! no backtracking. Three places need a decision after the current token, all of them
//! resolved by looking at the token *after* a `qident`, which costs nothing because the
//! token stream is a slice:
//!
//! - a statement beginning with a `qident` is an assignment or a call, decided by `(`;
//! - a factor beginning with a `qident` is a designator or a call, likewise;
//! - `import X is G(...)` among the declarations is an instance import [4.4].
//!
//! ## Recovery
//!
//! Phase 1 requires "recovery good enough to report several errors in one run", and the
//! conformance harness compares the set of lines carrying an error against the set of
//! `#!error` annotations, so a parser that stopped at the first error would make most
//! negative tests inexpressible. On an error the parser reports once, then skips to a
//! token it can resume from — a `;`, an `end`, or the keyword that starts a declaration
//! — and carries on. It never reports twice for the same mistake.

use alm_core::diag::Diagnostics;
use alm_core::span::{SourceId, Span};

use crate::ast::*;
use crate::token::{Glyph, Keyword, Token, TokenKind};

/// Parse a token stream into a program. Always returns a tree, possibly a partial one;
/// check `diags.has_errors()` before using it.
pub fn parse(id: SourceId, tokens: &[Token], diags: &mut Diagnostics) -> Program {
    // `id` is not read here: every span comes from a token, which already carries it.
    // It stays in the signature because a parser that synthesises a span — for a
    // missing token at end of input, say — will need it, and because the caller should
    // not have to know whether it does.
    let _ = id;
    let mut p = Parser {
        tokens,
        at: 0,
        diags,
        failed: false,
    };
    p.program()
}

struct Parser<'t, 'd> {
    tokens: &'t [Token],
    at: usize,
    diags: &'d mut Diagnostics,
    /// Set while recovering, to stop a cascade of reports for one mistake.
    failed: bool,
}

/// What a recovery step may stop at.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Sync {
    /// A declaration boundary: `;` at the top level, or a declaration keyword.
    Decl,
    /// A statement boundary: `;`, or a keyword that ends a construct.
    Stmt,
}

impl Parser<'_, '_> {
    // -- the token stream ---------------------------------------------------

    fn peek(&self) -> &Token {
        // The stream always ends with `Eof`, so this cannot run off the end.
        &self.tokens[self.at.min(self.tokens.len() - 1)]
    }

    fn peek_kind(&self) -> &TokenKind {
        &self.peek().kind
    }

    /// The token after the current one.
    fn peek_next(&self) -> &TokenKind {
        &self.tokens[(self.at + 1).min(self.tokens.len() - 1)].kind
    }

    fn at_eof(&self) -> bool {
        *self.peek_kind() == TokenKind::Eof
    }

    fn span(&self) -> Span {
        self.peek().span
    }

    fn eat(&mut self, k: Keyword) -> bool {
        if *self.peek_kind() == TokenKind::Keyword(k) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn eat_glyph(&mut self, g: Glyph) -> bool {
        if *self.peek_kind() == TokenKind::Glyph(g) {
            self.at += 1;
            true
        } else {
            false
        }
    }

    fn check(&self, k: Keyword) -> bool {
        *self.peek_kind() == TokenKind::Keyword(k)
    }

    fn check_glyph(&self, g: Glyph) -> bool {
        *self.peek_kind() == TokenKind::Glyph(g)
    }

    // -- errors and recovery ------------------------------------------------

    /// Report, unless we are already recovering from an earlier mistake.
    fn error(&mut self, span: Span, message: impl Into<String>) {
        if !self.failed {
            self.diags.error(span, message);
            self.failed = true;
        }
    }

    /// Report "expected X, found Y" at the current token.
    fn expected(&mut self, what: &str) {
        let found = self.peek().describe();
        let span = self.span();
        self.error(span, format!("expected {what}, found {found}"));
    }

    fn expect_glyph(&mut self, g: Glyph) -> bool {
        if self.eat_glyph(g) {
            return true;
        }
        self.expected(&format!("`{}`", g.text()));
        false
    }

    fn expect(&mut self, k: Keyword) -> bool {
        if self.eat(k) {
            return true;
        }
        self.expected(&format!("`{}`", k.text()));
        false
    }

    fn expect_ident(&mut self) -> Ident {
        if let TokenKind::Ident(name) = self.peek_kind().clone() {
            let span = self.span();
            self.at += 1;
            return Ident { text: name, span };
        }
        self.expected("a name");
        Ident {
            text: String::new(),
            span: self.span(),
        }
    }

    /// Recover, and guarantee that the cursor moved.
    ///
    /// `recover` stops *at* a token it can resume from, which is right when the parser
    /// has already consumed the bad text — but when it has not, the caller would fail
    /// at the same token again and loop for ever, appending a diagnostic each time.
    /// Every driver loop therefore goes through this, which steps over one token when
    /// recovery made no progress.
    fn recover_from(&mut self, sync: Sync, before: usize) {
        self.recover(sync);
        if self.at == before && !self.at_eof() {
            self.at += 1;
        }
    }

    /// Skip tokens until something we can resume from, then clear the failure flag.
    fn recover(&mut self, sync: Sync) {
        while !self.at_eof() {
            match self.peek_kind() {
                TokenKind::Glyph(Glyph::Semicolon) => {
                    self.at += 1;
                    break;
                }
                TokenKind::Keyword(k) if starts_decl(*k) => break,
                TokenKind::Keyword(Keyword::End) if sync == Sync::Stmt => break,
                TokenKind::Keyword(k)
                    if sync == Sync::Stmt
                        && matches!(
                            k,
                            Keyword::Else
                                | Keyword::Elsif
                                | Keyword::Catch
                                | Keyword::Begin
                                | Keyword::Do
                                | Keyword::Then
                        ) =>
                {
                    break;
                }
                _ => self.at += 1,
            }
        }
        self.failed = false;
    }

    // -- the grammar --------------------------------------------------------

    /// `program = { genparam } { importdecl } { declaration | instdecl }` [10].
    fn program(&mut self) -> Program {
        let mut generics = Vec::new();
        while self.check(Keyword::Generic) {
            let before = self.at;
            match self.generic_param() {
                Some(g) => generics.push(g),
                None => self.recover_from(Sync::Decl, before),
            }
        }
        // `generic` lines come first [4.4]; one appearing later is reported where it is
        // rather than being read as a declaration, which would give a worse message.
        let mut imports = Vec::new();
        while self.check(Keyword::Import) && !self.is_instance_import() {
            let before = self.at;
            match self.import() {
                Some(i) => imports.push(i),
                None => self.recover_from(Sync::Decl, before),
            }
        }
        let mut decls = Vec::new();
        while !self.at_eof() {
            let before = self.at;
            if self.check(Keyword::Generic) {
                let span = self.span();
                self.error(
                    span,
                    "`generic` lines must come before the declarations [4.4]",
                );
                // Step over the `generic` keyword first: recovery stops at anything
                // that starts a declaration, and `generic` is one of those.
                self.at += 1;
                self.recover_from(Sync::Decl, before);
                continue;
            }
            match self.decl() {
                Some(d) => decls.push(d),
                None => self.recover_from(Sync::Decl, before),
            }
        }
        Program {
            generics,
            imports,
            decls,
        }
    }

    /// Is the `import` at the cursor an instance import, `import X is G(...)`?
    fn is_instance_import(&self) -> bool {
        // import ident is ...
        matches!(self.peek_next(), TokenKind::Ident(_))
            && matches!(
                self.tokens.get(self.at + 2).map(|t| &t.kind),
                Some(TokenKind::Keyword(Keyword::Is))
            )
    }

    /// `genparam = "generic" ( "type" ident | "const" ident ":" type
    ///           | "proc" ident "(" [ params ] ")" [ ":" type ] ) ";"` [10].
    fn generic_param(&mut self) -> Option<GenericParam> {
        let start = self.span();
        self.expect(Keyword::Generic);
        let g = if self.eat(Keyword::Type) {
            let name = self.expect_ident();
            GenericParam::Type {
                name,
                span: start.to(self.prev_span()),
            }
        } else if self.eat(Keyword::Const) {
            let name = self.expect_ident();
            self.expect_glyph(Glyph::Colon);
            let ty = self.ty()?;
            GenericParam::Const {
                name,
                ty,
                span: start.to(self.prev_span()),
            }
        } else if self.eat(Keyword::Proc) {
            let name = self.expect_ident();
            self.expect_glyph(Glyph::LeftParen);
            let params = self.params()?;
            self.expect_glyph(Glyph::RightParen);
            let result = if self.eat_glyph(Glyph::Colon) {
                Some(self.ty()?)
            } else {
                None
            };
            GenericParam::Proc {
                name,
                params,
                result,
                span: start.to(self.prev_span()),
            }
        } else {
            self.expected("`type`, `const` or `proc` after `generic`");
            return None;
        };
        self.expect_glyph(Glyph::Semicolon);
        if self.failed { None } else { Some(g) }
    }

    fn prev_span(&self) -> Span {
        let i = self.at.saturating_sub(1);
        self.tokens[i.min(self.tokens.len() - 1)].span
    }

    /// `importdecl = "import" importitem { "," importitem } ";"` [10].
    fn import(&mut self) -> Option<Import> {
        let start = self.span();
        self.expect(Keyword::Import);
        let mut items = Vec::new();
        loop {
            let item_start = self.span();
            let name = self.expect_ident();
            let instance = if self.eat(Keyword::Is) {
                let generic = self.expect_ident();
                self.expect_glyph(Glyph::LeftParen);
                let mut args = Vec::new();
                if !self.check_glyph(Glyph::RightParen) {
                    loop {
                        args.push(self.generic_arg()?);
                        if !self.eat_glyph(Glyph::Comma) {
                            break;
                        }
                    }
                }
                self.expect_glyph(Glyph::RightParen);
                Some(Instance { generic, args })
            } else {
                None
            };
            items.push(ImportItem {
                name,
                instance,
                span: item_start.to(self.prev_span()),
            });
            if !self.eat_glyph(Glyph::Comma) {
                break;
            }
        }
        self.expect_glyph(Glyph::Semicolon);
        if self.failed {
            None
        } else {
            Some(Import {
                items,
                span: start.to(self.prev_span()),
            })
        }
    }

    /// `garg = type | expr` [10].
    ///
    /// A bare name is ambiguous — `Count` could be a type or a constant — and [4.4]
    /// resolves it in Phase 2, once names exist. So a name becomes a `Type`, which the
    /// semantic phase reinterprets if it turns out to name a value; anything that can
    /// only be one of the two is parsed as that.
    fn generic_arg(&mut self) -> Option<GenericArg> {
        if self.starts_type() {
            return Some(GenericArg::Type(self.ty()?));
        }
        Some(GenericArg::Expr(self.expr()?))
    }

    fn starts_type(&self) -> bool {
        match self.peek_kind() {
            TokenKind::Keyword(k) => matches!(
                k,
                Keyword::Int
                    | Keyword::Bool
                    | Keyword::Char
                    | Keyword::Fixed
                    | Keyword::Array
                    | Keyword::Record
                    | Keyword::Ref
                    | Keyword::Inbox
                    | Keyword::Mailbox
                    | Keyword::Reply
                    | Keyword::View
                    | Keyword::Publisher
                    | Keyword::Task
            ),
            // A bare name: a type by default, see `generic_arg`.
            TokenKind::Ident(_) => true,
            _ => false,
        }
    }

    /// A declaration, with its optional `also` and `private` prefixes [4.1, 4.3].
    fn decl(&mut self) -> Option<Decl> {
        let start = self.span();
        if self.check(Keyword::Import) {
            let import = self.import()?;
            return Some(Decl {
                also: false,
                private: false,
                span: import.span,
                kind: DeclKind::Instance(import),
            });
        }
        let also = self.eat(Keyword::Also);
        let private = self.eat(Keyword::Private);
        let kind = if self.eat(Keyword::Const) {
            let name = self.expect_ident();
            self.expect(Keyword::Is);
            let value = self.expr()?;
            self.expect_glyph(Glyph::Semicolon);
            DeclKind::Const { name, value }
        } else if self.eat(Keyword::Enum) {
            let mut names = vec![self.expect_ident()];
            while self.eat_glyph(Glyph::Comma) {
                names.push(self.expect_ident());
            }
            self.expect_glyph(Glyph::Semicolon);
            DeclKind::Enum { names }
        } else if self.eat(Keyword::Type) {
            let name = self.expect_ident();
            self.expect(Keyword::Is);
            let ty = self.ty()?;
            self.expect_glyph(Glyph::Semicolon);
            DeclKind::Type { name, ty }
        } else if self.eat(Keyword::Exception) {
            let name = self.expect_ident();
            self.expect_glyph(Glyph::Semicolon);
            DeclKind::Exception { name }
        } else if self.eat(Keyword::Share) {
            let name = self.expect_ident();
            self.expect_glyph(Glyph::Colon);
            let ty = self.ty()?;
            self.expect(Keyword::Is);
            let value = self.expr()?;
            self.expect_glyph(Glyph::Semicolon);
            DeclKind::Share { name, ty, value }
        } else if self.eat(Keyword::Proc) {
            let name = self.expect_ident();
            self.expect_glyph(Glyph::LeftParen);
            let params = self.params()?;
            self.expect_glyph(Glyph::RightParen);
            let result = if self.eat_glyph(Glyph::Colon) {
                Some(self.ty()?)
            } else {
                None
            };
            let body = self.block()?;
            self.expect_glyph(Glyph::Semicolon);
            DeclKind::Proc {
                name,
                params,
                result,
                body,
            }
        } else {
            self.expected("a declaration: `const`, `enum`, `type`, `exception`, `share` or `proc`");
            return None;
        };
        if self.failed {
            None
        } else {
            Some(Decl {
                also,
                private,
                kind,
                span: start.to(self.prev_span()),
            })
        }
    }

    /// `params = param { "," param }`, possibly empty [10].
    fn params(&mut self) -> Option<Vec<Param>> {
        let mut out = Vec::new();
        if self.check_glyph(Glyph::RightParen) {
            return Some(out);
        }
        loop {
            let start = self.span();
            let name = self.expect_ident();
            self.expect_glyph(Glyph::Colon);
            let ty = if self.check(Keyword::Proc) {
                self.proc_type()?
            } else {
                ParamType::Value(self.ty()?)
            };
            out.push(Param {
                name,
                ty,
                span: start.to(self.prev_span()),
            });
            if !self.eat_glyph(Glyph::Comma) {
                return Some(out);
            }
        }
    }

    /// `proctype = "proc" "(" [ type { "," type } ] ")" [ ":" type ]` [10].
    fn proc_type(&mut self) -> Option<ParamType> {
        let start = self.span();
        self.expect(Keyword::Proc);
        self.expect_glyph(Glyph::LeftParen);
        let mut params = Vec::new();
        if !self.check_glyph(Glyph::RightParen) {
            loop {
                params.push(self.ty()?);
                if !self.eat_glyph(Glyph::Comma) {
                    break;
                }
            }
        }
        self.expect_glyph(Glyph::RightParen);
        let result = if self.eat_glyph(Glyph::Colon) {
            Some(Box::new(self.ty()?))
        } else {
            None
        };
        Some(ParamType::Proc {
            params,
            result,
            span: start.to(self.prev_span()),
        })
    }

    /// `block = { vardecl } "begin" stmts "end"` [10].
    fn block(&mut self) -> Option<Block> {
        let start = self.span();
        let mut vars = Vec::new();
        while self.check(Keyword::Var) {
            let before = self.at;
            let var_start = self.span();
            self.at += 1;
            let name = self.expect_ident();
            self.expect_glyph(Glyph::Colon);
            match self.ty() {
                Some(ty) => {
                    self.expect_glyph(Glyph::Semicolon);
                    vars.push(VarDecl {
                        name,
                        ty,
                        span: var_start.to(self.prev_span()),
                    });
                }
                None => self.recover_from(Sync::Stmt, before),
            }
        }
        self.expect(Keyword::Begin);
        let body = self.stmts();
        self.expect(Keyword::End);
        Some(Block {
            vars,
            body,
            span: start.to(self.prev_span()),
        })
    }

    /// `type` [10].
    fn ty(&mut self) -> Option<Type> {
        let start = self.span();
        let kind = match self.peek_kind().clone() {
            TokenKind::Keyword(Keyword::Int) => {
                self.at += 1;
                TypeKind::Int
            }
            TokenKind::Keyword(Keyword::Bool) => {
                self.at += 1;
                TypeKind::Bool
            }
            TokenKind::Keyword(Keyword::Char) => {
                self.at += 1;
                TypeKind::Char
            }
            TokenKind::Keyword(Keyword::Fixed) => {
                self.at += 1;
                self.expect_glyph(Glyph::LeftParen);
                let precision = Box::new(self.expr()?);
                self.expect_glyph(Glyph::Comma);
                let scale = Box::new(self.expr()?);
                self.expect_glyph(Glyph::RightParen);
                TypeKind::Fixed { precision, scale }
            }
            TokenKind::Keyword(Keyword::Array) => {
                self.at += 1;
                self.expect_glyph(Glyph::LeftBracket);
                let length = Box::new(self.expr()?);
                self.expect_glyph(Glyph::RightBracket);
                self.expect(Keyword::Of);
                TypeKind::Array {
                    length,
                    element: Box::new(self.ty()?),
                }
            }
            TokenKind::Keyword(Keyword::Record) => {
                self.at += 1;
                let mut fields = Vec::new();
                while !self.check(Keyword::End) && !self.at_eof() {
                    let field_start_at = self.at;
                    let field_start = self.span();
                    let private = self.eat(Keyword::Private);
                    let name = self.expect_ident();
                    self.expect_glyph(Glyph::Colon);
                    match self.ty() {
                        Some(ty) => {
                            self.expect_glyph(Glyph::Semicolon);
                            fields.push(Field {
                                private,
                                name,
                                ty,
                                span: field_start.to(self.prev_span()),
                            });
                        }
                        None => {
                            self.recover_from(Sync::Stmt, field_start_at);
                            if self.check(Keyword::End) {
                                break;
                            }
                        }
                    }
                }
                self.expect(Keyword::End);
                TypeKind::Record { fields }
            }
            TokenKind::Keyword(Keyword::Ref) => {
                self.at += 1;
                if self.eat(Keyword::Array) {
                    self.expect(Keyword::Of);
                    TypeKind::RefArray(Box::new(self.ty()?))
                } else {
                    TypeKind::Ref(self.qident()?)
                }
            }
            TokenKind::Keyword(Keyword::Inbox) => {
                self.at += 1;
                self.expect(Keyword::Of);
                TypeKind::Inbox(self.qident()?)
            }
            TokenKind::Keyword(Keyword::Mailbox) => {
                self.at += 1;
                self.expect(Keyword::Of);
                TypeKind::Mailbox(self.qident()?)
            }
            TokenKind::Keyword(Keyword::Reply) => {
                self.at += 1;
                self.expect(Keyword::Of);
                TypeKind::Reply(self.qident()?)
            }
            TokenKind::Keyword(Keyword::View) => {
                self.at += 1;
                self.expect(Keyword::Of);
                TypeKind::View(self.qident()?)
            }
            TokenKind::Keyword(Keyword::Publisher) => {
                self.at += 1;
                self.expect(Keyword::Of);
                TypeKind::Publisher(self.qident()?)
            }
            TokenKind::Keyword(Keyword::Task) => {
                self.at += 1;
                // `task` alone is a handle to a task that takes no messages [8.10].
                if self.eat(Keyword::Of) {
                    TypeKind::Task(Some(self.qident()?))
                } else {
                    TypeKind::Task(None)
                }
            }
            TokenKind::Ident(_) => TypeKind::Named(self.qident()?),
            _ => {
                self.expected("a type");
                return None;
            }
        };
        Some(Type {
            kind,
            span: start.to(self.prev_span()),
        })
    }

    /// `qident = ident [ "::" ident ]` [10].
    fn qident(&mut self) -> Option<QIdent> {
        let start = self.span();
        let first = self.expect_ident();
        if self.failed {
            return None;
        }
        if self.eat_glyph(Glyph::ColonColon) {
            let name = self.expect_ident();
            return Some(QIdent {
                module: Some(first),
                name,
                span: start.to(self.prev_span()),
            });
        }
        Some(QIdent {
            module: None,
            name: first,
            span: start.to(self.prev_span()),
        })
    }

    /// `stmts = { statement ";" }` [10].
    fn stmts(&mut self) -> Vec<Stmt> {
        let mut out = Vec::new();
        loop {
            if self.at_eof() || self.ends_stmts() {
                return out;
            }
            let before = self.at;
            match self.stmt() {
                Some(s) => {
                    self.expect_glyph(Glyph::Semicolon);
                    if self.failed {
                        self.recover_from(Sync::Stmt, before);
                    }
                    out.push(s);
                }
                None => {
                    self.recover_from(Sync::Stmt, before);
                    if self.at_eof() {
                        return out;
                    }
                }
            }
        }
    }

    fn ends_stmts(&self) -> bool {
        matches!(
            self.peek_kind(),
            TokenKind::Keyword(Keyword::End | Keyword::Else | Keyword::Elsif | Keyword::Catch)
        )
    }

    /// `statement` [10].
    fn stmt(&mut self) -> Option<Stmt> {
        let start = self.span();
        let kind = match self.peek_kind().clone() {
            TokenKind::Keyword(Keyword::If) => {
                self.at += 1;
                let mut arms = Vec::new();
                let cond = self.expr()?;
                self.expect(Keyword::Then);
                arms.push((cond, self.stmts()));
                while self.eat(Keyword::Elsif) {
                    let cond = self.expr()?;
                    self.expect(Keyword::Then);
                    arms.push((cond, self.stmts()));
                }
                let otherwise = if self.eat(Keyword::Else) {
                    Some(self.stmts())
                } else {
                    None
                };
                self.expect(Keyword::End);
                StmtKind::If { arms, otherwise }
            }
            TokenKind::Keyword(Keyword::While) => {
                self.at += 1;
                let condition = self.expr()?;
                self.expect(Keyword::Do);
                let body = self.stmts();
                self.expect(Keyword::End);
                StmtKind::While { condition, body }
            }
            TokenKind::Keyword(Keyword::Return) => {
                self.at += 1;
                // `return` with no expression ends a procedure that has no result.
                if self.check_glyph(Glyph::Semicolon) {
                    StmtKind::Return(None)
                } else {
                    StmtKind::Return(Some(self.expr()?))
                }
            }
            TokenKind::Keyword(Keyword::Exit) => {
                self.at += 1;
                StmtKind::Exit
            }
            TokenKind::Keyword(Keyword::Next) => {
                self.at += 1;
                StmtKind::Next
            }
            TokenKind::Keyword(Keyword::Raise) => {
                self.at += 1;
                // A bare `raise` re-raises the exception being handled [5.1].
                if self.check_glyph(Glyph::Semicolon) {
                    StmtKind::Raise(None)
                } else {
                    StmtKind::Raise(Some(self.qident()?))
                }
            }
            TokenKind::Keyword(Keyword::Defer) => {
                self.at += 1;
                let void = self.eat(Keyword::Void);
                StmtKind::Defer {
                    void,
                    call: self.call()?,
                }
            }
            TokenKind::Keyword(Keyword::Void) => {
                self.at += 1;
                StmtKind::Call {
                    void: true,
                    call: self.call()?,
                }
            }
            TokenKind::Keyword(Keyword::Try) => {
                self.at += 1;
                let body = self.stmts();
                let mut handlers = Vec::new();
                while self.check(Keyword::Catch) {
                    let h_start = self.span();
                    self.at += 1;
                    let mut exceptions = Vec::new();
                    if !self.check(Keyword::Do) {
                        loop {
                            exceptions.push(self.qident()?);
                            if !self.eat_glyph(Glyph::Comma) {
                                break;
                            }
                        }
                    }
                    self.expect(Keyword::Do);
                    handlers.push(Handler {
                        exceptions,
                        body: self.stmts(),
                        span: h_start.to(self.prev_span()),
                    });
                }
                if handlers.is_empty() {
                    let span = self.span();
                    self.error(span, "a `try` needs at least one `catch`");
                }
                self.expect(Keyword::End);
                StmtKind::Try { body, handlers }
            }
            TokenKind::Ident(_) => {
                // An assignment or a call, decided by the token after the `qident`.
                let q = self.qident()?;
                if self.check_glyph(Glyph::LeftParen) {
                    StmtKind::Call {
                        void: false,
                        call: self.call_after(q)?,
                    }
                } else {
                    let target = self.designator_after(q)?;
                    self.expect_glyph(Glyph::Assign);
                    let rounded = self.eat(Keyword::Rounded);
                    StmtKind::Assign {
                        target,
                        rounded,
                        value: self.expr()?,
                    }
                }
            }
            _ => {
                self.expected("a statement");
                return None;
            }
        };
        if self.failed {
            None
        } else {
            Some(Stmt {
                kind,
                span: start.to(self.prev_span()),
            })
        }
    }

    fn call(&mut self) -> Option<Call> {
        let q = self.qident()?;
        self.call_after(q)
    }

    fn call_after(&mut self, callee: QIdent) -> Option<Call> {
        let start = callee.span;
        self.expect_glyph(Glyph::LeftParen);
        let mut args = Vec::new();
        if !self.check_glyph(Glyph::RightParen) {
            loop {
                args.push(self.expr()?);
                if !self.eat_glyph(Glyph::Comma) {
                    break;
                }
            }
        }
        self.expect_glyph(Glyph::RightParen);
        Some(Call {
            callee,
            args,
            span: start.to(self.prev_span()),
        })
    }

    fn designator_after(&mut self, base: QIdent) -> Option<Designator> {
        let start = base.span;
        let mut selectors = Vec::new();
        loop {
            if self.eat_glyph(Glyph::LeftBracket) {
                selectors.push(Selector::Index(self.expr()?));
                self.expect_glyph(Glyph::RightBracket);
            } else if self.eat_glyph(Glyph::Dot) {
                selectors.push(Selector::Field(self.expect_ident()));
            } else {
                return Some(Designator {
                    base,
                    selectors,
                    span: start.to(self.prev_span()),
                });
            }
        }
    }

    // -- expressions, at the precedence levels of [6] -----------------------

    /// `expr = conj { "or" conj }` [10].
    fn expr(&mut self) -> Option<Expr> {
        let mut left = self.conj()?;
        while self.eat(Keyword::Or) {
            let right = self.conj()?;
            left = binary(BinOp::Or, left, right);
        }
        Some(left)
    }

    /// `conj = neg { "and" neg }` [10].
    fn conj(&mut self) -> Option<Expr> {
        let mut left = self.neg()?;
        while self.eat(Keyword::And) {
            let right = self.neg()?;
            left = binary(BinOp::And, left, right);
        }
        Some(left)
    }

    /// `neg = "not" neg | rel` [10].
    ///
    /// `not` sits above the relations, which is what makes it bind *looser* than them:
    /// `not a = b` is `not (a = b)` [2].
    fn neg(&mut self) -> Option<Expr> {
        if self.check(Keyword::Not) {
            let start = self.span();
            self.at += 1;
            let inner = self.neg()?;
            let span = start.to(inner.span);
            return Some(Expr {
                kind: ExprKind::Not(Box::new(inner)),
                span,
            });
        }
        self.rel()
    }

    /// `rel = sum [ ( "=" | "<" | "<=" | ">" | ">=" | "~" ) sum ]` [10].
    ///
    /// At most one operator: relations are **non-associative** [6], so `a < b < c` is a
    /// syntax error rather than a chain. Reporting that specifically is worth more than
    /// "expected `;`".
    fn rel(&mut self) -> Option<Expr> {
        let left = self.sum()?;
        let Some(op) = self.relation_op() else {
            return Some(left);
        };
        self.at += 1;
        let right = self.sum()?;
        if let Some(second) = self.relation_op() {
            let span = self.span();
            self.error(
                span,
                format!(
                    "`{}` cannot follow `{}` here: comparisons do not chain [6]; \
                     write `a {} b and b {} c`",
                    second.text(),
                    op.text(),
                    op.text(),
                    second.text()
                ),
            );
            return None;
        }
        Some(binary(op, left, right))
    }

    fn relation_op(&self) -> Option<BinOp> {
        match self.peek_kind() {
            TokenKind::Glyph(Glyph::Equal) => Some(BinOp::Equal),
            TokenKind::Glyph(Glyph::Less) => Some(BinOp::Less),
            TokenKind::Glyph(Glyph::LessEqual) => Some(BinOp::LessEqual),
            TokenKind::Glyph(Glyph::Greater) => Some(BinOp::Greater),
            TokenKind::Glyph(Glyph::GreaterEqual) => Some(BinOp::GreaterEqual),
            TokenKind::Glyph(Glyph::Tilde) => Some(BinOp::Identical),
            _ => None,
        }
    }

    /// `sum = term { ( "+" | "-" ) term }` [10].
    fn sum(&mut self) -> Option<Expr> {
        let mut left = self.term()?;
        loop {
            let op = if self.check_glyph(Glyph::Plus) {
                BinOp::Add
            } else if self.check_glyph(Glyph::Minus) {
                BinOp::Subtract
            } else {
                return Some(left);
            };
            self.at += 1;
            let right = self.term()?;
            left = binary(op, left, right);
        }
    }

    /// `term = unary { ( "*" | "/" | "div" | "mod" ) unary }` [10].
    fn term(&mut self) -> Option<Expr> {
        let mut left = self.unary()?;
        loop {
            let op = match self.peek_kind() {
                TokenKind::Glyph(Glyph::Star) => BinOp::Multiply,
                TokenKind::Glyph(Glyph::Slash) => BinOp::Divide,
                TokenKind::Keyword(Keyword::Div) => BinOp::Div,
                TokenKind::Keyword(Keyword::Mod) => BinOp::Mod,
                _ => return Some(left),
            };
            self.at += 1;
            let right = self.unary()?;
            left = binary(op, left, right);
        }
    }

    /// `unary = "_" unary | factor` [10].
    fn unary(&mut self) -> Option<Expr> {
        if self.check_glyph(Glyph::Underscore) {
            let start = self.span();
            self.at += 1;
            // The one place the over-max literal is legal [2]: `_9223372036854775808`
            // is the smallest `int`. `IntValue::negated` applies the rule.
            if let TokenKind::Int(v) = *self.peek_kind() {
                let lit_span = self.span();
                self.at += 1;
                let Some(value) = v.negated() else {
                    self.error(lit_span, "this negation is out of range for an `int`");
                    return None;
                };
                let span = start.to(lit_span);
                return Some(Expr {
                    kind: ExprKind::Int { value },
                    span,
                });
            }
            let inner = self.unary()?;
            let span = start.to(inner.span);
            return Some(Expr {
                kind: ExprKind::Negate(Box::new(inner)),
                span,
            });
        }
        self.factor()
    }

    /// `factor` [10].
    fn factor(&mut self) -> Option<Expr> {
        let start = self.span();
        let kind = match self.peek_kind().clone() {
            TokenKind::Int(v) => {
                self.at += 1;
                match v.value() {
                    Some(value) => ExprKind::Int { value },
                    None => {
                        // 2^63 outside a negation: [2] allows it only under `_`.
                        self.error(
                            start,
                            "9223372036854775808 is larger than the largest `int`; it is \
                             allowed only as the operand of `_`, which gives the \
                             smallest `int`",
                        );
                        return None;
                    }
                }
            }
            TokenKind::Fixed { digits, scale } => {
                self.at += 1;
                ExprKind::Fixed { digits, scale }
            }
            TokenKind::Char(c) => {
                self.at += 1;
                ExprKind::Char(c)
            }
            TokenKind::Str(s) => {
                self.at += 1;
                ExprKind::Str(s)
            }
            TokenKind::Keyword(Keyword::True) => {
                self.at += 1;
                ExprKind::Bool(true)
            }
            TokenKind::Keyword(Keyword::False) => {
                self.at += 1;
                ExprKind::Bool(false)
            }
            TokenKind::Keyword(Keyword::Nil) => {
                self.at += 1;
                ExprKind::Nil
            }
            TokenKind::Keyword(Keyword::New) => {
                self.at += 1;
                self.expect_glyph(Glyph::LeftParen);
                if self.eat(Keyword::Array) {
                    self.expect(Keyword::Of);
                    let element = self.ty()?;
                    self.expect_glyph(Glyph::Comma);
                    let length = Box::new(self.expr()?);
                    self.expect_glyph(Glyph::RightParen);
                    ExprKind::NewArray { element, length }
                } else {
                    let ty = self.ty()?;
                    self.expect_glyph(Glyph::RightParen);
                    ExprKind::New(ty)
                }
            }
            TokenKind::Glyph(Glyph::LeftParen) => {
                self.at += 1;
                let inner = self.expr()?;
                self.expect_glyph(Glyph::RightParen);
                ExprKind::Paren(Box::new(inner))
            }
            TokenKind::Ident(_) => {
                let q = self.qident()?;
                if self.check_glyph(Glyph::LeftParen) {
                    let call = self.call_after(q)?;
                    // [6] allows selectors on a call's result.
                    let mut selectors = Vec::new();
                    loop {
                        if self.eat_glyph(Glyph::LeftBracket) {
                            selectors.push(Selector::Index(self.expr()?));
                            self.expect_glyph(Glyph::RightBracket);
                        } else if self.eat_glyph(Glyph::Dot) {
                            selectors.push(Selector::Field(self.expect_ident()));
                        } else {
                            break;
                        }
                    }
                    ExprKind::Call { call, selectors }
                } else {
                    ExprKind::Designator(self.designator_after(q)?)
                }
            }
            _ => {
                self.expected("an expression");
                return None;
            }
        };
        if self.failed {
            None
        } else {
            Some(Expr {
                kind,
                span: start.to(self.prev_span()),
            })
        }
    }
}

fn binary(op: BinOp, left: Expr, right: Expr) -> Expr {
    let span = left.span.to(right.span);
    Expr {
        kind: ExprKind::Binary {
            op,
            left: Box::new(left),
            right: Box::new(right),
        },
        span,
    }
}

/// Keywords that can only start a declaration, so recovery can stop at one.
fn starts_decl(k: Keyword) -> bool {
    matches!(
        k,
        Keyword::Also
            | Keyword::Private
            | Keyword::Const
            | Keyword::Enum
            | Keyword::Type
            | Keyword::Exception
            | Keyword::Share
            | Keyword::Proc
            | Keyword::Generic
            | Keyword::Import
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ast;
    use crate::lex::lex;

    const S: SourceId = SourceId(0);

    /// Parse, expecting no diagnostic, and return the dump.
    fn ok(src: &str) -> String {
        let mut d = Diagnostics::new();
        let tokens = lex(S, src, &mut d);
        let program = parse(S, &tokens, &mut d);
        let rendered: Vec<String> = d.items().iter().map(|x| x.render("T.alm", src)).collect();
        assert!(rendered.is_empty(), "unexpected diagnostics: {rendered:#?}");
        ast::dump(&program)
    }

    /// Parse one expression by wrapping it in the smallest legal program.
    fn expr_dump(src: &str) -> String {
        let text = format!("proc f()\nbegin\n  x := {src};\nend;\n");
        let dump = ok(&text);
        // Keep the lines of the assignment's value, de-indented.
        let lines: Vec<&str> = dump.lines().collect();
        let at = lines
            .iter()
            .position(|l| l.trim() == "assign")
            .expect("an assignment");
        let body: Vec<String> = lines[at + 1..]
            .iter()
            .map(|l| l.trim_start().to_owned())
            .collect();
        // Drop the target designator, which is always `x`.
        body[1..].join("\n")
    }

    fn errs(src: &str) -> Vec<String> {
        let mut d = Diagnostics::new();
        let tokens = lex(S, src, &mut d);
        let _ = parse(S, &tokens, &mut d);
        let rendered: Vec<String> = d.items().iter().map(|x| x.render("T.alm", src)).collect();
        assert!(!rendered.is_empty(), "expected a diagnostic for {src:?}");
        rendered
    }

    #[test]
    fn the_declaration_forms_of_section_four() {
        let dump = ok("const Size is 100;\n\
                       enum Get, Put, Delete;\n\
                       type Node is record key: int; private left: ref Node; end;\n\
                       exception NotFound;\n\
                       share Table: int is 3;\n\
                       proc f(a: int, g: proc(int): bool): bool\n\
                         var t: int;\n\
                       begin\n\
                         return true;\n\
                       end;\n");
        assert!(dump.contains("const Size"), "{dump}");
        assert!(dump.contains("enum Get Put Delete"), "{dump}");
        assert!(dump.contains("private field left"), "{dump}");
        assert!(dump.contains("exception NotFound"), "{dump}");
        assert!(dump.contains("share Table"), "{dump}");
        assert!(dump.contains("param-proc g"), "{dump}");
        assert!(dump.contains("var t"), "{dump}");
    }

    #[test]
    fn also_and_private_prefix_a_declaration() {
        let dump = ok("also private type A is int;\nalso type B is int;\n");
        assert!(dump.contains("also private type A"), "{dump}");
        assert!(dump.contains("also type B"), "{dump}");
    }

    #[test]
    fn generic_lines_come_first_and_imports_next() {
        let dump = ok("generic type T;\n\
                       generic const Count: int;\n\
                       generic proc less(a: T, b: T): bool;\n\
                       import Other;\n\
                       import Names is Lookup(int, bool, equal);\n\
                       type X is int;\n");
        let lines: Vec<&str> = dump.lines().map(str::trim).collect();
        let generic_at = lines
            .iter()
            .position(|l| l.starts_with("generic-type"))
            .unwrap();
        let import_at = lines.iter().position(|l| *l == "import").unwrap();
        let type_at = lines.iter().position(|l| l.starts_with("type X")).unwrap();
        assert!(generic_at < import_at && import_at < type_at, "{dump}");
        assert!(dump.contains("instance Names is Lookup"), "{dump}");
    }

    #[test]
    fn a_generic_line_after_the_declarations_is_reported() {
        // [4.4]: the `generic` lines come first.
        let e = errs("type X is int;\ngeneric type T;\n");
        assert!(e[0].contains("must come before the declarations"), "{e:?}");
    }

    #[test]
    fn an_instance_import_may_stand_among_the_declarations() {
        // [4.4], and `instdecl` in [10].
        let dump = ok("type X is int;\nimport Names is Lookup(X, int, equal);\ntype Y is int;\n");
        let lines: Vec<&str> = dump.lines().map(str::trim).collect();
        let x = lines.iter().position(|l| l.starts_with("type X")).unwrap();
        let inst = lines
            .iter()
            .position(|l| l.starts_with("instance Names"))
            .unwrap();
        let y = lines.iter().position(|l| l.starts_with("type Y")).unwrap();
        assert!(x < inst && inst < y, "order must be kept: {dump}");
    }

    #[test]
    fn every_type_of_section_three() {
        let dump = ok("type A is int;\n\
                       type B is bool;\n\
                       type C is char;\n\
                       type D is fixed(18, 2);\n\
                       type E is array [10] of int;\n\
                       type F is record x: int; end;\n\
                       type G is ref F;\n\
                       type H is ref array of char;\n\
                       type I is inbox of M;\n\
                       type J is mailbox of M;\n\
                       type K is reply of M;\n\
                       type L is view of V;\n\
                       type N is publisher of V;\n\
                       type O is task;\n\
                       type P is task of R;\n\
                       type Q is Other::Thing;\n");
        for want in [
            "int",
            "bool",
            "char",
            "fixed",
            "array",
            "record",
            "ref F",
            "ref-array",
            "inbox M",
            "mailbox M",
            "reply M",
            "view V",
            "publisher V",
            "task\n",
            "task R",
            "named Other::Thing",
        ] {
            assert!(dump.contains(want), "missing {want:?} in:\n{dump}");
        }
    }

    #[test]
    fn every_statement_of_section_five() {
        let dump = ok("proc f()\n\
                       begin\n\
                         x := 1;\n\
                         y := rounded 1v5;\n\
                         a.b[0] := 2;\n\
                         g(1, 2);\n\
                         void g(3);\n\
                         if a then x := 1; elsif b then x := 2; else x := 3; end;\n\
                         while a do next; exit; end;\n\
                         defer close(f);\n\
                         defer void close(f);\n\
                         raise NotFound;\n\
                         raise;\n\
                         try x := 1; catch A, B do y := 2; catch do y := 3; end;\n\
                         return;\n\
                       end;\n");
        for want in [
            "assign\n",
            "assign-rounded",
            "designator a",
            "call",
            "void-call",
            "if",
            "arm",
            "else",
            "while",
            "next",
            "exit",
            "defer\n",
            "defer-void",
            "raise NotFound",
            "raise\n",
            "try",
            "catch A B",
            "catch\n",
            "return\n",
        ] {
            assert!(dump.contains(want), "missing {want:?} in:\n{dump}");
        }
    }

    #[test]
    fn precedence_follows_the_levels_of_section_six() {
        // `*` binds tighter than `+`, which binds tighter than the relations, which
        // bind tighter than `and`, which binds tighter than `or`.
        assert_eq!(
            expr_dump("1 + 2 * 3"),
            "binary +\nint 1\nbinary *\nint 2\nint 3"
        );
        assert_eq!(
            expr_dump("1 * 2 + 3"),
            "binary +\nbinary *\nint 1\nint 2\nint 3"
        );
        assert_eq!(
            expr_dump("a or b and c"),
            "binary or\ndesignator a\nbinary and\ndesignator b\ndesignator c"
        );
        assert_eq!(
            expr_dump("a = 1 and b"),
            "binary and\nbinary =\ndesignator a\nint 1\ndesignator b"
        );
    }

    #[test]
    fn the_four_term_operators_are_one_level() {
        // `* / div mod` share a level and associate left.
        assert_eq!(
            expr_dump("a div b mod c"),
            "binary mod\nbinary div\ndesignator a\ndesignator b\ndesignator c"
        );
    }

    #[test]
    fn not_binds_looser_than_a_comparison() {
        // [2]: "`not` binds looser than the comparison operators, so `not a = b` means
        // `not (a = b)`".
        assert_eq!(
            expr_dump("not a = b"),
            "not\nbinary =\ndesignator a\ndesignator b"
        );
        assert_eq!(
            expr_dump("not a ~ b"),
            "not\nbinary ~\ndesignator a\ndesignator b"
        );
    }

    #[test]
    fn relations_do_not_chain() {
        // [6]: relations are non-associative, so this is a syntax error rather than a
        // chain or a left-associated comparison of a bool with an int.
        let e = errs("proc f()\nbegin\n  x := a < b < c;\nend;\n");
        assert!(e[0].contains("comparisons do not chain"), "{e:?}");
        assert_eq!(e.len(), 1, "one mistake, one diagnostic: {e:?}");
    }

    #[test]
    fn underscore_is_prefix_negation() {
        assert_eq!(expr_dump("_a"), "negate\ndesignator a");
        assert_eq!(expr_dump("_1"), "int -1");
        // The literal case is folded, so `_` on a literal is a negative literal rather
        // than a negation of a positive one. That matters for the next test.
        assert_eq!(expr_dump("_ _1"), "negate\nint -1");
    }

    #[test]
    fn the_over_max_literal_is_legal_only_under_underscore() {
        // [2]. Under `_` it is the smallest `int`.
        assert_eq!(
            expr_dump("_9223372036854775808"),
            "int -9223372036854775808"
        );
        assert_eq!(expr_dump("_0x8000000000000000"), "int -9223372036854775808");
        // Anywhere else it is an error, and the message says why.
        let e = errs("proc f()\nbegin\n  x := 9223372036854775808;\nend;\n");
        assert!(e[0].contains("only as the operand of `_`"), "{e:?}");
    }

    #[test]
    fn selectors_are_allowed_on_a_call_result() {
        // [6]: "a call can be followed by selectors".
        assert_eq!(
            expr_dump("f(1).field[2]"),
            "call\ncallee f\nint 1\nfield field\nindex\nint 2"
        );
    }

    #[test]
    fn parentheses_are_kept_in_the_tree() {
        // The single-rounding rule of [7.4] is syntactic — "a single division ...
        // possibly in parentheses" — so Phase 2 has to be able to see them.
        assert_eq!(
            expr_dump("(a / b)"),
            "paren\nbinary /\ndesignator a\ndesignator b"
        );
    }

    #[test]
    fn new_in_both_forms() {
        assert_eq!(expr_dump("new(Node)"), "new\nnamed Node");
        assert_eq!(
            expr_dump("new(array of char, n)"),
            "new-array\nchar\ndesignator n"
        );
    }

    #[test]
    fn the_literal_forms() {
        assert_eq!(expr_dump("42"), "int 42");
        assert_eq!(expr_dump("12v50"), "fixed 1250 2");
        assert_eq!(expr_dump("'a'"), "char 97");
        assert_eq!(expr_dump(r#""hi""#), "string hi");
        assert_eq!(expr_dump("true"), "bool true");
        assert_eq!(expr_dump("false"), "bool false");
        assert_eq!(expr_dump("nil"), "nil");
    }

    #[test]
    fn several_syntax_errors_are_reported_in_one_run() {
        // Phase 1 requires this, and the harness depends on it.
        let e = errs("type A is ;\ntype B is int;\ntype C is ;\nproc f( is\nbegin\nend;\n");
        assert!(e.len() >= 3, "expected several errors, got {e:#?}");
        assert!(e[0].starts_with("T.alm:1:11:"), "{:?}", e[0]);
        // Recovery must not lose the good declaration between the bad ones.
        let mut d = Diagnostics::new();
        let src = "type A is ;\ntype B is int;\n";
        let tokens = lex(S, src, &mut d);
        let program = parse(S, &tokens, &mut d);
        assert!(
            ast::dump(&program).contains("type B"),
            "the declaration after an error must still be parsed"
        );
    }

    #[test]
    fn a_negative_test_for_each_shape_of_syntax_error() {
        for (src, want) in [
            ("type A is int\n", "expected `;`"),
            ("proc f()\nbegin\n  x := 1\nend;\n", "expected `;`"),
            ("proc f()\n  x := 1;\nend;\n", "expected `begin`"),
            (
                "proc f()\nbegin\n  if a x := 1; end;\nend;\n",
                "expected `then`",
            ),
            (
                "proc f()\nbegin\n  while a x := 1; end;\nend;\n",
                "expected `do`",
            ),
            (
                "proc f()\nbegin\n  try x := 1; end;\nend;\n",
                "needs at least one `catch`",
            ),
            ("const X is ;\n", "expected an expression"),
            ("type A is array of int;\n", "expected `[`"),
            ("wibble X is int;\n", "expected a declaration"),
            ("proc f()\nbegin\n  1 := 2;\nend;\n", "expected a statement"),
        ] {
            let e = errs(src);
            assert!(
                e.iter().any(|d| d.contains(want)),
                "{src:?} should mention {want:?}, got {e:#?}"
            );
        }
    }

    #[test]
    fn parsing_never_panics_on_truncated_input() {
        // Every prefix of a valid program must either parse or report, never crash.
        // Truncation is the commonest real-world broken input: a file being edited.
        let src = "generic type T;\nimport Other;\nproc f(a: int): bool\n  var x: int;\n\
                   begin\n  if a < 1 then\n    x := new(array of char, 3);\n  end;\n\
                   try x := 1; catch A do raise; end;\nend;\n";
        for end in 0..=src.len() {
            if !src.is_char_boundary(end) {
                continue;
            }
            let prefix = &src[..end];
            let mut d = Diagnostics::new();
            let tokens = lex(S, prefix, &mut d);
            let _ = parse(S, &tokens, &mut d);
        }
    }
}

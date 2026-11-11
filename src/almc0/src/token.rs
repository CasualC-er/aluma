//! Tokens, and the keyword and glyph tables of [2].

use alm_core::Span;

/// The 52 reserved words of [2], in the order the specification lists them.
///
/// The list is checked against the specification itself by a test, which reads the
/// ```keywords block out of `documentation/language_reference.md` and compares it to
/// this table. A hand-copied keyword list is exactly the kind of thing that is wrong by
/// one entry and stays wrong for a year.
macro_rules! keywords {
    ($($variant:ident => $text:literal),+ $(,)?) => {
        #[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
        pub enum Keyword {
            $($variant),+
        }

        impl Keyword {
            pub const ALL: &'static [Keyword] = &[$(Keyword::$variant),+];

            pub const fn text(self) -> &'static str {
                match self {
                    $(Keyword::$variant => $text),+
                }
            }

            /// `None` for an ordinary identifier.
            pub fn lookup(word: &str) -> Option<Keyword> {
                // Case-sensitive: identifiers are case-sensitive [2], so `End` is a
                // name and `end` is a keyword.
                match word {
                    $($text => Some(Keyword::$variant),)+
                    _ => None,
                }
            }
        }
    };
}

keywords! {
    Also => "also", And => "and", Array => "array", Begin => "begin", Bool => "bool",
    Catch => "catch", Char => "char", Const => "const", Defer => "defer", Div => "div",
    Do => "do", Else => "else", Elsif => "elsif", End => "end", Enum => "enum",
    Exception => "exception", Exit => "exit", False => "false", Fixed => "fixed",
    Generic => "generic", If => "if", Import => "import", Inbox => "inbox", Int => "int",
    Is => "is", Mailbox => "mailbox", Mod => "mod", New => "new", Nil => "nil",
    Next => "next", Not => "not", Of => "of", Or => "or", Private => "private",
    Proc => "proc", Publisher => "publisher", Raise => "raise", Record => "record",
    Ref => "ref", Reply => "reply", Return => "return", Rounded => "rounded",
    Share => "share", Task => "task", Then => "then", True => "true", Try => "try",
    Type => "type", Var => "var", View => "view", Void => "void", While => "while",
}

/// The glyphs of [2], plus the delimiters the grammar of [10] uses.
///
/// Note what is *absent*: there is no not-equal and no not-identical glyph [2]. `not`
/// binds looser than the comparisons instead, so `not a = b` is `not (a = b)`. A lexer
/// that helpfully accepted `!=` or `<>` would be accepting a different language.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub enum Glyph {
    /// `:=` assignment
    Assign,
    /// `:` "has type"
    Colon,
    /// `::` module qualification
    ColonColon,
    /// `=` value equality
    Equal,
    /// `~` reference identity
    Tilde,
    /// `<`
    Less,
    /// `<=`
    LessEqual,
    /// `>`
    Greater,
    /// `>=`
    GreaterEqual,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/` fixed-point division
    Slash,
    /// `_` negation, prefix
    Underscore,
    /// `.` field selection
    Dot,
    /// `,`
    Comma,
    /// `;`
    Semicolon,
    /// `(`
    LeftParen,
    /// `)`
    RightParen,
    /// `[`
    LeftBracket,
    /// `]`
    RightBracket,
}

impl Glyph {
    /// Every glyph, for tests that must hold for all of them rather than a chosen few.
    pub const ALL: &'static [Glyph] = &[
        Glyph::Assign,
        Glyph::Colon,
        Glyph::ColonColon,
        Glyph::Equal,
        Glyph::Tilde,
        Glyph::Less,
        Glyph::LessEqual,
        Glyph::Greater,
        Glyph::GreaterEqual,
        Glyph::Plus,
        Glyph::Minus,
        Glyph::Star,
        Glyph::Slash,
        Glyph::Underscore,
        Glyph::Dot,
        Glyph::Comma,
        Glyph::Semicolon,
        Glyph::LeftParen,
        Glyph::RightParen,
        Glyph::LeftBracket,
        Glyph::RightBracket,
    ];

    pub const fn text(self) -> &'static str {
        match self {
            Glyph::Assign => ":=",
            Glyph::Colon => ":",
            Glyph::ColonColon => "::",
            Glyph::Equal => "=",
            Glyph::Tilde => "~",
            Glyph::Less => "<",
            Glyph::LessEqual => "<=",
            Glyph::Greater => ">",
            Glyph::GreaterEqual => ">=",
            Glyph::Plus => "+",
            Glyph::Minus => "-",
            Glyph::Star => "*",
            Glyph::Slash => "/",
            Glyph::Underscore => "_",
            Glyph::Dot => ".",
            Glyph::Comma => ",",
            Glyph::Semicolon => ";",
            Glyph::LeftParen => "(",
            Glyph::RightParen => ")",
            Glyph::LeftBracket => "[",
            Glyph::RightBracket => "]",
        }
    }
}

/// An integer literal's value.
///
/// [2]: "A literal larger than the largest `int` is a compile error, except that
/// 9223372036854775808 (and 0x8000000000000000) is accepted as the operand of `_`,
/// which gives the smallest `int`."
///
/// Whether the literal *is* the operand of `_` is not something the lexer can know, so
/// it records the one over-max value the rule allows and leaves the check to the
/// parser. Any larger literal is rejected here, where the span is to hand.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum IntValue {
    /// Fits in an `int`.
    InRange(i64),
    /// Exactly 2^63. Legal only directly under `_`.
    MinIntMagnitude,
}

impl IntValue {
    /// The value, when the literal denotes one on its own.
    ///
    /// `None` for `MinIntMagnitude`: 2^63 is not an `int`, and a literal that large is
    /// legal **only** as the operand of `_` [2]. The parser must therefore use
    /// `negated` where it handles unary `_`, and report this case as an error
    /// everywhere else — see the Phase 1 parser item in TODO.md.
    pub fn value(self) -> Option<i64> {
        match self {
            IntValue::InRange(v) => Some(v),
            IntValue::MinIntMagnitude => None,
        }
    }

    /// The value of `_` applied to this literal, which is where the over-max case
    /// becomes legal: `_9223372036854775808` is the smallest `int`.
    ///
    /// `None` when negation itself would overflow, which cannot happen for an in-range
    /// literal but is checked rather than assumed.
    pub fn negated(self) -> Option<i64> {
        match self {
            IntValue::InRange(v) => v.checked_neg(),
            IntValue::MinIntMagnitude => Some(i64::MIN),
        }
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub enum TokenKind {
    Ident(String),
    Keyword(Keyword),
    Glyph(Glyph),
    Int(IntValue),
    /// A `fixed` literal, kept as its digits and scale rather than a number: `fixed`
    /// has arbitrary precision [7.4], and the constant evaluator works in unbounded
    /// decimal, so narrowing it here would lose what it is for.
    Fixed {
        /// The digits with the `v` removed: `12v50` is `"1250"`.
        digits: String,
        /// Digits after the `v`: `12v50` has scale 2.
        scale: u32,
    },
    Char(char),
    /// The decoded contents. Each evaluation of the literal makes a fresh, mutable
    /// `String` [3.4], so this is the initial content, not a shared value.
    Str(String),
    /// End of input. Always the last token, so the parser needs no option.
    Eof,
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn is(&self, k: Keyword) -> bool {
        self.kind == TokenKind::Keyword(k)
    }

    pub fn is_glyph(&self, g: Glyph) -> bool {
        self.kind == TokenKind::Glyph(g)
    }

    /// How the token is named in a diagnostic.
    pub fn describe(&self) -> String {
        match &self.kind {
            TokenKind::Ident(n) => format!("`{n}`"),
            TokenKind::Keyword(k) => format!("`{}`", k.text()),
            TokenKind::Glyph(g) => format!("`{}`", g.text()),
            TokenKind::Int(_) => "an integer literal".to_owned(),
            TokenKind::Fixed { .. } => "a fixed literal".to_owned(),
            TokenKind::Char(_) => "a character literal".to_owned(),
            TokenKind::Str(_) => "a string literal".to_owned(),
            TokenKind::Eof => "end of input".to_owned(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_keyword_table_is_the_one_in_the_specification() {
        // Read the list out of the spec rather than trusting a copy of it.
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../documentation/language_reference.md"),
        )
        .expect("the specification must be readable");
        let block = spec
            .split("```keywords")
            .nth(1)
            .and_then(|s| s.split("```").next())
            .expect("the specification must hold a ```keywords block");
        let want: Vec<&str> = block.split_whitespace().collect();
        let got: Vec<&str> = Keyword::ALL.iter().map(|k| k.text()).collect();
        assert_eq!(got, want, "the keyword table has drifted from [2]");
        assert_eq!(want.len(), 52, "[2] lists 52 reserved words");
    }

    #[test]
    fn the_over_max_literal_is_only_a_value_once_negated() {
        // [2]: 9223372036854775808 is accepted "as the operand of `_`, which gives the
        // smallest `int`" — and nowhere else.
        assert_eq!(IntValue::MinIntMagnitude.value(), None);
        assert_eq!(IntValue::MinIntMagnitude.negated(), Some(i64::MIN));
        assert_eq!(IntValue::InRange(7).value(), Some(7));
        assert_eq!(IntValue::InRange(7).negated(), Some(-7));
        assert_eq!(IntValue::InRange(i64::MAX).negated(), Some(-i64::MAX));
    }

    #[test]
    fn lookup_is_case_sensitive() {
        assert_eq!(Keyword::lookup("end"), Some(Keyword::End));
        assert_eq!(Keyword::lookup("End"), None);
        assert_eq!(Keyword::lookup("ends"), None);
    }

    #[test]
    fn the_glyph_table_holds_what_the_specification_lists() {
        // Read the glyph cells out of [2]'s own table rather than comparing the table in
        // this file against itself. [2] also lists `is`, `div`, `mod`, `#`, `v` and `\`
        // in that table: those are two keywords, a third keyword, the comment character,
        // part of a fixed literal and the escape introducer — not glyphs — so they are
        // named here as exclusions rather than quietly skipped.
        let spec = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../documentation/language_reference.md"),
        )
        .expect("the specification must be readable");
        let section = spec
            .split("\n## 2. Lexical structure")
            .nth(1)
            .and_then(|s| s.split("\n## 3.").next())
            .expect("section 2 of the specification");

        let not_glyphs = ["is", "div", "mod", "#", "v", "\\", "div` `mod"];
        let mut checked = 0;
        for line in section.lines() {
            let Some(rest) = line.strip_prefix("| `") else {
                continue;
            };
            let Some((cell, _)) = rest.split_once("` |") else {
                continue;
            };
            if not_glyphs.contains(&cell) {
                continue;
            }
            // A cell may list several glyphs: "`<` `<=` `>` `>=`".
            for part in cell.split("` `") {
                assert!(
                    Glyph::ALL.iter().any(|g| g.text() == part),
                    "[2] lists the glyph `{part}`, which the table in token.rs lacks"
                );
                checked += 1;
            }
        }
        assert!(
            checked >= 15,
            "only {checked} glyph cells found; did the table move?"
        );

        // And nothing in our table pretends to be a negation: [2] says there is no
        // not-equal and no not-identical glyph.
        for g in Glyph::ALL {
            assert!(
                !g.text().contains('!'),
                "{} looks like a negation",
                g.text()
            );
            assert_ne!(g.text(), "<>");
        }
    }
}

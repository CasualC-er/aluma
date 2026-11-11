//! The lexer [2].
//!
//! Reads UTF-8 source into tokens. Errors are collected rather than returned: Phase 1
//! requires "recovery good enough to report several errors in one run", and the
//! conformance harness compares the whole diagnostic text, so a lexer that stopped at
//! the first bad escape would make most negative tests inexpressible.
//!
//! What the lexer decides, and what it deliberately does not:
//!
//! - It rejects an integer literal larger than `int`, except the one over-max value
//!   [2] allows under `_`, which it passes on as `IntValue::MinIntMagnitude` for the
//!   parser to check in context.
//! - It rejects hexadecimal digits in a `fixed` literal, per amendment A-001: the prose
//!   of [2] says "digits `v` digits" while the grammar of [10] would admit `0xFFv10`.
//! - It does not fold, evaluate or range-check `fixed` literals. Their precision is
//!   unbounded [7.4]; that is the constant evaluator's work.

use alm_core::diag::Diagnostics;
use alm_core::span::{SourceId, Span};

use crate::token::{Glyph, IntValue, Keyword, Token, TokenKind};

pub struct Lexer<'s> {
    source: &'s str,
    bytes: &'s [u8],
    id: SourceId,
    at: usize,
}

/// Lex a whole source file. Always returns a token list ending in `Eof`; check
/// `diags.has_errors()` for whether it is trustworthy.
pub fn lex(id: SourceId, source: &str, diags: &mut Diagnostics) -> Vec<Token> {
    let mut lexer = Lexer {
        source,
        bytes: source.as_bytes(),
        id,
        at: 0,
    };
    let mut out = Vec::new();
    loop {
        let token = lexer.next_token(diags);
        let done = token.kind == TokenKind::Eof;
        out.push(token);
        if done {
            return out;
        }
    }
}

impl<'s> Lexer<'s> {
    fn span(&self, start: usize) -> Span {
        Span::new(self.id, start as u32, self.at as u32)
    }

    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.at).copied()
    }

    fn peek_at(&self, ahead: usize) -> Option<u8> {
        self.bytes.get(self.at + ahead).copied()
    }

    /// Advance one *character*, not one byte: non-ASCII is legal in comments and in
    /// literals [2], and stepping a byte at a time would split a code point.
    fn bump_char(&mut self) -> Option<char> {
        let c = self.source[self.at..].chars().next()?;
        self.at += c.len_utf8();
        Some(c)
    }

    fn skip_trivia(&mut self) {
        loop {
            match self.peek() {
                // Any ASCII whitespace separates tokens. A line break inside a literal
                // is handled where literals are read, and is an error there.
                Some(b) if b.is_ascii_whitespace() => self.at += 1,
                // `#` runs to the end of the line [2]. Inside a literal it is an
                // ordinary character, which is why this is not a byte scan of the file.
                Some(b'#') => {
                    while let Some(b) = self.peek() {
                        if b == b'\n' {
                            break;
                        }
                        self.at += 1;
                    }
                }
                _ => return,
            }
        }
    }

    fn next_token(&mut self, diags: &mut Diagnostics) -> Token {
        self.skip_trivia();
        let start = self.at;
        let Some(b) = self.peek() else {
            return Token {
                kind: TokenKind::Eof,
                span: self.span(start),
            };
        };

        let kind = match b {
            b'A'..=b'Z' | b'a'..=b'z' => self.word(),
            b'0'..=b'9' => self.number(diags, start),
            b'\'' => self.char_literal(diags, start),
            b'"' => self.string_literal(diags, start),
            _ => match self.glyph() {
                Some(g) => TokenKind::Glyph(g),
                None => {
                    // Unknown byte. Consume one character so that lexing makes
                    // progress and the next error is a new one.
                    let c = self.bump_char().unwrap_or('\u{fffd}');
                    diags.error(
                        self.span(start),
                        if c.is_ascii() {
                            format!("`{c}` is not a glyph of the language")
                        } else {
                            // Identifiers are ASCII only [2]; the likely cause is a
                            // non-ASCII character pasted outside a comment or literal.
                            format!(
                                "`{c}` is not allowed here: only comments and char and \
                                 string literals may hold non-ASCII text"
                            )
                        },
                    );
                    return self.next_token(diags);
                }
            },
        };
        Token {
            kind,
            span: self.span(start),
        }
    }

    /// An identifier or a keyword: `letter { letter | digit }`, ASCII only, no
    /// underscores (`_` is the negation glyph, not a name character) [2].
    fn word(&mut self) -> TokenKind {
        let start = self.at;
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() {
                self.at += 1;
            } else {
                break;
            }
        }
        let word = &self.source[start..self.at];
        match Keyword::lookup(word) {
            Some(k) => TokenKind::Keyword(k),
            None => TokenKind::Ident(word.to_owned()),
        }
    }

    /// An integer or `fixed` literal.
    fn number(&mut self, diags: &mut Diagnostics, start: usize) -> TokenKind {
        // Hexadecimal: `0x` and hexadecimal digits [2, 10].
        if self.peek() == Some(b'0') && matches!(self.peek_at(1), Some(b'x')) {
            self.at += 2;
            let digits_start = self.at;
            while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
                self.at += 1;
            }
            let digits = &self.source[digits_start..self.at];
            if digits.is_empty() {
                diags.error(
                    self.span(start),
                    "`0x` must be followed by hexadecimal digits",
                );
                return TokenKind::Int(IntValue::InRange(0));
            }
            // A-001: a fixed literal is decimal. `0xFFv10` is rejected rather than
            // silently read as `0xFF` followed by the identifier `v10`.
            if self.peek() == Some(b'v') {
                self.at += 1;
                while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                    self.at += 1;
                }
                diags.error(
                    self.span(start),
                    "a fixed literal is written with decimal digits, as in `12v50`; \
                     hexadecimal is not allowed before `v` (amendment A-001)",
                );
                return TokenKind::Fixed {
                    digits: "0".to_owned(),
                    scale: 0,
                };
            }
            return self.int_from_str(diags, start, digits, 16);
        }

        let int_start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
        }
        let whole = &self.source[int_start..self.at];

        // `v` makes it a fixed literal, but only when digits follow: `12v` is an error
        // and `x.v` never reaches here.
        if self.peek() == Some(b'v') && self.peek_at(1).is_some_and(|b| b.is_ascii_digit()) {
            self.at += 1;
            let frac_start = self.at;
            while self.peek().is_some_and(|b| b.is_ascii_digit()) {
                self.at += 1;
            }
            let frac = &self.source[frac_start..self.at];
            // Trailing `v`-digits that run into a hex escape or a letter are a
            // different error; catch an identifier character directly after.
            if self.peek().is_some_and(|b| b.is_ascii_alphanumeric()) {
                let bad = self.at;
                while self.peek().is_some_and(|b| b.is_ascii_alphanumeric()) {
                    self.at += 1;
                }
                diags.error(
                    Span::new(self.id, bad as u32, self.at as u32),
                    "a fixed literal ends after its digits",
                );
            }
            // The scale is the number of digits after `v` [2], so `0v05` is scale 2 and
            // `5v0` is scale 1. Keeping the digits verbatim preserves that.
            return TokenKind::Fixed {
                digits: format!("{whole}{frac}"),
                scale: frac.len() as u32,
            };
        }
        if self.peek() == Some(b'v') {
            // `12v` with nothing after it.
            self.at += 1;
            diags.error(self.span(start), "a fixed literal needs digits after `v`");
            return TokenKind::Fixed {
                digits: whole.to_owned(),
                scale: 0,
            };
        }
        self.int_from_str(diags, start, whole, 10)
    }

    fn int_from_str(
        &mut self,
        diags: &mut Diagnostics,
        start: usize,
        digits: &str,
        radix: u32,
    ) -> TokenKind {
        match u128::from_str_radix(digits, radix) {
            // The one over-max literal [2] allows, as the operand of `_`.
            Ok(v) if v == 1u128 << 63 => TokenKind::Int(IntValue::MinIntMagnitude),
            Ok(v) if v <= i64::MAX as u128 => TokenKind::Int(IntValue::InRange(v as i64)),
            _ => {
                diags.error(
                    self.span(start),
                    "this literal is larger than the largest `int`; only \
                     9223372036854775808 (or 0x8000000000000000) is allowed, and only \
                     as the operand of `_`",
                );
                TokenKind::Int(IntValue::InRange(0))
            }
        }
    }

    /// `'` one character or escape `'`, denoting exactly one code point [2].
    fn char_literal(&mut self, diags: &mut Diagnostics, start: usize) -> TokenKind {
        self.at += 1; // the opening quote
        // If reading the contents already failed — a bad escape, an unclosed literal —
        // the length is not worth a second complaint. One mistake, one diagnostic:
        // the harness checks that the set of lines carrying an error is exactly the
        // annotated set, and a cascade makes a correct test look wrong.
        let errors_before = diags.len();
        let mut chars = Vec::new();
        loop {
            match self.peek() {
                None => {
                    diags.error(self.span(start), "this char literal is not closed");
                    break;
                }
                Some(b'\n') => {
                    // Do not consume the line break: it ends the literal's line and the
                    // next line should lex normally.
                    diags.error(self.span(start), "a literal cannot contain a line break");
                    break;
                }
                Some(b'\'') => {
                    self.at += 1;
                    break;
                }
                Some(b'\\') => {
                    if let Some(c) = self.escape(diags) {
                        chars.push(c);
                    }
                }
                Some(_) => {
                    if let Some(c) = self.bump_char() {
                        chars.push(c);
                    }
                }
            }
        }
        if diags.len() != errors_before {
            // Already reported; give the parser something and move on.
            return TokenKind::Char(chars.first().copied().unwrap_or('\0'));
        }
        match chars.len() {
            1 => TokenKind::Char(chars[0]),
            0 => {
                diags.error(self.span(start), "a char literal needs one character");
                TokenKind::Char('\0')
            }
            n => {
                diags.error(
                    self.span(start),
                    format!("a char literal denotes exactly one code point, not {n}"),
                );
                TokenKind::Char(chars[0])
            }
        }
    }

    fn string_literal(&mut self, diags: &mut Diagnostics, start: usize) -> TokenKind {
        self.at += 1; // the opening quote
        let mut text = String::new();
        loop {
            match self.peek() {
                None => {
                    diags.error(self.span(start), "this string literal is not closed");
                    break;
                }
                Some(b'\n') => {
                    diags.error(self.span(start), "a literal cannot contain a line break");
                    break;
                }
                Some(b'"') => {
                    self.at += 1;
                    break;
                }
                Some(b'\\') => {
                    if let Some(c) = self.escape(diags) {
                        text.push(c);
                    }
                }
                Some(_) => {
                    if let Some(c) = self.bump_char() {
                        text.push(c);
                    }
                }
            }
        }
        TokenKind::Str(text)
    }

    /// One escape, the backslash already at the cursor.
    ///
    /// [2] fixes the set: `\n \t \r \0 \\ \' \" \u{h}`. "Any other escape is a compile
    /// error" — so this rejects `\a`, `\e`, `\x41` and the rest rather than passing the
    /// character through, which would quietly accept a different language.
    fn escape(&mut self, diags: &mut Diagnostics) -> Option<char> {
        let start = self.at;
        self.at += 1; // the backslash
        let Some(b) = self.peek() else {
            diags.error(self.span(start), "this escape is not finished");
            return None;
        };
        let simple = match b {
            b'n' => Some('\n'),
            b't' => Some('\t'),
            b'r' => Some('\r'),
            b'0' => Some('\0'),
            b'\\' => Some('\\'),
            b'\'' => Some('\''),
            b'"' => Some('"'),
            _ => None,
        };
        if let Some(c) = simple {
            self.at += 1;
            return Some(c);
        }
        if b == b'u' {
            return self.unicode_escape(diags, start);
        }
        let c = self.bump_char().unwrap_or('\\');
        diags.error(
            self.span(start),
            format!(
                "`\\{c}` is not an escape; the escapes are \\n \\t \\r \\0 \\\\ \\' \\\" \
                 and \\u{{h}}"
            ),
        );
        None
    }

    /// `\u{h}`: one to six hexadecimal digits naming a valid code point [2].
    fn unicode_escape(&mut self, diags: &mut Diagnostics, start: usize) -> Option<char> {
        self.at += 1; // the `u`
        if self.peek() != Some(b'{') {
            diags.error(self.span(start), "`\\u` must be followed by `{`");
            return None;
        }
        self.at += 1;
        let digits_start = self.at;
        while self.peek().is_some_and(|b| b.is_ascii_hexdigit()) {
            self.at += 1;
        }
        let digits = &self.source[digits_start..self.at];
        let closed = self.peek() == Some(b'}');
        if closed {
            self.at += 1;
        }
        if !closed {
            diags.error(self.span(start), "this `\\u{...}` escape is not closed");
            return None;
        }
        if digits.is_empty() || digits.len() > 6 {
            diags.error(
                self.span(start),
                "`\\u{h}` takes one to six hexadecimal digits",
            );
            return None;
        }
        let value = u32::from_str_radix(digits, 16).expect("hex digits, at most six");
        match char::from_u32(value) {
            Some(c) => Some(c),
            None => {
                // A surrogate or a value above the Unicode maximum. `char` is a code
                // point [3.1], and there is no way to hold one of these.
                diags.error(
                    self.span(start),
                    format!("U+{value:04X} is not a Unicode code point"),
                );
                None
            }
        }
    }

    /// The glyph table of [2], longest match first.
    fn glyph(&mut self) -> Option<Glyph> {
        let two = self.bytes.get(self.at..self.at + 2);
        if let Some(g) = match two {
            Some(b":=") => Some(Glyph::Assign),
            Some(b"::") => Some(Glyph::ColonColon),
            Some(b"<=") => Some(Glyph::LessEqual),
            Some(b">=") => Some(Glyph::GreaterEqual),
            _ => None,
        } {
            self.at += 2;
            return Some(g);
        }
        let g = match self.peek()? {
            b':' => Glyph::Colon,
            b'=' => Glyph::Equal,
            b'~' => Glyph::Tilde,
            b'<' => Glyph::Less,
            b'>' => Glyph::Greater,
            b'+' => Glyph::Plus,
            b'-' => Glyph::Minus,
            b'*' => Glyph::Star,
            b'/' => Glyph::Slash,
            b'_' => Glyph::Underscore,
            b'.' => Glyph::Dot,
            b',' => Glyph::Comma,
            b';' => Glyph::Semicolon,
            b'(' => Glyph::LeftParen,
            b')' => Glyph::RightParen,
            b'[' => Glyph::LeftBracket,
            b']' => Glyph::RightBracket,
            _ => return None,
        };
        self.at += 1;
        Some(g)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: SourceId = SourceId(0);

    /// Lex, expecting no diagnostic. Returns the tokens without the trailing `Eof`.
    fn ok(src: &str) -> Vec<TokenKind> {
        let mut d = Diagnostics::new();
        let tokens = lex(S, src, &mut d);
        let rendered: Vec<String> = d.items().iter().map(|x| x.render("T.alm", src)).collect();
        assert!(rendered.is_empty(), "unexpected diagnostics: {rendered:#?}");
        assert_eq!(tokens.last().map(|t| &t.kind), Some(&TokenKind::Eof));
        tokens[..tokens.len() - 1]
            .iter()
            .map(|t| t.kind.clone())
            .collect()
    }

    /// Lex, expecting at least one diagnostic. Returns them rendered.
    fn errs(src: &str) -> Vec<String> {
        let mut d = Diagnostics::new();
        let _ = lex(S, src, &mut d);
        let rendered: Vec<String> = d.items().iter().map(|x| x.render("T.alm", src)).collect();
        assert!(!rendered.is_empty(), "expected a diagnostic for {src:?}");
        rendered
    }

    fn ident(n: &str) -> TokenKind {
        TokenKind::Ident(n.to_owned())
    }

    fn int(v: i64) -> TokenKind {
        TokenKind::Int(IntValue::InRange(v))
    }

    #[test]
    fn a_declaration_lexes_as_expected() {
        use Glyph::*;
        use Keyword::*;
        assert_eq!(
            ok("var total: int;"),
            [
                TokenKind::Keyword(Var),
                ident("total"),
                TokenKind::Glyph(Colon),
                TokenKind::Keyword(Int),
                TokenKind::Glyph(Semicolon),
            ]
        );
    }

    #[test]
    fn identifiers_are_ascii_and_hold_no_underscore() {
        // [2]: `letter { letter | digit }`, no underscores. So `a_b` is three tokens,
        // because `_` is the negation glyph.
        assert_eq!(
            ok("a_b"),
            [ident("a"), TokenKind::Glyph(Glyph::Underscore), ident("b")]
        );
        assert_eq!(ok("x9 End"), [ident("x9"), ident("End")]);
        // A keyword is not an identifier, and the match is case-sensitive.
        assert_eq!(ok("end"), [TokenKind::Keyword(Keyword::End)]);
        // Non-ASCII outside a comment or literal is an error [2].
        let e = errs("var ñ: int;");
        assert!(
            e[0].contains("only comments and char and string literals"),
            "{e:?}"
        );
    }

    #[test]
    fn comments_run_to_the_end_of_the_line() {
        assert_eq!(ok("a # b c\nd"), [ident("a"), ident("d")]);
        // Unicode is allowed in a comment [2].
        assert_eq!(ok("# héllo ✓\na"), [ident("a")]);
        // A `#` inside a literal is an ordinary character [2].
        assert_eq!(ok("\"a#b\""), [TokenKind::Str("a#b".to_owned())]);
        assert_eq!(ok("'#'"), [TokenKind::Char('#')]);
    }

    #[test]
    fn integer_literals_decimal_and_hex() {
        assert_eq!(ok("42"), [int(42)]);
        assert_eq!(ok("0xFF"), [int(255)]);
        assert_eq!(ok("0xff"), [int(255)], "both cases of hex digit [10]");
        assert_eq!(ok("0"), [int(0)]);
        assert_eq!(ok("9223372036854775807"), [int(i64::MAX)]);
    }

    #[test]
    fn the_over_max_rule_and_its_two_spellings() {
        // [2]: a literal larger than the largest `int` is an error, except
        // 9223372036854775808 and 0x8000000000000000 as the operand of `_`.
        assert_eq!(
            ok("_9223372036854775808"),
            [
                TokenKind::Glyph(Glyph::Underscore),
                TokenKind::Int(IntValue::MinIntMagnitude)
            ]
        );
        assert_eq!(
            ok("_0x8000000000000000"),
            [
                TokenKind::Glyph(Glyph::Underscore),
                TokenKind::Int(IntValue::MinIntMagnitude)
            ],
            "the hexadecimal spelling is allowed too"
        );
        // One past it, in both spellings.
        assert!(errs("9223372036854775809")[0].contains("larger than the largest `int`"));
        assert!(errs("0x8000000000000001")[0].contains("larger than the largest `int`"));
        assert!(errs("0xFFFFFFFFFFFFFFFFF")[0].contains("larger than the largest `int`"));
    }

    #[test]
    fn hex_needs_at_least_one_digit() {
        assert!(errs("0x")[0].contains("hexadecimal digits"));
        assert!(errs("0xg")[0].contains("hexadecimal digits"));
    }

    #[test]
    fn fixed_literals_keep_their_digits_and_scale() {
        // [2]: "the scale of a fixed literal is the number of digits after `v`, so
        // `0v05` is 0.05 with scale 2 and `5v0` is 5.0 with scale 1."
        assert_eq!(
            ok("12v50"),
            [TokenKind::Fixed {
                digits: "1250".to_owned(),
                scale: 2
            }]
        );
        assert_eq!(
            ok("0v05"),
            [TokenKind::Fixed {
                digits: "005".to_owned(),
                scale: 2
            }]
        );
        assert_eq!(
            ok("5v0"),
            [TokenKind::Fixed {
                digits: "50".to_owned(),
                scale: 1
            }]
        );
        // Arbitrary precision [7.4]: nothing is narrowed here.
        let long = "1".repeat(40);
        assert_eq!(
            ok(&format!("{long}v{long}")),
            [TokenKind::Fixed {
                digits: format!("{long}{long}"),
                scale: 40
            }]
        );
    }

    #[test]
    fn a_fixed_literal_is_decimal_only() {
        // Amendment A-001: the prose of [2] governs over the grammar of [10].
        let e = errs("0xFFv10");
        assert!(e[0].contains("amendment A-001"), "{e:?}");
        assert!(errs("12v")[0].contains("needs digits after `v`"));
    }

    #[test]
    fn the_escape_set_is_exactly_the_one_in_the_spec() {
        assert_eq!(
            ok(r#""\n\t\r\0\\\'\"""#),
            [TokenKind::Str("\n\t\r\0\\'\"".to_owned())]
        );
        assert_eq!(ok(r"'\n'"), [TokenKind::Char('\n')]);
        // "Any other escape is a compile error" [2].
        for bad in [r"'\a'", r"'\e'", r"'\x41'", r"'\123'"] {
            let e = errs(bad);
            assert!(e[0].contains("is not an escape"), "{bad}: {e:?}");
        }
    }

    #[test]
    fn unicode_escapes() {
        assert_eq!(ok(r"'\u{41}'"), [TokenKind::Char('A')]);
        assert_eq!(ok(r"'\u{1F600}'"), [TokenKind::Char('\u{1F600}')]);
        assert_eq!(ok(r"'\u{0}'"), [TokenKind::Char('\0')]);
        assert!(errs(r"'\u41'")[0].contains("must be followed by `{`"));
        assert!(errs(r"'\u{}'")[0].contains("one to six hexadecimal digits"));
        assert!(errs(r"'\u{1234567}'")[0].contains("one to six hexadecimal digits"));
        assert!(errs(r"'\u{41'")[0].contains("not closed"));
        // Above the Unicode maximum, and a surrogate: neither is a code point [3.1].
        assert!(errs(r"'\u{110000}'")[0].contains("not a Unicode code point"));
        assert!(errs(r"'\u{D800}'")[0].contains("not a Unicode code point"));
    }

    #[test]
    fn a_char_literal_is_exactly_one_code_point() {
        assert_eq!(ok("'a'"), [TokenKind::Char('a')]);
        assert_eq!(
            ok("'é'"),
            [TokenKind::Char('é')],
            "any Unicode text may appear [2]"
        );
        assert!(errs("'ab'")[0].contains("exactly one code point, not 2"));
        assert!(errs("''")[0].contains("needs one character"));
    }

    #[test]
    fn a_literal_cannot_hold_a_raw_line_break() {
        // [2]: "A literal cannot contain a raw line break."
        let e = errs("\"a\nb\"");
        assert!(e[0].contains("line break"), "{e:?}");
        let e = errs("'\n'");
        assert!(e[0].contains("line break"), "{e:?}");
        // And the line after it still lexes, so one mistake gives one error.
        let mut d = Diagnostics::new();
        let tokens = lex(S, "\"a\nvar x: int;", &mut d);
        assert_eq!(d.len(), 1, "one error, not a cascade");
        assert!(
            tokens
                .iter()
                .any(|t| t.kind == TokenKind::Keyword(Keyword::Var))
        );
    }

    #[test]
    fn an_unclosed_literal_is_reported_once() {
        assert!(errs("\"abc")[0].contains("not closed"));
        assert!(errs("'a")[0].contains("not closed"));
    }

    #[test]
    fn the_two_character_glyphs_win_over_the_one_character_ones() {
        use Glyph::*;
        assert_eq!(
            ok(":= : :: = <= < >= > ~ + - * / _ . , ; ( ) [ ]"),
            [
                Assign,
                Colon,
                ColonColon,
                Equal,
                LessEqual,
                Less,
                GreaterEqual,
                Greater,
                Tilde,
                Plus,
                Minus,
                Star,
                Slash,
                Underscore,
                Dot,
                Comma,
                Semicolon,
                LeftParen,
                RightParen,
                LeftBracket,
                RightBracket,
            ]
            .map(TokenKind::Glyph)
        );
        // No space needed: `a:=b` is three tokens, not `a`, `:`, `=`, `b`.
        assert_eq!(
            ok("a:=b"),
            [ident("a"), TokenKind::Glyph(Assign), ident("b")]
        );
        assert_eq!(
            ok("M::n"),
            [ident("M"), TokenKind::Glyph(ColonColon), ident("n")]
        );
    }

    #[test]
    fn there_is_no_not_equal_glyph() {
        // [2]. `!` is not a glyph at all, and `<>` lexes as two comparisons rather
        // than being quietly accepted as one operator.
        assert!(errs("a != b")[0].contains("is not a glyph"));
        assert_eq!(
            ok("a <> b"),
            [
                ident("a"),
                TokenKind::Glyph(Glyph::Less),
                TokenKind::Glyph(Glyph::Greater),
                ident("b")
            ]
        );
    }

    #[test]
    fn one_mistake_gives_one_diagnostic() {
        // A bad escape inside a char literal must not also report the literal's
        // length: that is one mistake, and the harness compares the set of lines
        // carrying an error against the set of annotated lines.
        assert_eq!(errs(r"'\q'").len(), 1);
        assert_eq!(errs(r"'\u{D800}'").len(), 1);
        assert_eq!(
            errs("'ab").len(),
            1,
            "unclosed and too long is still one mistake"
        );
    }

    #[test]
    fn several_errors_are_reported_in_one_run() {
        // Phase 1 requires recovery good enough for this, and the conformance harness
        // depends on it: it checks that the set of error lines is exactly the set of
        // annotated lines.
        let d = errs("a != b;\nc := 0xg;\nd := '\\q';");
        assert_eq!(d.len(), 3, "{d:#?}");
        assert!(d[0].starts_with("T.alm:1:3:"), "{:?}", d[0]);
        assert!(d[1].starts_with("T.alm:2:6:"), "{:?}", d[1]);
        assert!(d[2].starts_with("T.alm:3:7:"), "{:?}", d[2]);
    }

    #[test]
    fn spans_point_at_the_token() {
        let src = "var total: int;";
        let mut d = Diagnostics::new();
        let tokens = lex(S, src, &mut d);
        assert!(!d.has_errors());
        assert_eq!(tokens[1].span.text(src), "total");
        assert_eq!(tokens[0].span.text(src), "var");
        // The `Eof` span is empty and sits at the end, so a diagnostic about it points
        // past the last character rather than at nothing.
        let eof = tokens.last().unwrap();
        assert!(eof.span.is_empty());
        assert_eq!(eof.span.start as usize, src.len());
    }

    #[test]
    fn an_empty_file_is_just_eof() {
        assert_eq!(ok(""), []);
        assert_eq!(ok("   \n\t # only a comment\n"), []);
    }
}

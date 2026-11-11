//! Source positions.
//!
//! A span is a byte range in one source file. The lexer records byte, char, line
//! and column (TODO.md Phase 1) and the line/column pair is derived on demand from
//! the retained source rather than stored per token.
//!
//! Spans are *not* what the runtime reports. [7.5] requires the runtime to print
//! the source *text* of the expression being evaluated, and a span resolves only
//! against the original file, which is absent at run time and whose host path
//! ADR-008 forbids recording. So lowering copies the snippet text into a
//! per-procedure string table in the binary; see `documentation/ir.md` §6.

/// Index of a source file in the compilation's source table.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Debug)]
pub struct SourceId(pub u32);

/// A half-open byte range `[start, end)` within one source file.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Span {
    pub source: SourceId,
    pub start: u32,
    pub end: u32,
}

impl Span {
    pub fn new(source: SourceId, start: u32, end: u32) -> Self {
        debug_assert!(start <= end, "span start must not exceed its end");
        Span { source, start, end }
    }

    /// The smallest span covering both. Both must be in the same file.
    pub fn to(self, other: Span) -> Span {
        debug_assert_eq!(self.source, other.source, "cannot join spans across files");
        Span {
            source: self.source,
            start: self.start.min(other.start),
            end: self.end.max(other.end),
        }
    }

    pub fn len(self) -> u32 {
        self.end - self.start
    }

    pub fn is_empty(self) -> bool {
        self.start == self.end
    }

    /// The text this span covers, given the file it came from.
    pub fn text(self, source: &str) -> &str {
        &source[self.start as usize..self.end as usize]
    }
}

/// One-based line and column (counted in code points, not bytes), for diagnostics.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct LineCol {
    pub line: u32,
    pub column: u32,
}

/// Resolve a byte offset to a line and column by counting from the start of the
/// file. Diagnostics are rare, so this is not worth an index.
pub fn line_col(source: &str, offset: u32) -> LineCol {
    let offset = (offset as usize).min(source.len());
    let mut line = 1;
    let mut line_start = 0;
    for (i, b) in source.as_bytes()[..offset].iter().enumerate() {
        if *b == b'\n' {
            line += 1;
            line_start = i + 1;
        }
    }
    let column = 1 + source[line_start..offset].chars().count() as u32;
    LineCol { line, column }
}

#[cfg(test)]
mod tests {
    use super::*;

    const S: SourceId = SourceId(0);

    #[test]
    fn join_covers_both() {
        let a = Span::new(S, 4, 7);
        let b = Span::new(S, 10, 12);
        assert_eq!(a.to(b), Span::new(S, 4, 12));
        assert_eq!(b.to(a), Span::new(S, 4, 12));
    }

    #[test]
    fn text_is_the_covered_range() {
        let src = "total + x";
        assert_eq!(Span::new(S, 0, 9).text(src), "total + x");
        assert_eq!(Span::new(S, 6, 7).text(src), "+");
    }

    #[test]
    fn line_col_is_one_based() {
        let src = "a\nbb\nccc";
        assert_eq!(line_col(src, 0), LineCol { line: 1, column: 1 });
        assert_eq!(line_col(src, 2), LineCol { line: 2, column: 1 });
        assert_eq!(line_col(src, 3), LineCol { line: 2, column: 2 });
        assert_eq!(line_col(src, 5), LineCol { line: 3, column: 1 });
    }

    #[test]
    fn line_col_counts_code_points_not_bytes() {
        // Non-ASCII is legal in comments and literals [2].
        let src = "# ñé\nx";
        assert_eq!(line_col(src, 7), LineCol { line: 2, column: 1 });
    }
}

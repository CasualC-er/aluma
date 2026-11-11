//! Compile-time diagnostics.
//!
//! Shared with `almbe` and the harness because the conformance suite matches
//! diagnostics *exactly*: Phase 10 requires `almc` to produce "identical
//! diagnostics, text and order" as `almc0`, which makes the rendering a
//! specification, not a convenience.
//!
//! Rendered form, one line per diagnostic:
//!
//! ```text
//! Stats.alm:12:7: error: `total` is not declared
//! ```
//!
//! The file is written as the *module path as given to the compiler*, never an
//! absolute host path, so that output does not depend on where the tree lives
//! (ADR-008).

use crate::span::{Span, line_col};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Debug)]
pub enum Severity {
    Error,
    Warning,
}

impl Severity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Severity::Error => "error",
            Severity::Warning => "warning",
        }
    }
}

impl std::fmt::Display for Severity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

#[derive(Clone, PartialEq, Eq, Debug)]
pub struct Diagnostic {
    pub severity: Severity,
    pub span: Span,
    pub message: String,
}

impl Diagnostic {
    pub fn error(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Error,
            span,
            message: message.into(),
        }
    }

    pub fn warning(span: Span, message: impl Into<String>) -> Self {
        Diagnostic {
            severity: Severity::Warning,
            span,
            message: message.into(),
        }
    }

    /// Render against the file the span points into. `file` is the name the
    /// compiler was given, not a resolved path.
    pub fn render(&self, file: &str, source: &str) -> String {
        let at = line_col(source, self.span.start);
        format!(
            "{file}:{}:{}: {}: {}",
            at.line, at.column, self.severity, self.message
        )
    }
}

/// Collected diagnostics for one compilation.
///
/// Order is the order they were reported in, and that order is part of the
/// conformance contract, so nothing here sorts or deduplicates.
#[derive(Clone, Default, Debug)]
pub struct Diagnostics {
    items: Vec<Diagnostic>,
}

impl Diagnostics {
    pub fn new() -> Self {
        Diagnostics::default()
    }

    pub fn push(&mut self, d: Diagnostic) {
        self.items.push(d);
    }

    pub fn error(&mut self, span: Span, message: impl Into<String>) {
        self.push(Diagnostic::error(span, message));
    }

    pub fn items(&self) -> &[Diagnostic] {
        &self.items
    }

    pub fn has_errors(&self) -> bool {
        self.items.iter().any(|d| d.severity == Severity::Error)
    }

    pub fn is_empty(&self) -> bool {
        self.items.is_empty()
    }

    pub fn len(&self) -> usize {
        self.items.len()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::span::SourceId;

    #[test]
    fn renders_with_one_based_line_and_column() {
        let src = "proc f() is\n  x := 1;\nend\n";
        let d = Diagnostic::error(Span::new(SourceId(0), 14, 15), "`x` is not declared");
        assert_eq!(
            d.render("F.alm", src),
            "F.alm:2:3: error: `x` is not declared"
        );
    }

    #[test]
    fn order_is_preserved_not_sorted() {
        let s = SourceId(0);
        let mut ds = Diagnostics::new();
        ds.error(Span::new(s, 10, 11), "second position, reported first");
        ds.error(Span::new(s, 0, 1), "first position, reported second");
        assert_eq!(ds.items()[0].span.start, 10);
        assert!(ds.has_errors());
        assert_eq!(ds.len(), 2);
    }
}
